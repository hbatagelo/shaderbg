// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

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

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WrapMode {
    #[default]
    Clamp,
    Repeat,
}

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    #[default]
    Linear,
    Nearest,
    Mipmap,
}

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenBoundsPolicy {
    #[default]
    AllMonitors,
    SelectedMonitors,
    Cloned,
}

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    #[default]
    Stretch,
    Center,
    Repeat,
    MirroredRepeat,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Input {
    #[serde(default, rename = "type")]
    pub _type: InputType,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub wrap: WrapMode,
    #[serde(default)]
    pub filter: FilterMode,
    #[serde(default)]
    pub vflip: bool,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Pass {
    #[serde(default)]
    pub shader: String,
    #[serde(default)]
    pub input_0: Option<Input>,
    #[serde(default)]
    pub input_1: Option<Input>,
    #[serde(default)]
    pub input_2: Option<Input>,
    #[serde(default)]
    pub input_3: Option<Input>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Preset {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub username: String,

    #[serde(default)]
    pub description: String,

    #[serde(
        default = "defaults::resolution_scale",
        deserialize_with = "validators::clamp_resolution_scale"
    )]
    pub resolution_scale: f32,

    #[serde(
        default = "defaults::time_scale",
        deserialize_with = "validators::clamp_time_scale"
    )]
    pub time_scale: f64,

    #[serde(default, with = "humantime_serde")]
    pub time_offset: Duration,

    #[serde(default, with = "humantime_serde")]
    pub interval_between_frames: Duration,

    #[serde(default)]
    pub screen_bounds_policy: ScreenBoundsPolicy,

    #[serde(default = "defaults::monitor_selection")]
    pub monitor_selection: Vec<String>,

    #[serde(default)]
    pub layout_mode: LayoutMode,

    #[serde(default)]
    pub filter_mode: FilterMode,

    #[serde(default, deserialize_with = "validators::clamp_crossfade")]
    pub crossfade_overlap_ratio: f64,

    #[serde(default)]
    pub common: Option<Pass>,

    #[serde(default)]
    pub buffer_a: Option<Pass>,

    #[serde(default)]
    pub buffer_b: Option<Pass>,

    #[serde(default)]
    pub buffer_c: Option<Pass>,

    #[serde(default)]
    pub buffer_d: Option<Pass>,

    #[serde(default)]
    pub cube_a: Option<Pass>,

    #[serde(default = "defaults::image")]
    pub image: Pass,
}

impl Preset {
    pub fn from_toml_file(path: &Path) -> Result<Self, PresetError> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
    pub fn from_json_file(path: &Path) -> Result<Self, PresetError> {
        crate::shadertoy::importer::import_from_json_file(path)
    }
    pub fn with_serde_defaults() -> Self {
        toml::from_str("").expect("Failed to create default preset")
    }
}

pub mod defaults {
    use super::*;

    pub fn resolution_scale() -> f32 {
        1.0
    }

    pub fn time_scale() -> f64 {
        1.0
    }

    pub fn monitor_selection() -> Vec<String> {
        vec!["*".into()]
    }

    pub fn image() -> Pass {
        Pass {
            shader: default_image_shader(),
            input_0: None,
            input_1: None,
            input_2: None,
            input_3: None,
        }
    }

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

mod validators {
    use super::*;

    pub fn clamp_resolution_scale<'de, D>(deserializer: D) -> Result<f32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f32::deserialize(deserializer)?;
        Ok(value.max(0.0))
    }

    pub fn clamp_time_scale<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Ok(value.max(0.0))
    }

    pub fn clamp_crossfade<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Ok(value.clamp(0.0, 1.0))
    }
}

pub fn load_preset_from_toml_file(path: &Path) -> Result<(Preset, Option<PathBuf>), PresetError> {
    Ok((Preset::from_toml_file(path)?, Some(path.to_path_buf())))
}

pub fn load_preset_from_json_file(path: &Path) -> Result<(Preset, Option<PathBuf>), PresetError> {
    let preset = Preset::from_json_file(path)?;
    let saved_path = save_to_presets_directory(&preset)?;
    Ok((preset, Some(saved_path)))
}

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

fn random_index(len: usize) -> usize {
    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .hash(&mut hasher);
    (hasher.finish() as usize) % len
}

fn preset_filename(preset: &Preset) -> Result<String, PresetError> {
    if preset.id.trim().is_empty() {
        Err(PresetError::Import("Preset has no shader ID".into()))
    } else {
        Ok(format!("{}.toml", preset.id))
    }
}

fn save_to_presets_directory(preset: &Preset) -> Result<PathBuf, PresetError> {
    let path = presets_dir().join(preset_filename(preset)?);
    save_preset_to_file(preset, &path)?;
    log::debug!("Saved preset '{}'", preset.id);
    Ok(path)
}

fn save_preset_to_file(preset: &Preset, path: &Path) -> Result<(), PresetError> {
    let toml = toml::to_string_pretty(preset).map_err(|e| PresetError::Import(e.to_string()))?;
    fs::write(path, toml)?;
    Ok(())
}

pub fn presets_dir() -> PathBuf {
    fn fallback_dir() -> PathBuf {
        std::env::current_dir().expect("Failed to get current working directory")
    }

    let dir = dirs::data_local_dir()
        .map(|p| p.join(APP_NAME).join("presets"))
        .unwrap_or_else(|| {
            log::warn!(
                "Could not find $XDG_DATA_HOME or $HOME/.local/share; using current directory."
            );
            fallback_dir()
        });

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

    let app_clone = app.clone();
    monitor.connect_changed(move |_, changed_file, _, event_type| {
        if event_type == gio::FileMonitorEvent::ChangesDoneHint {
            if let Some(path) = changed_file.path() {
                log::info!("Preset file changed: {}", path.display());
                on_change(&app_clone, &path);
            }
        }
    });

    let app_data = get_data!(app, AppData, as_mut());
    app_data.preset_monitor = Some(monitor);
    app_data.preset_file = Some(preset_path.to_path_buf());
}
