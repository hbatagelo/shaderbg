// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command-line interface and startup configuration.
//!
//! Responsible for parsing command-line arguments and loading the
//! initial [`Preset`].
//!
//! Produces a [`CliConfig`] describing the runtime configuration used
//! to initialize the application.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use clap::{Arg, ArgAction, Command};
use gtk4_layer_shell::Layer;
use thiserror::Error;

use crate::{preset::*, *};

/// Layer-shell layer on which to place the input-capture window.
#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum InputLayer {
    Background,
    Bottom,
    Top,
    Overlay,
}

impl From<InputLayer> for Layer {
    fn from(layer: InputLayer) -> Self {
        match layer {
            InputLayer::Background => Layer::Background,
            InputLayer::Bottom => Layer::Bottom,
            InputLayer::Top => Layer::Top,
            InputLayer::Overlay => Layer::Overlay,
        }
    }
}

/// Errors that may occur during CLI parsing or startup initialization.
#[derive(Debug, Error)]
pub enum CliError {
    #[error("Failed to initialize user data directory")]
    DataDir(#[from] io::Error),

    #[error("Preset error: {0}")]
    Preset(#[from] PresetError),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Result of command-line parsing.
///
/// Describes the runtime configuration chosen at startup.
#[derive(Debug)]
pub struct CliConfig {
    /// Loaded preset configuration.
    ///
    /// May change after initialization due to hot-reloading.
    pub preset: Preset,

    /// Path to the preset file used to initialize the application.
    ///
    /// `None` when a generated or default preset is used.
    pub preset_path: Option<PathBuf>,

    /// Enables the on-screen shader information overlay.
    pub show_overlay: bool,

    /// Exclusive zone of the render window. Default: -1, or 0 on River.
    pub exclusive_zone: i32,

    /// Layer of the input-capture window. Default: background, or bottom on COSMIC.
    pub input_layer: Layer,

    /// IPC instance name (determines the socket filename).
    pub ipc_name: String,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            preset: Preset::with_serde_defaults(),
            preset_path: None,
            show_overlay: true,
            exclusive_zone: default_exclusive_zone(),
            input_layer: default_input_layer(),
            ipc_name: "default".into(),
        }
    }
}

/// Outcome of command-line parsing.
pub enum Invocation {
    /// Runs shaderbg as a normal application.
    App(Box<CliConfig>),

    /// Runs shaderbg as an IPC client (`shaderbg ctl`).
    IpcClient(IpcCliConfig),
}

/// Configuration for `shaderbg ctl`.
pub struct IpcCliConfig {
    /// Target instance name.
    pub ipc_name: String,

    // Subcommand to execute.
    pub command: IpcCommand,
}

/// Subcommand executed by `shaderbg ctl`.
pub enum IpcCommand {
    /// Enumerate live instances and print their preset metadata.
    List,

    /// Print preset metadata for the target instance.
    Info,

    /// Print current values of the named uniforms.
    Get { uniforms: Vec<String> },

