use std::sync::Arc;

use crate::{
    domain::{
        AcquisitionResult, AddonDescriptor, AddonTransport, HomeFeed, MediaItem, MediaType,
        SourceSearchResult, StreamLookup,
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
        let mut addons: Vec<Arc<dyn SolAddon>> = vec![
            Arc::new(TmdbMetadataAddon::new()),
            Arc::new(TorboxStreamAddon::new()),
            Arc::new(ProwlarrSearchAddon::new()),
            Arc::new(DemoCatalogAddon::new()),
        ];

        // Future remote addon clients can be inserted ahead of the builtin fallback.
        addons.retain(|addon| !addon.descriptor().id.is_empty());

        Self { addons }
    }

    pub fn descriptors(&self) -> Vec<AddonDescriptor> {
        self.addons.iter().map(|addon| addon.descriptor()).collect()
    }

    pub fn home_feed(&self) -> HomeFeed {
        self.addons
            .iter()
            .filter_map(|addon| addon.home_feed())
            .find(|feed| !feed.trending.is_empty())
            .unwrap_or_else(|| HomeFeed {
                hero: unavailable_placeholder(),
                trending: vec![],
                continue_watching: vec![],
            })
    }

    pub fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        self.addons
            .iter()
            .filter_map(|addon| addon.catalog(media_type.clone()))
            .find(|items| !items.is_empty())
            .unwrap_or_default()
    }

    pub fn search(&self, query: &str) -> Vec<MediaItem> {
        self.addons
            .iter()
            .filter_map(|addon| addon.search(query))
            .find(|items| !items.is_empty())
            .unwrap_or_default()
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
