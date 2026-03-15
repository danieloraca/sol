use std::{
    collections::HashMap,
    env,
    sync::{Arc, OnceLock},
};

use reqwest::{
    blocking::Client,
    blocking::multipart::Form,
    header::{ACCEPT, AUTHORIZATION, HeaderValue},
};
use serde::Deserialize;

use crate::domain::{
    AcquisitionResult, HomeFeed, MediaItem, MediaType, StreamCandidate, StreamLookup, StreamSource,
};

const TMDB_API_BASE: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_BASE_FALLBACK: &str = "https://image.tmdb.org/t/p/w500";

pub trait MetadataProvider: Send + Sync {
    fn home_feed(&self) -> HomeFeed;
    fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem>;
    fn search(&self, query: &str) -> Vec<MediaItem>;
    fn item(&self, id: &str) -> Option<MediaItem>;
}

pub trait StreamProvider: Send + Sync {
    fn lookup(&self, item: &MediaItem) -> StreamLookup;
}

#[derive(Clone)]
pub struct FallbackMetadataProvider {
    primary: Arc<dyn MetadataProvider>,
    fallback: Arc<dyn MetadataProvider>,
}

impl FallbackMetadataProvider {
    pub fn new(primary: Arc<dyn MetadataProvider>, fallback: Arc<dyn MetadataProvider>) -> Self {
        Self { primary, fallback }
    }
}

impl MetadataProvider for FallbackMetadataProvider {
    fn home_feed(&self) -> HomeFeed {
        let feed = self.primary.home_feed();
        if feed.trending.is_empty() {
            self.fallback.home_feed()
        } else {
            feed
        }
    }

    fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        let items = self.primary.catalog(media_type.clone());
        if items.is_empty() {
            self.fallback.catalog(media_type)
        } else {
            items
        }
    }

    fn search(&self, query: &str) -> Vec<MediaItem> {
        let items = self.primary.search(query);
        if items.is_empty() {
            self.fallback.search(query)
        } else {
            items
        }
    }

    fn item(&self, id: &str) -> Option<MediaItem> {
        self.primary.item(id).or_else(|| self.fallback.item(id))
    }
}

#[derive(Debug, Clone)]
pub struct SeededLibraryProvider {
    catalog: Arc<Vec<MediaItem>>,
}

impl SeededLibraryProvider {
    pub fn demo() -> Self {
        Self {
            catalog: Arc::new(seed_catalog()),
        }
    }
}

impl MetadataProvider for SeededLibraryProvider {
    fn home_feed(&self) -> HomeFeed {
        let hero = self.catalog[0].clone();
        let trending = self.catalog.iter().take(3).cloned().collect();
        let continue_watching = self.catalog.iter().skip(1).take(2).cloned().collect();

        HomeFeed {
            hero,
            trending,
            continue_watching,
        }
    }

    fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        self.catalog
            .iter()
            .filter(|item| {
                media_type
                    .as_ref()
                    .is_none_or(|expected| &item.media_type == expected)
            })
            .cloned()
            .collect()
    }

    fn search(&self, query: &str) -> Vec<MediaItem> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return self.catalog.iter().cloned().collect();
        }

        self.catalog
            .iter()
            .filter(|item| {
                item.title.to_lowercase().contains(&query)
                    || item.description.to_lowercase().contains(&query)
                    || item
                        .genres
                        .iter()
                        .any(|genre| genre.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }

    fn item(&self, id: &str) -> Option<MediaItem> {
        self.catalog.iter().find(|item| item.id == id).cloned()
    }
}

impl StreamProvider for SeededLibraryProvider {
    fn lookup(&self, item: &MediaItem) -> StreamLookup {
        let streams = self
            .catalog
            .iter()
            .find(|candidate| candidate.id == item.id)
            .map(|candidate| candidate.streams.clone())
            .unwrap_or_default();

        if streams.is_empty() {
            StreamLookup {
                provider: "Demo".into(),
                status: "no_match".into(),
                message: format!("No demo streams are available for {}.", item.title),
                streams,
                candidates: vec![],
            }
        } else {
            StreamLookup {
                provider: "Demo".into(),
                status: "ready".into(),
                message: format!("Using bundled demo streams for {}.", item.title),
                streams,
                candidates: vec![],
            }
        }
    }
}

#[derive(Clone)]
pub struct TmdbMetadataProvider {
    client: Client,
    auth: TmdbAuth,
    image_base_url: OnceLock<String>,
    movie_genres: OnceLock<HashMap<u64, String>>,
    language: String,
    region: String,
}

impl TmdbMetadataProvider {
    pub fn from_env() -> Option<Self> {
        let auth = env::var("TMDB_API_READ_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(TmdbAuth::Bearer)
            .or_else(|| {
                env::var("TMDB_API_KEY")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .map(TmdbAuth::ApiKey)
            })?;

        Some(Self::new(auth))
    }

    fn new(auth: TmdbAuth) -> Self {
        Self {
            client: Client::builder()
                .user_agent(format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
                .build()
                .expect("tmdb client should build"),
            auth,
            image_base_url: OnceLock::new(),
            movie_genres: OnceLock::new(),
            language: "en-US".into(),
            region: "US".into(),
        }
    }

    fn discover_movies(&self, page: u8) -> Vec<MediaItem> {
        self.request::<TmdbListResponse<TmdbMovieSummary>>(
            "/discover/movie",
            &[
                ("include_adult", "false".into()),
                ("include_video", "false".into()),
                ("language", self.language.clone()),
                ("page", page.to_string()),
                ("region", self.region.clone()),
                ("sort_by", "popularity.desc".into()),
            ],
        )
        .map(|response| {
            response
                .results
                .into_iter()
                .map(|movie| self.map_movie_summary(movie))
                .collect()
        })
        .unwrap_or_default()
    }

    fn trending_movies(&self) -> Vec<MediaItem> {
        self.request::<TmdbListResponse<TmdbMovieSummary>>(
            "/trending/movie/day",
            &[("language", self.language.clone())],
        )
        .map(|response| {
            response
                .results
                .into_iter()
                .map(|movie| self.map_movie_summary(movie))
                .collect()
        })
        .unwrap_or_default()
    }

    fn movie_genre_names(&self) -> &HashMap<u64, String> {
        self.movie_genres.get_or_init(|| {
            self.request::<TmdbGenreListResponse>(
                "/genre/movie/list",
                &[("language", self.language.clone())],
            )
            .map(|response| {
                response
                    .genres
                    .into_iter()
                    .map(|genre| (genre.id, genre.name))
                    .collect()
            })
            .unwrap_or_default()
        })
    }

    fn poster_base_url(&self) -> &str {
        self.image_base_url.get_or_init(|| {
            self.request::<TmdbConfigurationResponse>("/configuration", &[])
                .map(|response| {
                    let poster_sizes = response.images.poster_sizes;
                    let size = poster_sizes
                        .iter()
                        .find(|value| value.as_str() == "w500")
                        .cloned()
                        .or_else(|| {
                            poster_sizes
                                .iter()
                                .find(|value| value.starts_with('w'))
                                .cloned()
                        })
                        .unwrap_or_else(|| "original".into());

                    format!(
                        "{}/{}",
                        response.images.secure_base_url.trim_end_matches('/'),
                        size
                    )
                })
                .unwrap_or_else(|| TMDB_IMAGE_BASE_FALLBACK.into())
        })
    }

    fn request<T>(&self, path: &str, params: &[(&str, String)]) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url = format!("{TMDB_API_BASE}{path}");
        let mut builder = self.client.get(url).header(ACCEPT, "application/json");
        let mut query: Vec<(String, String)> =
            params.iter().map(|(key, value)| ((*key).into(), value.clone())).collect();

        match &self.auth {
            TmdbAuth::Bearer(token) => {
                builder = builder.header(AUTHORIZATION, bearer_header(token)?);
            }
            TmdbAuth::ApiKey(key) => query.push(("api_key".into(), key.clone())),
        }

        builder
            .query(&query)
            .send()
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .ok()
    }

    fn map_movie_summary(&self, movie: TmdbMovieSummary) -> MediaItem {
        let genre_lookup = self.movie_genre_names();
        let genres = movie
            .genre_ids
            .unwrap_or_default()
            .into_iter()
            .filter_map(|genre_id| genre_lookup.get(&genre_id).cloned())
            .collect();

        MediaItem {
            id: tmdb_movie_id(movie.id),
            title: movie.title,
            description: movie.overview.unwrap_or_else(|| "No overview available yet.".into()),
            media_type: MediaType::Movie,
            genres,
            poster_url: self.poster_url(movie.poster_path.as_deref()),
            year: parse_year(movie.release_date.as_deref()),
            streams: vec![],
        }
    }

    fn map_movie_detail(&self, movie: TmdbMovieDetails) -> MediaItem {
        MediaItem {
            id: tmdb_movie_id(movie.id),
            title: movie.title,
            description: movie.overview.unwrap_or_else(|| "No overview available yet.".into()),
            media_type: MediaType::Movie,
            genres: movie.genres.into_iter().map(|genre| genre.name).collect(),
            poster_url: self.poster_url(movie.poster_path.as_deref()),
            year: parse_year(movie.release_date.as_deref()),
            streams: vec![],
        }
    }

    fn poster_url(&self, poster_path: Option<&str>) -> String {
        poster_path
            .map(|path| {
                format!(
                    "{}/{}",
                    self.poster_base_url().trim_end_matches('/'),
                    path.trim_start_matches('/')
                )
            })
            .unwrap_or_default()
    }
}

impl MetadataProvider for TmdbMetadataProvider {
    fn home_feed(&self) -> HomeFeed {
        let trending = self.trending_movies();
        let hero = trending
            .first()
            .cloned()
            .unwrap_or_else(unavailable_placeholder);
        let continue_watching = self.discover_movies(1).into_iter().take(3).collect();

        HomeFeed {
            hero,
            trending,
            continue_watching,
        }
    }

    fn catalog(&self, media_type: Option<MediaType>) -> Vec<MediaItem> {
        match media_type {
            Some(MediaType::Movie) | None => self.discover_movies(1),
            Some(MediaType::Series) | Some(MediaType::Channel) => Vec::new(),
        }
    }

    fn search(&self, query: &str) -> Vec<MediaItem> {
        let query = query.trim();
        if query.is_empty() {
            return self.catalog(Some(MediaType::Movie));
        }

        self.request::<TmdbListResponse<TmdbMovieSummary>>(
            "/search/movie",
            &[
                ("include_adult", "false".into()),
                ("language", self.language.clone()),
                ("page", "1".into()),
                ("query", query.into()),
                ("region", self.region.clone()),
            ],
        )
        .map(|response| {
            response
                .results
                .into_iter()
                .map(|movie| self.map_movie_summary(movie))
                .collect()
        })
        .unwrap_or_default()
    }

    fn item(&self, id: &str) -> Option<MediaItem> {
        let movie_id = parse_tmdb_movie_id(id)?;
        self.request::<TmdbMovieDetails>(
            &format!("/movie/{movie_id}"),
            &[("language", self.language.clone())],
        )
        .map(|movie| self.map_movie_detail(movie))
    }
}

#[derive(Debug, Clone)]
pub struct TorboxStreamProvider {
    api_key: Option<String>,
    client: Client,
}

impl TorboxStreamProvider {
    pub fn from_env() -> Self {
        Self {
            api_key: env::var("TORBOX_API_KEY")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            client: Client::builder()
                .user_agent(format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
                .build()
                .expect("torbox client should build"),
        }
    }

    fn auth_header(&self) -> Option<HeaderValue> {
        let token = self.api_key.as_ref()?;
        bearer_header(token)
    }

    fn request<T>(&self, path: &str, params: &[(&str, String)]) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let auth_header = self.auth_header()?;
        self.client
            .get(format!("https://api.torbox.app/v1/api{path}"))
            .header(ACCEPT, "application/json")
            .header(AUTHORIZATION, auth_header)
            .query(params)
            .send()
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .ok()
    }

    fn video_candidates(&self) -> Vec<TorboxTorrent> {
        self.request::<TorboxResponse<Vec<TorboxTorrent>>>("/torrents/mylist", &[])
            .filter(|response| response.success)
            .map(|response| {
                response
                    .data
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|torrent| torrent.cached || torrent.download_finished)
                    .filter(|torrent| torrent.files.iter().any(TorboxTorrentFile::is_video))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn best_match<'a>(&self, item: &MediaItem, torrents: &'a [TorboxTorrent]) -> Option<&'a TorboxTorrent> {
        let item_title = normalize_title(&item.title);

        torrents
            .iter()
            .filter_map(|torrent| {
                let score = score_torrent_match(torrent, &item_title, item.year);
                (score > 0).then_some((score, torrent))
            })
            .max_by_key(|(score, torrent)| (*score, torrent.cached as i32, torrent.download_finished as i32))
            .map(|(_, torrent)| torrent)
    }

    fn best_video_file<'a>(&self, torrent: &'a TorboxTorrent) -> Option<&'a TorboxTorrentFile> {
        torrent
            .files
            .iter()
            .filter(|file| file.is_video())
            .max_by_key(|file| file.size)
    }

    fn create_stream(&self, torrent_id: i64, file_id: i64) -> Option<TorboxCreateStreamData> {
        self.request::<TorboxResponse<TorboxCreateStreamData>>(
            "/stream/createstream",
            &[
                ("id", torrent_id.to_string()),
                ("file_id", file_id.to_string()),
                ("type", "torrent".into()),
                ("chosen_audio_index", "0".into()),
            ],
        )
        .and_then(|response| response.data)
    }

    pub fn submit_magnet(
        &self,
        item: &MediaItem,
        magnet: &str,
        only_if_cached: bool,
    ) -> AcquisitionResult {
        if self.api_key.is_none() {
            return AcquisitionResult {
                provider: "TorBox".into(),
                status: "unavailable".into(),
                message: "Set TORBOX_API_KEY before sending magnets to TorBox.".into(),
            };
        }

        let magnet = magnet.trim();
        if magnet.is_empty() {
            return AcquisitionResult {
                provider: "TorBox".into(),
                status: "missing_magnet".into(),
                message: "Paste a magnet link before sending it to TorBox.".into(),
            };
        }

        let Some(auth_header) = self.auth_header() else {
            return AcquisitionResult {
                provider: "TorBox".into(),
                status: "unavailable".into(),
                message: "TorBox authentication is not available.".into(),
            };
        };

        let form = Form::new()
            .text("magnet", magnet.to_string())
            .text("name", format!("{} ({})", item.title, item.year))
            .text("allow_zip", "false")
            .text(
                "add_only_if_cached",
                if only_if_cached { "true" } else { "false" },
            )
            .text("as_queued", if only_if_cached { "false" } else { "true" });

        let response = match self
            .client
            .post("https://api.torbox.app/v1/api/torrents/createtorrent")
            .header(ACCEPT, "application/json")
            .header(AUTHORIZATION, auth_header)
            .multipart(form)
            .send()
        {
            Ok(response) => response,
            Err(_) => {
                return AcquisitionResult {
                    provider: "TorBox".into(),
                    status: "request_failed".into(),
                    message: "TorBox could not be reached while sending this magnet.".into(),
                }
            }
        };

        let status = response.status();
        let parsed = match response.json::<TorboxResponse<TorboxCreateTorrentData>>() {
            Ok(parsed) => parsed,
            Err(_) => {
                return AcquisitionResult {
                    provider: "TorBox".into(),
                    status: "bad_response".into(),
                    message: "TorBox accepted the magnet request but returned an unreadable response.".into(),
                }
            }
        };

        if parsed.success {
            let detail = parsed
                .detail
                .unwrap_or_else(|| "Torrent added successfully.".into());
            let suffix = parsed
                .data
                .map(|data| format!(" TorBox item {} was created.", data.torrent_id))
                .unwrap_or_default();

            return AcquisitionResult {
                provider: "TorBox".into(),
                status: if only_if_cached {
                    "submitted_cached_only".into()
                } else {
                    "submitted".into()
                },
                message: format!("{detail}{suffix}"),
            };
        }

        let fallback_message = if status.is_client_error() || status.is_server_error() {
            format!("TorBox rejected the magnet request with HTTP {}.", status)
        } else {
            "TorBox could not accept that magnet.".into()
        };

        AcquisitionResult {
            provider: "TorBox".into(),
            status: parsed.error.unwrap_or_else(|| "request_failed".into()),
            message: parsed.detail.unwrap_or(fallback_message),
        }
    }
}