    /// Set the named uniforms.
    Set { uniforms: Vec<(String, [f32; 4])> },
}

/// Parses command-line arguments and resolves the initial preset.
///
/// - No arguments: load a random preset.
/// - One argument:
///   - If file path: load a TOML preset or ShaderToy JSON export.
///   - If `ctl`: run the IPC client subcommand.
pub fn parse_args() -> Result<Invocation, CliError> {
    ensure_user_data_dir()?;

    let presets_directory = presets_dir();

    let matches = Command::new(APP_NAME)
        .author(APP_AUTHOR)
        .version(APP_SEMVER)
        .about(APP_ABOUT)
        .subcommand(ctl_subcommand())
        .arg(
            Arg::new("file")
                .value_name("FILE")
                .help("Path to TOML preset or ShaderToy JSON export. If empty, a random preset is loaded.")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("no-overlay")
                .long("no-overlay")
                .help("Disable the shader info overlay")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("exclusive-zone")
                .long("exclusive-zone")
                .value_name("ZONE")
                .help("Exclusive zone of the render window [default: -1, or 0 on River]")
                .value_parser(clap::value_parser!(i32))
                .allow_negative_numbers(true),
        )
        .arg(
            Arg::new("input-layer")
                .long("input-layer")
                .value_name("LAYER")
                .help(
                    "Layer of the input-capture window [default: background, or bottom on COSMIC]",
                )
                .value_parser(clap::value_parser!(InputLayer)),
        )
        .arg(
            Arg::new("ipc-name")
                .long("ipc-name")
                .value_name("NAME")
                .help("IPC instance name (default: \"default\")")
                .global(true),
        )
        .args_conflicts_with_subcommands(true)
        .get_matches();

    let show_overlay = !matches.get_flag("no-overlay");

    let exclusive_zone = matches
        .get_one::<i32>("exclusive-zone")
        .copied()
        .unwrap_or_else(default_exclusive_zone);
    let input_layer = matches
        .get_one::<InputLayer>("input-layer")
        .copied()
        .map(Into::into)
        .unwrap_or_else(default_input_layer);
    let ipc_name = matches
        .get_one::<String>("ipc-name")
        .cloned()
        .unwrap_or_else(|| "default".into());

    if let Some(("ctl", ctl)) = matches.subcommand() {
        return parse_ctl(ctl, ipc_name);
    }

    log::debug!("exclusive_zone={exclusive_zone}, input_layer={input_layer:?}");

    let (preset, preset_path) = match matches.get_one::<PathBuf>("file") {
        // No arguments: use a random preset from the presets directory
        None => load_preset_from_directory(&presets_directory)?,
        // One argument: treat as a file (TOML or JSON)
        Some(path) => load_preset_from_file_or_json(path)?,
    };

    if let Some(path) = &preset_path {
        log::info!("Loaded {}", path.display());
    }

    Ok(Invocation::App(Box::new(CliConfig {
        preset,
        preset_path,
        show_overlay,
        exclusive_zone,
        input_layer,
        ipc_name,
    })))
}

/// Loads a preset from either an explicit filesystem path,
/// or a filename located inside the presets directory.
///
/// File type is determined by extension when possible,
/// otherwise TOML and JSON loaders are attempted sequentially.
fn load_preset_from_file_or_json(file: &Path) -> Result<(Preset, Option<PathBuf>), CliError> {
    let resolved = if file.exists() {
        file.to_path_buf()
    } else {
        presets_dir().join(file)
    };

    if !resolved.exists() {
        return Err(CliError::InvalidInput(format!(
            "File not found: {}",
            file.display()
        )));
    };

    match resolved.extension().and_then(|s| s.to_str()) {
        Some("toml") => Ok(load_preset_from_toml_file(&resolved)?),
        Some("json") => Ok(load_preset_from_json_file(&resolved)?),
        _ => load_preset_from_toml_file(&resolved)
            .or_else(|_| load_preset_from_json_file(&resolved))
            .map_err(Into::into),
    }
}

/// Ensures the per-user application data directory exists.
///
/// On first run:
/// - Creates `$XDG_DATA_HOME/<APP_NAME>`
/// - Copies bundled default assets from:
///     - `/usr/share/<APP_NAME>` (system install), or
///     - `/app/share/<APP_NAME>` (Flatpak sandbox)
///
/// This allows writable user configuration while keeping
/// packaged defaults read-only.
fn ensure_user_data_dir() -> std::io::Result<()> {
    // Resolve user data directory according to XDG Base Directory spec.
    // Falls back to current working directory when unavailable.
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
        log::info!("Creating {:?}", app_data_dir);
        fs::create_dir_all(&app_data_dir)?;

        // Determine the system-wide data directory based on the environment.
        // Priority: compile-time override > Flatpak > FHS system install.
        let system_data_dir = if let Some(dir) = option_env!("SHADERBG_SYSTEM_DATA_DIR") {
            // Baked in at build time (e.g. Nix sets this to the store path)
            Path::new(dir)
        } else if env::var("FLATPAK_ID").is_ok() {
            // Flatpak sandbox
            Path::new("/app/share")
        } else {
            // Standard system installation
            Path::new("/usr/share")
        }
        .join(APP_NAME);

        log::info!("Copying from {:?} to {:?}", system_data_dir, app_data_dir);

        if system_data_dir.exists() {
            copy_recursively_overwriting(system_data_dir, &app_data_dir)?;
        } else {
            log::warn!("No source data directory found: {:?}", system_data_dir);
        }
    }
    Ok(())
}

