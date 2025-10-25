// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::{preset::*, shadertoy::*, *};

pub fn parse_args() -> Result<(Preset, Option<PathBuf>, bool), String> {
    ensure_user_data_dir().map_err(|err| format!("Failed to setup data directory: {err}"))?;

    let presets_dir = presets_dir();

    let cmd = clap::Command::new(APP_NAME)
        .author(APP_AUTHOR)
        .version(APP_SEMVER)
        .about(APP_ABOUT)
        .override_usage(format!(
            "{} [<file>] | [<shader_id> <api_key>] [--no-overlay]",
            APP_NAME
        ))
        .arg(
            clap::Arg::new("arg1")
                .index(1)
                .help("Path to TOML preset file OR ShaderToy shader ID")
                .required(false),
        )
        .arg(
            clap::Arg::new("arg2")
                .index(2)
                .help("ShaderToy API key (when first argument is shader ID)")
                .required(false),
        )
        .arg(
            clap::Arg::new("no-overlay")
                .long("no-overlay")
                .help("Disable the shader info overlay")
                .action(clap::ArgAction::SetTrue),
        )
        .after_help("Run with no arguments to use a random preset");

    let matches = cmd.get_matches();

    let arg1 = matches.get_one::<String>("arg1").map(|s| s.as_str());
    let arg2 = matches.get_one::<String>("arg2").map(|s| s.as_str());
    let show_overlay = !matches.get_flag("no-overlay");

    let (preset, preset_file) = match (arg1, arg2) {
        (None, None) => load_preset_from_directory(&presets_dir)?,
        (Some(preset_file), None) => {
            let file = PathBuf::from(preset_file);
            load_preset_from_file(&file)?
        }
        (Some(shader_id), Some(api_key)) => load_from_web(shader_id, api_key)?,
        (None, Some(_)) => unreachable!("API key provided without shader ID"),
    };

    if let Some(path) = &preset_file {
        log::info!("Loaded {}", path.display());
    }

    Ok((preset, preset_file, show_overlay))
}

fn ensure_user_data_dir() -> std::io::Result<()> {
    let user_data_dir = dirs::data_local_dir().unwrap_or_else(|| {
        log::warn!("Could not find $XDG_DATA_HOME or $HOME/.local/share; using current directory.");
        std::env::current_dir().expect("Failed to get current working directory")
    });
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
            copy_recursively(system_data_dir, &app_data_dir)?;
        } else {
            log::warn!("No source data directory found: {:?}", &system_data_dir);
        }
    }
    Ok(())
}

fn copy_recursively(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> std::io::Result<()> {
    fs::create_dir_all(&destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = destination.as_ref().join(entry.file_name());

        if file_type.is_dir() {
            copy_recursively(entry.path(), dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}
