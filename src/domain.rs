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
    #[serde(default)]
    pub provider: String,
    pub name: String,
    #[serde(default)]
    pub full_title: String,
    #[serde(default)]
    pub details: Vec<String>,
    pub quality: String,
    pub language: String,
    pub url: String,
    pub playback_kind: String,
    pub playback_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamCandidate {
    pub name: String,
    pub detail: String,
    #[serde(default)]
    pub magnet_url: Option<String>,
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
#[serde(rename_all = "lowercase")]
pub enum AddonTransport {
    Builtin,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddonDescriptor {
    pub id: String,
    pub name: String,
    pub version: String,
    pub transport: AddonTransport,
    pub enabled: bool,
    pub configured: bool,
    pub health_status: String,
    pub health_message: String,
    pub capabilities: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaItem {
    pub id: String,
    pub alternate_ids: Vec<String>,
    pub title: String,
    pub description: String,
    pub media_type: MediaType,
    pub genres: Vec<String>,
    pub poster_url: String,
    pub backdrop_url: String,
    pub year: u16,
    pub streams: Vec<StreamSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HomeFeed {
    pub hero: MediaItem,
    pub trending: Vec<MediaItem>,
    pub continue_watching: Vec<MediaItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatchProgressEntry {
    pub id: String,
    pub progress_percent: f32,
    pub position_seconds: u32,
    pub duration_seconds: u32,
    pub updated_at_ms: i64,
    pub source_provider: Option<String>,
    pub source_name: Option<String>,
    pub source_quality: Option<String>,
    pub source_language: Option<String>,
    pub source_url: Option<String>,
    pub source_playback_kind: Option<String>,
    pub source_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiMessage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub routes: Vec<&'static str>,
}
