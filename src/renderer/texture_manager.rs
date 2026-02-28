// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! Texture resource management for ShaderToy-style inputs.
//!
//! Responsible for loading external textures (2D, cubemap, 3D),
//! registering render-pass outputs as textures, and managing
//! texture lifetime, including the ShaderToy keyboard input texture.

use gl::types::*;
use image::*;
use std::{collections::HashMap, path::PathBuf};

use crate::{geometry::Size, keyboard_controller::KeyboardData, preset::*, APP_NAME};

use super::render_pass::RenderPass;

/// GPU texture wrapper with ownership semantics.
///
/// Textures created from external inputs are owned and deleted
/// by the manager. Textures originating from framebuffer outputs
/// are borrowed and must not be deleted.
struct Texture {
    id: GLuint,
    input_type: InputType,
}

impl Texture {
    fn new(id: u32, input_type: InputType) -> Self {
        Self { id, input_type }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        // Only delete textures created by the manager.
        // Framebuffer attachments are owned elsewhere.
        let managed = self.input_type != InputType::Misc;
        if managed {
            unsafe { gl::DeleteTextures(1, &self.id) };
        }
    }
}

/// Width matches JavaScript keycode range (0–255).
const KEYBOARD_TEXTURE_WIDTH: usize = 256;

/// Three rows encoding ShaderToy keyboard state:
/// Row #0: Keydown
/// Row #1: keypressed (one-frame pulse)
/// Row #2: Toggled
const KEYBOARD_TEXTURE_HEIGHT: usize = 3;

