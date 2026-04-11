use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use crate::{
    addons::{AddonRegistry, AddonStore, MoveDirection, RemoteHttpAddon, SolAddon},
    domain::{
        AcquisitionResult, AddonDescriptor, HomeFeed, MediaItem, MediaType, ProviderSecretStatus,
        StreamLookup, StreamSource, WatchProgressEntry,
    },
    secrets::SecretStore,
    storage::WatchProgressStore,
};

#[derive(Clone)]
pub struct AppServices {
    addons: Arc<RwLock<AddonRegistry>>,
    cache: Arc<RwLock<ServiceCache>>,
    store: AddonStore,
    watch_progress: WatchProgressStore,
    secret_store: SecretStore,
}

const HOME_FEED_TTL: Duration = Duration::from_secs(20);
const CATALOG_TTL: Duration = Duration::from_secs(20);
const SEARCH_TTL: Duration = Duration::from_secs(20);
const ITEM_TTL: Duration = Duration::from_secs(30);
const STREAM_LOOKUP_TTL: Duration = Duration::from_secs(10);
const PERF_LOG_THRESHOLD_MS: u128 = 120;
const CACHE_MAX_ENTRIES: usize = 128;

#[derive(Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

#[derive(Default)]
struct ServiceCache {
    home_feed: Option<CacheEntry<HomeFeed>>,
    catalog: HashMap<String, CacheEntry<Vec<MediaItem>>>,
    search: HashMap<String, CacheEntry<Vec<MediaItem>>>,
    item: HashMap<String, CacheEntry<MediaItem>>,
    stream_lookup: HashMap<String, CacheEntry<StreamLookup>>,
}

impl ServiceCache {
    fn clear(&mut self) {
        self.home_feed = None;
        self.catalog.clear();
        self.search.clear();
        self.item.clear();
        self.stream_lookup.clear();
    }
}

impl AppServices {
    pub fn demo() -> Self {
        let store = AddonStore::default();
        let secret_store = SecretStore;
        if let Err(error) = secret_store.load_into_env() {
            eprintln!("[secrets] {error}");
        }
        let registry = AddonRegistry::from_manifest_urls(&store.enabled_urls());
        let watch_progress =
            WatchProgressStore::new().expect("watch progress store should initialize");

        Self {
            addons: Arc::new(RwLock::new(registry)),
            cache: Arc::new(RwLock::new(ServiceCache::default())),
            store,
            watch_progress,
            secret_store,
        }
    }

    pub fn home_feed(&self) -> HomeFeed {
        let now = Instant::now();
        if let Some(feed) = self
            .cache
            .read()
            .expect("service cache read lock")
            .home_feed
            .as_ref()
            .filter(|entry| entry.expires_at > now)
            .map(|entry| entry.value.clone())
        {
            return feed;
        }

        let started = Instant::now();
        let feed = self
            .addons
            .read()
            .expect("addon registry read lock")
            .home_feed();
        self.cache
            .write()
            .expect("service cache write lock")
            .home_feed = Some(CacheEntry {
            value: feed.clone(),
            expires_at: Instant::now() + HOME_FEED_TTL,
        });
        log_perf("home_feed", started);
        feed
    }

    pub fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        let cache_key = catalog_cache_key(media_type.as_ref());
        let now = Instant::now();
        if let Some(items) = self
            .cache
            .read()
            .expect("service cache read lock")
            .catalog
            .get(&cache_key)
            .filter(|entry| entry.expires_at > now)
            .map(|entry| entry.value.clone())
        {
            return items;
        }

