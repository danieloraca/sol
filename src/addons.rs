use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::Arc,
};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        AcquisitionResult, AddonDescriptor, AddonTransport, HomeFeed, MediaItem, MediaType,
        SourceRelease, SourceSearchResult, StreamLookup, StreamSource,
    },
    providers::{
        MetadataProvider, ProwlarrSourceProvider, SeededLibraryProvider, SourceSearchProvider,
        StreamProvider, TmdbMetadataProvider, TorboxStreamProvider,
    },
};

pub trait SolAddon: Send + Sync {
    fn descriptor(&self) -> AddonDescriptor;

    fn home_feed(&self) -> Option<HomeFeed> {
        None
    }

    fn catalog(&self, _media_type: Option<MediaType>) -> Option<Vec<MediaItem>> {
        None
    }

    fn search(&self, _query: &str) -> Option<Vec<MediaItem>> {
        None
    }

    fn item(&self, _id: &str) -> Option<MediaItem> {
        None
    }

    fn stream_lookup(&self, _item: &MediaItem) -> Option<StreamLookup> {
        None
    }

    fn source_search(&self, _item: &MediaItem) -> Option<SourceSearchResult> {
        None
    }

    fn submit_magnet(
        &self,
        _item: &MediaItem,
        _magnet: &str,
        _only_if_cached: bool,
    ) -> Option<AcquisitionResult> {
        None
    }
}

#[derive(Clone, Default)]
pub struct AddonRegistry {
    addons: Vec<Arc<dyn SolAddon>>,
}

impl AddonRegistry {
    pub fn builtin() -> Self {
        Self::from_manifest_urls(&[])
    }

    pub fn from_manifest_urls(urls: &[String]) -> Self {
        let mut addons: Vec<Arc<dyn SolAddon>> = urls
            .iter()
            .filter_map(|url| RemoteHttpAddon::install(url).ok())
            .map(|addon| Arc::new(addon) as Arc<dyn SolAddon>)
            .collect();

        addons.push(Arc::new(TmdbMetadataAddon::new()) as Arc<dyn SolAddon>);
        addons.push(Arc::new(TorboxStreamAddon::new()) as Arc<dyn SolAddon>);
        addons.push(Arc::new(ProwlarrSearchAddon::new()) as Arc<dyn SolAddon>);
        addons.push(Arc::new(DemoCatalogAddon::new()) as Arc<dyn SolAddon>);

        addons.retain(|addon| !addon.descriptor().id.is_empty());

        Self { addons }
    }

    pub fn descriptors(&self) -> Vec<AddonDescriptor> {
        self.addons.iter().map(|addon| addon.descriptor()).collect()
    }

    pub fn home_feed(&self) -> HomeFeed {
        let feeds = self
            .addons
            .iter()
            .filter_map(|addon| addon.home_feed())
            .collect::<Vec<_>>();

        let hero = feeds
            .iter()
            .find_map(|feed| (!feed.trending.is_empty()).then_some(feed.hero.clone()))
            .unwrap_or_else(unavailable_placeholder);

        let trending = dedupe_media_items(
            feeds
                .iter()
                .flat_map(|feed| feed.trending.clone())
                .collect::<Vec<_>>(),
        );
        let continue_watching = dedupe_media_items(
            feeds
                .iter()
                .flat_map(|feed| feed.continue_watching.clone())
                .collect::<Vec<_>>(),
        );

        if trending.is_empty() {
            HomeFeed {
                hero: unavailable_placeholder(),
                trending,
                continue_watching,
            }
        } else {
            HomeFeed {
                hero,
                trending,
                continue_watching,
            }
        }
    }