impl Default for TorboxStreamProvider {
    fn default() -> Self {
        Self::from_env()
    }
}

impl StreamProvider for TorboxStreamProvider {
    fn lookup(&self, item: &MediaItem) -> StreamLookup {
        if self.api_key.is_none() {
            return StreamLookup {
                provider: "TorBox".into(),
                status: "unavailable".into(),
                message: "Set TORBOX_API_KEY to query your TorBox library for streams.".into(),
                streams: vec![],
                candidates: vec![],
            };
        }

        let torrents = self.video_candidates();
        if torrents.is_empty() {
            return StreamLookup {
                provider: "TorBox".into(),
                status: "no_library_items".into(),
                message: "TorBox did not return any cached or finished video items from your library.".into(),
                streams: vec![],
                candidates: vec![],
            };
        }

        let candidates = top_torrent_candidates(item, &torrents);
        let Some(torrent) = self.best_match(item, &torrents) else {
            return StreamLookup {
                provider: "TorBox".into(),
                status: "no_match".into(),
                message: format!(
                    "No matching item was found in your TorBox library for {} ({}).",
                    item.title, item.year
                ),
                streams: vec![],
                candidates,
            };
        };

        let Some(file) = self.best_video_file(torrent) else {
            return StreamLookup {
                provider: "TorBox".into(),
                status: "no_video_file".into(),
                message: format!("TorBox found \"{}\" but no playable video file was detected.", torrent.name),
                streams: vec![],
                candidates,
            };
        };

        let Some(stream) = self.create_stream(torrent.id, file.id) else {
            return StreamLookup {
                provider: "TorBox".into(),
                status: "stream_failed".into(),
                message: format!(
                    "TorBox found \"{}\" but could not create a stream for the selected file.",
                    torrent.name
                ),
                streams: vec![],
                candidates,
            };
        };

        let quality = stream
            .metadata
            .video
            .as_ref()
            .and_then(|video| match (video.width, video.height) {
                (_, Some(height)) if height >= 2160 => Some("4K".into()),
                (_, Some(height)) if height >= 1440 => Some("1440p".into()),
                (_, Some(height)) if height >= 1080 => Some("1080p".into()),
                (_, Some(height)) if height >= 720 => Some("720p".into()),
                (Some(width), _) if width >= 3840 => Some("4K".into()),
                (Some(width), _) if width >= 1920 => Some("1080p".into()),
                _ => None,
            })
            .unwrap_or_else(|| "Auto".into());

        let language = stream
            .metadata
            .audios
            .first()
            .and_then(|audio| audio.language.clone().or(audio.language_full.clone()))
            .unwrap_or_else(|| "unknown".into());

        StreamLookup {
            provider: "TorBox".into(),
            status: "ready".into(),
            message: format!("Streaming from TorBox item \"{}\".", torrent.name),
            streams: vec![StreamSource {
                name: format!("TorBox • {}", file.short_name.as_deref().unwrap_or(&file.name)),
                quality,
                language,
                url: stream.hls_url,
            }],
            candidates,
        }
    }
}

