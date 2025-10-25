// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use chrono::prelude::*;
use gl::types::*;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

use crate::{
    frame_controller::*,
    geometry::{Offset, Size},
    preset::*,
    shadertoy::to_glsl_version,
    APP_NAME, GL_VERSION,
};

use super::{framebuffer::*, program::*, shader::*, texture_manager::*, vertex_array::*};

const VERTEX_SHADER: &str = r#"
layout(location=0) in vec2 position;
layout(location=1) in vec2 texCoord;
#ifdef SHADERBG_CUBEMAP
layout(location=2) in vec3 rayDir;
#endif

out vec2 sbg_FragTexCoord;
#ifdef SHADERBG_CUBEMAP
out vec3 sbg_FragRayDir;
#endif

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    sbg_FragTexCoord = texCoord;
#ifdef SHADERBG_CUBEMAP
    sbg_FragRayDir = rayDir;
#endif
}
"#;

const CUBEMAP_DEFINITION: &str = "#define SHADERBG_CUBEMAP\n";

const SHADERBG_DEFINITION: &str = "#define SHADERBG\n";

const FRAGMENT_SHADER_HEADER: &str = r#"
in vec2 sbg_FragTexCoord;
#ifdef SHADERBG_CUBEMAP
in vec3 sbg_FragRayDir;
#endif
out vec4 sbg_FragColor;

const vec4 sbg_AssertColor[] = vec4[](vec4(1, 0, 0, 1), vec4(0, 1, 0, 1), vec4(0, 0, 1, 1), vec4(1, 1, 0, 1));
int sbg_AssertChannel = -1;
void st_assert(bool cond, int channel) {
    if (!cond) {
        sbg_AssertChannel = max(sbg_AssertChannel, clamp(channel, 0, 3));
    }
}
void st_assert(bool cond) {
    st_assert(cond, 0);
}

uniform vec3  iResolution;           // viewport resolution (in pixels)
uniform float iTime;                 // shader playback time (in seconds)
uniform float iGlobalTime;           // same as iTime
uniform float iTimeDelta;            // render time (in seconds)
uniform float iFrameRate;            // shader frame rate
uniform int   iFrame;                // shader playback frame
uniform vec4  iMouse;                // mouse pixel coords. xy: current (if MLB down), zw: click
uniform vec4  iDate;                 // (year, month, day, time in seconds)
uniform vec3  iChannelResolution[4]; // channel resolution (in pixels)
uniform float iChannelTime[4];       // TODO: channel playback time (in seconds)
uniform float iSampleRate;           // TODO: sound sample rate (i.e., 44100)

uniform vec2  iResolutionOffset;     // Offset to adjust gl_FragCoord when rendering to multiple monitors
"#;

const FRAGMENT_SHADER_FOOTER: &str = r#"
void main() {
    vec4 color;
#ifdef SHADERBG_CUBEMAP
    mainCubemap(color, gl_FragCoord.xy, vec3(0), normalize(sbg_FragRayDir));
#else
    mainImage(color, gl_FragCoord.xy + iResolutionOffset);
#endif
    sbg_FragColor = sbg_AssertChannel < 0 ? color : sbg_AssertColor[sbg_AssertChannel];
}
"#;

const CUBEMAP_NUM_FACES: usize = 6;
const CUBEMAP_FACE_RESOLUTION: u32 = 1024;

enum PassType {
    Buffer2D,
    Cubemap,
}

struct UniformLocations {
    i_resolution: GLint,
    i_time: GLint,
    i_global_time: GLint,
    i_time_delta: GLint,
    i_frame_rate: GLint,
    i_frame: GLint,
    i_mouse: GLint,
    i_date: GLint,
    i_channel_resolution: GLint,
    i_resolution_offset: GLint,
    i_channel: [GLint; 4],
}

pub struct RenderPass {
    name: String,
    program: Program,
    framebuffers: [Framebuffer; 2],
    pass_type: PassType,
    inputs: [Option<Input>; 4],
    is_image_pass: bool,
    uniform_locations: UniformLocations,
    texture_manager: Rc<RefCell<TextureManager>>,
}