    pub fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        dedupe_media_items(
            self.addons
            .iter()
            .filter_map(|addon| addon.catalog(media_type.clone()))
            .flatten()
            .collect(),
        )
    }

    pub fn search(&self, query: &str) -> Vec<MediaItem> {
        dedupe_media_items(
            self.addons
            .iter()
            .filter_map(|addon| addon.search(query))
            .flatten()
            .collect(),
        )
    }

    pub fn item(&self, id: &str) -> Option<MediaItem> {
        self.addons.iter().find_map(|addon| addon.item(id))
    }

    pub fn stream_lookup(&self, item: &MediaItem) -> StreamLookup {
        let mut first = None;

        for addon in &self.addons {
            let Some(lookup) = addon.stream_lookup(item) else {
                continue;
            };

            if !lookup.streams.is_empty() {
                return lookup;
            }

            if first.is_none() {
                first = Some(lookup);
            }
        }

        first.unwrap_or_else(|| StreamLookup {
            provider: "Addons".into(),
            status: "unavailable".into(),
            message: format!("No addon returned streams for {}.", item.title),
            streams: vec![],
            candidates: vec![],
        })
    }

    pub fn source_search(&self, item: &MediaItem) -> SourceSearchResult {
        self.addons
            .iter()
            .find_map(|addon| addon.source_search(item))
            .unwrap_or_else(|| SourceSearchResult {
                provider: "Addons".into(),
                status: "unavailable".into(),
                message: "No source-search addon is configured.".into(),
                releases: vec![],
            })
    }

    pub fn submit_magnet(
        &self,
        item: &MediaItem,
        magnet: &str,
        only_if_cached: bool,
    ) -> AcquisitionResult {
        self.addons
            .iter()
            .find_map(|addon| addon.submit_magnet(item, magnet, only_if_cached))
            .unwrap_or_else(|| AcquisitionResult {
                provider: "Addons".into(),
                status: "unavailable".into(),
                message: "No addon is configured to send magnets for playback.".into(),
            })
    }
}

#[derive(Clone)]
pub struct AddonStore {
    path: PathBuf,
}

impl Default for AddonStore {
    fn default() -> Self {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("sol.addons.json");
        Self { path }
    }
}

impl AddonStore {
    pub fn load_urls(&self) -> Vec<String> {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|raw| serde_json::from_str::<StoredAddonUrls>(&raw).ok())
            .map(|stored| stored.manifest_urls)
            .unwrap_or_default()
    }

    pub fn install_url(&self, url: &str) -> Result<(), String> {
        let mut urls = self.load_urls();
        let normalized = url.trim().to_string();
        if normalized.is_empty() {
            return Err("Paste an addon manifest URL first.".into());
        }

        if !urls.iter().any(|existing| existing == &normalized) {
            urls.push(normalized);
        }

        let payload = StoredAddonUrls {
            manifest_urls: urls,
        };
        let raw = serde_json::to_string_pretty(&payload)
            .map_err(|error| format!("Could not serialize addon settings: {error}"))?;
        fs::write(&self.path, raw).map_err(|error| format!("Could not save addon settings: {error}"))
    }
}

#[derive(Serialize, Deserialize)]
struct StoredAddonUrls {
    manifest_urls: Vec<String>,
}

#[derive(Clone)]
pub struct RemoteHttpAddon {
    manifest_url: String,
    base_url: String,
    manifest: RemoteManifest,
    client: Client,
}

struct DemoCatalogAddon {
    provider: SeededLibraryProvider,
}

impl DemoCatalogAddon {
    fn new() -> Self {
        Self {
            provider: SeededLibraryProvider::demo(),
        }
    }
}

impl SolAddon for DemoCatalogAddon {
    fn descriptor(&self) -> AddonDescriptor {
        AddonDescriptor {
            id: "builtin.demo".into(),
            name: "Demo Catalog".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            transport: AddonTransport::Builtin,
            configured: true,
            capabilities: vec!["catalog".into(), "meta".into(), "stream".into()],
            source: "bundled".into(),
        }
    }

    fn home_feed(&self) -> Option<HomeFeed> {
        Some(self.provider.home_feed())
    }

    fn catalog(&self, media_type: Option<MediaType>) -> Option<Vec<MediaItem>> {
        Some(self.provider.catalog(media_type))
    }

