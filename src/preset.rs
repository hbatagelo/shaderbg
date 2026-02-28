// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! Preset system and configuration model.
//!
//! Defines the serialized shader preset format, including render passes,
//! inputs, timing behavior, and display configuration.
//!
//! Supports loading presets from TOML files, importing ShaderToy JSON exports,
//! applying validated defaults during deserialization, and monitoring preset
//! files for live reloading at runtime.

use gtk::{gio, prelude::*};
use serde::*;
use std::{
    collections::hash_map::DefaultHasher,
    ffi::OsStr,
    fs,
    hash::{Hash, Hasher},
    io,
    path::*,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

use crate::{app::*, *};

#[derive(Debug, Error)]
pub enum PresetError {
    #[error("I/O error")]
    Io(#[from] io::Error),
    #[error("TOML parse error")]
    TomlParse(#[from] toml::de::Error),
    #[error("JSON parse error")]
    JsonParse(#[from] serde_json::Error),
    #[error("Failed to import from JSON: {0}")]
    Import(String),
    #[error("No .toml presets found in directory")]
    NoPresets,
}

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputType {
    #[default]
    Misc,
    Texture,
    Cubemap,
    Volume,
    Video,
    Music,
    MusicStream,
    Keyboard,
    Webcam,
    Microphone,
}

/// Specifies how texture coordinates outside the 0-1 range are handled.
#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WrapMode {
    #[default]
    /// Clamps texture coordinates to the edge (extends border pixels).
    Clamp,
    /// Repeats the texture by wrapping coordinates (tiles the texture).
    Repeat,
}

/// Specifies the texture filtering method used for sampling.
#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    #[default]
    /// Linear interpolation.
    Linear,
    /// Nearest-neighbor sampling.
    Nearest,
    /// Mipmapping with trilinear filtering.
    Mipmap,
}

/// Specifies how the virtual screen bounds are calculated.
#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenBoundsPolicy {
    #[default]
    /// Union of all available monitors.
    AllMonitors,
    /// Union of user-selected monitors (see `Preset::monitor_selection`).
    SelectedMonitors,
    /// Per-monitor isolation (clone mode).
    Cloned,
}

/// Specifies how the framebuffer is laid out on screen.
#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    #[default]
    /// Scales the framebuffer to fill the screen.
    Stretch,
    /// Centers the framebuffer without scaling (may underscan).
    Center,
    /// Tiles the framebuffer by repeating it.
    Repeat,
    /// Tiles the framebuffer using mirror-repeat wrapping.
    MirroredRepeat,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Input {
    /// Type of input resource.
    #[serde(default, rename = "type")]
    pub _type: InputType,
    /// Identifier or path of the input resource.
    #[serde(default)]
    pub name: String,
    /// Texture coordinate wrapping mode used when sampling.
    #[serde(default)]
    pub wrap: WrapMode,
    /// Filtering mode applied during texture sampling.
    #[serde(default)]
    pub filter: FilterMode,
    /// Whether to vertically flip the input.
    #[serde(default)]
    pub vflip: bool,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Pass {
    /// Shader source code.
    #[serde(default)]
    pub shader: String,
    /// iChannel0 input.
    #[serde(default)]
    pub input_0: Option<Input>,
    /// iChannel1 input.
    #[serde(default)]
    pub input_1: Option<Input>,
    /// iChannel2 input.
    #[serde(default)]
    pub input_2: Option<Input>,
    /// iChannel3 input.
    #[serde(default)]
    pub input_3: Option<Input>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Preset {
    /// Shader ID.
    #[serde(default)]
    pub id: String,
    /// Shader name.
    #[serde(default)]
    pub name: String,
    /// Shader username.
    #[serde(default)]
    pub username: String,
    // Shader description.
    #[serde(default)]
    pub description: String,
    /// Scaling factor for the frame resolution.
    #[serde(
        default = "defaults::resolution_scale",
        deserialize_with = "validators::clamp_resolution_scale"
    )]
    pub resolution_scale: f32,
    /// Scaling factor for time-based shader uniforms (`iTime`/`iTimeDelta`).
    #[serde(
        default = "defaults::time_scale",
        deserialize_with = "validators::clamp_time_scale"
    )]
    pub time_scale: f64,
    /// Constant offset added to `iTime` shader uniform.
    #[serde(default, with = "humantime_serde")]
    pub time_offset: Duration,
    /// Minimum time between frames.
    #[serde(default, with = "humantime_serde")]
    pub interval_between_frames: Duration,
    /// How the bounds of the virtual screen are calculated.
    #[serde(default)]
    pub screen_bounds_policy: ScreenBoundsPolicy,
    /// Monitor selection using DRM connector names.
    #[serde(default = "defaults::monitor_selection")]
    pub monitor_selection: Vec<String>,
    /// How the framebuffer is laid out on the screen.
    #[serde(default)]
    pub layout_mode: LayoutMode,
    /// Filtering mode when scaling the framebuffer.
    #[serde(default)]
    pub filter_mode: FilterMode,
    /// Controls smooth frame transitions through cross fading.
    /// (0.0 = no overlap, 1.0 = always transitioning)
    #[serde(default, deserialize_with = "validators::clamp_crossfade")]
    pub crossfade_overlap_ratio: f64,
    /// "Common" pass (shader-only).
    #[serde(default)]
    pub common: Option<Pass>,
    /// "Buffer A" render pass.
    #[serde(default)]
    pub buffer_a: Option<Pass>,
    /// "Buffer B" render pass.
    #[serde(default)]
    pub buffer_b: Option<Pass>,
    /// "Buffer C" render pass.
    #[serde(default)]
    pub buffer_c: Option<Pass>,
    /// "Buffer D" render pass.
    #[serde(default)]
    pub buffer_d: Option<Pass>,
    /// "Cube A" render pass.
    #[serde(default)]
    pub cube_a: Option<Pass>,
    /// "Image" render pass.
    #[serde(default = "defaults::image")]
    pub image: Pass,
}

