use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    domain::{ApiMessage, MediaType},
    state::AppState,
};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/home", get(home))
        .route("/api/catalog", get(catalog))
        .route("/api/meta/{id}", get(meta))
        .route("/api/search", get(search))
        .route("/api/streams/{id}", get(streams))
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
            "/api/catalog?type=movie|series|channel",
            "/api/meta/{id}",
            "/api/search?q=atlas",
            "/api/streams/{id}",
        ],
    })
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn home(State(state): State<AppState>) -> Json<crate::domain::HomeFeed> {
    Json(state.home_feed())
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
    state
        .streams(&id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

fn parse_media_type(raw: &String) -> Option<MediaType> {
    match raw.trim().to_lowercase().as_str() {
        "movie" => Some(MediaType::Movie),
        "series" => Some(MediaType::Series),
        "channel" => Some(MediaType::Channel),
        _ => None,
    }
}
