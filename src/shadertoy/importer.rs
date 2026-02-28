// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! ShaderToy JSON importer.
//!
//! Converts ShaderToy JSON export files into internal [`Preset`]
//! representations used by the renderer.
//!
//! This importer is intentionally permissive: unsupported ShaderToy
//! features degrade gracefully instead of aborting import.

use crate::preset::*;
use std::{fs, path::Path};

/// Imports a ShaderToy JSON export into a [`Preset`].
///
/// The JSON must follow the structure produced by the ShaderToy API.
///
/// The importer initializes a preset using serde defaults, fills
/// metadata fields from `Shader.info`, and then reconstructs the
/// render passes and channel inputs.
///
/// Unsupported passes or channel types are ignored with warnings.
///
/// Returns an error when mandatory JSON fields are missing.
pub fn import_from_json_file(path: &Path) -> Result<Preset, PresetError> {
    let json_str = fs::read_to_string(path)?;

    let json_value: serde_json::Value = serde_json::from_str(&json_str)?;

    let shader_obj = json_value
        .get("Shader")
        .ok_or_else(|| PresetError::Import("Missing 'Shader' key".into()))?;
    let info = shader_obj
        .get("info")
        .ok_or_else(|| PresetError::Import("Missing 'info' key".into()))?;

    let mut preset = Preset::with_serde_defaults();

    preset.id = info
        .get("id")
        .and_then(|id| id.as_str())
        .unwrap_or("")
        .to_string();

    preset.name = info
        .get("name")
        .and_then(|name| name.as_str())
        .unwrap_or("")
        .to_string();

    preset.username = info
        .get("username")
        .and_then(|username| username.as_str())
        .unwrap_or("")
        .to_string();

    preset.description = info
        .get("description")
        .and_then(|description| description.as_str())
        .unwrap_or("")
        .to_string();

    let renderpasses = shader_obj
        .get("renderpass")
        .and_then(|rp| rp.as_array())
        .ok_or_else(|| PresetError::Import("Missing or invalid 'renderpass' array".into()))?;

    for pass in renderpasses {
        process_single_pass(&mut preset, pass)?;
    }

    log::debug!("Loaded JSON successfully");

    Ok(preset)
}

/// Converts a single ShaderToy render pass into an internal [`Pass`].
///
/// Unknown or unsupported pass types are ignored.
fn process_single_pass(preset: &mut Preset, pass: &serde_json::Value) -> Result<(), PresetError> {
    let name = pass
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| PresetError::Import("Missing pass 'name'".into()))?;

    let code = pass
        .get("code")
        .and_then(|c| c.as_str())
        .ok_or_else(|| PresetError::Import("Missing pass 'code'".into()))?
        .to_string();

    let inputs = pass
        .get("inputs")
        .and_then(|i| i.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    let pass_inputs = process_pass_inputs(inputs, name)?;

    let [input_0, input_1, input_2, input_3] = pass_inputs;

    let pass_config = Pass {
        shader: code,
        input_0,
        input_1,
        input_2,
        input_3,
    };

    match name {
        "Common" => preset.common = Some(pass_config),
        "Buffer A" | "Buf A" => preset.buffer_a = Some(pass_config),
        "Buffer B" | "Buf B" => preset.buffer_b = Some(pass_config),
        "Buffer C" | "Buf C" => preset.buffer_c = Some(pass_config),
        "Buffer D" | "Buf D" => preset.buffer_d = Some(pass_config),
        "Cube A" => preset.cube_a = Some(pass_config),
        "Image" => preset.image = pass_config,
        "Sound" => {
            log::warn!("'Sound' pass type ignored (not supported)");
        }
        _ => {
            log::warn!("Unknown pass type '{name}'");
        }
    }

    Ok(())
}

/// Builds the four ShaderToy input channels (`iChannel0..3`)
/// for a render pass.
///
/// Missing channels remain `None`.
/// Channels outside the valid range `[0,3]` are ignored.
fn process_pass_inputs(
    inputs: &[serde_json::Value],
    pass_name: &str,
) -> Result<[Option<Input>; 4], PresetError> {
    let mut pass_inputs: [Option<Input>; 4] = core::array::from_fn(|_| None);

    for input in inputs {
        if let Some(processed_input) = process_single_input(input, pass_name)? {
            let channel = input.get("channel").and_then(|c| c.as_i64()).unwrap_or(-1);
            if (0..=3).contains(&channel) {
                pass_inputs[channel as usize] = Some(processed_input);
            }
        }
    }

    Ok(pass_inputs)
}

