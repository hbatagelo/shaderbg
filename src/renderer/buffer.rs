// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use gl::types::*;
use std::os::raw::c_void;

pub struct Buffer {
    id: GLuint,
    target: GLuint,
}

impl Buffer {
    pub fn new(target: GLuint) -> Self {
        let mut id = 0;
        unsafe { gl::GenBuffers(1, &mut id) };
        Self { id, target }
    }

    pub fn set_data<T>(&self, data: &[T], usage: GLuint) {
        self.bind();
        unsafe {
            gl::BufferData(
                self.target,
                std::mem::size_of_val(data) as GLsizeiptr,
                data.as_ptr() as *const c_void,
                usage,
            );
        }
    }

    pub fn bind(&self) {
        unsafe { gl::BindBuffer(self.target, self.id) };
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe { gl::DeleteBuffers(1, &self.id) };
    }
}
