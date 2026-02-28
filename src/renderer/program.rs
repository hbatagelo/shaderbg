// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! OpenGL program wrapper.
//!
//! Provides an RAII abstraction over an OpenGL program object,
//! handling shader linking, uniform lookup, and automatic deletion
//! of the program on drop.

use gl::types::*;

use super::shader::*;

/// RAII wrapper around an OpenGL shader program.
///
/// Owns the underlying OpenGL program handle and deletes it
/// automatically when dropped.
///
/// Attached shaders are not owned and may be safely dropped
/// after linking.
pub struct Program {
    id: GLuint,
}

impl Program {
    /// Links the provided shaders into an OpenGL program.
    ///
    /// All shaders must be successfully compiled and compatible.
    /// Returns a [`ShaderError::ProgramLink`] containing the driver log
    /// if linking fails.
    ///
    /// Shaders do not need to outlive the returned `Program`.
    pub fn new(shaders: &[Shader]) -> Result<Self, ShaderError> {
        unsafe {
            let id = gl::CreateProgram();

            for shader in shaders {
                gl::AttachShader(id, shader.id());
            }
            gl::LinkProgram(id);

            let mut success = 0;
            gl::GetProgramiv(id, gl::LINK_STATUS, &mut success);
            if success == 0 {
                gl::DeleteProgram(id);

                let mut log_len = 0;
                gl::GetProgramiv(id, gl::INFO_LOG_LENGTH, &mut log_len);

                let mut log = Vec::with_capacity(log_len as usize);
                gl::GetProgramInfoLog(id, log_len, &mut log_len, log.as_mut_ptr() as *mut _);
                log.set_len(log_len as usize);

                Err(ShaderError::ProgramLink(String::from_utf8(log)?))
            } else {
                for shader in shaders {
                    gl::DetachShader(id, shader.id());
                }
                Ok(Self { id })
            }
        }
    }

    /// Binds this program as the current OpenGL program.
    pub fn bind(&self) {
        unsafe { gl::UseProgram(self.id) };
    }

    /// Returns the location of a uniform variable.
    ///
    /// Returns `Ok(-1)` if the uniform does not exist or was optimized
    /// out by the shader compiler.
    pub fn uniform_location(&self, name: &str) -> Result<GLint, ShaderError> {
        let name = std::ffi::CString::new(name)?;
        Ok(unsafe { gl::GetUniformLocation(self.id, name.as_ptr()) })
    }
}

impl Drop for Program {
    // Delete program object when no longer needed.
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.id) };
    }
}
