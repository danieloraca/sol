use std::sync::Arc;

use crate::{
    domain::{HomeFeed, MediaItem, MediaType, StreamLookup, StreamSource},
    services::AppServices,
};

#[derive(Clone)]
pub struct AppState {
    services: Arc<AppServices>,
}

impl AppState {
    pub fn demo() -> Self {
        Self {
            services: Arc::new(AppServices::demo()),
        }
    }

    pub fn home_feed(&self) -> HomeFeed {
        self.services.home_feed()
    }

    pub fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        self.services.catalog(media_type)
    }

    pub fn search(&self, query: &str) -> Vec<MediaItem> {
        self.services.search(query)
    }

    pub fn streams(&self, id: &str) -> Option<Vec<StreamSource>> {
        self.services.streams(id)
    }

    pub fn stream_lookup(&self, id: &str) -> Option<StreamLookup> {
        self.services.stream_lookup(id)
    }

    pub fn item(&self, id: &str) -> Option<MediaItem> {
        self.services.item(id)
    }
}