/// Converts a single ShaderToy channel description into an [`Input`].
///
/// Unsupported channel types are logged and replaced by a
/// fallback input so the preset remains loadable.
fn process_single_input(
    input: &serde_json::Value,
    pass_name: &str,
) -> Result<Option<Input>, PresetError> {
    let ctype = input.get("ctype").and_then(|t| t.as_str()).unwrap_or("");

    if !is_supported_channel_type(ctype) {
        let channel = input.get("channel").and_then(|c| c.as_i64()).unwrap_or(-1);
        log::warn!("{pass_name}: 'iChannel{channel}' input '{ctype}' not supported");
    }

    let sampler = input.get("sampler").unwrap_or(&serde_json::Value::Null);
    let src = input.get("src").and_then(|s| s.as_str()).unwrap_or("");
    let input_config = create_input_config(ctype, src, sampler)?;

    Ok(Some(input_config))
}

/// Returns whether a ShaderToy channel type is supported by ShaderBG.
///
/// Unsupported types are accepted during import but replaced with
/// fallback inputs.
fn is_supported_channel_type(ctype: &str) -> bool {
    !matches!(
        ctype,
        // Unsupported types
        "video" | "music" | "musicstream" | "webcam" | "mic"
    )
}

/// Translates ShaderToy channel metadata into an [`Input`] configuration.
///
/// Some ShaderToy media identifiers are rewritten into logical
/// buffer names (`Buffer A`, `Cubemap A`, etc.).
fn create_input_config(
    ctype: &str,
    src: &str,
    sampler: &serde_json::Value,
) -> Result<Input, PresetError> {
    let _type = match ctype {
        "texture" => InputType::Texture,
        "cubemap" => InputType::Cubemap,
        "volume" => InputType::Volume,
        "video" => InputType::Video,
        "music" => InputType::Music,
        "musicstream" => InputType::MusicStream,
        "keyboard" => InputType::Keyboard,
        "webcam" => InputType::Webcam,
        "mic" => InputType::Microphone,
        _ => InputType::Misc,
    };

    let name = if !is_supported_channel_type(ctype) {
        "fallback".to_string()
    } else if _type == InputType::Keyboard {
        "".to_string()
    } else if let Some(filename) = std::path::Path::new(src)
        .file_name()
        .and_then(|s| s.to_str())
    {
        match filename {
            "buffer00.png" => "Buffer A",
            "buffer01.png" => "Buffer B",
            "buffer02.png" => "Buffer C",
            "buffer03.png" => "Buffer D",
            "cubemap00.png" => "Cubemap A",
            _ => asset_name_from_src(src)?,
        }
        .to_string()
    } else {
        src.to_string()
    };

    let wrap = match sampler
        .get("wrap")
        .and_then(|w| w.as_str())
        .unwrap_or("clamp")
    {
        "repeat" if ctype != "cubemap" => WrapMode::Repeat,
        _ => WrapMode::Clamp,
    };

    let filter = match sampler
        .get("filter")
        .and_then(|f| f.as_str())
        .unwrap_or("linear")
    {
        "nearest" => FilterMode::Nearest,
        "mipmap" => FilterMode::Mipmap,
        _ => FilterMode::Linear,
    };

    let vflip = sampler
        .get("vflip")
        .and_then(|v| v.as_str())
        .unwrap_or("false")
        == "true";

    Ok(Input {
        _type,
        name,
        wrap,
        filter,
        vflip,
    })
}

