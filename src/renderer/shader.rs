// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! OpenGL shader compilation utilities.
//!
//! Provides RAII management for shader objects and structured
//! error reporting for preprocessing, compilation, and linking.

use gl::types::*;

/// Errors that may occur during shader creation or compilation.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ShaderError {
    /// Error emitted by the custom shader preprocessor.
    #[error("Shader preprocess #error at line {1}: {0}")]
    ShaderPreprocess(String, usize),
    /// GLSL compilation failed.
    #[error("Shader compile error: {0}")]
    ShaderCompile(String),
    /// Shader program linking failed.
    #[error("Program link error: {0}")]
    ProgramLink(String),
    /// OpenGL log contained invalid UTF-8.
    #[error("{0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    /// Shader source contained an interior NUL byte.
    #[error("{0}")]
    Nul(#[from] std::ffi::NulError),
}

/// RAII wrapper around an OpenGL shader object.
///
/// Owns a compiled shader stage (vertex, fragment, etc.)
/// and deletes it automatically when dropped.
pub struct Shader {
    id: GLuint,
}

impl Shader {
    /// Compiles a GLSL shader.
    ///
    /// `type_` must be a valid OpenGL shader stage, e.g.
    /// [`gl::VERTEX_SHADER`] or [`gl::FRAGMENT_SHADER`].
    ///
    /// Returns a compiled shader on success or a detailed
    /// compilation error containing the driver log.
    ///
    /// Requires a current OpenGL context.
    pub fn new(source: &str, type_: GLenum) -> Result<Self, ShaderError> {
        let source = std::ffi::CString::new(source)?;
        unsafe {
            let id = gl::CreateShader(type_);
            gl::ShaderSource(id, 1, &source.as_ptr(), std::ptr::null());
            gl::CompileShader(id);

            // Query compilation result
            let mut success = 0;
            gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut success);
            if success == 0 {
                gl::DeleteShader(id);

                // Retrieve driver-provided compilation log
                let mut log_len = 0;
                gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut log_len);

                let mut log = Vec::with_capacity(log_len as usize);
                gl::GetShaderInfoLog(id, log_len, &mut log_len, log.as_mut_ptr() as *mut _);
                log.set_len(log_len as usize);

                Err(ShaderError::ShaderCompile(String::from_utf8(log)?))
            } else {
                Ok(Self { id })
            }
        }
    }

    /// Returns the underlying OpenGL shader handle (non-owning).
    pub fn id(&self) -> GLuint {
        self.id
    }
}

impl Drop for Shader {
    // Delete shader object when no longer needed.
    fn drop(&mut self) {
        unsafe { gl::DeleteShader(self.id) };
    }
}
