// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use gl::types::*;
use std::os::raw::c_void;

/// RAII wrapper around an OpenGL Vertex Array Object (VAO).
///
/// Encapsulates vertex attribute state, including enabled attributes
/// and their layout descriptions.
pub struct VertexArray {
    id: GLuint,
}

impl VertexArray {
    /// Creates a new OpenGL Vertex Array Object.
    pub fn new() -> Self {
        let mut id = 0;
        unsafe { gl::GenVertexArrays(1, &mut id) };
        Self { id }
    }

    /// Binds this VAO as the current vertex array.
    ///
    /// All subsequent vertex attribute configuration affects this VAO.
    pub fn bind(&self) {
        unsafe { gl::BindVertexArray(self.id) };
    }

    /// Configures a floating-point vertex attribute.
    ///
    /// Parameters:
    /// `location`  : Shader attribute location
    /// `components`: Number of `f32` components (1–4)
    /// `stride`    : Byte size of one vertex
    /// `offset`    : Byte offset of the attribute within the vertex
    ///
    /// The vertex buffer must already be bound to `GL_ARRAY_BUFFER`.
    pub fn set_attribute(
        &self,
        location: GLint,
        components: GLint,
        stride: usize,
        offset: usize,
    ) -> &Self {
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
        self
    }
}

impl Drop for VertexArray {
    // Deletes the VAO when it goes out of scope.
    fn drop(&mut self) {
        unsafe { gl::DeleteVertexArrays(1, &self.id) };
    }
}

/// Convenience macro that derives vertex attribute layout
/// information directly from a struct field.
///
/// # Example
/// ```
/// #[repr(C)]
/// struct Vertex(Vec3, Vec2); // position, texCoord
///
/// let vao = VertexArray::new();
/// set_attribute!(vao, 0, Vertex::0); // position attribute at location 0
/// set_attribute!(vao, 1, Vertex::1); // texCoord attribute at location 1
/// ```
///
/// This expands to calculate:
/// - `components`: Number of f32 components in the field (3 for Vec3, 2 for Vec2).
/// - `stride`: Size of the entire Vertex struct
/// - `offset`: Byte offset of the field within the struct
///
/// Then calls:
/// ```
/// vao.set_attribute(location, components, stride, offset);
/// ```
///
/// This macro only configures attribute layout. It does not bind a vertex buffer.
///
/// Remarks:
/// This macro performs pointer arithmetic on an uninitialized value
/// to compute field offsets. No memory is read, but the following
/// requirements must hold:
///
/// - The vertex type must be `#[repr(C)]` or `#[repr(transparent)]`.
/// - Field types must be tightly packed `f32` data.
/// - The associated vertex buffer must be bound.
#[macro_export]
macro_rules! set_attribute {
    ($vao:expr, $loc:expr, $t:ident :: $field:tt) => {{
        // Create an uninitialized instance solely to compute field offsets.
        // No memory is read or written.
        let dummy = std::mem::MaybeUninit::<$t>::uninit();
        let dummy_ptr = dummy.as_ptr();
        let field_ptr = unsafe { std::ptr::addr_of!((*dummy_ptr).$field) };
        let components =
            unsafe { (std::mem::size_of_val(&*field_ptr) / std::mem::size_of::<f32>()) as GLint };
        let offset = field_ptr as usize - dummy_ptr as usize;
        let stride = std::mem::size_of::<$t>();
        $vao.set_attribute($loc, components, stride, offset)
    }};
}