#[derive(Clone)]
pub struct FallbackStreamProvider {
    primary: Arc<dyn StreamProvider>,
    fallback: Arc<dyn StreamProvider>,
}

impl FallbackStreamProvider {
    pub fn new(primary: Arc<dyn StreamProvider>, fallback: Arc<dyn StreamProvider>) -> Self {
        Self { primary, fallback }
    }
}

impl StreamProvider for FallbackStreamProvider {
    fn lookup(&self, item: &MediaItem) -> StreamLookup {
        let primary = self.primary.lookup(item);
        if !primary.streams.is_empty() {
            return primary;
        }

        let fallback = self.fallback.lookup(item);
        if !fallback.streams.is_empty() {
            return fallback;
        }

        primary
    }
}

#[derive(Clone)]
enum TmdbAuth {
    Bearer(String),
    ApiKey(String),
}

#[derive(Deserialize)]
struct TmdbListResponse<T> {
    results: Vec<T>,
}

#[derive(Deserialize)]
struct TmdbMovieSummary {
    id: u64,
    title: String,
    overview: Option<String>,
    poster_path: Option<String>,
    release_date: Option<String>,
    genre_ids: Option<Vec<u64>>,
}

#[derive(Deserialize)]
struct TmdbMovieDetails {
    id: u64,
    title: String,
    overview: Option<String>,
    poster_path: Option<String>,
    release_date: Option<String>,
    genres: Vec<TmdbGenre>,
}

