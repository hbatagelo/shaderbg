// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use clap::{Arg, ArgAction, Command};
use thiserror::Error;

use crate::{preset::*, *};

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Failed to initialize user data directory")]
    DataDir(#[from] io::Error),

    #[error("Preset error: {0}")]
    Preset(#[from] PresetError),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

pub fn parse_args() -> Result<(Preset, Option<PathBuf>, bool), CliError> {
    ensure_user_data_dir()?;

    let presets_dir = presets_dir();

    let matches = Command::new(APP_NAME)
        .author(APP_AUTHOR)
        .version(APP_SEMVER)
        .about(APP_ABOUT)
        .arg(
            Arg::new("file")
                .value_name("FILE")
                .help("Path to TOML preset file or ShaderToy JSON export")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("no-overlay")
                .long("no-overlay")
                .help("Disable the shader info overlay")
                .action(ArgAction::SetTrue),
        )
        .after_help("Run with no arguments to use a random preset")
        .get_matches();

    let show_overlay = !matches.get_flag("no-overlay");

    let (preset, preset_path) = match matches.get_one::<PathBuf>("file") {
        None => load_preset_from_directory(&presets_dir)?,
        Some(path) => load_preset_from_file_or_json(path)?,
    };

    if let Some(path) = &preset_path {
        log::info!("Loaded {}", path.display());
    }

    Ok((preset, preset_path, show_overlay))
}

fn load_preset_from_file_or_json(file: &Path) -> Result<(Preset, Option<PathBuf>), CliError> {
    let resolved = if file.exists() {
        file.to_path_buf()
    } else {
        let candidate = presets_dir().join(file);
        if candidate.exists() {
            candidate
        } else {
            return Err(CliError::InvalidInput(format!(
                "File not found: {}",
                file.display()
            )));
        }
    };

    match resolved.extension().and_then(|s| s.to_str()) {
        Some("toml") => Ok(load_preset_from_toml_file(&resolved)?),
        Some("json") => Ok(load_preset_from_json_file(&resolved)?),
        _ => load_preset_from_toml_file(&resolved)
            .or_else(|_| load_preset_from_json_file(&resolved))
            .map_err(Into::into),
    }
}

fn ensure_user_data_dir() -> std::io::Result<()> {
    let user_data_dir = dirs::data_local_dir()
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Could not determine user data directory ($XDG_DATA_HOME or $HOME/.local/share)",
            )
        })?;

    let app_data_dir = user_data_dir.join(APP_NAME);

    if !app_data_dir.exists() {
        log::info!("Creating {:?}", &app_data_dir);
        fs::create_dir_all(&app_data_dir)?;

        let system_data_dir = if env::var("FLATPAK_ID").is_ok() {
            Path::new("/app/share")
        } else {
            Path::new("/usr/share")
        }
        .join(APP_NAME);

        log::info!("Copying from {:?} to {:?}", system_data_dir, app_data_dir);

        if system_data_dir.exists() {
            copy_recursively_overwriting(system_data_dir, &app_data_dir)?;
        } else {
            log::warn!("No source data directory found: {:?}", &system_data_dir);
        }
    }
    Ok(())
}

fn copy_recursively_overwriting(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> std::io::Result<()> {
    fs::create_dir_all(&destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let dest = destination.as_ref().join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_recursively_overwriting(entry.path(), dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}
