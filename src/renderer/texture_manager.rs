// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use gl::types::*;
use image::*;
use std::{collections::HashMap, path::PathBuf};

use crate::{geometry::Size, preset::*, APP_NAME};

use super::render_pass::RenderPass;

struct Texture {
    id: GLuint,
    input_type: InputType,
    frame_number: u32,
}

impl Texture {
    fn new(id: u32, input_type: InputType) -> Self {
        Self {
            id,
            input_type,
            frame_number: u32::MAX,
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        let managed = self.input_type != InputType::Misc;
        if managed {
            unsafe { gl::DeleteTextures(1, &self.id) };
        }
    }
}

pub struct TextureManager {
    map: HashMap<String, Texture>,
}

impl TextureManager {
    pub fn new() -> Self {
        let map = HashMap::new();

        Self { map }
    }

    pub fn id(&self, name: &str) -> Option<GLuint> {
        if let Some(texture) = self.map.get(name) {
            return Some(texture.id);
        }
        None
    }

    pub fn update_frame_number(&mut self, name: &str, frame_number: u32) -> Option<u32> {
        if let Some(texture) = self.map.get_mut(name) {
            let current_frame_number = texture.frame_number;
            texture.frame_number = frame_number;
            return Some(current_frame_number);
        }
        None
    }

    pub fn load(&mut self, passes: &Vec<RenderPass>) {
        for pass in passes {
            for input in pass.inputs().iter().filter_map(|opt| opt.as_ref()) {
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
                    && !self.map.contains_key(&key)
                {
                    let assets_dir = assets_dir();
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
}

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
        );
    };

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
                );
            }

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

fn load_2d_texture(path: PathBuf, vflip: bool, build_mipmaps: bool) -> GLuint {
    let mut texture_id = 0;

    unsafe {
        gl::GenTextures(1, &mut texture_id);
        gl::BindTexture(gl::TEXTURE_2D, texture_id);
    }

    let define_texture = |internal_format: GLenum, format: GLenum, size: Size, data: *const u8| unsafe {
        let num_mipmap_levels = if build_mipmaps {
            (size.width().max(size.height()) as f32).log2().floor() as i32 + 1
        } else {
            1
        };
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

fn load_3d_texture(path: PathBuf, build_mipmaps: bool) -> GLuint {
    let mut texture_id = 0;

    unsafe {
        gl::GenTextures(1, &mut texture_id);
        gl::BindTexture(gl::TEXTURE_3D, texture_id);
    }

    let fallback = || unsafe {
        let fallback_data: [u8; 4] = [0, 0, 0, 0];
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
                );
            }

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
                    );
                }
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
