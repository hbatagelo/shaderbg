// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(test)]
mod tests {
    mod convert_to_desktop_glsl;
    mod glsl_initializer;
    mod glsl_preprocessor;
    mod glsl_utils;
}
mod glsl_depth_tracker;
mod glsl_initializer;
mod glsl_preprocessor;
mod glsl_utils;
mod importer;

use std::path::PathBuf;

use crate::{preset::*, renderer::shader::ShaderError, shadertoy::importer::fetch_from_web};

#[rustfmt::skip]
pub const DIFF_RESERVED_WORDS_4_2: [&str; 63] = [
    "double",
    "dvec2", "dvec3", "dvec4",
    "dmat2", "dmat3", "dmat4",
    "dmat2x2", "dmat2x3", "dmat2x4",
    "dmat3x2", "dmat3x3", "dmat3x4",
    "dmat4x2", "dmat4x3", "dmat4x4",

    "imageCubeArray", "iimageCubeArray", "uimageCubeArray",
    "image2DMS", "iimage2DMS", "uimage2DMS",
    "image2DMSArray", "iimage2DMSArray", "uimage2DMSArray",

    "uaddCarry", "usubBorrow", "umulExtended", "imulExtended",
    "bitfieldExtract", "bitfieldInsert", "bitfieldReverse",
    "bitCount", "findLSB", "findMSB",

    "textureQueryLod", "textureGather", "textureGatheOffset", "textureGatherOffsets",

    "atomicCounterIncrement", "atomicCounterDecrement", "atomicCounter",

    "imageLoad", "imageStore",
    "imageAtomicAdd", "imageAtomicMin", "imageAtomicMax",
    "imageAtomicAnd", "imageAtomicOr", "imageAtomicXor",
    "imageAtomicExchange", "imageAtomicCompSwap", "imageAtomicCompSwap",

    "interpolateAtCentroid", "interpolateAtSample", "interpolateAtOffset",

    "noise1", "noise2", "noise3", "noise4",

    "memoryBarrier",

    "packed", "precise",
];

#[rustfmt::skip]
pub const DIFF_RESERVED_WORDS_3_0_ES_REV_2: [&str; 1] = [
    "packed",
];

pub fn load_from_web(shader_id: &str, api_key: &str) -> Result<(Preset, Option<PathBuf>), String> {
    match fetch_from_web(shader_id, api_key) {
        Ok(preset) => {
            let saved_path = save_to_presets_directory(&preset, shader_id);
            Ok((preset, saved_path))
        }
        Err(err) => {
            if err.contains("Shader not found") {
                log::warn!("Shader '{shader_id}' not found - it may be private or unlisted",);
            } else {
                log::warn!("Failed to fetch from web: {err}");
            }
            load_from_presets_directory(shader_id)
        }
    }
}

pub fn to_glsl_version(
    source: &str,
    version: (i32, i32),
    glsl_es: bool,
) -> Result<String, ShaderError> {
    let mut source = source.to_string();
    let glsl_version = format!("{}{}0", version.0, version.1);

    source = glsl_utils::replace_in_preprocessor_conditionals(&source, "GL_ES", "SHADERBG");
    source =
        glsl_utils::replace_in_preprocessor_conditionals(&source, "__VERSION__", &glsl_version);

    source = glsl_initializer::initialize_uninitialized_variables(&source)?;

    fn rename_with_trailing_underscore(text: &str, word: &str) -> String {
        let pattern = format!(r"\b{}\b", regex::escape(word));
        let re = regex::Regex::new(&pattern).expect("Invalid regex pattern");
        re.replace_all(text, &format!("{word}_")).to_string()
    }

    if version == (3, 0) && glsl_es {
        for word in DIFF_RESERVED_WORDS_3_0_ES_REV_2 {
            source = rename_with_trailing_underscore(&source, word);
        }
    }
    if version == (4, 2) && !glsl_es {
        for word in DIFF_RESERVED_WORDS_4_2 {
            source = rename_with_trailing_underscore(&source, word);
        }
    }

    Ok(source)
}

fn load_from_presets_directory(shader_id: &str) -> Result<(Preset, Option<PathBuf>), String> {
    let presets_dir = presets_dir();
    let file = presets_dir.join(format!("{shader_id}.toml"));

    match Preset::from_file(&file) {
        Ok(preset) => {
            log::warn!("Using preset from: {:?}", file);
            Ok((preset, Some(file)))
        }
        Err(err) => {
            log::warn!("Failed to load from presets: {err}");
            Err("Failed to load shader".to_string())
        }
    }
}