/// Central registry for all textures used by the renderer.
///
/// Maintains the external input textures, the framebuffer output
/// textures and ShaderToy keyboard texture. Guarantees that each
/// logical input name maps to exactly one GPU texture instance.
///
/// Framebuffer outputs are exposed as textures using a
/// double-buffered ("ping-pong") naming convention based on
/// appending "0" or "1" to the render pass name, e.g.:
///
/// Buffer A0 / Buffer A1
/// Buffer B0 / Buffer B1
/// .
/// .
/// .
/// Cubemap A0 / Cubemap A1
///
/// This allows passes to safely read previous-frame results.
pub struct TextureManager {
    map: HashMap<String, Texture>,
    keyboard_texture: Option<Texture>,
    // index = row * 256 + keycode
    keyboard_state: [u8; KEYBOARD_TEXTURE_WIDTH * KEYBOARD_TEXTURE_HEIGHT],
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            keyboard_texture: None,
            keyboard_state: [0; KEYBOARD_TEXTURE_WIDTH * KEYBOARD_TEXTURE_HEIGHT],
        }
    }

    /// Returns the OpenGL texture ID associated with an input name.
    pub fn id(&self, name: &str) -> Option<GLuint> {
        self.map.get(name).map(|t| t.id)
    }

    pub fn keyboard_id(&self) -> Option<GLuint> {
        self.keyboard_texture.as_ref().map(|t| t.id)
    }

    /// Loads textures required by the render pipeline.
    ///
    /// Performs three passes:
    /// 1. Creates keyboard texture if any pass requires it.
    /// 2. Loads external input textures (deduplicated).
    /// 3. Registers framebuffer outputs as named textures.
    pub fn load(&mut self, passes: &[RenderPass]) {
        if self.keyboard_texture.is_none() {
            let uses_keyboard = passes.iter().any(|pass| {
                pass.inputs()
                    .iter()
                    .filter_map(|opt| opt.as_ref())
                    .any(|input| input._type == InputType::Keyboard)
            });
            if uses_keyboard {
                self.keyboard_texture =
                    Some(Texture::new(create_keyboard_texture(), InputType::Keyboard));
            }
        }

        let assets_dir = assets_dir();

        // Load external textures and register pass outputs
        for pass in passes {
            for input in pass.inputs().iter().filter_map(|opt| opt.as_ref()) {
                // Texture key uniquely identifies texture + vertical flip state.
                // Prevents duplicate GPU uploads.
                let key = input.name.clone()
                    + if input.vflip
                        && matches!(input._type, InputType::Texture | InputType::Cubemap)
                    {
                        "vflip"
                    } else {
                        ""
                    };
                if !input.name.is_empty()
                    && input._type != InputType::Misc
                    && input._type != InputType::Keyboard
                    && !self.map.contains_key(&key)
                {
                    // Determine whether any pass requests mipmapped sampling
                    // for this texture before loading it.
                    let build_mipmaps = passes.iter().any(|pass| {
                        pass.inputs()
                            .iter()
                            .filter_map(|opt| opt.as_ref())
                            .any(|ci| {
                                ci.name == input.name
                                    && ci._type == input._type
                                    && ci.filter == FilterMode::Mipmap
                            })
                    });

                    let external_input_id = match input._type {
                        InputType::Texture => {
                            let dir = assets_dir.join("textures");
                            let file = match input.name.as_str() {
                                "Abstract 1" => dir.join("abstract_1.jpg"),
                                "Abstract 2" => dir.join("abstract_2.jpg"),
                                "Abstract 3" => dir.join("abstract_3.jpg"),
                                "Bayer" => dir.join("bayer.png"),
                                "Blue Noise" => dir.join("blue_noise.png"),
                                "Font 1" => dir.join("font_1.png"),
                                "Gray Noise Medium" => dir.join("gray_noise_medium.png"),
                                "Gray Noise Small" => dir.join("gray_noise_small.png"),
                                "Lichen" => dir.join("lichen.jpg"),
                                "London" => dir.join("london.jpg"),
                                "Nyancat" => dir.join("nyancat.png"),
                                "Organic 1" => dir.join("organic_1.jpg"),
                                "Organic 2" => dir.join("organic_2.jpg"),
                                "Organic 3" => dir.join("organic_3.jpg"),
                                "Organic 4" => dir.join("organic_4.jpg"),
                                "Pebbles" => dir.join("pebbles.png"),
                                "RGBA Noise Medium" => dir.join("rgba_noise_medium.png"),
                                "RGBA Noise Small" => dir.join("rgba_noise_small.png"),
                                "Rock Tiles" => dir.join("rock_tiles.jpg"),
                                "Rusty Metal" => dir.join("rusty_metal.jpg"),
                                "Stars" => dir.join("stars.jpg"),
                                "Wood" => dir.join("wood.jpg"),
                                _ => PathBuf::from(input.name.clone()),
                            };
                            load_2d_texture(file, input.vflip, build_mipmaps)
                        }
                        InputType::Cubemap => {
                            let dir = assets_dir.join("cubemaps");
                            let file = match input.name.as_str() {
                                "Forest" => dir.join("forest.png"),
                                "Forest Blurred" => dir.join("forest_blurred.png"),
                                "St. Peter's Basilica" => dir.join("st_peters_basilica.png"),
                                "St. Peter's Basilica Blurred" => {
                                    dir.join("st_peters_basilica_blurred.png")
                                }
                                "Uffizi Gallery" => dir.join("uffizi_gallery.png"),
                                "Uffizi Gallery Blurred" => dir.join("uffizi_gallery_blurred.png"),
                                _ => PathBuf::from(input.name.clone()),
                            };
                            load_cubemap_texture(file, build_mipmaps)
                        }
                        InputType::Volume => {
                            let dir = assets_dir.join("volumes");
                            let file = match input.name.as_str() {
                                "Grey Noise3D" => dir.join("grey_noise_3d.png"),
                                "RGBA Noise3D" => dir.join("rgba_noise_3d.png"),
                                _ => PathBuf::from(input.name.clone()),
                            };
                            load_3d_texture(file, build_mipmaps)
                        }
                        _ => load_2d_texture(PathBuf::default(), false, false),
                    };

                    self.map
                        .insert(key, Texture::new(external_input_id, input._type));
                }
            }
            let name = if pass.name() == "Cube A" {
                "Cubemap A"
            } else {
                pass.name()
            };

            // Register framebuffer outputs as textures using a ping-pong scheme.
            //
            // Each render pass owns two framebuffers:
            //
            // - "<PassName>0": Previous frame output
            // - "<PassName>1": Current frame output
            //
            // The renderer alternates between them every frame to avoid
            // reading from a texture that is simultaneously being written.
            // This enables feedback effects where a pass samples its own
            // result from the previous frame.
            self.map.insert(
                name.to_string() + "0",
                Texture::new(pass.framebuffers()[0].texture(), InputType::Misc),
            );
            self.map.insert(
                name.to_string() + "1",
                Texture::new(pass.framebuffers()[1].texture(), InputType::Misc),
            );
        }
    }

    /// Uploads keyboard state to the ShaderToy-compatible keyboard texture.
    pub fn update_keyboard_texture(&mut self, data: &KeyboardData) {
        let Some(tex) = &self.keyboard_texture else {
            return;
        };

        let (row0, rest) = self.keyboard_state.split_at_mut(KEYBOARD_TEXTURE_WIDTH);
        let (row1, row2) = rest.split_at_mut(KEYBOARD_TEXTURE_WIDTH);

        for i in 0..KEYBOARD_TEXTURE_WIDTH {
            row0[i] = 255 * data.keydown()[i] as u8;
            row1[i] = 255 * data.keypressed()[i] as u8;
            row2[i] = 255 * data.toggled()[i] as u8;
        }

        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, tex.id);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                KEYBOARD_TEXTURE_WIDTH as i32,
                KEYBOARD_TEXTURE_HEIGHT as i32,
                gl::RED,
                gl::UNSIGNED_BYTE,
                self.keyboard_state.as_ptr() as *const _,
            );
        }

        // keypressed row must be cleared after upload because
        // ShaderToy treats it as a one-frame pulse
        self.clear_keypressed();
    }

    /// Clears one-frame keypress state after GPU upload.
    fn clear_keypressed(&mut self) {
        let keypressed_start = KEYBOARD_TEXTURE_WIDTH;
        let keypressed_end = 2 * KEYBOARD_TEXTURE_WIDTH;
        self.keyboard_state[keypressed_start..keypressed_end].fill(0);
    }
}

