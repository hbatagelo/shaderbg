// ShaderBG
// Copyright (c) 2026 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unix-domain-socket server.

use gtk::{gio, glib, prelude::*};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use crate::{app::*, *};

/// Returns the absolute path of the socket for `ipc_name`, or `None`
/// when `$XDG_RUNTIME_DIR` is unset.
pub fn socket_path(ipc_name: &str) -> Option<PathBuf> {
    let dir = dirs::runtime_dir()?.join(APP_NAME);
    Some(dir.join(format!("{ipc_name}.sock")))
}

/// Returns `true` if the socket at `path` is a leftover from a dead process.
fn is_stale_socket(path: &Path) -> bool {
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => false, // Someone is listening
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => true, // Stale
        Err(_) => false, // Unknown; treat as in use
    }
}

/// Binds the IPC socket for the configured instance name and starts the
/// listener on the GTK main loop.
///
/// If the name is already owned by a live instance, logs a warning and
/// returns without IPC.
/// The service is stored on `AppData` to keep it alive.
pub fn setup_ipc(app: &gtk::Application) {
    let ipc_name = {
        let app_data = get_data!(app, AppData, as_ref());
        app_data.cli_config.ipc_name.clone()
    };

    let Some(socket_path) = socket_path(&ipc_name) else {
        log::warn!("$XDG_RUNTIME_DIR not set; IPC disabled");
        return;
    };

    if let Some(parent) = socket_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        log::warn!("Cannot create socket dir {}: {e}", parent.display());
        return;
    }

    if socket_path.exists() {
        if !is_stale_socket(&socket_path) {
            log::warn!(
                "Socket {} already in use by another instance; \
                 pass --ipc-name <NAME> to run a second instance",
                socket_path.display()
            );
            return;
        }
        log::debug!("Removing stale socket: {}", socket_path.display());
        if let Err(e) = std::fs::remove_file(&socket_path) {
            log::warn!("Failed to remove stale socket: {e}");
            return;
        }
    }

    let service = gio::SocketService::new();
    let address = gio::UnixSocketAddress::new(&socket_path);
    if let Err(err) = service.add_address(
        &address,
        gio::SocketType::Stream,
        gio::SocketProtocol::Default,
        None::<&gio::Cancellable>,
    ) {
        log::warn!("Failed to bind socket {}: {err}", socket_path.display());
        return;
    }

    service.connect_incoming(glib::clone!(
        #[weak]
        app,
        #[upgrade_or]
        false,
        move |_service, connection, _source| {
            glib::spawn_future_local(glib::clone!(
                #[weak]
                app,
                #[strong]
                connection,
                async move {
                    serve_client(&app, connection).await;
                }
            ));
            true
        }
    ));

    service.start();
    log::info!("Listening on {}", socket_path.display());

    let app_data = get_data!(app, AppData, as_mut());
    app_data.ipc_service = Some(service);
    app_data.ipc_socket_path = Some(socket_path);
}

/// Serves one client connection until it disconnects.
async fn serve_client(app: &gtk::Application, connection: gio::SocketConnection) {
    let input = gio::DataInputStream::new(&connection.input_stream());
    let output = connection.output_stream();

    // Reads and responds to the JSONL messages.
    loop {
        let line = match input.read_line_future(glib::Priority::DEFAULT).await {
            Ok(bytes) => {
                let Some(slice) = bytes.as_ref() else { break };
                let Ok(s) = std::str::from_utf8(&slice[..]) else {
                    break;
                };
                s.to_owned()
            }
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        let response = handle_message(app, line.trim_end());
        let mut payload = response;
        payload.push('\n');

        if output
            .write_all_future(payload.into_bytes(), glib::Priority::DEFAULT)
            .await
            .is_err()
        {
            break;
        }
    }
}

/// Parses a JSON request, dispatches on its `op` field and returns a
/// JSON response.
fn handle_message(app: &gtk::Application, line: &str) -> String {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return json!({"ok": false, "error": format!("invalid json: {e}")}).to_string(),
    };

    match req.get("op").and_then(|v| v.as_str()) {
        Some("set") => handle_set(app, &req),
        Some("get") => handle_get(app, &req),
        Some("info") => handle_info(app),
        _ => json!({"ok": false, "error": "unknown or missing op"}).to_string(),
    }
}

/// `set` operation.
fn handle_set(app: &gtk::Application, req: &Value) -> String {
    let Some(obj) = req.get("uniforms").and_then(|v| v.as_object()) else {
        return json!({"ok": false, "error": "missing 'uniforms' object"}).to_string();
    };

    // Validate first, then apply (atomic per-message).
    let mut updates: Vec<(usize, [f32; 4])> = Vec::new();
    for (name, value) in obj {
        let Some(idx) = custom_uniform_index(name) else {
            return json!({"ok": false, "error": format!("unknown uniform: {name}")}).to_string();
        };
        let Some(arr) = value.as_array() else {
            return json!({"ok": false, "error": format!("{name}: expected 4-element array")})
                .to_string();
        };
        if arr.len() != 4 {
            return json!({"ok": false, "error": format!("{name}: expected 4 elements")})
                .to_string();
        }
        let mut v = [0.0f32; 4];
        for (i, x) in arr.iter().enumerate() {
            let Some(f) = x.as_f64() else {
                return json!({"ok": false, "error": format!("{name}[{i}]: not a number")})
                    .to_string();
            };
            v[i] = f as f32;
        }
        updates.push((idx, v));
    }

    let app_data = get_data!(app, AppData, as_mut());
    for (idx, v) in updates {
        app_data.custom_uniforms[idx] = v;
    }

    json!({"ok": true}).to_string()
}

/// `get` operation. Returns the current values of the requested uniforms,
/// or all four when `uniforms` is omitted.
fn handle_get(app: &gtk::Application, req: &Value) -> String {
    let names = req.get("uniforms").and_then(|v| v.as_array());
    let app_data = get_data!(app, AppData, as_ref());

    let mut out = serde_json::Map::new();
    match names {
        Some(list) if !list.is_empty() => {
            for name in list {
                let Some(s) = name.as_str() else { continue };
                let Some(idx) = custom_uniform_index(s) else {
                    return json!({"ok": false, "error": format!("unknown uniform: {s}")})
                        .to_string();
                };
                out.insert(s.into(), json!(app_data.custom_uniforms[idx]));
            }
        }
        _ => {
            for i in 0..4 {
                out.insert(format!("iCustom{i}"), json!(app_data.custom_uniforms[i]));
            }
        }
    }

    json!({"ok": true, "uniforms": out}).to_string()
}

/// `info` operation. Returns the instance name and preset metadata.
fn handle_info(app: &gtk::Application) -> String {
    let app_data = get_data!(app, AppData, as_ref());
    let preset = &app_data.cli_config.preset;
    json!({
        "ok": true,
        "name": app_data.cli_config.ipc_name,
        "preset": {
            "id": preset.id,
            "name": preset.name,
            "username": preset.username,
        }
    })
    .to_string()
}

/// Maps `iCustom0..3` to an index (0..3).
fn custom_uniform_index(name: &str) -> Option<usize> {
    match name {
        "iCustom0" => Some(0),
        "iCustom1" => Some(1),
        "iCustom2" => Some(2),
        "iCustom3" => Some(3),
        _ => None,
    }
}
