// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! OpenGL framebuffer abstraction.
//!
//! Provides an RAII wrapper over framebuffer objects used as offscreen
//! render targets. Supports single-sampled 2D render targets,
//! multisampled rendering with automatic resolvem floating-point buffers
//! and cubemap rendering.
//!
//! The framebuffer owns all attached textures and deletes them on drop.

use crate::geometry::*;
use gl::types::*;

/// Type of color attachment stored in the framebuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramebufferFormat {
    /// Standard 8-bit RGB texture.
    Tex2D,
    /// Floating-point RGBA texture.
    Tex2DFloat,
    /// Floating-point cubemap texture.
    Cubemap,
}

/// Offscreen render target backed by an OpenGL framebuffer and its attachments.
///
/// The framebuffer owns its color attachments and manages their lifetime.
/// When MSAA is enabled, rendering occurs into a multisampled texture which
/// must be resolved before sampling.
pub struct Framebuffer {
    /// ID of the framebuffer object.
    fbo_id: GLuint,

    /// ID of the color attachment texture.
    /// When MSAA is disabled, this texture is directly sampleable.
    /// When MSAA is enabled, this is a multisampled render target.
    texture_id: GLuint,

    /// Dimensions of the color attachment.
    size: Size,

    /// Indicates whether multisample anti-aliasing is active.
    msaa_enabled: bool,

    /// ID of the framebuffer receiving the resolved MSAA image.
    msaa_resolve_fbo_id: GLuint,

    /// ID of the texture containing the resolved image used for shader sampling.
    msaa_resolve_texture_id: GLuint,
}

impl Framebuffer {
    /// Creates a framebuffer with a color attachment of the given format.
    ///
    /// The previously bound framebuffer is restored before returning.
    pub fn new(size: Size, msaa_samples: u32, format: FramebufferFormat) -> Self {
        let previous_fbo = current_framebuffer();

        let msaa_enabled = msaa_samples > 0 && format == FramebufferFormat::Tex2D;

        let fbo_id = gen_framebuffer();
        bind_framebuffer(fbo_id);

        let texture_id = match format {
            FramebufferFormat::Tex2D | FramebufferFormat::Tex2DFloat => {
                create_2d_color_attachment(size, format, msaa_samples, msaa_enabled)
            }
            FramebufferFormat::Cubemap => create_cubemap_attachment(size),
        };

        check_framebuffer_status();

        let (resolve_fbo, resolve_texture) = if msaa_enabled {
            create_msaa_resolve_target(size)
        } else {
            (0, 0)
        };

        bind_framebuffer(previous_fbo);

        Self {
            fbo_id,
            texture_id,
            size,
            msaa_enabled,
            msaa_resolve_fbo_id: resolve_fbo,
            msaa_resolve_texture_id: resolve_texture,
        }
    }

    /// Binds this framebuffer as the current draw framebuffer.
    pub fn bind(&self) {
        bind_framebuffer(self.fbo_id);
    }

    /// Selects a cubemap face as the active color attachment.
    ///
    /// Used when rendering each face of a cubemap sequentially.
    pub fn bind_cubemap_face(&self, face: GLenum) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo_id);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                face,
                self.texture_id,
                0,
            );
        }
    }

    /// Blits the color attachment into another framebuffer.
    ///
    /// The source region always spans the entire framebuffer.
    /// The destination region is defined by `origin` and `size`.
    pub fn blit_to(&self, dst_fbo: GLuint, origin: Point, size: Size, filter: GLenum) {
        unsafe {
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, self.fbo_id);
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, dst_fbo);

            gl::BlitFramebuffer(
                0,
                0,
                self.size.width() as i32,
                self.size.height() as i32,
                origin.x(),
                origin.y(),
                origin.x() + size.width() as i32,
                origin.y() + size.height() as i32,
                gl::COLOR_BUFFER_BIT,
                filter,
            );
        }
    }

    /// Resolves the multisampled render target into a single-sampled texture.
    ///
    /// Must be called before sampling the framebuffer texture when MSAA is enabled.
    pub fn resolve(&self) {
        if self.msaa_enabled {
            self.blit_to(
                self.msaa_resolve_fbo_id,
                Point::default(),
                self.size,
                gl::NEAREST,
            );
        }
    }

    #[inline]
    pub fn size(&self) -> Size {
        self.size
    }

    /// Returns the texture handle that should be used for sampling.
    ///
    /// When MSAA is enabled this returns the resolved texture,
    /// otherwise the directly rendered color attachment.
    #[inline]
    pub fn texture(&self) -> GLuint {
        if self.msaa_enabled {
            self.msaa_resolve_texture_id
        } else {
            self.texture_id
        }
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe { gl::DeleteTextures(1, &self.texture_id) };

        if self.msaa_enabled {
            unsafe {
                gl::DeleteTextures(1, &self.msaa_resolve_texture_id);
                gl::DeleteFramebuffers(1, &self.msaa_resolve_fbo_id);
            }
        }

        unsafe { gl::DeleteFramebuffers(1, &self.fbo_id) };
    }
}

