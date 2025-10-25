// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::preset::*;

const SHADERTOY_URL: &str = "https://www.shadertoy.com/";

pub fn fetch_from_web(shader_id: &str, api_key: &str) -> Result<Preset, String> {
    let url = format!("{SHADERTOY_URL}api/v1/shaders/{shader_id}?key={api_key}");
    log::debug!("Requesting URL: {url}");

    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {err}"))?;

    let response = client
        .get(&url)
        .send()
        .map_err(|err| format!("Request failed: {err}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP status: {}", response.status()));
    }

    let json_str = response
        .text()
        .map_err(|err| format!("Failed to read response: {err}"))?;

    let json_value: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|err| format!("Failed to parse JSON: {err}"))?;

    if let Some(err) = json_value.get("Error").and_then(|err| err.as_str()) {
        if err == "Shader not found" {
            return Err("Shader not found (may be private or unlisted)".to_string());
        } else {
            return Err(format!("Shadertoy API error: {err}"));
        }
    }

    let shader_obj = json_value
        .get("Shader")
        .ok_or("Missing 'Shader' key in JSON")?;
    let info = shader_obj.get("info").ok_or("Missing 'info' key in JSON")?;

    let mut preset = Preset::with_serde_defaults();
    preset.id = info
        .get("id")
        .and_then(|id| id.as_str())
        .unwrap_or(shader_id)
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
        .ok_or("Missing or invalid 'renderpass' array")?;

    for pass in renderpasses {
        process_single_pass(&client, &mut preset, pass)?;
    }

    log::debug!("Fetched '{shader_id}' successfully");

    Ok(preset)
}

fn process_single_pass(
    client: &reqwest::blocking::Client,
    preset: &mut Preset,
    pass: &serde_json::Value,
) -> Result<(), String> {
    let name = pass
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or("Missing pass name")?;

    let code = pass
        .get("code")
        .and_then(|c| c.as_str())
        .ok_or("Missing pass code")?
        .to_string();

    let inputs = pass
        .get("inputs")
        .and_then(|i| i.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    let pass_inputs = process_pass_inputs(client, inputs, name)?;
    let pass_config = Pass {
        shader: code,
        input_0: pass_inputs[0].clone(),
        input_1: pass_inputs[1].clone(),
        input_2: pass_inputs[2].clone(),
        input_3: pass_inputs[3].clone(),
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

fn process_pass_inputs(
    client: &reqwest::blocking::Client,
    inputs: &[serde_json::Value],
    pass_name: &str,
) -> Result<[Option<Input>; 4], String> {
    let mut pass_inputs: [Option<Input>; 4] = core::array::from_fn(|_| None);

    for input in inputs {
        if let Some(processed_input) = process_single_input(client, input, pass_name)? {
            let channel = input.get("channel").and_then(|c| c.as_i64()).unwrap_or(-1);
            if (0..=3).contains(&channel) {
                pass_inputs[channel as usize] = Some(processed_input);
            }
        }
    }

    Ok(pass_inputs)
}

fn process_single_input(
    client: &reqwest::blocking::Client,
    input: &serde_json::Value,
    pass_name: &str,
) -> Result<Option<Input>, String> {
    let ctype = input.get("ctype").and_then(|t| t.as_str()).unwrap_or("");

    if !is_supported_channel_type(ctype) {
        let channel = input.get("channel").and_then(|c| c.as_i64()).unwrap_or(-1);
        log::warn!("{pass_name}: 'iChannel{channel}' input '{ctype}' not supported");
    }

    let sampler = input.get("sampler").unwrap_or(&serde_json::Value::Null);
    let src = input.get("src").and_then(|s| s.as_str()).unwrap_or("");
    let input_config = create_input_config(client, ctype, src, sampler)?;

    Ok(Some(input_config))
}

fn is_supported_channel_type(ctype: &str) -> bool {
    !matches!(
        ctype,
        "video" | "music" | "musicstream" | "keyboard" | "webcam" | "mic"
    )
}

fn create_input_config(
    client: &reqwest::blocking::Client,
    ctype: &str,
    src: &str,
    sampler: &serde_json::Value,
) -> Result<Input, String> {
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
            _ => asset_name_from_src(client, src)?,
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

fn asset_name_from_src(
    client: &reqwest::blocking::Client,
    src: &str,
) -> Result<&'static str, String> {
    let url = format!("{SHADERTOY_URL}{src}");
    let response = client.head(&url).send().map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        return Err(format!("HTTP status: {}", response.status()));
    }

    let content_length = response
        .headers()
        .get("content-length")
        .ok_or_else(|| "No content-length header".to_string())?
        .to_str()
        .map_err(|_| "Invalid content-length header".to_string())?
        .parse::<u64>()
        .map_err(|_| "Failed to parse content-length".to_string())?;

    // A quick (but brittle) identification method that takes advantage of the
    // fact that each asset has a distinct length. Currently maps 27 known assets.
    match content_length {
        112578 => Ok("Abstract 1"),
        149508 => Ok("Abstract 2"),
        204227 => Ok("Abstract 3"),
        241 => Ok("Bayer"),
        4202841 => Ok("Blue Noise"),
        1320842 => Ok("Font 1"),
        67474 => Ok("Gray Noise Medium"),
        4241 => Ok("Gray Noise Small"),
        204414 => Ok("Lichen"),
        87761 => Ok("London"),
        1269 => Ok("Nyancat"),
        183069 => Ok("Organic 1"),
        174949 => Ok("Organic 2"),
        396818 => Ok("Organic 3"),
        305501 => Ok("Organic 4"),
        101929 => Ok("Pebbles"),
        264082 => Ok("RGBA Noise Medium"),
        16558 => Ok("RGBA Noise Small"),
        68242 => Ok("Rock Tiles"),
        49498 => Ok("Rusty Metal"),
        87562 => Ok("Stars"),
        154431 => Ok("Wood"),
        94156 => Ok("Forest"),
        3459 => Ok("Forest Blurred"),
        47339 => Ok("St. Peter's Basilica"),
        5719 => Ok("St. Peter's Basilica Blurred"),
        93210 => Ok("Uffizi Gallery"),
        3742 => Ok("Uffizi Gallery Blurred"),
        32788 => Ok("Grey Noise3D"),
        131092 => Ok("RGBA Noise3D"),
        _ => Err(format!(
            "Unknown content length ({content_length} bytes) for {src}"
        )),
    }
}
