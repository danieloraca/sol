use std::sync::Arc;

use crate::{
    domain::{HomeFeed, MediaItem, MediaType, StreamLookup, StreamSource},
    providers::{
        FallbackMetadataProvider, FallbackStreamProvider, MetadataProvider, SeededLibraryProvider,
        StreamProvider, TmdbMetadataProvider, TorboxStreamProvider,
    },
};

#[derive(Clone)]
pub struct AppServices {
    metadata: Arc<dyn MetadataProvider>,
    streams: Arc<dyn StreamProvider>,
}

impl AppServices {
    pub fn demo() -> Self {
        let seeded = Arc::new(SeededLibraryProvider::demo());
        let metadata: Arc<dyn MetadataProvider> = TmdbMetadataProvider::from_env()
            .map(|provider| {
                Arc::new(FallbackMetadataProvider::new(Arc::new(provider), seeded.clone()))
                    as Arc<dyn MetadataProvider>
            })
            .unwrap_or_else(|| seeded.clone());
        let torbox = Arc::new(TorboxStreamProvider::from_env());
        let streams = Arc::new(FallbackStreamProvider::new(torbox, seeded.clone()));

        Self {
            metadata,
            streams,
        }
    }

    pub fn home_feed(&self) -> HomeFeed {
        self.metadata.home_feed()
    }

    pub fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        self.metadata.catalog(media_type)
    }

    pub fn search(&self, query: &str) -> Vec<MediaItem> {
        self.metadata.search(query)
    }

    pub fn item(&self, id: &str) -> Option<MediaItem> {
        self.metadata.item(id)
    }

    pub fn streams(&self, id: &str) -> Option<Vec<StreamSource>> {
        let item = self.metadata.item(id)?;
        let lookup = self.streams.lookup(&item);
        (!lookup.streams.is_empty()).then_some(lookup.streams)
    }

    pub fn stream_lookup(&self, id: &str) -> Option<StreamLookup> {
        let item = self.metadata.item(id)?;
        Some(self.streams.lookup(&item))
    }
}