    fn search(&self, query: &str) -> Option<Vec<MediaItem>> {
        Some(MetadataProvider::search(&self.provider, query))
    }

    fn item(&self, id: &str) -> Option<MediaItem> {
        self.provider.item(id)
    }

    fn stream_lookup(&self, item: &MediaItem) -> Option<StreamLookup> {
        Some(self.provider.lookup(item))
    }
}

struct TmdbMetadataAddon {
    provider: Option<TmdbMetadataProvider>,
}

impl TmdbMetadataAddon {
    fn new() -> Self {
        Self {
            provider: TmdbMetadataProvider::from_env(),
        }
    }
}

impl SolAddon for TmdbMetadataAddon {
    fn descriptor(&self) -> AddonDescriptor {
        AddonDescriptor {
            id: "builtin.tmdb".into(),
            name: "TMDB Metadata".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            transport: AddonTransport::Builtin,
            configured: self.provider.is_some(),
            capabilities: vec!["catalog".into(), "meta".into(), "search".into()],
            source: "env:TMDB_API_READ_TOKEN|TMDB_API_KEY".into(),
        }
    }

    fn home_feed(&self) -> Option<HomeFeed> {
        self.provider.as_ref().map(MetadataProvider::home_feed)
    }

    fn catalog(&self, media_type: Option<MediaType>) -> Option<Vec<MediaItem>> {
        self.provider
            .as_ref()
            .map(|provider| provider.catalog(media_type))
    }

    fn search(&self, query: &str) -> Option<Vec<MediaItem>> {
        self.provider.as_ref().map(|provider| provider.search(query))
    }

    fn item(&self, id: &str) -> Option<MediaItem> {
        self.provider.as_ref().and_then(|provider| provider.item(id))
    }
}

struct TorboxStreamAddon {
    provider: TorboxStreamProvider,
}

impl TorboxStreamAddon {
    fn new() -> Self {
        Self {
            provider: TorboxStreamProvider::from_env(),
        }
    }
}

impl SolAddon for TorboxStreamAddon {
    fn descriptor(&self) -> AddonDescriptor {
        AddonDescriptor {
            id: "builtin.torbox".into(),
            name: "TorBox Streams".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            transport: AddonTransport::Builtin,
            configured: self.provider.is_configured(),
            capabilities: vec!["stream".into(), "submit".into()],
            source: "env:TORBOX_API_KEY".into(),
        }
    }

    fn stream_lookup(&self, item: &MediaItem) -> Option<StreamLookup> {
        Some(self.provider.lookup(item))
    }

    fn submit_magnet(
        &self,
        item: &MediaItem,
        magnet: &str,
        only_if_cached: bool,
    ) -> Option<AcquisitionResult> {
        Some(self.provider.submit_magnet(item, magnet, only_if_cached))
    }
}

struct ProwlarrSearchAddon {
    provider: ProwlarrSourceProvider,
}

impl ProwlarrSearchAddon {
    fn new() -> Self {
        Self {
            provider: ProwlarrSourceProvider::from_env(),
        }
    }
}

impl SolAddon for ProwlarrSearchAddon {
    fn descriptor(&self) -> AddonDescriptor {
        AddonDescriptor {
            id: "builtin.prowlarr".into(),
            name: "Prowlarr Search".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            transport: AddonTransport::Builtin,
            configured: self.provider.is_configured(),
            capabilities: vec!["source_search".into()],
            source: "env:PROWLARR_URL|PROWLARR_API_KEY".into(),
        }
    }

    fn source_search(&self, item: &MediaItem) -> Option<SourceSearchResult> {
        Some(self.provider.search(item))
    }
}

impl RemoteHttpAddon {
    pub fn install(manifest_url: &str) -> Result<Self, String> {
        let manifest_url = manifest_url.trim().to_string();
        if manifest_url.is_empty() {
            return Err("Addon manifest URL cannot be empty.".into());
        }

        let client = Client::builder()
            .user_agent(format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("Could not build addon client: {error}"))?;

        let manifest = client
            .get(&manifest_url)
            .send()
            .map_err(|error| format!("Could not fetch addon manifest: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Addon manifest request failed: {error}"))?
            .json::<RemoteManifest>()
            .map_err(|error| format!("Could not parse addon manifest: {error}"))?;

        Ok(Self {
            base_url: addon_base_url(&manifest_url),
            manifest_url,
            manifest,
            client,
        })
    }