/// Maps ShaderToy media hashes to human-readable asset names.
///
/// ShaderToy references built-in assets using hashed filenames.
/// This table converts known hashes into stable logical names
/// understood by the texture manager.
///
/// Returns an error if the asset hash is unknown.
fn asset_name_from_src(src: &str) -> Result<&'static str, PresetError> {
    let stem = std::path::Path::new(src)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            PresetError::Import(format!("Could not extract asset filename from: {src}"))
        })?;

    match stem {
        // Textures
        "52d2a8f514c4fd2d9866587f4d7b2a5bfa1a11a0e772077d7682deb8b3b517e5" => Ok("Abstract 1"),
        "bd6464771e47eed832c5eb2cd85cdc0bfc697786b903bfd30f890f9d4fc36657" => Ok("Abstract 2"),
        "8979352a182bde7c3c651ba2b2f4e0615de819585cc37b7175bcefbca15a6683" => Ok("Abstract 3"),
        "85a6d68622b36995ccb98a89bbb119edf167c914660e4450d313de049320005c" => Ok("Bayer"),
        "cb49c003b454385aa9975733aff4571c62182ccdda480aaba9a8d250014f00ec" => Ok("Blue Noise"),
        "08b42b43ae9d3c0605da11d0eac86618ea888e62cdd9518ee8b9097488b31560" => Ok("Font 1"),
        "0c7bf5fe9462d5bffbd11126e82908e39be3ce56220d900f633d58fb432e56f5" => {
            Ok("Gray Noise Medium")
        }
        "0a40562379b63dfb89227e6d172f39fdce9022cba76623f1054a2c83d6c0ba5d" => {
            Ok("Gray Noise Small")
        }
        "fb918796edc3d2221218db0811e240e72e340350008338b0c07a52bd353666a6" => Ok("Lichen"),
        "8de3a3924cb95bd0e95a443fff0326c869f9d4979cd1d5b6e94e2a01f5be53e9" => Ok("London"),
        "cbcbb5a6cfb55c36f8f021fbb0e3f69ac96339a39fa85cd96f2017a2192821b5" => Ok("Nyancat"),
        "cd4c518bc6ef165c39d4405b347b51ba40f8d7a065ab0e8d2e4f422cbc1e8a43" => Ok("Organic 1"),
        "92d7758c402f0927011ca8d0a7e40251439fba3a1dac26f5b8b62026323501aa" => Ok("Organic 2"),
        "79520a3d3a0f4d3caa440802ef4362e99d54e12b1392973e4ea321840970a88a" => Ok("Organic 3"),
        "3871e838723dd6b166e490664eead8ec60aedd6b8d95bc8e2fe3f882f0fd90f0" => Ok("Organic 4"),
        "ad56fba948dfba9ae698198c109e71f118a54d209c0ea50d77ea546abad89c57" => Ok("Pebbles"),
        "f735bee5b64ef98879dc618b016ecf7939a5756040c2cde21ccb15e69a6e1cfb" => {
            Ok("RGBA Noise Medium")
        }
        "3083c722c0c738cad0f468383167a0d246f91af2bfa373e9c5c094fb8c8413e0" => {
            Ok("RGBA Noise Small")
        }
        "10eb4fe0ac8a7dc348a2cc282ca5df1759ab8bf680117e4047728100969e7b43" => Ok("Rock Tiles"),
        "95b90082f799f48677b4f206d856ad572f1d178c676269eac6347631d4447258" => Ok("Rusty Metal"),
        "e6e5631ce1237ae4c05b3563eda686400a401df4548d0f9fad40ecac1659c46c" => Ok("Stars"),
        "1f7dca9c22f324751f2a5a59c9b181dfe3b5564a04b724c657732d0bf09c99db" => Ok("Wood"),
        // Cubemaps
        "94284d43be78f00eb6b298e6d78656a1b34e2b91b34940d02f1ca8b22310e8a0" => Ok("Forest"),
        "0681c014f6c88c356cf9c0394ffe015acc94ec1474924855f45d22c3e70b5785" => Ok("Forest Blurred"),
        "488bd40303a2e2b9a71987e48c66ef41f5e937174bf316d3ed0e86410784b919" => {
            Ok("St. Peter's Basilica")
        }
        "550a8cce1bf403869fde66dddf6028dd171f1852f4a704a465e1b80d23955663" => {
            Ok("St. Peter's Basilica Blurred")
        }
        "585f9546c092f53ded45332b343144396c0b2d70d9965f585ebc172080d8aa58" => Ok("Uffizi Gallery"),
        "793a105653fbdadabdc1325ca08675e1ce48ae5f12e37973829c87bea4be3232" => {
            Ok("Uffizi Gallery Blurred")
        }
        // Volumes
        "27012b4eadd0c3ce12498b867058e4f717ce79e10a99568cca461682d84a4b04" => Ok("Grey Noise3D"),
        "aea6b99da1d53055107966b59ac5444fc8bc7b3ce2d0bbb6a4a3cbae1d97f3aa" => Ok("RGBA Noise3D"),
        _ => Err(PresetError::Import(format!("Unknown asset name: {stem}"))),
    }
}
