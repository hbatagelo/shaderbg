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
pub mod importer;

use crate::renderer::shader::ShaderError;

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