/// Returns the directory containing bundled texture assets.
///
/// Uses XDG data directory when available and falls back
/// to the current working directory.
fn assets_dir() -> PathBuf {
    dirs::data_local_dir()
        .map(|mut path| {
            path.push(APP_NAME);
            path.push("assets");
            path
        })
        .unwrap_or_else(|| {
            log::warn!(
                "Could not find $XDG_DATA_HOME or $HOME/.local/share; using current directory."
            );
            std::env::current_dir()
                .expect("Failed to get current working directory")
                .join(APP_NAME)
                .join("assets")
        })
}

/// Loads a cubemap texture from a horizontally stacked image.
///
/// Expected layout:
/// +X | -X | +Y | -Y | +Z | -Z
fn load_cubemap_texture(path: PathBuf, build_mipmaps: bool) -> GLuint {
    const CUBEMAP_NUM_FACES: usize = 6;

    let mut texture_id = 0;

    unsafe {
        gl::GenTextures(1, &mut texture_id);
        gl::BindTexture(gl::TEXTURE_CUBE_MAP, texture_id);
    }

    let define_texture = |target: GLenum, size: Size, data: *const u8| unsafe {
        gl::TexSubImage2D(
            target,
            0,
            0,
            0,
            size.width() as i32,
            size.height() as i32,
            gl::RGB,
            gl::UNSIGNED_BYTE,
            data as *const _,
        )
    };

    // Fallback ensures shader execution continues even if
    // asset loading fails
    let fallback = || {
        unsafe { gl::TexStorage2D(gl::TEXTURE_CUBE_MAP, 1, gl::RGB8, 1, 1) };

        let fallback_data: [u8; 3] = [0, 0, 0];
        for i in 0..CUBEMAP_NUM_FACES {
            define_texture(
                gl::TEXTURE_CUBE_MAP_POSITIVE_X + i as u32,
                Size::new(1, 1),
                fallback_data.as_ptr(),
            );
        }
    };

    if let Ok(img) = image::open(path.as_path()) {
        let img = img.to_rgb8();
        let (width, height) = img.dimensions();
        let face_size = Size::new(width / 6, height);

        if face_size.width() > 0 {
            let num_mipmap_levels = if build_mipmaps {
                (face_size.width().max(face_size.height()) as f32)
                    .log2()
                    .floor() as i32
                    + 1
            } else {
                1
            };
            unsafe {
                gl::TexStorage2D(
                    gl::TEXTURE_CUBE_MAP,
                    num_mipmap_levels,
                    gl::RGB8,
                    face_size.width() as i32,
                    face_size.height() as i32,
                )
            };

            for i in 0..CUBEMAP_NUM_FACES {
                let x_offset = i as u32 * face_size.width();
                let face = img.view(x_offset, 0, face_size.width(), face_size.height());

                define_texture(
                    gl::TEXTURE_CUBE_MAP_POSITIVE_X + i as u32,
                    face_size,
                    face.to_image().as_ptr(),
                );
            }
        } else {
            fallback();
        }
    } else {
        fallback();
    }

    if build_mipmaps {
        unsafe { gl::GenerateMipmap(gl::TEXTURE_CUBE_MAP) };
    }

    texture_id
}