        let started = Instant::now();
        let items = self
            .addons
            .read()
            .expect("addon registry read lock")
            .catalog(media_type);
        let mut cache = self.cache.write().expect("service cache write lock");
        if cache.catalog.len() >= CACHE_MAX_ENTRIES {
            cache.catalog.clear();
        }
        cache.catalog.insert(
            cache_key,
            CacheEntry {
                value: items.clone(),
                expires_at: Instant::now() + CATALOG_TTL,
            },
        );
        log_perf("catalog", started);
        items
    }

    pub fn search(&self, query: &str) -> Vec<MediaItem> {
        let normalized_query = query.trim().to_lowercase();
        let now = Instant::now();
        if let Some(items) = self
            .cache
            .read()
            .expect("service cache read lock")
            .search
            .get(&normalized_query)
            .filter(|entry| entry.expires_at > now)
            .map(|entry| entry.value.clone())
        {
            return items;
        }

        let started = Instant::now();
        let registry = self.addons.read().expect("addon registry read lock");
        let results = registry.search(query);
        let final_results = if !results.is_empty() || normalized_query.is_empty() {
            results
        } else {
            registry
                .catalog(None)
                .into_iter()
                .filter(|item| media_item_matches_query(item, &normalized_query))
                .collect()
        };

        let mut cache = self.cache.write().expect("service cache write lock");
        if cache.search.len() >= CACHE_MAX_ENTRIES {
            cache.search.clear();
        }
        cache.search.insert(
            normalized_query,
            CacheEntry {
                value: final_results.clone(),
                expires_at: Instant::now() + SEARCH_TTL,
            },
        );
        log_perf("search", started);
        final_results
    }

    pub fn item(&self, id: &str) -> Option<MediaItem> {
        let now = Instant::now();
        if let Some(item) = self
            .cache
            .read()
            .expect("service cache read lock")
            .item
            .get(id)
            .filter(|entry| entry.expires_at > now)
            .map(|entry| entry.value.clone())
        {
            return Some(item);
        }

        let started = Instant::now();
        let item = self
            .addons
            .read()
            .expect("addon registry read lock")
            .item(id)?;
        let mut cache = self.cache.write().expect("service cache write lock");
        if cache.item.len() >= CACHE_MAX_ENTRIES {
            cache.item.clear();
        }
        cache.item.insert(
            id.to_string(),
            CacheEntry {
                value: item.clone(),
                expires_at: Instant::now() + ITEM_TTL,
            },
        );
        log_perf("item", started);
        Some(item)
    }

    pub fn streams(&self, id: &str) -> Option<Vec<StreamSource>> {
        let lookup = self.stream_lookup(id)?;
        (!lookup.streams.is_empty()).then_some(lookup.streams)
    }

    pub fn stream_lookup(&self, id: &str) -> Option<StreamLookup> {
        let now = Instant::now();
        if let Some(lookup) = self
            .cache
            .read()
            .expect("service cache read lock")
            .stream_lookup
            .get(id)
            .filter(|entry| entry.expires_at > now)
            .map(|entry| entry.value.clone())
        {
            return Some(lookup);
        }

        let started = Instant::now();
        let item = self.item(id)?;
        let registry = self.addons.read().expect("addon registry read lock");
        let lookup = registry.stream_lookup(&item);
        let mut cache = self.cache.write().expect("service cache write lock");
        if cache.stream_lookup.len() >= CACHE_MAX_ENTRIES {
            cache.stream_lookup.clear();
        }
        cache.stream_lookup.insert(
            id.to_string(),
            CacheEntry {
                value: lookup.clone(),
                expires_at: Instant::now() + STREAM_LOOKUP_TTL,
            },
        );
        log_perf("stream_lookup", started);
        Some(lookup)
    }

    pub fn submit_torbox_magnet(
        &self,
        id: &str,
        magnet: &str,
        only_if_cached: bool,
    ) -> Option<AcquisitionResult> {
        let started = Instant::now();
        let item = self.item(id)?;
        let registry = self.addons.read().expect("addon registry read lock");
        let result = registry.submit_magnet(&item, magnet, only_if_cached);

        let mut cache = self.cache.write().expect("service cache write lock");
        cache.stream_lookup.remove(id);
        cache.home_feed = None;
        log_perf("submit_torbox_magnet", started);
        Some(result)
    }

    pub fn addons(&self) -> Vec<AddonDescriptor> {
        let started = Instant::now();
        let registry_descriptors = self
            .addons
            .read()
            .expect("addon registry read lock")
            .descriptors();
        let mut descriptors = registry_descriptors
            .iter()
            .filter(|addon| matches!(addon.transport, crate::domain::AddonTransport::Remote))
            .map(|addon| (addon.source.clone(), addon.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();

        let mut ordered = self
            .store
            .remote_addons()
            .into_iter()
            .map(|stored| {
                descriptors
                    .remove(&stored.manifest_url)
                    .unwrap_or(AddonDescriptor {
                        id: if stored.id.is_empty() {
                            stored.manifest_url.clone()
                        } else {
                            stored.id.clone()
                        },
                        name: if stored.name.is_empty() {
                            "Remote addon".into()
                        } else {
                            stored.name.clone()
                        },
                        version: stored.version.clone(),
                        transport: crate::domain::AddonTransport::Remote,
                        enabled: stored.enabled,
                        configured: true,
                        health_status: if stored.enabled {
                            "error".into()
                        } else {
                            "disabled".into()
                        },
                        health_message: if stored.enabled {
                            "Sol could not load this addon manifest right now.".into()
                        } else {
                            "This addon is disabled in Sol.".into()
                        },
                        capabilities: stored.capabilities.clone(),
                        source: stored.manifest_url.clone(),
                    })
            })
            .collect::<Vec<_>>();

        ordered.extend(
            registry_descriptors
                .into_iter()
                .filter(|addon| matches!(addon.transport, crate::domain::AddonTransport::Builtin)),
        );
        log_perf("addons", started);
        ordered
    }

    pub fn install_addon_url(&self, manifest_url: &str) -> Result<AddonDescriptor, String> {
        let addon = RemoteHttpAddon::install(manifest_url)?;
        let descriptor = addon.descriptor();
        self.store.install_remote_addon(manifest_url, &descriptor)?;
        self.reload_registry();

        Ok(descriptor)
    }

    pub fn set_remote_addon_enabled(
        &self,
        manifest_url: &str,
        enabled: bool,
    ) -> Result<(), String> {
        self.store.set_remote_enabled(manifest_url, enabled)?;
        self.reload_registry();
        Ok(())
    }

    pub fn remove_remote_addon(&self, manifest_url: &str) -> Result<(), String> {
        self.store.remove_remote_addon(manifest_url)?;
        self.reload_registry();
        Ok(())
    }

    pub fn move_remote_addon(
        &self,
        manifest_url: &str,
        direction: MoveDirection,
    ) -> Result<(), String> {
        self.store.move_remote_addon(manifest_url, direction)?;
        self.reload_registry();
        Ok(())
    }

    pub fn watch_progress(&self) -> Result<Vec<WatchProgressEntry>, String> {
        self.watch_progress.list()
    }

    pub fn save_watch_progress(
        &self,
        id: &str,
        progress_percent: f32,
        position_seconds: u32,
        duration_seconds: u32,
        source_provider: Option<&str>,
        source_name: Option<&str>,
        source_quality: Option<&str>,
        source_language: Option<&str>,
        source_url: Option<&str>,
        source_playback_kind: Option<&str>,
        source_fingerprint: Option<&str>,
    ) -> Result<(), String> {
        self.watch_progress.upsert(
            id,
            progress_percent,
            position_seconds,
            duration_seconds,
            source_provider,
            source_name,
            source_quality,
            source_language,
            source_url,
            source_playback_kind,
            source_fingerprint,
        )
    }

    pub fn delete_watch_progress(&self, id: &str) -> Result<(), String> {
        self.watch_progress.delete(id)
    }

    pub fn provider_secret_status(&self) -> Result<ProviderSecretStatus, String> {
        self.secret_store.status()
    }

    pub fn save_provider_secrets(
        &self,
        torbox_api_key: Option<&str>,
        tmdb_api_read_token: Option<&str>,
    ) -> Result<ProviderSecretStatus, String> {
        if let Some(value) = torbox_api_key {
            if value.trim().is_empty() {
                self.secret_store.clear_torbox_api_key()?;
            } else {
                self.secret_store.set_torbox_api_key(value)?;
            }
        }

        if let Some(value) = tmdb_api_read_token {
            if value.trim().is_empty() {
                self.secret_store.clear_tmdb_api_read_token()?;
            } else {
                self.secret_store.set_tmdb_api_read_token(value)?;
            }
        }

        self.reload_registry();
        self.secret_store.status()
    }

    pub fn clear_provider_secret(&self, provider: &str) -> Result<ProviderSecretStatus, String> {
        match provider.trim().to_ascii_lowercase().as_str() {
            "torbox" => self.secret_store.clear_torbox_api_key()?,
            "tmdb" => self.secret_store.clear_tmdb_api_read_token()?,
            _ => return Err("Provider must be 'torbox' or 'tmdb'.".into()),
        }
        self.reload_registry();
        self.secret_store.status()
    }

    fn reload_registry(&self) {
        if let Err(error) = self.secret_store.load_into_env() {
            eprintln!("[secrets] {error}");
        }
        let urls = self.store.enabled_urls();
        *self.addons.write().expect("addon registry write lock") =
            AddonRegistry::from_manifest_urls(&urls);
        self.cache
            .write()
            .expect("service cache write lock")
            .clear();
    }
}

fn catalog_cache_key(media_type: Option<&MediaType>) -> String {
    match media_type {
        Some(MediaType::Movie) => "movie".into(),
        Some(MediaType::Series) => "series".into(),
        Some(MediaType::Channel) => "channel".into(),
        None => "all".into(),
    }
}

fn log_perf(operation: &str, started: Instant) {
    let elapsed = started.elapsed();
    if elapsed.as_millis() >= PERF_LOG_THRESHOLD_MS {
        eprintln!("[perf] services.{operation} took {}ms", elapsed.as_millis());
    }
}

fn media_item_matches_query(item: &MediaItem, query: &str) -> bool {
    item.title.to_lowercase().contains(query)
        || item.description.to_lowercase().contains(query)
        || item.id.to_lowercase().contains(query)
        || item
            .genres
            .iter()
            .any(|genre| genre.to_lowercase().contains(query))
}