/// Creates and attaches a 2D color texture (optionally multisampled)
/// to the currently bound framebuffer.
fn create_2d_color_attachment(
    size: Size,
    format: FramebufferFormat,
    samples: u32,
    msaa: bool,
) -> GLuint {
    let texture = gen_texture();

    let target = if msaa {
        gl::TEXTURE_2D_MULTISAMPLE
    } else {
        gl::TEXTURE_2D
    };

    unsafe { gl::BindTexture(target, texture) };

    if msaa {
        let internal = match format {
            FramebufferFormat::Tex2D => gl::RGB8,
            FramebufferFormat::Tex2DFloat => gl::RGBA32F,
            _ => unreachable!(),
        };

        unsafe {
            gl::TexImage2DMultisample(
                target,
                samples as i32,
                internal,
                size.width() as i32,
                size.height() as i32,
                gl::TRUE,
            )
        };
    } else {
        let (internal, format, ty) = match format {
            FramebufferFormat::Tex2D => (gl::RGB8, gl::RGB, gl::UNSIGNED_BYTE),
            FramebufferFormat::Tex2DFloat => (gl::RGBA32F, gl::RGBA, gl::FLOAT),
            _ => unreachable!(),
        };

        unsafe {
            gl::TexImage2D(
                target,
                0,
                internal as GLint,
                size.width() as i32,
                size.height() as i32,
                0,
                format,
                ty,
                std::ptr::null(),
            )
        };

        set_default_texture_params(gl::TEXTURE_2D);
    }

    unsafe { gl::FramebufferTexture2D(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, target, texture, 0) };

    texture
}

/// Creates a floating-point cubemap color attachment.
///
/// Each face is attached sequentially to validate framebuffer completeness.
fn create_cubemap_attachment(size: Size) -> GLuint {
    let texture = gen_texture();

    unsafe { gl::BindTexture(gl::TEXTURE_CUBE_MAP, texture) };

    let levels = (size.width().max(size.height()) as f32).log2().floor() as i32 + 1;

    unsafe {
        gl::TexStorage2D(
            gl::TEXTURE_CUBE_MAP,
            levels,
            gl::RGB16F,
            size.width() as i32,
            size.height() as i32,
        )
    };

    for face in 0..6 {
        unsafe {
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_CUBE_MAP_POSITIVE_X + face,
                texture,
                0,
            )
        };

        check_framebuffer_status();
    }

    texture
}

/// Creates a single-sampled framebuffer used to resolve MSAA rendering.
fn create_msaa_resolve_target(size: Size) -> (GLuint, GLuint) {
    let fbo = gen_framebuffer();
    bind_framebuffer(fbo);

    let texture = gen_texture();

    unsafe {
        gl::BindTexture(gl::TEXTURE_2D, texture);

        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGB8 as i32,
            size.width() as i32,
            size.height() as i32,
            0,
            gl::RGB,
            gl::UNSIGNED_BYTE,
            std::ptr::null(),
        );
    }

    set_default_texture_params(gl::TEXTURE_2D);

    unsafe {
        gl::FramebufferTexture2D(
            gl::FRAMEBUFFER,
            gl::COLOR_ATTACHMENT0,
            gl::TEXTURE_2D,
            texture,
            0,
        )
    };

    check_framebuffer_status();
    (fbo, texture)
}

/// Generates a framebuffer object.
fn gen_framebuffer() -> GLuint {
    let mut id = 0;
    unsafe { gl::GenFramebuffers(1, &mut id) };
    id
}

/// Generates a texture object.
fn gen_texture() -> GLuint {
    let mut id = 0;
    unsafe { gl::GenTextures(1, &mut id) };
    id
}

/// Binds the given framebuffer.
fn bind_framebuffer(id: GLuint) {
    unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, id) };
}

/// Returns the currently bound framebuffer.
fn current_framebuffer() -> GLuint {
    let mut id = 0;
    unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut id) };
    id as GLuint
}

/// Applies default sampling parameters used for render targets.
fn set_default_texture_params(target: GLenum) {
    unsafe {
        gl::TexParameteri(target, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
        gl::TexParameteri(target, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
    }
}

/// Verifies framebuffer completeness.
///
/// Logs the specific failure reason and panics if the framebuffer
/// is not complete.
fn check_framebuffer_status() {
    let status = unsafe { gl::CheckFramebufferStatus(gl::FRAMEBUFFER) };
    match status {
        gl::FRAMEBUFFER_COMPLETE => {}
        gl::FRAMEBUFFER_UNDEFINED => {
            log::error!("Framebuffer status: GL_FRAMEBUFFER_UNDEFINED.");
        }
        gl::FRAMEBUFFER_INCOMPLETE_ATTACHMENT => {
            log::error!("Framebuffer status: GL_FRAMEBUFFER_INCOMPLETE_ATTACHMENT.");
        }
        gl::FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT => {
            log::error!("Framebuffer status: GL_FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT.");
        }
        gl::FRAMEBUFFER_INCOMPLETE_DRAW_BUFFER => {
            log::error!("Framebuffer status: GL_FRAMEBUFFER_INCOMPLETE_DRAW_BUFFER.");
        }
        gl::FRAMEBUFFER_INCOMPLETE_READ_BUFFER => {
            log::error!("Framebuffer status: GL_FRAMEBUFFER_INCOMPLETE_READ_BUFFER.");
        }
        gl::FRAMEBUFFER_UNSUPPORTED => {
            log::error!("Framebuffer status: GL_FRAMEBUFFER_UNSUPPORTED.");
        }
        gl::FRAMEBUFFER_INCOMPLETE_MULTISAMPLE => {
            log::error!("Framebuffer status: GL_FRAMEBUFFER_INCOMPLETE_MULTISAMPLE.");
        }
        gl::FRAMEBUFFER_INCOMPLETE_LAYER_TARGETS => {
            log::error!("Framebuffer status: GL_FRAMEBUFFER_INCOMPLETE_LAYER_TARGETS.");
        }
        other => {
            log::error!("Framebuffer status: Unknown error code 0x{:X}", other);
        }
    }
    if status != gl::FRAMEBUFFER_COMPLETE {
        panic!("Framebuffer not complete!");
    }
}
