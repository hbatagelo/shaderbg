// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

mod app;
mod cli;
mod drm;
mod frame_controller;
mod geometry;
mod ipc_client;
mod ipc_server;
mod keyboard_controller;
mod mouse_controller;
mod preset;
mod renderer;
mod screen_controller;
mod shadertoy;

pub const APP_NAME: &str = "shaderbg";
pub const APP_ABOUT: &str = "Shader wallpaper utility for Wayland";
pub const APP_AUTHOR: &str = "Harlen Batagelo, hbatagelo@gmail.com";
pub const APP_ID: &str = "com.github.hbatagelo.shaderbg";
pub const APP_SEMVER: &str = "1.4.0";
pub const GL_VERSION: (i32, i32) = (4, 2);

fn main() -> gtk::glib::ExitCode {
    if let Err(err) = app::init_logging() {
        eprintln!("Failed to initialize logging: {err}");
    }

    let invocation = match cli::parse_args() {
        Ok(inv) => inv,
        Err(cli::CliError::InvalidInput(warn)) => {
            log::warn!("{warn}. Using default settings.");
            cli::Invocation::App(Box::default())
        }
        Err(err) => {
            eprintln!("{err}");
            return gtk::glib::ExitCode::FAILURE;
        }
    };

    match invocation {
        cli::Invocation::App(config) => app::run(*config),
        cli::Invocation::IpcClient(config) => ipc_client::run(&config),
    }
}