impl Preset {
    /// Creates a Preset from a TOML file.
    pub fn from_toml_file(path: &Path) -> Result<Self, PresetError> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    /// Creates a Preset from a ShaderToy JSON export file.
    pub fn from_json_file(path: &Path) -> Result<Self, PresetError> {
        crate::shadertoy::importer::import_from_json_file(path)
    }

    /// Creates a Preset with all serde defaults applied.
    pub fn with_serde_defaults() -> Self {
        toml::from_str("").expect("Failed to create default preset")
    }
}

/// Default values for preset fields.
pub mod defaults {
    use super::*;

    /// Default framebuffer resolution scale (`1.0` = native screen resolution).
    pub fn resolution_scale() -> f32 {
        1.0
    }

    /// Default time progression multiplier (`1.0` = real-time speed).
    pub fn time_scale() -> f64 {
        1.0
    }

    /// Default monitor selection ( `*` = all available monitors).
    pub fn monitor_selection() -> Vec<String> {
        vec!["*".into()]
    }

    /// Default "Image" pass configuration.
    /// Provides a minimal shader so a preset remains valid even when
    /// no render passes are explicitly defined.
    pub fn image() -> Pass {
        Pass {
            shader: default_image_shader(),
            input_0: None,
            input_1: None,
            input_2: None,
            input_3: None,
        }
    }

    /// Built-in fallback fragment shader.
    ///
    /// Displays an animated color pattern.
    pub fn default_image_shader() -> String {
        r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;
    vec3 col = .5 + .5 * cos(iTime + uv.xyx + vec3(0, 2, 4));
    fragColor = vec4(col, 1);
}
    "#
        .trim()
        .to_string()
    }
}

/// Validation functions applied during deserialization.
///
/// Invalid values are clamped instead of producing hard errors.
mod validators {
    use super::*;

    /// Ensures `resolution_scale` is non-negative.
    pub fn clamp_resolution_scale<'de, D>(deserializer: D) -> Result<f32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f32::deserialize(deserializer)?;
        Ok(value.max(0.0))
    }

    /// Ensures `time_scale` is non-negative.
    pub fn clamp_time_scale<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Ok(value.max(0.0))
    }

    /// Restricts crossfade overlap ratio to the valid range `[0.0, 1.0]`.
    pub fn clamp_crossfade<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Ok(value.clamp(0.0, 1.0))
    }
}

/// Loads preset from a file.
pub fn load_preset_from_toml_file(path: &Path) -> Result<(Preset, Option<PathBuf>), PresetError> {
    Ok((Preset::from_toml_file(path)?, Some(path.to_path_buf())))
}