    fn supports_resource(&self, name: &str, media_type: Option<MediaType>, id: Option<&str>) -> bool {
        self.manifest.resources.iter().any(|resource| match resource {
            RemoteResourceEntry::Name(resource_name) => resource_name_matches(resource_name, name),
            RemoteResourceEntry::Object(resource_obj) => {
                resource_name_matches(&resource_obj.name, name)
                    && media_type
                        .as_ref()
                        .is_none_or(|expected| resource_obj.supports_type(expected))
                    && id.is_none_or(|value| resource_obj.supports_id(value))
            }
        })
    }

    fn catalogs_for(&self, media_type: Option<MediaType>, search: bool) -> Vec<&RemoteCatalog> {
        self.manifest
            .catalogs
            .iter()
            .filter(|catalog| media_type.as_ref().is_none_or(|expected| catalog.matches_type(expected)))
            .filter(|catalog| {
                if search {
                    catalog.supports_extra("search")
                } else {
                    !catalog.requires_extra()
                }
            })
            .collect()
    }

    fn fetch_json<T: for<'de> Deserialize<'de>>(&self, url: String) -> Option<T> {
        self.client
            .get(url)
            .send()
            .ok()?
            .error_for_status()
            .ok()?
            .json::<T>()
            .ok()
    }

    fn catalog_items(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        if !self.supports_resource("catalog", media_type.clone(), None) {
            return vec![];
        }

        self.catalogs_for(media_type, false)
            .into_iter()
            .filter_map(|catalog| {
                self.fetch_json::<RemoteCatalogResponse>(format!(
                    "{}/catalog/{}/{}.json",
                    self.base_url, catalog.catalog_type, catalog.id
                ))
            })
            .flat_map(|response| response.metas)
            .filter_map(map_remote_meta_preview)
            .collect()
    }

    fn search_items(&self, query: &str) -> Vec<MediaItem> {
        if query.trim().is_empty() {
            return self.catalog_items(None);
        }

        self.catalogs_for(None, true)
            .into_iter()
            .filter_map(|catalog| {
                self.fetch_json::<RemoteCatalogResponse>(format!(
                    "{}/catalog/{}/{}/search={}.json",
                    self.base_url,
                    catalog.catalog_type,
                    catalog.id,
                    encode_path_value(query)
                ))
            })
            .flat_map(|response| response.metas)
            .filter_map(map_remote_meta_preview)
            .collect()
    }

    fn fetch_item_meta(&self, media_type: MediaType, id: &str) -> Option<MediaItem> {
        if !self.supports_resource("meta", Some(media_type.clone()), Some(id)) {
            return None;
        }

        let remote_type = media_type_to_remote(&media_type);
        let response = self.fetch_json::<RemoteMetaResponse>(format!(
            "{}/meta/{}/{}.json",
            self.base_url,
            remote_type,
            encode_path_value(id)
        ))?;
        map_remote_meta_value(response.meta)
    }

    fn source_results_from_streams(&self, item: &MediaItem) -> Vec<SourceRelease> {
        let Some(response) = self.stream_response(item) else {
            return vec![];
        };

        response
            .streams
            .into_iter()
            .filter_map(map_remote_stream_source_release)
            .collect()
    }

    fn stream_response(&self, item: &MediaItem) -> Option<RemoteStreamResponse> {
        if !self.supports_resource("stream", Some(item.media_type.clone()), Some(&item.id)) {
            return None;
        }

        self.fetch_json::<RemoteStreamResponse>(format!(
            "{}/stream/{}/{}.json",
            self.base_url,
            media_type_to_remote(&item.media_type),
            encode_path_value(&item.id)
        ))
    }
}

