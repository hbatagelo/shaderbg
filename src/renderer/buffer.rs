// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! OpenGL buffer object wrapper.
//!
//! Provides an RAII abstraction over an OpenGL buffer object,
//! handling creation, binding, data upload, and automatic deletion.

use gl::types::*;
use std::os::raw::c_void;

/// OpenGL buffer object.
///
/// `Buffer` owns the underlying GPU buffer and deletes it on drop.
/// The buffer target (e.g. [`gl::ARRAY_BUFFER`], [`gl::UNIFORM_BUFFER`])
/// is fixed at creation time.
pub struct Buffer {
    id: GLuint,
    target: GLuint,
}

impl Buffer {
    /// Creates a new buffer for the given OpenGL target.
    pub fn new(target: GLuint) -> Self {
        let mut id = 0;
        unsafe { gl::GenBuffers(1, &mut id) };
        Self { id, target }
    }

    /// Uploads data to the buffer, replacing its current contents.
    ///
    /// This binds the buffer before calling `glBufferData`.
    ///
    /// `usage` corresponds to OpenGL usage hints such as:
    /// [`gl::STATIC_DRAW`], [`gl::DYNAMIC_DRAW`], or [`gl::STREAM_DRAW`].
    ///
    /// # Safety
    /// `T` must be plain data without padding-sensitive layout assumptions.
    /// Typically `[f32]`, `[u32]`, or vertex structs with `#[repr(C)]`.
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

    /// Binds this buffer to its OpenGL target.
    pub fn bind(&self) {
        unsafe { gl::BindBuffer(self.target, self.id) };
    }
}

impl Drop for Buffer {
    // Delete buffer object when no longer needed.
    fn drop(&mut self) {
        unsafe { gl::DeleteBuffers(1, &self.id) };
    }
}
