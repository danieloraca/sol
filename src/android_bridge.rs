#![cfg(target_os = "android")]

use jni::{
    objects::{JClass, JString},
    sys::{jboolean, jstring, JNI_TRUE},
    JNIEnv,
};
use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use crate::{
    addons::{AddonRegistry, AddonStore, MoveDirection, RemoteHttpAddon},
    secrets::SecretStore,
};

static ADDON_STORE_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

fn addon_store_path_cell() -> &'static Mutex<Option<PathBuf>> {
    ADDON_STORE_PATH.get_or_init(|| Mutex::new(None))
}

fn into_jstring(env: JNIEnv, value: &str) -> jstring {
    match env.new_string(value) {
        Ok(v) => v.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn from_jstring(env: &mut JNIEnv, value: JString) -> Option<String> {
    env.get_string(&value)
        .ok()
        .map(|raw| raw.to_string_lossy().into_owned())
}

fn addon_store() -> AddonStore {
    let maybe_path = addon_store_path_cell()
        .lock()
        .ok()
        .and_then(|guard| (*guard).clone());
    if let Some(path) = maybe_path {
        AddonStore::with_path(path)
    } else {
        AddonStore::default()
    }
}

fn build_registry() -> AddonRegistry {
    let _ = SecretStore.load_into_env();
    let store = addon_store();
    AddonRegistry::from_manifest_urls(&store.enabled_urls())
}

fn json_error(message: &str) -> String {
    serde_json::json!({
        "ok": false,
        "error": message
    })
    .to_string()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_soltv_bridge_RustBridge_nativeInitialize(
    mut env: JNIEnv,
    _class: JClass,
    storage_dir: JString,
    default_addons_json: JString,
) -> jstring {
    let storage_dir = from_jstring(&mut env, storage_dir).unwrap_or_default();
    let default_addons_json = from_jstring(&mut env, default_addons_json).unwrap_or_default();
    let path = PathBuf::from(storage_dir).join("sol.addons.json");

    let result = (|| -> Result<String, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create addon directory: {error}"))?;
        }

        if !path.exists() && !default_addons_json.trim().is_empty() {
            // Validate seed JSON before writing.
            serde_json::from_str::<serde_json::Value>(&default_addons_json)
                .map_err(|error| format!("Invalid default addon JSON: {error}"))?;
            fs::write(&path, default_addons_json)
                .map_err(|error| format!("Could not seed addon settings: {error}"))?;
        }

        if let Ok(mut guard) = addon_store_path_cell().lock() {
            *guard = Some(path);
        }

        Ok(
            serde_json::json!({
                "ok": true,
                "message": "Addon store initialized."
            })
            .to_string(),
        )
    })();

    let payload = result.unwrap_or_else(|error| json_error(&error));
    into_jstring(env, &payload)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_soltv_bridge_RustBridge_nativePing(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    into_jstring(env, "sol native core ready")
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_soltv_bridge_RustBridge_nativeGetInstalledAddonsJson(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let store = addon_store();
    let json = serde_json::to_string(&store.remote_addons()).unwrap_or_else(|_| "[]".to_string());
    into_jstring(env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_soltv_bridge_RustBridge_nativeInstallAddonUrl(
    mut env: JNIEnv,
    _class: JClass,
    manifest_url: JString,
) -> jstring {
    let manifest_url = from_jstring(&mut env, manifest_url).unwrap_or_default();
    let result = (|| -> Result<String, String> {
        let addon = RemoteHttpAddon::install(&manifest_url)?;
        let descriptor = addon.descriptor();
        let store = addon_store();
        store.install_remote_addon(&manifest_url, &descriptor)?;

        Ok(
            serde_json::json!({
                "ok": true,
                "descriptor": descriptor
            })
            .to_string(),
        )
    })();
    let payload = result.unwrap_or_else(|error| json_error(&error));
    into_jstring(env, &payload)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_soltv_bridge_RustBridge_nativeSetRemoteAddonEnabled(
    mut env: JNIEnv,
    _class: JClass,
    manifest_url: JString,
    enabled: jboolean,
) -> jstring {
    let manifest_url = from_jstring(&mut env, manifest_url).unwrap_or_default();
    let enabled = enabled == JNI_TRUE;
    let result = addon_store().set_remote_enabled(&manifest_url, enabled);
    let payload = match result {
        Ok(_) => serde_json::json!({ "ok": true }).to_string(),
        Err(error) => json_error(&error),
    };
    into_jstring(env, &payload)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_soltv_bridge_RustBridge_nativeRemoveRemoteAddon(
    mut env: JNIEnv,
    _class: JClass,
    manifest_url: JString,
) -> jstring {
    let manifest_url = from_jstring(&mut env, manifest_url).unwrap_or_default();
    let result = addon_store().remove_remote_addon(&manifest_url);
    let payload = match result {
        Ok(_) => serde_json::json!({ "ok": true }).to_string(),
        Err(error) => json_error(&error),
    };
    into_jstring(env, &payload)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_soltv_bridge_RustBridge_nativeMoveRemoteAddon(
    mut env: JNIEnv,
    _class: JClass,
    manifest_url: JString,
    direction: JString,
) -> jstring {
    let manifest_url = from_jstring(&mut env, manifest_url).unwrap_or_default();
    let direction = from_jstring(&mut env, direction).unwrap_or_default();
    let direction = match direction.trim().to_lowercase().as_str() {
        "up" => MoveDirection::Up,
        "down" => MoveDirection::Down,
        _ => {
            return into_jstring(
                env,
                &json_error("Direction must be 'up' or 'down'."),
            );
        }
    };

    let result = addon_store().move_remote_addon(&manifest_url, direction);
    let payload = match result {
        Ok(_) => serde_json::json!({ "ok": true }).to_string(),
        Err(error) => json_error(&error),
    };
    into_jstring(env, &payload)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_soltv_bridge_RustBridge_nativeGetHomeFeedJson(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let json = std::panic::catch_unwind(|| {
        let registry = build_registry();
        let feed = registry.home_feed();
        serde_json::to_string(&feed)
            .unwrap_or_else(|_| r#"{"hero":{"title":"Serialize error"},"trending":[]}"#.to_string())
    })
    .unwrap_or_else(|_| {
        r#"{
          "hero": { "title": "Sol Native Hero" },
          "trending": []
        }"#
        .to_string()
    });

    into_jstring(env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_soltv_bridge_RustBridge_nativeGetCatalogJson(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let json = std::panic::catch_unwind(|| {
        let registry = build_registry();
        let catalog = registry.catalog(None);
        serde_json::to_string(&catalog).unwrap_or_else(|_| "[]".to_string())
    })
    .unwrap_or_else(|_| "[]".to_string());

    into_jstring(env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_soltv_bridge_RustBridge_nativeGetStreamsJson(
    mut env: JNIEnv,
    _class: JClass,
    item_id: JString,
) -> jstring {
    let item_id = from_jstring(&mut env, item_id).unwrap_or_else(|| "unknown".to_string());
    let escaped_item = item_id.replace('"', "");

    let streams_json = std::panic::catch_unwind(|| {
        let registry = build_registry();
        let maybe_item = registry.item(&item_id).or_else(|| {
            registry
                .catalog(None)
                .into_iter()
                .find(|item| item.id == item_id)
        });

        if let Some(item) = maybe_item {
            let lookup = registry.stream_lookup(&item);
            serde_json::to_string(&lookup).unwrap_or_else(|_| {
                format!(
                    r#"{{
                      "provider": "Addons",
                      "status": "error",
                      "message": "Could not serialize stream lookup for {escaped_item}.",
                      "streams": [],
                      "candidates": []
                    }}"#
                )
            })
        } else {
            format!(
                r#"{{
                  "provider": "Addons",
                  "status": "not_found",
                  "message": "Item not found for id {escaped_item}.",
                  "streams": [],
                  "candidates": []
                }}"#
            )
        }
    })
    .unwrap_or_else(|_| {
        format!(
            r#"{{
              "provider": "Addons",
              "status": "panic",
              "message": "Native stream lookup panicked for {escaped_item}.",
              "streams": [],
              "candidates": []
            }}"#
        )
    });

    into_jstring(env, &streams_json)
}