/// Loads a 2D texture with optional vertical flip and mipmaps.
///
/// Automatically selects internal format based on image channels.
fn load_2d_texture(path: PathBuf, vflip: bool, build_mipmaps: bool) -> GLuint {
    let mut texture_id = 0;

    unsafe {
        gl::GenTextures(1, &mut texture_id);
        gl::BindTexture(gl::TEXTURE_2D, texture_id);
    }

    let define_texture = |internal_format: GLenum, format: GLenum, size: Size, data: *const u8| {
        let num_mipmap_levels = if build_mipmaps {
            (size.width().max(size.height()) as f32).log2().floor() as i32 + 1
        } else {
            1
        };
        unsafe {
            gl::TexStorage2D(
                gl::TEXTURE_2D,
                num_mipmap_levels,
                internal_format,
                size.width() as i32,
                size.height() as i32,
            );
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                size.width() as i32,
                size.height() as i32,
                format,
                gl::UNSIGNED_BYTE,
                data as *const _,
            );
        }
    };

    match image::open(path.as_path()) {
        Ok(img) => {
            let img = if vflip { img.flipv() } else { img };
            let size = {
                let (width, height) = img.dimensions();
                Size::new(width, height)
            };

            if img.color() == ColorType::L8 {
                define_texture(gl::R8, gl::RED, size, img.to_luma8().as_ptr());
            } else if img.color().has_alpha() {
                define_texture(gl::RGBA8, gl::RGBA, size, img.to_rgba8().as_ptr());
            } else {
                define_texture(gl::RGB8, gl::RGB, size, img.to_rgb8().as_ptr());
            }
        }
        Err(_) => {
            let fallback_data: [u8; 3] = [0, 0, 0];
            define_texture(gl::RGB8, gl::RGB, Size::new(1, 1), fallback_data.as_ptr());
        }
    }

    if build_mipmaps {
        unsafe { gl::GenerateMipmap(gl::TEXTURE_2D) };
    }

    texture_id
}

/// Loads a 3D texture encoded as a horizontal strip of square slices.
///
/// Image layout:
/// [slice0][slice1][slice2]...
fn load_3d_texture(path: PathBuf, build_mipmaps: bool) -> GLuint {
    let mut texture_id = 0;

    unsafe {
        gl::GenTextures(1, &mut texture_id);
        gl::BindTexture(gl::TEXTURE_3D, texture_id);
    }

    let fallback = || {
        let fallback_data: [u8; 4] = [0, 0, 0, 0];
        unsafe {
            gl::TexStorage3D(gl::TEXTURE_3D, 1, gl::RGBA8, 1, 1, 1);
            gl::TexSubImage3D(
                gl::TEXTURE_3D,
                0,
                0,
                0,
                0,
                1,
                1,
                1,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                fallback_data.as_ptr() as *const _,
            );
        }
    };

    if let Ok(img) = image::open(path.as_path()) {
        let img = img.flipv().to_rgba8();
        let (width, height) = img.dimensions();

        if height > 0 && width % height == 0 {
            let slice_size = height;
            let depth = width / slice_size;
            let num_mipmap_levels = if build_mipmaps {
                (slice_size as f32).log2().floor() as i32 + 1
            } else {
                1
            };
            unsafe {
                gl::TexStorage3D(
                    gl::TEXTURE_3D,
                    num_mipmap_levels,
                    gl::RGBA8,
                    slice_size as i32,
                    slice_size as i32,
                    depth as i32,
                )
            };

            for z in 0..depth {
                let x_offset = z * slice_size;
                let slice = img.view(x_offset, 0, slice_size, slice_size);
                let slice_img = slice.to_image();

                unsafe {
                    gl::TexSubImage3D(
                        gl::TEXTURE_3D,
                        0,
                        0,
                        0,
                        z as i32,
                        slice_size as i32,
                        slice_size as i32,
                        1,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        slice_img.as_ptr() as *const _,
                    )
                };
            }
        } else {
            fallback();
        }
    } else {
        fallback();
    }

    if build_mipmaps {
        unsafe { gl::GenerateMipmap(gl::TEXTURE_3D) };
    }

    texture_id
}

/// Creates the ShaderToy keyboard input texture.
///
/// Uses single-channel R8 format and nearest sampling.
fn create_keyboard_texture() -> GLuint {
    let mut texture_id = 0;

    unsafe {
        gl::GenTextures(1, &mut texture_id);
        gl::BindTexture(gl::TEXTURE_2D, texture_id);

        gl::TexStorage2D(
            gl::TEXTURE_2D,
            1,
            gl::R8,
            KEYBOARD_TEXTURE_WIDTH as i32,
            KEYBOARD_TEXTURE_HEIGHT as i32,
        );

        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
    }

    texture_id
}
