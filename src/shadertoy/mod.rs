// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! ShaderToy GLSL compatibility layer.
//!
//! This module converts ShaderToy-style GLSL ES shaders into valid
//! desktop OpenGL GLSL source code.
//!
//! Responsibilities:
//! - Preprocess ShaderToy shaders and normalize preprocessor conditionals
//! - Adapt GLSL ES semantics to desktop GLSL versions
//! - Initialize undefined variables for stricter desktop compilers
//! - Rename identifiers conflicting with desktop GLSL reserved words
//! - Import ShaderToy JSON exports into application presets
//!
//! The main entry point is [`to_glsl_version`], which transforms shader
//! source code according to the requested OpenGL version.

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
pub mod importer;

use crate::renderer::shader::ShaderError;

/// Reserved words or built-in function names in GLSL 4.20 that are not in GLSL ES 3.00.
#[rustfmt::skip]
pub const DIFF_RESERVED_WORDS_4_2: [&str; 63] = [
    // Double precision types
    "double",
    "dvec2", "dvec3", "dvec4",
    "dmat2", "dmat3", "dmat4",
    "dmat2x2", "dmat2x3", "dmat2x4",
    "dmat3x2", "dmat3x3", "dmat3x4",
    "dmat4x2", "dmat4x3", "dmat4x4",

    // Image types
    "imageCubeArray", "iimageCubeArray", "uimageCubeArray",
    "image2DMS", "iimage2DMS", "uimage2DMS",
    "image2DMSArray", "iimage2DMSArray", "uimage2DMSArray",

    // Integer functions
    "uaddCarry", "usubBorrow", "umulExtended", "imulExtended",
    "bitfieldExtract", "bitfieldInsert", "bitfieldReverse",
    "bitCount", "findLSB", "findMSB",

    // Texture functions
    "textureQueryLod", "textureGather", "textureGatheOffset", "textureGatherOffsets",

    // Atomic-counter functions
    "atomicCounterIncrement", "atomicCounterDecrement", "atomicCounter",

    // Image functions
    "imageLoad", "imageStore",
    "imageAtomicAdd", "imageAtomicMin", "imageAtomicMax",
    "imageAtomicAnd", "imageAtomicOr", "imageAtomicXor",
    "imageAtomicExchange", "imageAtomicCompSwap", "imageAtomicCompSwap",

    // Interpolation functions
    "interpolateAtCentroid", "interpolateAtSample", "interpolateAtOffset",

    // Noise functions,
    "noise1", "noise2", "noise3", "noise4",

    // Shader memory control functions
    "memoryBarrier",

    // Other reserved words
    "packed", "precise",
];

/// Reserved words in GLSL 3.00 rev 2 that are not in latest revision.
#[rustfmt::skip]
pub const DIFF_RESERVED_WORDS_3_0_ES_REV_2: [&str; 1] = [
    "packed",
];

/// Makes a ShaderToy shader compatible with the given GLSL version.
/// Currently works only with 3.0 es and 4.2.
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
