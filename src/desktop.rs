use tauri::Manager;
use std::process::Command;

use crate::{
    addons::MoveDirection,
    domain::{
        AcquisitionResult, AddonDescriptor, HomeFeed, MediaItem, SourceSearchResult, StreamLookup,
        StreamSource,
    },
    state::AppState,
};

#[tauri::command]
fn get_home_feed(state: tauri::State<'_, AppState>) -> HomeFeed {
    state.home_feed()
}

#[tauri::command]
fn get_addons(state: tauri::State<'_, AppState>) -> Vec<AddonDescriptor> {
    state.addons()
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
fn get_catalog(state: tauri::State<'_, AppState>, media_type: Option<String>) -> Vec<MediaItem> {
    state.catalog(media_type.as_ref().and_then(|raw| parse_media_type(raw)))
}

#[tauri::command]
fn search_catalog(state: tauri::State<'_, AppState>, query: String) -> Vec<MediaItem> {
    state.search(&query)
}

#[tauri::command]
fn get_media_item(state: tauri::State<'_, AppState>, id: String) -> Result<MediaItem, String> {
    state
        .item(&id)
        .ok_or_else(|| format!("No media item found for {id}"))
}

#[tauri::command]
fn get_streams(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Vec<StreamSource>, String> {
    state
        .streams(&id)
        .ok_or_else(|| format!("No streams found for {id}"))
}

#[tauri::command]
fn get_stream_lookup(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<StreamLookup, String> {
    state
        .stream_lookup(&id)
        .ok_or_else(|| format!("No stream lookup available for {id}"))
}

#[tauri::command]
fn submit_torbox_magnet(
    state: tauri::State<'_, AppState>,
    id: String,
    magnet: String,
    only_if_cached: bool,
) -> Result<AcquisitionResult, String> {
    state
        .submit_torbox_magnet(&id, &magnet, only_if_cached)
        .ok_or_else(|| format!("No media item found for {id}"))
}

#[tauri::command]
fn search_sources(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<SourceSearchResult, String> {
    state
        .search_sources(&id)
        .ok_or_else(|| format!("No media item found for {id}"))
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
            search_sources,
            open_external_url,
            toggle_window_maximize
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