impl SolAddon for RemoteHttpAddon {
    fn descriptor(&self) -> AddonDescriptor {
        AddonDescriptor {
            id: self.manifest.id.clone(),
            name: self.manifest.name.clone(),
            version: self.manifest.version.clone(),
            transport: AddonTransport::Remote,
            configured: true,
            capabilities: self.manifest.capabilities(),
            source: self.manifest_url.clone(),
        }
    }

    fn home_feed(&self) -> Option<HomeFeed> {
        let items = self.catalog_items(None);
        if items.is_empty() {
            return None;
        }

        let hero = items.first()?.clone();
        let trending = items.iter().take(6).cloned().collect::<Vec<_>>();
        let continue_watching = items.iter().skip(1).take(4).cloned().collect::<Vec<_>>();

        Some(HomeFeed {
            hero,
            trending,
            continue_watching,
        })
    }

    fn catalog(&self, media_type: Option<MediaType>) -> Option<Vec<MediaItem>> {
        let items = self.catalog_items(media_type);
        (!items.is_empty()).then_some(dedupe_media_items(items))
    }

    fn search(&self, query: &str) -> Option<Vec<MediaItem>> {
        let items = self.search_items(query);
        (!items.is_empty()).then_some(dedupe_media_items(items))
    }

    fn item(&self, id: &str) -> Option<MediaItem> {
        for media_type in remote_supported_types(&self.manifest) {
            if let Some(item) = self.fetch_item_meta(media_type, id) {
                return Some(item);
            }
        }

        None
    }

    fn stream_lookup(&self, item: &MediaItem) -> Option<StreamLookup> {
        let response = self.stream_response(item)?;
        let streams = response
            .streams
            .into_iter()
            .filter_map(map_remote_stream_source)
            .collect::<Vec<_>>();

        Some(StreamLookup {
            provider: self.manifest.name.clone(),
            status: if streams.is_empty() {
                "no_direct_streams".into()
            } else {
                "ready".into()
            },
            message: if streams.is_empty() {
                format!("{} did not return any directly playable stream URLs.", self.manifest.name)
            } else {
                format!("Streaming from addon {}.", self.manifest.name)
            },
            streams,
            candidates: vec![],
        })
    }

    fn source_search(&self, item: &MediaItem) -> Option<SourceSearchResult> {
        let releases = self.source_results_from_streams(item);
        Some(SourceSearchResult {
            provider: self.manifest.name.clone(),
            status: if releases.is_empty() {
                "no_results".into()
            } else {
                "ready".into()
            },
            message: if releases.is_empty() {
                format!("{} did not return any addable stream sources for {}.", self.manifest.name, item.title)
            } else {
                format!("{} returned {} source candidates.", self.manifest.name, releases.len())
            },
            releases,
        })
    }
}

#[derive(Clone, Deserialize)]
struct RemoteManifest {
    id: String,
    version: String,
    name: String,
    #[serde(default)]
    resources: Vec<RemoteResourceEntry>,
    #[serde(default)]
    catalogs: Vec<RemoteCatalog>,
    #[serde(default)]
    types: Vec<String>,
}

impl RemoteManifest {
    fn capabilities(&self) -> Vec<String> {
        let mut capabilities = BTreeSet::new();
        for resource in &self.resources {
            capabilities.insert(normalize_resource_name(match resource {
                RemoteResourceEntry::Name(name) => name,
                RemoteResourceEntry::Object(obj) => &obj.name,
            }));
        }
        for catalog in &self.catalogs {
            let _ = capabilities.insert("catalog".into());
            if catalog.supports_extra("search") {
                let _ = capabilities.insert("search".into());
            }
        }
        capabilities.into_iter().collect()
    }
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum RemoteResourceEntry {
    Name(String),
    Object(RemoteResourceObject),
}

#[derive(Clone, Deserialize)]
struct RemoteResourceObject {
    name: String,
    #[serde(default)]
    types: Vec<String>,
    #[serde(rename = "idPrefixes", default)]
    id_prefixes: Vec<String>,
}

impl RemoteResourceObject {
    fn supports_type(&self, media_type: &MediaType) -> bool {
        self.types.is_empty() || self.types.iter().any(|value| value == media_type_to_remote(media_type))
    }

