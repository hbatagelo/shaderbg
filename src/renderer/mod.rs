// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod shader;

mod buffer;
mod check_gl_error;
mod framebuffer;
mod program;
mod render_pass;
mod texture_manager;
mod vertex_array;

use gl::types::*;
use std::{cell::RefCell, rc::Rc};

#[cfg(debug_assertions)]
use check_gl_error::*;
use {buffer::*, program::*, render_pass::*, shader::*, texture_manager::*, vertex_array::*};

use crate::{frame_controller::*, geometry::*, preset::*, *};

const BLIT_VERTEX_SHADER: &str = r#"
layout(location=0) in vec2 position;
layout(location=1) in vec2 texCoord;

out vec2 fragTexCoord;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    fragTexCoord = texCoord;
}
"#;

const DEFINE_CROSSFADE: &str = "#define SHADERBG_CROSSFADE\n";

const BLIT_FRAGMENT_SHADER: &str = r#"
in vec2 fragTexCoord;
out vec4 fragColor;

uniform sampler2D iBlitTexture[2];
#ifdef SHADERBG_CROSSFADE
uniform float iCrossfadeT;
#endif

void main() {
    vec4 color0 = texture(iBlitTexture[0], fragTexCoord);
#ifdef SHADERBG_CROSSFADE
    vec4 color1 = texture(iBlitTexture[1], fragTexCoord);
    fragColor = mix(color0, color1, iCrossfadeT);
#else
    fragColor = color0;
#endif
}
"#;

const MSAA_SAMPLES: u32 = 0;

type Position = [f32; 2];
type TexCoord = [f32; 2];
type RayDir = [f32; 3];

#[repr(C)]
struct Vertex(Position, TexCoord);
#[repr(C)]
struct VertexCubemap(Position, TexCoord, RayDir);

struct BlitUniformLocations {
    i_blit_texture: GLint,
    i_crossfade_t: GLint,
}

struct ViewportSettings {
    filter: FilterMode,
    mapping: LayoutMode,
    size: Size,
}

pub struct Renderer {
    blit_program: Program,
    blit_uniform_locations: BlitUniformLocations,
    vaos: Vec<VertexArray>,
    _vbos: Vec<Buffer>,
    _ebo: Buffer,
    original_fbo_id: GLuint,
    passes: Vec<RenderPass>,
    screen_size: Size,
    framebuffer_scale: f32,
    viewport_settings: ViewportSettings,
    msaa_samples: u32,
}