#[derive(Deserialize)]
struct TmdbGenreListResponse {
    genres: Vec<TmdbGenre>,
}

#[derive(Deserialize)]
struct TmdbGenre {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct TmdbConfigurationResponse {
    images: TmdbConfigurationImages,
}

#[derive(Deserialize)]
struct TmdbConfigurationImages {
    secure_base_url: String,
    poster_sizes: Vec<String>,
}

#[derive(Deserialize)]
struct TorboxResponse<T> {
    success: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    detail: Option<String>,
    data: Option<T>,
}

#[derive(Deserialize)]
struct TorboxTorrent {
    id: i64,
    name: String,
    files: Vec<TorboxTorrentFile>,
    cached: bool,
    download_finished: bool,
}

#[derive(Deserialize)]
struct TorboxTorrentFile {
    id: i64,
    name: String,
    size: u64,
    short_name: Option<String>,
    mimetype: Option<String>,
}

impl TorboxTorrentFile {
    fn is_video(&self) -> bool {
        self.mimetype
            .as_ref()
            .is_some_and(|value| value.starts_with("video/"))
            || has_video_extension(&self.name)
    }
}

#[derive(Deserialize)]
struct TorboxCreateStreamData {
    hls_url: String,
    metadata: TorboxStreamMetadata,
}

#[derive(Deserialize)]
struct TorboxCreateTorrentData {
    torrent_id: i64,
}

#[derive(Deserialize)]
struct TorboxStreamMetadata {
    video: Option<TorboxVideoMetadata>,
    #[serde(default)]
    audios: Vec<TorboxAudioMetadata>,
}

#[derive(Deserialize)]
struct TorboxVideoMetadata {
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Deserialize)]
struct TorboxAudioMetadata {
    language: Option<String>,
    language_full: Option<String>,
}