    fn supports_id(&self, id: &str) -> bool {
        self.id_prefixes.is_empty() || self.id_prefixes.iter().any(|prefix| id.starts_with(prefix))
    }
}

#[derive(Clone, Deserialize)]
struct RemoteCatalog {
    #[serde(rename = "type")]
    catalog_type: String,
    id: String,
    #[serde(default)]
    extra: Vec<RemoteCatalogExtra>,
}

impl RemoteCatalog {
    fn matches_type(&self, media_type: &MediaType) -> bool {
        self.catalog_type == media_type_to_remote(media_type)
    }

    fn supports_extra(&self, name: &str) -> bool {
        self.extra.iter().any(|extra| extra.name == name)
    }

    fn requires_extra(&self) -> bool {
        self.extra.iter().any(|extra| extra.is_required)
    }
}

#[derive(Clone, Deserialize)]
struct RemoteCatalogExtra {
    name: String,
    #[serde(rename = "isRequired", default)]
    is_required: bool,
}

#[derive(Deserialize)]
struct RemoteCatalogResponse {
    #[serde(default)]
    metas: Vec<RemoteMetaPreview>,
}

#[derive(Deserialize)]
struct RemoteMetaResponse {
    meta: serde_json::Value,
}

#[derive(Clone, Deserialize)]
struct RemoteMetaPreview {
    id: String,
    #[serde(rename = "type")]
    media_type: Option<String>,
    #[serde(alias = "name")]
    title: String,
    #[serde(default)]
    poster: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "releaseInfo", default)]
    release_info: Option<String>,
    #[serde(default)]
    genres: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RemoteStreamResponse {
    #[serde(default)]
    streams: Vec<RemoteStream>,
}

#[derive(Clone, Deserialize)]
struct RemoteStream {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(rename = "externalUrl", default)]
    external_url: Option<String>,
    #[serde(rename = "infoHash", default)]
    info_hash: Option<String>,
}

fn unavailable_placeholder() -> MediaItem {
    MediaItem {
        id: "placeholder:addons".into(),
        title: "No addons available".into(),
        description: "Configure at least one metadata addon to load a real catalog.".into(),
        media_type: MediaType::Movie,
        genres: vec!["Setup".into()],
        poster_url: String::new(),
        year: 0,
        streams: vec![],
    }
}

fn dedupe_media_items(items: Vec<MediaItem>) -> Vec<MediaItem> {
    let mut seen = BTreeSet::new();
    items.into_iter()
        .filter(|item| seen.insert(item.id.clone()))
        .collect()
}

fn addon_base_url(manifest_url: &str) -> String {
    if let Some(base) = manifest_url.strip_suffix("/manifest.json") {
        base.to_string()
    } else if let Some((base, _)) = manifest_url.rsplit_once('/') {
        base.to_string()
    } else {
        manifest_url.to_string()
    }
}

fn media_type_to_remote(media_type: &MediaType) -> &'static str {
    match media_type {
        MediaType::Movie => "movie",
        MediaType::Series => "series",
        MediaType::Channel => "channel",
    }
}

fn remote_supported_types(manifest: &RemoteManifest) -> Vec<MediaType> {
    let mut types = manifest
        .types
        .iter()
        .filter_map(|value| match value.as_str() {
            "movie" => Some(MediaType::Movie),
            "series" => Some(MediaType::Series),
            "channel" => Some(MediaType::Channel),
            _ => None,
        })
        .collect::<Vec<_>>();

    if types.is_empty() {
        types.push(MediaType::Movie);
    }

    types
}

