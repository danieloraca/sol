use std::sync::Arc;

use crate::{
    addons::MoveDirection,
    domain::{
        AcquisitionResult, AddonDescriptor, HomeFeed, MediaItem, MediaType, StreamLookup,
        StreamSource, WatchProgressEntry,
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
        self.services
            .submit_torbox_magnet(id, magnet, only_if_cached)
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
        self.services
            .set_remote_addon_enabled(manifest_url, enabled)
    }

    pub fn remove_remote_addon(&self, manifest_url: &str) -> Result<(), String> {
        self.services.remove_remote_addon(manifest_url)
    }

    pub fn move_remote_addon(
        &self,
        manifest_url: &str,
        direction: MoveDirection,
    ) -> Result<(), String> {
        self.services.move_remote_addon(manifest_url, direction)
    }

    pub fn watch_progress(&self) -> Result<Vec<WatchProgressEntry>, String> {
        self.services.watch_progress()
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
        self.services.save_watch_progress(
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
        self.services.delete_watch_progress(id)
    }
}
