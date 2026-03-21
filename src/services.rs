use std::sync::{Arc, RwLock};

use crate::{
    addons::{AddonRegistry, AddonStore, MoveDirection, RemoteHttpAddon, SolAddon},
    domain::{
        AcquisitionResult, AddonDescriptor, HomeFeed, MediaItem, MediaType, SourceSearchResult,
        StreamLookup, StreamSource,
    },
};

#[derive(Clone)]
pub struct AppServices {
    addons: Arc<RwLock<AddonRegistry>>,
    store: AddonStore,
}

impl AppServices {
    pub fn demo() -> Self {
        let store = AddonStore::default();
        let registry = AddonRegistry::from_manifest_urls(&store.enabled_urls());

        Self {
            addons: Arc::new(RwLock::new(registry)),
            store,
        }
    }

    pub fn home_feed(&self) -> HomeFeed {
        self.addons.read().expect("addon registry read lock").home_feed()
    }

    pub fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        self.addons
            .read()
            .expect("addon registry read lock")
            .catalog(media_type)
    }

    pub fn search(&self, query: &str) -> Vec<MediaItem> {
        let registry = self.addons.read().expect("addon registry read lock");
        let results = registry.search(query);
        if !results.is_empty() || query.trim().is_empty() {
            return results;
        }

        // Fallback: if addons don't expose a dedicated search resource,
        // run a local query across catalog items so sidebar search still works.
        let query = query.trim().to_lowercase();
        registry
            .catalog(None)
            .into_iter()
            .filter(|item| media_item_matches_query(item, &query))
            .collect()
    }

    pub fn item(&self, id: &str) -> Option<MediaItem> {
        self.addons.read().expect("addon registry read lock").item(id)
    }

    pub fn streams(&self, id: &str) -> Option<Vec<StreamSource>> {
        let registry = self.addons.read().expect("addon registry read lock");
        let item = registry.item(id)?;
        let lookup = registry.stream_lookup(&item);
        (!lookup.streams.is_empty()).then_some(lookup.streams)
    }

    pub fn stream_lookup(&self, id: &str) -> Option<StreamLookup> {
        let registry = self.addons.read().expect("addon registry read lock");
        let item = registry.item(id)?;
        Some(registry.stream_lookup(&item))
    }

    pub fn submit_torbox_magnet(
        &self,
        id: &str,
        magnet: &str,
        only_if_cached: bool,
    ) -> Option<AcquisitionResult> {
        let registry = self.addons.read().expect("addon registry read lock");
        let item = registry.item(id)?;
        Some(registry.submit_magnet(&item, magnet, only_if_cached))
    }

    pub fn search_sources(&self, id: &str) -> Option<SourceSearchResult> {
        let registry = self.addons.read().expect("addon registry read lock");
        let item = registry.item(id)?;
        Some(registry.source_search(&item))
    }

    pub fn addons(&self) -> Vec<AddonDescriptor> {
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
                descriptors.remove(&stored.manifest_url).unwrap_or(AddonDescriptor {
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

    pub fn move_remote_addon(&self, manifest_url: &str, direction: MoveDirection) -> Result<(), String> {
        self.store.move_remote_addon(manifest_url, direction)?;
        self.reload_registry();
        Ok(())
    }

    fn reload_registry(&self) {
        let urls = self.store.enabled_urls();
        *self.addons.write().expect("addon registry write lock") =
            AddonRegistry::from_manifest_urls(&urls);
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
