// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use gl::types::*;
use std::os::raw::c_void;

pub struct VertexArray {
    id: GLuint,
}

impl VertexArray {
    pub fn new() -> Self {
        let mut id = 0;
        unsafe { gl::GenVertexArrays(1, &mut id) };
        Self { id }
    }

    pub fn bind(&self) {
        unsafe { gl::BindVertexArray(self.id) };
    }

    pub fn set_attribute(&self, location: GLint, components: GLint, stride: usize, offset: usize) {
        self.bind();
        unsafe {
            gl::EnableVertexAttribArray(location as GLuint);
            gl::VertexAttribPointer(
                location as GLuint,
                components,
                gl::FLOAT,
                gl::FALSE,
                stride as GLsizei,
                offset as *const c_void,
            );
        }
    }
}

impl Drop for VertexArray {
    fn drop(&mut self) {
        unsafe { gl::DeleteVertexArrays(1, &self.id) };
    }
}

#[macro_export]
macro_rules! set_attribute {
    ($va:expr, $loc:expr, $t:ident :: $field:tt) => {{
        let dummy = std::mem::MaybeUninit::<$t>::uninit();
        let dummy_ptr = dummy.as_ptr();
        let field_ptr = unsafe { std::ptr::addr_of!((*dummy_ptr).$field) };
        let components =
            unsafe { (std::mem::size_of_val(&*field_ptr) / std::mem::size_of::<f32>()) as GLint };
        let offset = field_ptr as usize - dummy_ptr as usize;
        let stride = std::mem::size_of::<$t>();
        $va.set_attribute($loc, components, stride, offset)
    }};
}
