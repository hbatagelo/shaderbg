// ShaderBG
// Copyright (c) 2026 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! A small client for the shaderbg IPC socket, invoked with `shaderbg ctl`.

use gtk::glib;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
};

use crate::{
    APP_NAME,
    cli::{IpcCliConfig, IpcCommand},
    ipc_server::socket_path,
};

/// Entry point.
pub fn run(config: &IpcCliConfig) -> glib::ExitCode {
    match &config.command {
        IpcCommand::List => list_instances(),
        IpcCommand::Info => run_and_print(&config.ipc_name, &json!({"op": "info"})),
        IpcCommand::Get { uniforms } => {
            let req = if uniforms.is_empty() {
                json!({"op": "get"})
            } else {
                json!({"op": "get", "uniforms": uniforms})
            };
            run_and_print(&config.ipc_name, &req)
        }
        IpcCommand::Set { uniforms } => {
            let mut map = serde_json::Map::new();
            for (name, v) in uniforms {
                map.insert(name.clone(), json!(v));
            }
            run_and_print(&config.ipc_name, &json!({"op": "set", "uniforms": map}))
        }
    }
}

/// Sends one request to the named instance and prints the JSON response.
fn run_and_print(ipc_name: &str, request: &Value) -> glib::ExitCode {
    let Some(path) = socket_path(ipc_name) else {
        eprintln!("$XDG_RUNTIME_DIR not set");
        return glib::ExitCode::FAILURE;
    };
    match roundtrip(&path, request) {
        Ok(resp) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&resp).unwrap_or_default()
            );
            let ok = resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            if ok {
                glib::ExitCode::SUCCESS
            } else {
                glib::ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("Cannot talk to '{ipc_name}' at {}: {e}", path.display());
            glib::ExitCode::FAILURE
        }
    }
}

/// Enumerates instances by scanning `$XDG_RUNTIME_DIR/shaderbg/` for sockets
/// and querying each one for preset metadata. Sockets that fail to respond
/// are labeled `(stale)`.
fn list_instances() -> glib::ExitCode {
    let Some(dir) = dirs::runtime_dir().map(|p| p.join(APP_NAME)) else {
        eprintln!("$XDG_RUNTIME_DIR not set");
        return glib::ExitCode::FAILURE;
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Cannot read {}: {e}", dir.display());
            return glib::ExitCode::FAILURE;
        }
    };

    let mut found = false;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("sock") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        found = true;
        match roundtrip(&path, &json!({"op": "info"})) {
            Ok(info) if info.get("ok").and_then(|v| v.as_bool()) == Some(true) => {
                let preset = info.get("preset").cloned().unwrap_or(Value::Null);
                let id = preset.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let pname = preset.get("name").and_then(|v| v.as_str()).unwrap_or("");
                println!("{name}\t\"{pname}\" ({id})");
            }
            _ => println!("{name}\t(stale)"),
        }
    }

    if found {
        glib::ExitCode::SUCCESS
    } else {
        glib::ExitCode::FAILURE
    }
}

/// Opens a short-lived connection, sends one JSONL request and reads
/// one JSONL reply (every invocation uses a fresh connection).
fn roundtrip(path: &PathBuf, request: &Value) -> std::io::Result<Value> {
    let mut stream = UnixStream::connect(path)?;
    let mut line = serde_json::to_string(request).unwrap();
    line.push('\n');
    stream.write_all(line.as_bytes())?;

    let mut reader = BufReader::new(stream);
    let mut buf = String::new();
    reader.read_line(&mut buf)?;
    serde_json::from_str(&buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
