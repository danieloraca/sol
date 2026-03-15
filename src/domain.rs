use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Movie,
    Series,
    Channel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamSource {
    pub name: String,
    pub quality: String,
    pub language: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamCandidate {
    pub name: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamLookup {
    pub provider: String,
    pub status: String,
    pub message: String,
    pub streams: Vec<StreamSource>,
    pub candidates: Vec<StreamCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcquisitionResult {
    pub provider: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRelease {
    pub title: String,
    pub indexer: String,
    pub protocol: String,
    pub quality: String,
    pub size: String,
    pub seeders: String,
    pub age: String,
    pub magnet_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSearchResult {
    pub provider: String,
    pub status: String,
    pub message: String,
    pub releases: Vec<SourceRelease>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub media_type: MediaType,
    pub genres: Vec<String>,
    pub poster_url: String,
    pub year: u16,
    pub streams: Vec<StreamSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HomeFeed {
    pub hero: MediaItem,
    pub trending: Vec<MediaItem>,
    pub continue_watching: Vec<MediaItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiMessage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub routes: Vec<&'static str>,
}
