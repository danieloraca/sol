use std::sync::Arc;

use crate::{
    addons::AddonRegistry,
    domain::{
        AcquisitionResult, AddonDescriptor, HomeFeed, MediaItem, MediaType, SourceSearchResult,
        StreamLookup, StreamSource,
    },
};

#[derive(Clone)]
pub struct AppServices {
    addons: Arc<AddonRegistry>,
}

impl AppServices {
    pub fn demo() -> Self {
        Self {
            addons: Arc::new(AddonRegistry::builtin()),
        }
    }

    pub fn home_feed(&self) -> HomeFeed {
        self.addons.home_feed()
    }

    pub fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        self.addons.catalog(media_type)
    }

    pub fn search(&self, query: &str) -> Vec<MediaItem> {
        self.addons.search(query)
    }

    pub fn item(&self, id: &str) -> Option<MediaItem> {
        self.addons.item(id)
    }

    pub fn streams(&self, id: &str) -> Option<Vec<StreamSource>> {
        let item = self.addons.item(id)?;
        let lookup = self.addons.stream_lookup(&item);
        (!lookup.streams.is_empty()).then_some(lookup.streams)
    }

    pub fn stream_lookup(&self, id: &str) -> Option<StreamLookup> {
        let item = self.addons.item(id)?;
        Some(self.addons.stream_lookup(&item))
    }

    pub fn submit_torbox_magnet(
        &self,
        id: &str,
        magnet: &str,
        only_if_cached: bool,
    ) -> Option<AcquisitionResult> {
        let item = self.addons.item(id)?;
        Some(self.addons.submit_magnet(&item, magnet, only_if_cached))
    }

    pub fn search_sources(&self, id: &str) -> Option<SourceSearchResult> {
        let item = self.addons.item(id)?;
        Some(self.addons.source_search(&item))
    }

    pub fn addons(&self) -> Vec<AddonDescriptor> {
        self.addons.descriptors()
    }
}