/// Recursively copies directory contents, overwriting existing files.
///
/// Existing files are replaced unconditionally.
/// Directory structure is created as needed.
///
/// Intended for bootstrapping user data from packaged defaults.
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

/// Normalized value of $XDG_CURRENT_DESKTOP.
fn current_desktop() -> String {
    env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase()
}

/// River is the outlier that requires exclusive zone 0. With -1 the render
/// window behaves correctly everywhere else tested (Sway, Hyprland, Niri,
/// Miriway, COSMIC).
fn default_exclusive_zone() -> i32 {
    if current_desktop().contains("river") {
        0
    } else {
        -1
    }
}

/// COSMIC does not grant focus to surfaces on the background layer, so the
/// input-capture window must sit on the bottom layer there.
fn default_input_layer() -> Layer {
    if current_desktop().contains("cosmic") {
        Layer::Bottom
    } else {
        Layer::Background
    }
}

/// Builds the `ctl` subcommand tree.
fn ctl_subcommand() -> Command {
    Command::new("ctl")
        .about("Control a running shaderbg instance via IPC")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("list").about("List running instances"))
        .subcommand(Command::new("info").about("Show preset metadata"))
        .subcommand(
            Command::new("get")
                .about("Print current uniform values, e.g. get iCustom0 iCustom1")
                .arg(Arg::new("uniforms").num_args(0..).value_name("iCustomN")),
        )
        .subcommand(
            Command::new("set")
                .about("Set uniform values, e.g. set iCustom0=0.5,0.5,0.5,1.0")
                .arg(
                    Arg::new("uniforms")
                        .num_args(1..)
                        .value_name("NAME=x,y,z,w")
                        .required(true)
                        .value_parser(parse_uniform_arg),
                ),
        )
}

/// Parses one `set` argument of the form `NAME=x,y,z,w`.
fn parse_uniform_arg(s: &str) -> Result<(String, [f32; 4]), String> {
    let (name, values) = s
        .split_once('=')
        .ok_or_else(|| format!("expected NAME=x,y,z,w, got '{s}'"))?;
    let comps: Vec<&str> = values.split(',').collect();
    if comps.len() != 4 {
        return Err(format!(
            "expected 4 comma-separated floats, got '{}'",
            values
        ));
    }
    let mut out = [0.0f32; 4];
    for (i, c) in comps.iter().enumerate() {
        out[i] = c
            .trim()
            .parse::<f32>()
            .map_err(|e| format!("{name}[{i}]: {e}"))?;
    }
    Ok((name.to_string(), out))
}

/// Converts the parsed `ctl` subcommand matches into an `Invocation`.
fn parse_ctl(matches: &clap::ArgMatches, ipc_name: String) -> Result<Invocation, CliError> {
    let (cmd_name, cmd_matches) = matches.subcommand().unwrap();
    let command = match cmd_name {
        "list" => IpcCommand::List,
        "info" => IpcCommand::Info,
        "get" => IpcCommand::Get {
            uniforms: cmd_matches
                .get_many::<String>("uniforms")
                .map(|v| v.cloned().collect())
                .unwrap_or_default(),
        },
        "set" => IpcCommand::Set {
            uniforms: cmd_matches
                .get_many::<(String, [f32; 4])>("uniforms")
                .unwrap()
                .cloned()
                .collect(),
        },
        _ => unreachable!(),
    };
    Ok(Invocation::IpcClient(IpcCliConfig { ipc_name, command }))
}
