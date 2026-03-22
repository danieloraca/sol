use std::sync::Arc;

use crate::{
    addons::MoveDirection,
    domain::{
        AcquisitionResult, AddonDescriptor, HomeFeed, MediaItem, MediaType, StreamLookup,
        StreamSource,
    },
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

    pub fn submit_torbox_magnet(
        &self,
        id: &str,
        magnet: &str,
        only_if_cached: bool,
    ) -> Option<AcquisitionResult> {
        self.services.submit_torbox_magnet(id, magnet, only_if_cached)
    }

    pub fn addons(&self) -> Vec<AddonDescriptor> {
        self.services.addons()
    }

    pub fn install_addon_url(&self, manifest_url: &str) -> Result<AddonDescriptor, String> {
        self.services.install_addon_url(manifest_url)
    }

    pub fn set_remote_addon_enabled(
        &self,
        manifest_url: &str,
        enabled: bool,
    ) -> Result<(), String> {
        self.services.set_remote_addon_enabled(manifest_url, enabled)
    }

    pub fn remove_remote_addon(&self, manifest_url: &str) -> Result<(), String> {
        self.services.remove_remote_addon(manifest_url)
    }

    pub fn move_remote_addon(&self, manifest_url: &str, direction: MoveDirection) -> Result<(), String> {
        self.services.move_remote_addon(manifest_url, direction)
    }
}
