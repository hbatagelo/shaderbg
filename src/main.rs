// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

mod app;
mod cli;
mod drm;
mod frame_controller;
mod geometry;
mod mouse_controller;
mod preset;
mod renderer;
mod screen_controller;
mod shadertoy;

pub const APP_NAME: &str = "shaderbg";
pub const APP_ABOUT: &str = "Shader wallpaper utility for Wayland";
pub const APP_AUTHOR: &str = "Harlen Batagelo, hbatagelo@gmail.com";
pub const APP_ID: &str = "com.github.hbatagelo.shaderbg";
pub const APP_SEMVER: &str = "1.1.0";
pub const GL_VERSION: (i32, i32) = (4, 2);

fn main() -> gtk::glib::ExitCode {
    if let Err(err) = app::init_logging() {
        eprintln!("Failed to initialize logging: {err}");
    }

    let (preset, preset_file, show_overlay) = match cli::parse_args() {
        Ok(v) => v,
        Err(cli::CliError::InvalidInput(err)) => {
            log::warn!("{err}. Using default settings.");
            (preset::Preset::with_serde_defaults(), None, true)
        }
        Err(err) => {
            eprintln!("{err}");
            return gtk::glib::ExitCode::FAILURE;
        }
    };

    app::run(preset, preset_file, show_overlay)
}
