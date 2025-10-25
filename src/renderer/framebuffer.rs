// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use gl::types::*;

use crate::geometry::*;

#[derive(PartialEq, Clone)]
pub enum FramebufferFormat {
    Tex2D,
    Tex2DFloat,
    Cubemap,
}

pub struct Framebuffer {
    fbo_id: GLuint,
    texture_id: GLuint,
    size: Size,
    msaa_resolve_fbo_id: GLuint,
    msaa_resolve_texture_id: GLuint,
    msaa_enabled: bool,
}

impl Framebuffer {
    pub fn new(size: Size, msaa_samples: u32, kind: FramebufferFormat) -> Self {
        let mut original_fbo_id = 0;
        unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut original_fbo_id) };

        let msaa_enabled = msaa_samples > 0 && kind == FramebufferFormat::Tex2D;
        let mut fbo_id = 0;
        let mut texture_id = 0;

        unsafe {
            gl::GenFramebuffers(1, &mut fbo_id);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo_id);

            gl::GenTextures(1, &mut texture_id);

            match kind {
                FramebufferFormat::Tex2D | FramebufferFormat::Tex2DFloat => {
                    let target = if msaa_enabled {
                        gl::TEXTURE_2D_MULTISAMPLE
                    } else {
                        gl::TEXTURE_2D
                    };

                    gl::BindTexture(target, texture_id);

                    if msaa_enabled {
                        let internal_format = if kind == FramebufferFormat::Tex2D {
                            gl::RGB8
                        } else {
                            gl::RGBA32F
                        };
                        gl::TexImage2DMultisample(
                            target,
                            msaa_samples as i32,
                            internal_format,
                            size.width() as i32,
                            size.height() as i32,
                            gl::TRUE,
                        );
                    } else {
                        let (internal_format, format, type_) = if kind == FramebufferFormat::Tex2D {
                            (gl::RGB8, gl::RGB, gl::UNSIGNED_BYTE)
                        } else {
                            (gl::RGBA32F, gl::RGBA, gl::FLOAT)
                        };
                        gl::TexImage2D(
                            target,
                            0,
                            internal_format as GLint,
                            size.width() as i32,
                            size.height() as i32,
                            0,
                            format,
                            type_,
                            std::ptr::null(),
                        );
                    }

                    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
                    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);

                    gl::FramebufferTexture2D(
                        gl::FRAMEBUFFER,
                        gl::COLOR_ATTACHMENT0,
                        target,
                        texture_id,
                        0,
                    );
                    check_framebuffer_status();
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                }
                FramebufferFormat::Cubemap => {
                    gl::BindTexture(gl::TEXTURE_CUBE_MAP, texture_id);

                    let levels = (size.width().max(size.height()) as f32).log2().floor() as i32 + 1;
                    gl::TexStorage2D(
                        gl::TEXTURE_CUBE_MAP,
                        levels,
                        gl::RGB16F,
                        size.width() as i32,
                        size.height() as i32,
                    );

                    gl::FramebufferTexture2D(
                        gl::FRAMEBUFFER,
                        gl::COLOR_ATTACHMENT0,
                        gl::TEXTURE_CUBE_MAP_POSITIVE_X,
                        texture_id,
                        0,
                    );

                    const CUBEMAP_NUM_FACES: u32 = 6;
                    for face_idx in 0..CUBEMAP_NUM_FACES {
                        let face_target = gl::TEXTURE_CUBE_MAP_POSITIVE_X + face_idx;
                        gl::FramebufferTexture2D(
                            gl::FRAMEBUFFER,
                            gl::COLOR_ATTACHMENT0,
                            face_target,
                            texture_id,
                            0,
                        );
                        check_framebuffer_status();
                        gl::Clear(gl::COLOR_BUFFER_BIT);
                    }
                }
            }
        }

        let mut msaa_resolve_fbo_id = 0;
        let mut msaa_resolve_texture_id = 0;

        if msaa_enabled {
            unsafe {
                gl::GenFramebuffers(1, &mut msaa_resolve_fbo_id);
                gl::BindFramebuffer(gl::FRAMEBUFFER, msaa_resolve_fbo_id);

                gl::GenTextures(1, &mut msaa_resolve_texture_id);
                gl::BindTexture(gl::TEXTURE_2D, msaa_resolve_texture_id);
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
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);

                gl::FramebufferTexture2D(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    gl::TEXTURE_2D,
                    msaa_resolve_texture_id,
                    0,
                );
                check_framebuffer_status();
            }
        }

        unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, original_fbo_id as GLuint) };

        Self {
            fbo_id,
            texture_id,
            size,
            msaa_resolve_texture_id,
            msaa_resolve_fbo_id,
            msaa_enabled,
        }
    }

    pub fn bind(&self) {
        unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo_id) };
    }

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

    pub fn size(&self) -> Size {
        self.size
    }

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
        unsafe {
            gl::DeleteTextures(1, &self.texture_id);
            if self.msaa_enabled {
                gl::DeleteTextures(1, &self.msaa_resolve_texture_id);
                gl::DeleteFramebuffers(1, &self.msaa_resolve_fbo_id);
            }
            gl::DeleteFramebuffers(1, &self.fbo_id);
        }
    }
}

unsafe fn check_framebuffer_status() {
    let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
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
