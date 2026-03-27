use std::process::Command;
use tauri::Manager;

use crate::{
    addons::MoveDirection,
    domain::{AcquisitionResult, AddonDescriptor, HomeFeed, MediaItem, StreamLookup, StreamSource},
    state::AppState,
};

#[tauri::command]
async fn get_home_feed(state: tauri::State<'_, AppState>) -> Result<HomeFeed, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.home_feed())
        .await
        .map_err(|error| format!("Home feed task failed: {error}"))
}

#[tauri::command]
async fn get_addons(state: tauri::State<'_, AppState>) -> Result<Vec<AddonDescriptor>, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.addons())
        .await
        .map_err(|error| format!("Addon task failed: {error}"))
}

#[tauri::command]
fn install_addon_url(
    state: tauri::State<'_, AppState>,
    manifest_url: String,
) -> Result<AddonDescriptor, String> {
    state.install_addon_url(&manifest_url)
}

#[tauri::command]
fn set_remote_addon_enabled(
    state: tauri::State<'_, AppState>,
    manifest_url: String,
    enabled: bool,
) -> Result<(), String> {
    state.set_remote_addon_enabled(&manifest_url, enabled)
}

#[tauri::command]
fn remove_remote_addon(
    state: tauri::State<'_, AppState>,
    manifest_url: String,
) -> Result<(), String> {
    state.remove_remote_addon(&manifest_url)
}

#[tauri::command]
fn move_remote_addon(
    state: tauri::State<'_, AppState>,
    manifest_url: String,
    direction: String,
) -> Result<(), String> {
    let direction = match direction.trim().to_lowercase().as_str() {
        "up" => MoveDirection::Up,
        "down" => MoveDirection::Down,
        _ => return Err("Direction must be 'up' or 'down'.".into()),
    };
    state.move_remote_addon(&manifest_url, direction)
}

#[tauri::command]
async fn get_catalog(
    state: tauri::State<'_, AppState>,
    media_type: Option<String>,
) -> Result<Vec<MediaItem>, String> {
    let state = state.inner().clone();
    let parsed = media_type.as_ref().and_then(|raw| parse_media_type(raw));
    tauri::async_runtime::spawn_blocking(move || state.catalog(parsed))
        .await
        .map_err(|error| format!("Catalog task failed: {error}"))
}

#[tauri::command]
async fn search_catalog(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<MediaItem>, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.search(&query))
        .await
        .map_err(|error| format!("Search task failed: {error}"))
}

#[tauri::command]
async fn get_media_item(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<MediaItem, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .item(&id)
            .ok_or_else(|| format!("No media item found for {id}"))
    })
    .await
    .map_err(|error| format!("Meta task failed: {error}"))?
}

#[tauri::command]
fn get_streams(state: tauri::State<'_, AppState>, id: String) -> Result<Vec<StreamSource>, String> {
    state
        .streams(&id)
        .ok_or_else(|| format!("No streams found for {id}"))
}

#[tauri::command]
async fn get_stream_lookup(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<StreamLookup, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .stream_lookup(&id)
            .ok_or_else(|| format!("No stream lookup available for {id}"))
    })
    .await
    .map_err(|error| format!("Stream lookup task failed: {error}"))?
}

#[tauri::command]
async fn submit_torbox_magnet(
    state: tauri::State<'_, AppState>,
    id: String,
    magnet: String,
    only_if_cached: bool,
) -> Result<AcquisitionResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .submit_torbox_magnet(&id, &magnet, only_if_cached)
            .ok_or_else(|| format!("No media item found for {id}"))
    })
    .await
    .map_err(|error| format!("TorBox submit task failed: {error}"))?
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    let url = url.trim().to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("Only http and https URLs can be opened externally.".into());
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(&url);
        command
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(&url);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", &url]);
        command
    };

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open the source externally: {error}"))
}

#[tauri::command]
fn toggle_window_maximize(window: tauri::WebviewWindow) -> Result<(), String> {
    let is_maximized = window
        .is_maximized()
        .map_err(|error| format!("Could not check window state: {error}"))?;

    if is_maximized {
        window
            .unmaximize()
            .map_err(|error| format!("Could not restore window: {error}"))?;
    } else {
        window
            .maximize()
            .map_err(|error| format!("Could not maximize window: {error}"))?;
    }

    Ok(())
}

#[tauri::command]
fn toggle_window_fullscreen(window: tauri::WebviewWindow) -> Result<bool, String> {
    let is_fullscreen = window
        .is_fullscreen()
        .map_err(|error| format!("Could not check fullscreen state: {error}"))?;
    let next_state = !is_fullscreen;

    window
        .set_fullscreen(next_state)
        .map_err(|error| format!("Could not change fullscreen state: {error}"))?;

    Ok(next_state)
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::demo())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.set_title("Sol")?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_addons,
            install_addon_url,
            set_remote_addon_enabled,
            remove_remote_addon,
            move_remote_addon,
            get_home_feed,
            get_catalog,
            search_catalog,
            get_media_item,
            get_stream_lookup,
            get_streams,
            submit_torbox_magnet,
            open_external_url,
            toggle_window_maximize,
            toggle_window_fullscreen
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Sol desktop application");
}

fn parse_media_type(raw: &str) -> Option<crate::domain::MediaType> {
    match raw.trim().to_lowercase().as_str() {
        "movie" => Some(crate::domain::MediaType::Movie),
        "series" => Some(crate::domain::MediaType::Series),
        "channel" => Some(crate::domain::MediaType::Channel),
        _ => None,
    }
}