/// Loads a preset from a JSON file exported from ShaderToy.
///
/// Parses the JSON file and saves the resulting configuration to the local presets directory.
pub fn load_preset_from_json_file(path: &Path) -> Result<(Preset, Option<PathBuf>), PresetError> {
    let preset = Preset::from_json_file(path)?;
    let saved_path = save_to_presets_directory(&preset)?;
    Ok((preset, Some(saved_path)))
}

/// Loads a random preset from the given directory.
pub fn load_preset_from_directory(dir: &Path) -> Result<(Preset, Option<PathBuf>), PresetError> {
    let toml_files: Vec<_> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension() == Some(OsStr::new("toml")))
        .collect();

    let chosen = toml_files
        .get(random_index(toml_files.len()))
        .ok_or(PresetError::NoPresets)?;

    Ok((Preset::from_toml_file(chosen)?, Some(chosen.clone())))
}

/// Returns a random index in the range `[0, len)` using system time as seed.
fn random_index(len: usize) -> usize {
    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .hash(&mut hasher);
    (hasher.finish() as usize) % len
}

/// Derives the preset filename from its shader ID.
fn preset_filename(preset: &Preset) -> Result<String, PresetError> {
    if preset.id.trim().is_empty() {
        Err(PresetError::Import("Preset has no shader ID".into()))
    } else {
        Ok(format!("{}.toml", preset.id))
    }
}

/// Saves preset to the presets directory.
fn save_to_presets_directory(preset: &Preset) -> Result<PathBuf, PresetError> {
    let path = presets_dir().join(preset_filename(preset)?);
    save_preset_to_file(preset, &path)?;
    log::debug!("Saved preset '{}'", preset.id);
    Ok(path)
}

/// Saves preset to a TOML file.
fn save_preset_to_file(preset: &Preset, path: &Path) -> Result<(), PresetError> {
    let toml = toml::to_string_pretty(preset).map_err(|e| PresetError::Import(e.to_string()))?;
    fs::write(path, toml)?;
    Ok(())
}

/// Returns the directory used to store presets.
///
/// Resolution order:
/// 1. `$XDG_DATA_HOME/shaderg/presets`
/// 2. `$HOME/.local/share/shaderg/presets`
/// 3. Current working directory (fallback)
///
/// The directory is created if it does not exist.
///
/// If the standard data directory cannot be determined or created,
/// a warning is logged and the current working directory is used
/// instead to ensure the application remains functional.
pub fn presets_dir() -> PathBuf {
    /// Fallback directory used when no data directory is available.
    fn fallback_dir() -> PathBuf {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    let dir = dirs::data_local_dir()
        .map(|p| p.join(APP_NAME).join("presets"))
        .unwrap_or_else(|| {
            log::warn!("Could not determine data directory; using current directory.");
            fallback_dir()
        });

    // Ensure the directory exists before returning it.
    if !dir.exists() {
        if let Err(err) = std::fs::create_dir_all(&dir) {
            log::warn!(
                "Failed to create presets directory at {}: {err}",
                dir.display()
            );
            return fallback_dir();
        }
    }

    dir
}

/// Sets up filesystem monitoring for a preset file.
///
/// Registers a `gio::FileMonitor` that watches `preset_path` and invokes
/// `on_change` after the file has finished changing (`ChangesDoneHint`).
///
/// The monitor is stored inside application data to keep it alive for the
/// lifetime of the application; dropping the monitor would stop event delivery.
pub fn setup_preset_monitor<F>(app: &gtk::Application, preset_path: &Path, on_change: F)
where
    F: Fn(&gtk::Application, &Path) + 'static,
{
    let file = gio::File::for_path(preset_path);

    let monitor = match file.monitor(
        gio::FileMonitorFlags::NONE,
        None::<gio::Cancellable>.as_ref(),
    ) {
        Ok(monitor) => monitor,
        Err(err) => {
            log::error!("Failed to create preset file monitor: {err}");
            return;
        }
    };

    // Trigger callback once file modifications are fully written.
    let app_clone = app.clone();
    monitor.connect_changed(move |_, changed_file, _, event_type| {
        if event_type == gio::FileMonitorEvent::ChangesDoneHint {
            if let Some(path) = changed_file.path() {
                log::info!("Preset file changed: {}", path.display());
                on_change(&app_clone, &path);
            }
        }
    });

    // Store the monitor so it is not dropped.
    // Dropping the monitor cancels filesystem notifications.
    let app_data = get_data!(app, AppData, as_mut());
    app_data.preset_monitor = Some(monitor);
}
