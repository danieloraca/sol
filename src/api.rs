use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, put},
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    domain::{ApiMessage, MediaType, WatchProgressEntry},
    state::AppState,
};

#[derive(serde::Deserialize)]
struct WatchProgressPayload {
    progress_percent: f32,
    position_seconds: u32,
    duration_seconds: u32,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/home", get(home))
        .route("/api/addons", get(addons))
        .route("/api/catalog", get(catalog))
        .route("/api/meta/{id}", get(meta))
        .route("/api/search", get(search))
        .route("/api/streams/{id}", get(streams))
        .route("/api/watch-progress", get(list_watch_progress))
        .route(
            "/api/watch-progress/{id}",
            put(upsert_watch_progress).delete(remove_watch_progress),
        )
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn index() -> Json<ApiMessage> {
    Json(ApiMessage {
        name: "sol".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: "Starter backend for a Stremio-like media platform in Rust.".into(),
        routes: vec![
            "/health",
            "/api/home",
            "/api/addons",
            "/api/catalog?type=movie|series|channel",
            "/api/meta/{id}",
            "/api/search?q=atlas",
            "/api/streams/{id}",
            "/api/watch-progress",
            "/api/watch-progress/{id}",
        ],
    })
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn home(State(state): State<AppState>) -> Json<crate::domain::HomeFeed> {
    Json(state.home_feed())
}

async fn addons(State(state): State<AppState>) -> Json<Vec<crate::domain::AddonDescriptor>> {
    Json(state.addons())
}

async fn catalog(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<crate::domain::MediaItem>> {
    let media_type = params.get("type").and_then(parse_media_type);
    Json(state.catalog(media_type))
}

async fn meta(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::domain::MediaItem>, StatusCode> {
    state.item(&id).map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn search(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<crate::domain::MediaItem>> {
    let query = params.get("q").map_or("", String::as_str);
    Json(state.search(query))
}

async fn streams(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<crate::domain::StreamSource>>, StatusCode> {
    state.streams(&id).map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn list_watch_progress(
    State(state): State<AppState>,
) -> Result<Json<Vec<WatchProgressEntry>>, StatusCode> {
    state
        .watch_progress()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn upsert_watch_progress(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<WatchProgressPayload>,
) -> StatusCode {
    match state.save_watch_progress(
        &id,
        payload.progress_percent,
        payload.position_seconds,
        payload.duration_seconds,
    ) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn remove_watch_progress(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> StatusCode {
    match state.delete_watch_progress(&id) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn parse_media_type(raw: &String) -> Option<MediaType> {
    match raw.trim().to_lowercase().as_str() {
        "movie" => Some(MediaType::Movie),
        "series" => Some(MediaType::Series),
        "channel" => Some(MediaType::Channel),
        _ => None,
    }
}
