// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use gl::types::*;

use super::shader::*;

pub struct Program {
    id: GLuint,
}

impl Program {
    pub fn new(shaders: &[Shader]) -> Result<Self, ShaderError> {
        unsafe {
            let id = gl::CreateProgram();

            for shader in shaders {
                gl::AttachShader(id, shader.id);
            }
            gl::LinkProgram(id);

            let mut success = 0;
            gl::GetProgramiv(id, gl::LINK_STATUS, &mut success);
            if success == 0 {
                let mut log_len = 0;
                gl::GetProgramiv(id, gl::INFO_LOG_LENGTH, &mut log_len);
                let mut log = Vec::with_capacity(log_len as usize);
                gl::GetProgramInfoLog(id, log_len, &mut log_len, log.as_mut_ptr() as *mut _);
                log.set_len(log_len as usize);
                Err(ShaderError::LinkError(String::from_utf8(log)?))
            } else {
                Ok(Self { id })
            }
        }
    }

    pub fn use_program(&self) {
        unsafe { gl::UseProgram(self.id) };
    }

    pub fn uniform_location(&self, name: &str) -> Result<GLint, ShaderError> {
        let name = std::ffi::CString::new(name)?;
        Ok(unsafe { gl::GetUniformLocation(self.id, name.as_ptr()) })
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.id) };
    }
}