fn bearer_header(token: &str) -> Option<HeaderValue> {
    let value = format!("Bearer {token}");
    HeaderValue::from_str(&value).ok()
}

fn tmdb_movie_id(id: u64) -> String {
    format!("tmdb:movie:{id}")
}

fn parse_tmdb_movie_id(id: &str) -> Option<u64> {
    id.strip_prefix("tmdb:movie:")?.parse().ok()
}

fn parse_year(release_date: Option<&str>) -> u16 {
    release_date
        .and_then(|value| value.split('-').next())
        .and_then(|year| year.parse::<u16>().ok())
        .unwrap_or(0)
}

fn unavailable_placeholder() -> MediaItem {
    MediaItem {
        id: "placeholder:unavailable".into(),
        title: "No TMDB results available".into(),
        description: "Set a TMDB API key or read token to load a real movie catalog.".into(),
        media_type: MediaType::Movie,
        genres: vec!["Setup".into()],
        poster_url: String::new(),
        year: 0,
        streams: vec![],
    }
}

fn normalize_title(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn score_torrent_match(torrent: &TorboxTorrent, normalized_title: &str, year: u16) -> i32 {
    let torrent_name = normalize_title(&torrent.name);

    let mut score = 0;
    if torrent_name.contains(normalized_title) {
        score += 50;
    }

    if normalized_title.contains(&torrent_name) {
        score += 20;
    }

    if year > 0 {
        let year_text = year.to_string();
        if torrent_name.contains(&year_text) {
            score += 15;
        }
    }

    for token in normalized_title.split_whitespace() {
        if torrent_name.contains(token) {
            score += 3;
        }
    }

    if torrent.files.iter().any(|file| normalize_title(&file.name).contains(normalized_title)) {
        score += 25;
    }

    score
}

fn has_video_extension(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".mkv", ".mp4", ".avi", ".mov", ".webm", ".m4v", ".ts"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

fn top_torrent_candidates(item: &MediaItem, torrents: &[TorboxTorrent]) -> Vec<StreamCandidate> {
    let normalized_title = normalize_title(&item.title);
    let mut scored = torrents
        .iter()
        .filter_map(|torrent| {
            let score = score_torrent_match(torrent, &normalized_title, item.year);
            (score > 0).then_some((score, torrent))
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| right.0.cmp(&left.0));

    scored
        .into_iter()
        .take(5)
        .map(|(score, torrent)| StreamCandidate {
            name: torrent.name.clone(),
            detail: format!(
                "match score {} • {} file{}",
                score,
                torrent.files.len(),
                if torrent.files.len() == 1 { "" } else { "s" }
            ),
        })
        .collect()
}

fn seed_catalog() -> Vec<MediaItem> {
    vec![
        MediaItem {
            id: "movie:solstice".into(),
            title: "Solstice Run".into(),
            description: "A courier races across flooded cities to deliver a memory core before sunrise.".into(),
            media_type: MediaType::Movie,
            genres: vec!["Sci-Fi".into(), "Thriller".into()],
            poster_url: "https://images.example.com/solstice-run.jpg".into(),
            year: 2026,
            streams: vec![
                StreamSource {
                    name: "Primary CDN".into(),
                    quality: "4K".into(),
                    language: "en".into(),
                    url: "https://stream.example.com/solstice-run/4k".into(),
                },
                StreamSource {
                    name: "Fallback Edge".into(),
                    quality: "1080p".into(),
                    language: "en".into(),
                    url: "https://stream.example.com/solstice-run/1080p".into(),
                },
            ],
        },
        MediaItem {
            id: "series:night-shift".into(),
            title: "Night Shift Atlas".into(),
            description: "A crew of orbital cartographers uncover a signal buried in forgotten star maps.".into(),
            media_type: MediaType::Series,
            genres: vec!["Sci-Fi".into(), "Mystery".into()],
            poster_url: "https://images.example.com/night-shift-atlas.jpg".into(),
            year: 2025,
            streams: vec![StreamSource {
                name: "Season 1".into(),
                quality: "1080p".into(),
                language: "en".into(),
                url: "https://stream.example.com/night-shift-atlas/s1".into(),
            }],
        },
        MediaItem {
            id: "channel:lofi-cosmos".into(),
            title: "Lo-Fi Cosmos".into(),
            description: "Continuous chill beats and slow nebula visuals for deep focus sessions.".into(),
            media_type: MediaType::Channel,
            genres: vec!["Music".into(), "Ambient".into()],
            poster_url: "https://images.example.com/lofi-cosmos.jpg".into(),
            year: 2026,
            streams: vec![StreamSource {
                name: "Live".into(),
                quality: "720p".into(),
                language: "instrumental".into(),
                url: "https://stream.example.com/lofi-cosmos/live".into(),
            }],
        },
        MediaItem {
            id: "movie:quiet-voltage".into(),
            title: "Quiet Voltage".into(),
            description: "An audio engineer discovers a citywide blackout is being choreographed like a symphony.".into(),
            media_type: MediaType::Movie,
            genres: vec!["Drama".into(), "Mystery".into()],
            poster_url: "https://images.example.com/quiet-voltage.jpg".into(),
            year: 2024,
            streams: vec![StreamSource {
                name: "Theatrical".into(),
                quality: "1080p".into(),
                language: "en".into(),
                url: "https://stream.example.com/quiet-voltage/main".into(),
            }],
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use reqwest::blocking::Client;

    use super::{
        FallbackMetadataProvider, FallbackStreamProvider, MetadataProvider, SeededLibraryProvider,
        StreamProvider, TmdbMetadataProvider, TorboxStreamProvider, TorboxTorrent,
        has_video_extension, score_torrent_match,
    };
    use crate::domain::{MediaItem, MediaType};

    #[test]
    fn seeded_provider_filters_catalog_by_type() {
        let provider = SeededLibraryProvider::demo();

        let items = provider.catalog(Some(MediaType::Movie));

        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| item.id.starts_with("movie:")));
    }

    #[test]
    fn seeded_provider_searches_title_and_genre() {
        let provider = SeededLibraryProvider::demo();

        let by_title = provider.search("atlas");
        let by_genre = provider.search("ambient");

        assert_eq!(by_title.len(), 1);
        assert_eq!(by_title[0].id, "series:night-shift");
        assert_eq!(by_genre.len(), 1);
        assert_eq!(by_genre[0].id, "channel:lofi-cosmos");
    }

    #[test]
    fn fallback_stream_provider_uses_seeded_data_when_torbox_has_no_match() {
        let seeded: Arc<dyn StreamProvider> = Arc::new(SeededLibraryProvider::demo());
        let fallback =
            FallbackStreamProvider::new(Arc::new(TorboxStreamProvider::default()), seeded);

        let item = MediaItem {
            id: "movie:solstice".into(),
            title: "Solstice Run".into(),
            description: String::new(),
            media_type: MediaType::Movie,
            genres: vec![],
            poster_url: String::new(),
            year: 2026,
            streams: vec![],
        };
        let lookup = fallback.lookup(&item);

        assert_eq!(lookup.streams.len(), 2);
        assert_eq!(lookup.streams[0].name, "Primary CDN");
    }

    #[test]
    fn torrent_match_scoring_prefers_title_and_year() {
        let torrent = TorboxTorrent {
            id: 1,
            name: "War Machine 2026 1080p BluRay".into(),
            files: vec![],
            cached: true,
            download_finished: true,
        };

        let score = score_torrent_match(&torrent, "war machine", 2026);

        assert!(score >= 65);
    }

    #[test]
    fn fallback_metadata_provider_uses_seeded_results_when_primary_is_empty() {
        let primary: Arc<dyn MetadataProvider> =
            Arc::new(TmdbMetadataProvider::new(super::TmdbAuth::ApiKey("demo".into())));
        let fallback: Arc<dyn MetadataProvider> = Arc::new(SeededLibraryProvider::demo());
        let provider = FallbackMetadataProvider::new(primary, fallback);

        let items = provider.catalog(Some(MediaType::Movie));

        assert_eq!(items.len(), 2);
    }

    #[test]
    fn detects_common_video_extensions() {
        assert!(has_video_extension("movie.mkv"));
        assert!(has_video_extension("movie.mp4"));
        assert!(!has_video_extension("subtitle.srt"));
    }

    #[test]
    fn submit_magnet_requires_api_key() {
        let provider = TorboxStreamProvider {
            api_key: None,
            client: Client::builder()
                .build()
                .expect("test reqwest client should build"),
        };
        let item = MediaItem {
            id: "tmdb:movie:1".into(),
            title: "War Machine".into(),
            description: String::new(),
            media_type: MediaType::Movie,
            genres: vec![],
            poster_url: String::new(),
            year: 2026,
            streams: vec![],
        };

        let result = provider.submit_magnet(&item, "magnet:?xt=urn:btih:demo", true);

        assert_eq!(result.status, "unavailable");
    }
}