impl RenderPass {
    pub fn new(
        name: &str,
        common_shader: &str,
        pass_shader: &str,
        framebuffer_size: Size,
        inputs: [Option<Input>; 4],
        texture_manager: Rc<RefCell<TextureManager>>,
        msaa_samples: u32,
    ) -> Result<Self, ShaderError> {
        let mut is_cubemap_pass = name == "Cube A";
        let mut channel_uniform_declarations = String::default();

        for (i, input_opt) in inputs.iter().enumerate() {
            let _type = input_opt.as_ref().map_or("2D", |input| match input._type {
                InputType::Cubemap => "Cube",
                InputType::Misc if input.name == "Cubemap A" => "Cube",
                InputType::Volume => "3D",
                _ => "2D",
            });
            channel_uniform_declarations += &format!("uniform sampler{_type} iChannel{i};\n");
        }

        let version_directive = || format!("#version {}{}0 core\n", GL_VERSION.0, GL_VERSION.1);

        let vertex_shader_source = version_directive()
            + if is_cubemap_pass {
                CUBEMAP_DEFINITION
            } else {
                ""
            }
            + VERTEX_SHADER;

        let fragment_shader_source = &(version_directive()
            + SHADERBG_DEFINITION
            + if is_cubemap_pass {
                CUBEMAP_DEFINITION
            } else {
                ""
            }
            + FRAGMENT_SHADER_HEADER
            + &channel_uniform_declarations
            + "\n"
            + &to_glsl_version(
                &(SHADERBG_DEFINITION.to_string() + common_shader + "\n" + pass_shader + "\n"),
                GL_VERSION,
                false,
            )?
            + "\n"
            + FRAGMENT_SHADER_FOOTER);

        let vertex_shader = Shader::new(&vertex_shader_source, gl::VERTEX_SHADER)?;

        let default_fragment_shader = || {
            let default_shader_source = version_directive()
                + FRAGMENT_SHADER_HEADER
                + &defaults::default_image_shader()
                + FRAGMENT_SHADER_FOOTER;
            Shader::new(&default_shader_source, gl::FRAGMENT_SHADER)
                .expect("Error compiling default fragment shader")
        };

        let fragment_shader = {
            let result = Shader::new(fragment_shader_source, gl::FRAGMENT_SHADER);
            if let Err(err) = result {
                let mut err_msg = format!("Error compiling '{name}' pass shader: {err}")
                    .trim()
                    .to_string();
                let log_file = log_dir().join(format!("{}.frag", name.to_lowercase()));

                if std::fs::write(&log_file, fragment_shader_source).is_ok() {
                    err_msg += &format!(" - Shader saved to {}", log_file.to_str().unwrap());
                }

                log::error!("{}", err_msg);
                is_cubemap_pass = false;
                default_fragment_shader()
            } else {
                result?
            }
        };

        let program = {
            let result = Program::new(&[vertex_shader, fragment_shader]);
            if let Err(err) = result {
                log::error!("Error linking '{name}' pass program: {err}");
                let vertex_shader = Shader::new(&vertex_shader_source, gl::VERTEX_SHADER)?;
                is_cubemap_pass = false;
                Program::new(&[vertex_shader, default_fragment_shader()])?
            } else {
                result?
            }
        };

        let uniform_locations = UniformLocations {
            i_resolution: program.uniform_location("iResolution")?,
            i_time: program.uniform_location("iTime")?,
            i_global_time: program.uniform_location("iGlobalTime")?,
            i_time_delta: program.uniform_location("iTimeDelta")?,
            i_frame_rate: program.uniform_location("iFrameRate")?,
            i_frame: program.uniform_location("iFrame")?,
            i_mouse: program.uniform_location("iMouse")?,
            i_date: program.uniform_location("iDate")?,
            i_channel_resolution: program.uniform_location("iChannelResolution")?,
            i_resolution_offset: program.uniform_location("iResolutionOffset")?,
            i_channel: [
                program.uniform_location("iChannel0")?,
                program.uniform_location("iChannel1")?,
                program.uniform_location("iChannel2")?,
                program.uniform_location("iChannel3")?,
            ],
        };

        let is_image_pass = name == "Image";

        let (pass_type, size, framebuffer_kind) = if is_cubemap_pass {
            (
                PassType::Cubemap,
                Size::new(CUBEMAP_FACE_RESOLUTION, CUBEMAP_FACE_RESOLUTION),
                FramebufferFormat::Cubemap,
            )
        } else {
            (
                PassType::Buffer2D,
                framebuffer_size,
                if is_image_pass {
                    FramebufferFormat::Tex2D
                } else {
                    FramebufferFormat::Tex2DFloat
                },
            )
        };

        Ok(Self {
            name: name.to_string(),
            program,
            framebuffers: [
                Framebuffer::new(size, msaa_samples, framebuffer_kind.clone()),
                Framebuffer::new(size, msaa_samples, framebuffer_kind),
            ],
            pass_type,
            inputs,
            is_image_pass,
            uniform_locations,
            texture_manager,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn inputs(&self) -> &[Option<Input>; 4] {
        &self.inputs
    }

    pub fn framebuffers(&self) -> &[Framebuffer; 2] {
        &self.framebuffers
    }

    pub fn render_pass(
        &self,
        vaos: &[VertexArray],
        i_resolution_offset_data: Offset,
        i_mouse_data: [i32; 4],
        screen_size: Size,
        framebuffer_scale: f32,
        frame_stats: &FrameStats,
    ) {
        let i_resolution_offset_data = if self.is_image_pass {
            i_resolution_offset_data
        } else {
            Offset::default()
        };

        match self.pass_type {
            PassType::Buffer2D => self.render_2d_pass(
                &vaos[0],
                i_resolution_offset_data,
                i_mouse_data,
                screen_size,
                framebuffer_scale,
                frame_stats,
            ),
            PassType::Cubemap => {
                self.render_cubemap_pass(&vaos[1..=CUBEMAP_NUM_FACES], i_mouse_data, frame_stats)
            }
        }
    }

    fn render_2d_pass(
        &self,
        vao: &VertexArray,
        i_resolution_offset_data: Offset,
        i_mouse_data: [i32; 4],
        screen_size: Size,
        framebuffer_scale: f32,
        frame_stats: &FrameStats,
    ) {
        let framebuffer_idx = ((frame_stats.frame_number + 1) % 2) as usize;
        let framebuffer = &self.framebuffers[framebuffer_idx];
        let framebuffer_size = framebuffer.size();

        self.program.use_program();

        self.set_common_uniforms(screen_size, i_mouse_data, framebuffer_scale, frame_stats);
        self.set_channel_uniforms(frame_stats);

        if self.uniform_locations.i_resolution_offset >= 0 {
            let resolution_offset = i_resolution_offset_data * framebuffer_scale;
            unsafe {
                gl::Uniform2f(
                    self.uniform_locations.i_resolution_offset,
                    resolution_offset.dx() as GLfloat,
                    resolution_offset.dy() as GLfloat,
                );
            }
        }

        vao.bind();

        framebuffer.bind();

        unsafe {
            gl::Viewport(
                0,
                0,
                framebuffer_size.width() as i32,
                framebuffer_size.height() as i32,
            );
            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
        }

        if self.is_image_pass {
            framebuffer.resolve();
        }
    }

    fn render_cubemap_pass(
        &self,
        cubemap_vaos: &[VertexArray],
        i_mouse_data: [i32; 4],
        frame_stats: &FrameStats,
    ) {
        const CUBEMAP_FACES: [GLenum; CUBEMAP_NUM_FACES] = [
            gl::TEXTURE_CUBE_MAP_POSITIVE_X,
            gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
            gl::TEXTURE_CUBE_MAP_POSITIVE_Y,
            gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
            gl::TEXTURE_CUBE_MAP_POSITIVE_Z,
            gl::TEXTURE_CUBE_MAP_NEGATIVE_Z,
        ];

        let resolution = Size::new(CUBEMAP_FACE_RESOLUTION, CUBEMAP_FACE_RESOLUTION);
        let framebuffer_idx = ((frame_stats.frame_number + 1) % 2) as usize;

        for (face_idx, &face) in CUBEMAP_FACES.iter().enumerate() {
            self.program.use_program();

            self.set_common_uniforms(resolution, i_mouse_data, 1., frame_stats);
            self.set_channel_uniforms(frame_stats);

            cubemap_vaos[face_idx].bind();

            self.framebuffers[framebuffer_idx].bind_cubemap_face(face);

            unsafe {
                gl::Viewport(0, 0, resolution.width() as i32, resolution.height() as i32);
                gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
            }
        }
    }

    fn set_common_uniforms(
        &self,
        screen_size: Size,
        i_mouse_data: [i32; 4],
        framebuffer_scale: f32,
        frame_stats: &FrameStats,
    ) {
        let i_resolution_location = self.uniform_locations.i_resolution;
        if i_resolution_location >= 0 {
            let mut resolution = screen_size * framebuffer_scale;
            resolution.set_width(resolution.width().max(1));
            resolution.set_height(resolution.height().max(1));

            unsafe {
                gl::Uniform3f(
                    i_resolution_location,
                    resolution.width() as GLfloat,
                    resolution.height() as GLfloat,
                    1.,
                );
            }
        }

        let i_time_location = self.uniform_locations.i_time;
        if i_time_location >= 0 {
            unsafe { gl::Uniform1f(i_time_location, frame_stats.time.as_secs_f32()) };
        }

        let i_global_time_location = self.uniform_locations.i_global_time;
        if i_global_time_location >= 0 {
            unsafe { gl::Uniform1f(i_global_time_location, frame_stats.time.as_secs_f32()) };
        }

        let i_time_delta_location = self.uniform_locations.i_time_delta;
        if i_time_delta_location >= 0 {
            unsafe { gl::Uniform1f(i_time_delta_location, frame_stats.time_delta.as_secs_f32()) };
        }

        let i_frame_rate_location = self.uniform_locations.i_frame_rate;
        if i_frame_rate_location >= 0 {
            unsafe { gl::Uniform1f(i_frame_rate_location, frame_stats.frame_rate as f32) };
        }

        let i_frame_location = self.uniform_locations.i_frame;
        if i_frame_location >= 0 {
            unsafe { gl::Uniform1i(i_frame_location, frame_stats.frame_number as i32 % i32::MAX) };
        }

        let i_mouse_location = self.uniform_locations.i_mouse;
        if i_mouse_location >= 0 {
            let data = if i_mouse_data[0] >= 0 {
                i_mouse_data.map(|v| (v as f32 * framebuffer_scale).round())
            } else {
                [0.; 4]
            };
            unsafe { gl::Uniform4fv(i_mouse_location, 1, data.as_ptr()) };
        }

        let i_date_location = self.uniform_locations.i_date;
        if i_date_location >= 0 {
            let now = Local::now();
            let year = now.year() as f32;
            let month = (now.month() - 1) as f32;
            let day = now.day() as f32;

            const NANOS_PER_SEC: u32 = 1_000_000_000;
            let time = now.num_seconds_from_midnight() as f32
                + (now.nanosecond() as f32) / (NANOS_PER_SEC as f32);

            unsafe { gl::Uniform4f(i_date_location, year, month, day, time) };
        }
    }

    fn set_channel_uniforms(&self, frame_stats: &FrameStats) {
        let mut channel_resolutions = Vec::<f32>::default();

        for (idx, input) in self
            .inputs
            .iter()
            .enumerate()
            .filter_map(|(idx, opt)| opt.as_ref().map(|input| (idx, input)))
        {
            let mut texture_name = input.name.clone();

            if matches!(
                input.name.as_str(),
                "Buffer A" | "Buffer B" | "Buffer C" | "Buffer D" | "Cubemap A"
            ) {
                let mut offset = 0;
                if self.name > input.name {
                    let texture_name_with_suffix =
                        input.name.clone() + &(frame_stats.frame_number % 2).to_string();
                    let previous_frame_number = self
                        .texture_manager
                        .borrow_mut()
                        .update_frame_number(&texture_name_with_suffix, frame_stats.frame_number)
                        .unwrap();
                    if previous_frame_number != frame_stats.frame_number {
                        offset = 1;
                    }
                };
                texture_name += &((frame_stats.frame_number + offset) % 2).to_string();
            }

            if input._type == InputType::Texture && input.vflip {
                texture_name += "vflip";
            }

            if let Some(texture_id) = self.texture_manager.borrow().id(&texture_name) {
                let target = if input._type == InputType::Cubemap || input.name == "Cubemap A" {
                    gl::TEXTURE_CUBE_MAP
                } else if input._type == InputType::Volume {
                    gl::TEXTURE_3D
                } else {
                    gl::TEXTURE_2D
                };

                unsafe {
                    gl::ActiveTexture(gl::TEXTURE0 + idx as GLuint);
                    gl::BindTexture(target, texture_id);
                }

                let wrap_mode = if input.wrap == WrapMode::Repeat {
                    gl::REPEAT
                } else {
                    gl::CLAMP_TO_EDGE
                };
                unsafe {
                    gl::TexParameteri(target, gl::TEXTURE_WRAP_S, wrap_mode as i32);
                    gl::TexParameteri(target, gl::TEXTURE_WRAP_T, wrap_mode as i32);
                    if target == gl::TEXTURE_3D {
                        gl::TexParameteri(target, gl::TEXTURE_WRAP_R, wrap_mode as i32);
                    }
                }

                let (min_filter, mag_filter) = match input.filter {
                    FilterMode::Nearest => (gl::NEAREST, gl::NEAREST),
                    FilterMode::Linear => (gl::LINEAR, gl::LINEAR),
                    FilterMode::Mipmap => (gl::LINEAR_MIPMAP_LINEAR, gl::LINEAR),
                };
                unsafe {
                    gl::TexParameteri(target, gl::TEXTURE_MIN_FILTER, min_filter as i32);
                    gl::TexParameteri(target, gl::TEXTURE_MAG_FILTER, mag_filter as i32);
                }

                if input._type == InputType::Misc && input.filter == FilterMode::Mipmap {
                    unsafe { gl::GenerateMipmap(target) };
                }

                let i_channel_location = self.uniform_locations.i_channel[idx];
                if i_channel_location >= 0 {
                    unsafe { gl::Uniform1i(i_channel_location, idx as i32) };
                }

                let (mut width, mut height, mut depth): (i32, i32, i32) = (0, 0, 1);
                if input._type == InputType::Cubemap {
                    let target = gl::TEXTURE_CUBE_MAP_POSITIVE_X;
                    unsafe {
                        gl::GetTexLevelParameteriv(target, 0, gl::TEXTURE_WIDTH, &mut width);
                        gl::GetTexLevelParameteriv(target, 0, gl::TEXTURE_HEIGHT, &mut height);
                    }
                } else if input._type == InputType::Volume {
                    unsafe {
                        gl::GetTexLevelParameteriv(target, 0, gl::TEXTURE_WIDTH, &mut width);
                        gl::GetTexLevelParameteriv(target, 0, gl::TEXTURE_HEIGHT, &mut height);
                        gl::GetTexLevelParameteriv(target, 0, gl::TEXTURE_DEPTH, &mut depth);
                    }
                } else {
                    unsafe {
                        gl::GetTexLevelParameteriv(target, 0, gl::TEXTURE_WIDTH, &mut width);
                        gl::GetTexLevelParameteriv(target, 0, gl::TEXTURE_HEIGHT, &mut height);
                    }
                }

                channel_resolutions.push(width as f32);
                channel_resolutions.push(height as f32);
                channel_resolutions.push(depth as f32);
            }
        }

        let i_channel_resolution_location = self.uniform_locations.i_channel_resolution;
        if i_channel_resolution_location >= 0 {
            unsafe {
                gl::Uniform3fv(
                    i_channel_resolution_location,
                    4,
                    channel_resolutions.as_ptr(),
                )
            };
        }
    }
}

fn log_dir() -> PathBuf {
    fn fallback_dir() -> PathBuf {
        std::env::current_dir().expect("Failed to get current working directory")
    }

    let dir = dirs::cache_dir()
        .map(|p| p.join(APP_NAME))
        .unwrap_or_else(|| {
            log::warn!("Could not find $XDG_CACHE_HOME or $HOME/.cache; using current directory.");
            fallback_dir()
        });

    if !dir.exists() {
        if let Err(err) = std::fs::create_dir_all(&dir) {
            log::warn!("Failed to create log directory at {}: {err}", dir.display());
            return fallback_dir();
        }
    }

    dir
}