impl Renderer {
    pub fn new(
        screen_size: Size,
        viewport_size: Size,
        monitor_size: Size,
        preset: &Preset,
    ) -> Result<Self, ShaderError> {
        #[cfg(debug_assertions)]
        setup_opengl_debugging();

        let version_directive = format!("#version {}{}0 core\n", GL_VERSION.0, GL_VERSION.1);

        let blit_vertex_source_code = version_directive.clone() + BLIT_VERTEX_SHADER;
        let blit_vertex_shader = Shader::new(&blit_vertex_source_code, gl::VERTEX_SHADER)?;

        let crossfade_enabled = preset.crossfade_overlap_ratio > 0.0;

        let blit_fragment_source_code = version_directive
            + if crossfade_enabled {
                DEFINE_CROSSFADE
            } else {
                ""
            }
            + BLIT_FRAGMENT_SHADER;
        let blit_fragment_shader = Shader::new(&blit_fragment_source_code, gl::FRAGMENT_SHADER)?;

        let blit_program = Program::new(&[blit_vertex_shader, blit_fragment_shader])?;
        let i_blit_texture = blit_program.uniform_location("iBlitTexture")?;
        let i_crossfade_t = if crossfade_enabled {
            blit_program.uniform_location("iCrossfadeT")?
        } else {
            0
        };

        let msaa_samples = {
            let mut max_msaa_samples = 0;
            unsafe { gl::GetIntegerv(gl::MAX_SAMPLES, &mut max_msaa_samples) };
            std::cmp::min(MSAA_SAMPLES, max_msaa_samples as u32)
        };

        let mut original_fbo_id = 0;
        unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut original_fbo_id) };

        let framebuffer_scale = preset.resolution_scale.max(0.0);
        let framebuffer_size = {
            let mut size = monitor_size * framebuffer_scale;
            size.set_width(size.width().max(1));
            size.set_height(size.height().max(1));
            size
        };

        let vao = VertexArray::new();
        vao.bind();

        let mut max_u = (viewport_size.width() as f32 / framebuffer_size.width() as f32).max(1.0);
        let mut max_v = (viewport_size.height() as f32 / framebuffer_size.height() as f32).max(1.0);

        if preset.layout_mode == LayoutMode::Stretch || preset.layout_mode == LayoutMode::Center {
            max_u = 1.0;
            max_v = 1.0;
        }

        #[rustfmt::skip]
        let vertices: [Vertex; 4] = [
            Vertex([-1.0, -1.0], [  0.0,   0.0]),
            Vertex([ 1.0, -1.0], [max_u,   0.0]),
            Vertex([ 1.0,  1.0], [max_u, max_v]),
            Vertex([-1.0,  1.0], [  0.0, max_v]),
            ];

        let vbo = Buffer::new(gl::ARRAY_BUFFER);
        vbo.set_data(&vertices, gl::STATIC_DRAW);
        let ebo = Buffer::new(gl::ELEMENT_ARRAY_BUFFER);

        const INDICES: [i32; 6] = [0, 1, 2, 2, 3, 0];
        ebo.set_data(&INDICES, gl::STATIC_DRAW);

        set_attribute!(vao, 0, Vertex::0);
        set_attribute!(vao, 1, Vertex::1);

        let mut vaos = Vec::<VertexArray>::new();
        let mut vbos = Vec::<Buffer>::new();
        vaos.push(vao);
        vbos.push(vbo);

        if preset.cube_a.is_some() {
            let ray_dir = |face: GLenum, u: f32, v: f32| {
                let u = u * 2.0 - 1.0;
                let v = v * 2.0 - 1.0;
                match face {
                    #[rustfmt::skip]
                    gl::TEXTURE_CUBE_MAP_POSITIVE_X => [ 1.0,    v,    -u],
                    #[rustfmt::skip]
                    gl::TEXTURE_CUBE_MAP_NEGATIVE_X => [-1.0,    v,     u],
                    #[rustfmt::skip]
                    gl::TEXTURE_CUBE_MAP_POSITIVE_Y => [   u,  1.0,    -v],
                    #[rustfmt::skip]
                    gl::TEXTURE_CUBE_MAP_NEGATIVE_Y => [   u, -1.0,     v],
                    #[rustfmt::skip]
                    gl::TEXTURE_CUBE_MAP_POSITIVE_Z => [   u,    v,   1.0],
                    #[rustfmt::skip]
                    gl::TEXTURE_CUBE_MAP_NEGATIVE_Z => [  -u,    v,  -1.0],
                    _ => [0.0, 0.0, 0.0],
                }
            };

            const CUBEMAP_NUM_FACES: usize = 6;
            const CUBEMAP_FACES: [GLenum; CUBEMAP_NUM_FACES] = [
                gl::TEXTURE_CUBE_MAP_POSITIVE_X,
                gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
                gl::TEXTURE_CUBE_MAP_POSITIVE_Y,
                gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
                gl::TEXTURE_CUBE_MAP_POSITIVE_Z,
                gl::TEXTURE_CUBE_MAP_NEGATIVE_Z,
            ];

            for &face in CUBEMAP_FACES.iter() {
                let cube_vao = VertexArray::new();
                cube_vao.bind();
                let cube_vbo = Buffer::new(gl::ARRAY_BUFFER);

                #[rustfmt::skip]
                let vertices: [VertexCubemap; 4] = [
                    VertexCubemap([-1.0, -1.0], [0.0, 0.0], ray_dir(face, 0.0, 0.0)),
                    VertexCubemap([ 1.0, -1.0], [1.0, 0.0], ray_dir(face, 1.0, 0.0)),
                    VertexCubemap([ 1.0,  1.0], [1.0, 1.0], ray_dir(face, 1.0, 1.0)),
                    VertexCubemap([-1.0,  1.0], [0.0, 1.0], ray_dir(face, 0.0, 1.0)),
                ];

                cube_vbo.set_data(&vertices, gl::STATIC_DRAW);

                ebo.bind();

                set_attribute!(cube_vao, 0, VertexCubemap::0);
                set_attribute!(cube_vao, 1, VertexCubemap::1);
                set_attribute!(cube_vao, 2, VertexCubemap::2);

                vaos.push(cube_vao);
                vbos.push(cube_vbo);
            }
        }
        let texture_manager = Rc::new(RefCell::new(TextureManager::new()));

        let offscreen_size = screen_size * framebuffer_scale;
        let passes_settings = [
            ("Buffer A", preset.buffer_a.as_ref(), offscreen_size),
            ("Buffer B", preset.buffer_b.as_ref(), offscreen_size),
            ("Buffer C", preset.buffer_c.as_ref(), offscreen_size),
            ("Buffer D", preset.buffer_d.as_ref(), offscreen_size),
            ("Cube A", preset.cube_a.as_ref(), offscreen_size),
            ("Image", Some(&preset.image), framebuffer_size),
        ];

        let common_shader = if let Some(common_pass) = preset.common.as_ref() {
            &common_pass.shader
        } else {
            ""
        };

        let mut passes = Vec::new();
        for (name, pass_cfg_opt, size) in passes_settings.iter() {
            if let Some(pass_cfg) = pass_cfg_opt {
                let inputs: [Option<Input>; 4] = [
                    pass_cfg.input_0.clone(),
                    pass_cfg.input_1.clone(),
                    pass_cfg.input_2.clone(),
                    pass_cfg.input_3.clone(),
                ];
                let pass = RenderPass::new(
                    name,
                    common_shader,
                    &pass_cfg.shader,
                    *size,
                    inputs,
                    texture_manager.clone(),
                    msaa_samples,
                )?;
                passes.push(pass);
            }
        }

        texture_manager.borrow_mut().load(&passes);

        Ok(Self {
            blit_program,
            blit_uniform_locations: BlitUniformLocations {
                i_blit_texture,
                i_crossfade_t,
            },
            vaos,
            _vbos: vbos,
            _ebo: ebo,
            original_fbo_id: original_fbo_id as GLuint,
            passes,
            screen_size,
            framebuffer_scale,
            viewport_settings: ViewportSettings {
                filter: preset.filter_mode,
                mapping: preset.layout_mode,
                size: viewport_size,
            },
            msaa_samples,
        })
    }

    pub fn render(
        &self,
        i_resolution_offset_data: Offset,
        i_mouse_data: [i32; 4],
        frame_stats: &FrameStats,
    ) {
        log::trace!(
            "Frame {}: t={:.2} s, Δt {:.1} ms, {:.1} FPS",
            frame_stats.frame_number,
            frame_stats.time.as_secs_f64(),
            frame_stats.time_delta.as_secs_f64() * 1000.0,
            frame_stats.frame_rate
        );

        for pass in &self.passes {
            pass.render_pass(
                &self.vaos,
                i_resolution_offset_data,
                i_mouse_data,
                self.screen_size,
                self.framebuffer_scale,
                frame_stats,
            );
        }
    }

    pub fn blit(&self, crossfade_t: f32) {
        let crossfade_enabled = self.blit_uniform_locations.i_crossfade_t > 0;
        let mipmapping_enabled = self.viewport_settings.filter == FilterMode::Mipmap;

        let framebuffer_size = self.passes.last().unwrap().framebuffers()[0].size();
        let origin = match self.viewport_settings.mapping {
            LayoutMode::Center => {
                Point::new(
                    self.viewport_settings.size.width() as i32 - framebuffer_size.width() as i32,
                    self.viewport_settings.size.height() as i32 - framebuffer_size.height() as i32,
                ) * 0.5
            }
            _ => Point::default(),
        };
        let size = match self.viewport_settings.mapping {
            LayoutMode::Stretch => self.viewport_settings.size,
            _ => framebuffer_size,
        };

        if self.msaa_samples > 0
            || self.framebuffer_scale > 1.0
            || crossfade_enabled
            || mipmapping_enabled
            || self.viewport_settings.mapping == LayoutMode::Repeat
            || self.viewport_settings.mapping == LayoutMode::MirroredRepeat
        {
            let viewport_size = match self.viewport_settings.mapping {
                LayoutMode::Stretch | LayoutMode::Center => size,
                _ => self.viewport_settings.size,
            };

            unsafe {
                gl::Viewport(
                    origin.x(),
                    origin.y(),
                    viewport_size.width() as i32,
                    viewport_size.height() as i32,
                );
            }

            self.blit_program.use_program();
            self.vaos[0].bind();

            let set_texture_parameters = || unsafe {
                if mipmapping_enabled {
                    gl::GenerateMipmap(gl::TEXTURE_2D);
                }

                let (min_filter, mag_filter) = match self.viewport_settings.filter {
                    FilterMode::Linear => (gl::LINEAR, gl::LINEAR),
                    FilterMode::Nearest => (gl::NEAREST, gl::NEAREST),
                    FilterMode::Mipmap => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
                };

                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, min_filter as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, mag_filter as i32);

                let wrap_mode = match self.viewport_settings.mapping {
                    LayoutMode::MirroredRepeat => gl::MIRRORED_REPEAT,
                    _ => gl::REPEAT,
                } as i32;

                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, wrap_mode);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, wrap_mode);
            };

            unsafe {
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(
                    gl::TEXTURE_2D,
                    self.passes.last().unwrap().framebuffers()[0].texture(),
                );
                set_texture_parameters();

                if crossfade_enabled {
                    gl::ActiveTexture(gl::TEXTURE1);
                    gl::BindTexture(
                        gl::TEXTURE_2D,
                        self.passes.last().unwrap().framebuffers()[1].texture(),
                    );
                    set_texture_parameters();

                    gl::Uniform1f(self.blit_uniform_locations.i_crossfade_t, crossfade_t);
                }

                if self.blit_uniform_locations.i_blit_texture >= 0 {
                    const DATA: [i32; 2] = [0, 1];
                    gl::Uniform1iv(self.blit_uniform_locations.i_blit_texture, 2, DATA.as_ptr());
                }

                gl::BindFramebuffer(gl::FRAMEBUFFER, self.original_fbo_id);

                gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
            }
        } else {
            let filter = match self.viewport_settings.filter {
                FilterMode::Nearest => gl::NEAREST,
                _ => gl::LINEAR,
            };

            self.passes.last().unwrap().framebuffers()[0].blit_to(
                self.original_fbo_id,
                origin,
                size,
                filter,
            );
        }
    }
}