fn map_remote_meta_preview(meta: RemoteMetaPreview) -> Option<MediaItem> {
    Some(MediaItem {
        id: meta.id,
        title: meta.title,
        description: meta.description.unwrap_or_else(|| "No description provided by addon.".into()),
        media_type: parse_remote_media_type(meta.media_type.as_deref())?,
        genres: meta.genres.unwrap_or_default(),
        poster_url: meta.poster.unwrap_or_default(),
        year: parse_release_year(meta.release_info.as_deref()),
        streams: vec![],
    })
}

fn map_remote_meta_value(value: serde_json::Value) -> Option<MediaItem> {
    let preview = serde_json::from_value::<RemoteMetaPreview>(value).ok()?;
    map_remote_meta_preview(preview)
}

fn map_remote_stream_source(stream: RemoteStream) -> Option<StreamSource> {
    let url = stream.url?;
    let playback_kind = if url.starts_with("http://") {
        "blocked".to_string()
    } else {
        "embedded".to_string()
    };
    let playback_note = if playback_kind == "blocked" {
        "This source uses plain HTTP and cannot be embedded here.".to_string()
    } else {
        "Playable in the in-app player.".to_string()
    };
    Some(StreamSource {
        name: stream.name.or(stream.title).unwrap_or_else(|| "Remote stream".into()),
        quality: infer_quality_from_text(&url),
        language: "unknown".into(),
        url,
        playback_kind,
        playback_note,
    })
}

fn map_remote_stream_source_release(stream: RemoteStream) -> Option<SourceRelease> {
    let title = stream
        .title
        .or(stream.name.clone())
        .unwrap_or_else(|| "Remote source".into());

    let magnet_url = stream
        .info_hash
        .map(|hash| format!("magnet:?xt=urn:btih:{hash}"))
        .or(stream.url)
        .or(stream.external_url)?;

    Some(SourceRelease {
        title: title.clone(),
        indexer: "Remote addon".into(),
        protocol: if magnet_url.starts_with("magnet:?") {
            "torrent".into()
        } else {
            "direct".into()
        },
        quality: infer_quality_from_text(&title),
        size: "unknown size".into(),
        seeders: "unknown".into(),
        age: "unknown".into(),
        magnet_url,
    })
}

fn parse_release_year(value: Option<&str>) -> u16 {
    value
        .and_then(|raw| raw.split(|ch: char| !ch.is_ascii_digit()).find(|part| part.len() == 4))
        .and_then(|year| year.parse::<u16>().ok())
        .unwrap_or(0)
}

fn parse_remote_media_type(value: Option<&str>) -> Option<MediaType> {
    match value.unwrap_or("movie") {
        "movie" => Some(MediaType::Movie),
        "series" => Some(MediaType::Series),
        "channel" => Some(MediaType::Channel),
        _ => None,
    }
}

fn infer_quality_from_text(value: &str) -> String {
    let normalized = value.to_ascii_lowercase();
    if normalized.contains("2160p") || normalized.contains("4k") {
        "4K".into()
    } else if normalized.contains("1440p") {
        "1440p".into()
    } else if normalized.contains("1080p") {
        "1080p".into()
    } else if normalized.contains("720p") {
        "720p".into()
    } else {
        "Auto".into()
    }
}

fn encode_path_value(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b':' => {
                vec![byte as char]
            }
            b' ' => "%20".chars().collect(),
            _ => format!("%{:02X}", byte).chars().collect(),
        })
        .collect()
}

fn normalize_resource_name(value: &str) -> String {
    match value {
        "streams" => "stream".into(),
        "metadata" => "meta".into(),
        other => other.to_string(),
    }
}

fn resource_name_matches(actual: &str, expected: &str) -> bool {
    normalize_resource_name(actual) == normalize_resource_name(expected)
}

#[cfg(test)]
mod tests {
    use super::AddonRegistry;

    #[test]
    fn builtin_registry_contains_demo_addon() {
        let registry = AddonRegistry::builtin();

        let descriptors = registry.descriptors();

        assert!(descriptors.iter().any(|addon| addon.id == "builtin.demo"));
        assert!(descriptors.iter().any(|addon| addon.id == "builtin.torbox"));
    }
}
