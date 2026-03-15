use std::sync::{Arc, RwLock};

use crate::{
    addons::{AddonRegistry, AddonStore, RemoteHttpAddon, SolAddon},
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
        let registry = AddonRegistry::from_manifest_urls(&store.load_urls());

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
        self.addons.read().expect("addon registry read lock").search(query)
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
        self.addons
            .read()
            .expect("addon registry read lock")
            .descriptors()
    }

    pub fn install_addon_url(&self, manifest_url: &str) -> Result<AddonDescriptor, String> {
        let addon = RemoteHttpAddon::install(manifest_url)?;
        self.store.install_url(manifest_url)?;
        let urls = self.store.load_urls();
        let descriptor = addon.descriptor();

        *self.addons.write().expect("addon registry write lock") =
            AddonRegistry::from_manifest_urls(&urls);

        Ok(descriptor)
    }
}
