// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use gl::types::*;

#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ShaderError {
    #[error("Shader preprocess #error at line {1}: {0}")]
    PreprocessError(String, usize),
    #[error("Shader compile error: {0}")]
    CompileError(String),
    #[error("Shader link error: {0}")]
    LinkError(String),
    #[error{"{0}"}]
    Utf8Error(#[from] std::string::FromUtf8Error),
    #[error{"{0}"}]
    NulError(#[from] std::ffi::NulError),
}

pub struct Shader {
    pub id: GLuint,
}

impl Shader {
    pub fn new(source: &str, type_: GLenum) -> Result<Self, ShaderError> {
        let source = std::ffi::CString::new(source)?;
        unsafe {
            let id = gl::CreateShader(type_);
            gl::ShaderSource(id, 1, &source.as_ptr(), std::ptr::null());
            gl::CompileShader(id);

            let mut success = 0;
            gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut success);
            if success == 0 {
                let mut log_len = 0;
                gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut log_len);
                let mut log = Vec::with_capacity(log_len as usize);
                gl::GetShaderInfoLog(id, log_len, &mut log_len, log.as_mut_ptr() as *mut _);
                log.set_len(log_len as usize);
                Err(ShaderError::CompileError(String::from_utf8(log)?))
            } else {
                Ok(Self { id })
            }
        }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe { gl::DeleteShader(self.id) };
    }
}
