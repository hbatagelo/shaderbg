// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mouse input controller and ShaderToy compatibility layer.
//!
//! Captures GTK pointer motion and button events and converts them into
//! ShaderToy-compatible `iMouse` uniform data expressed in global
//! OpenGL screen coordinates.

use gtk::{glib, prelude::*};
use owo_colors::OwoColorize;

use crate::{app::*, geometry::*, *};

/// Maintains global mouse interaction state used by shaders.
///
/// The controller converts GTK input events into ShaderToy-compatible
/// `iMouse` uniform data expressed in OpenGL screen coordinates.
#[derive(Debug)]
pub struct MouseController {
    /// Application reference used to access shared state.
    app: gtk::Application,

    /// Current cursor position in OpenGL screen space.
    current_position: Point,

    /// Cursor position where the most recent press began.
    last_press_position: Point,

    /// Cursor position recorded at the most recent release.
    last_release_position: Point,

    /// Indicates whether the primary mouse button is currently held.
    pressed: bool,

    /// Frames elapsed since the last press event.
    ///
    /// Used to guarantee that multi-monitor rendering pipelines observe
    /// at least one frame where the click is reported as "just pressed".
    frames_since_pressed: usize,
}

/// Raw mouse data formatted for ShaderToy's `iMouse` uniform.
#[derive(Clone, Copy, Debug)]
pub struct MouseData {
    raw: [i32; 4],
}

impl MouseData {
    /// Returns data suitable for uploading directly to the `iMouse` uniform.
    pub fn as_shadertoy_uniform(&self) -> &[i32; 4] {
        &self.raw
    }
}

impl MouseController {
    pub fn new(app: gtk::Application) -> Self {
        Self {
            app,
            current_position: Point::default(),
            last_press_position: Point::default(),
            last_release_position: Point::default(),
            pressed: false,
            frames_since_pressed: 0,
        }
    }

    /// Installs mouse motion and click handlers on a GTK widget.
    ///
    /// Events are translated from widget-local coordinates into
    /// global screen coordinates using `gl_offset`, which specifies
    /// the OpenGL-space origin of the widget's monitor.
    pub fn setup_widget(&self, widget: &impl gtk::prelude::IsA<gtk::Widget>, gl_offset: Offset) {
        let widget = widget.as_ref();

        let motion_controller = gtk::EventControllerMotion::new();
        motion_controller.connect_motion(glib::clone!(
            #[weak(rename_to = app)]
            self.app,
            #[weak]
            widget,
            move |_, x, y| {
                let (x, y) = (x.round() as i32, y.round() as i32);
                let flipped_y = widget.height() - y;

                let app_data = get_data!(app, AppData, as_mut());
                let mouse = &mut app_data.mouse_controller;
                mouse.current_position = Point::new(x + gl_offset.dx(), flipped_y + gl_offset.dy());

                log::trace!("{} {:?}", "motion".white().bold(), mouse);
            }
        ));
        widget.add_controller(motion_controller);

        let click_controller = gtk::GestureClick::new();
        click_controller.set_button(1);
        click_controller.connect_pressed(glib::clone!(
            #[weak(rename_to = app)]
            self.app,
            #[weak]
            widget,
            move |_, _, x, y| {
                let (x, y) = (x.round() as i32, y.round() as i32);
                let flipped_y = widget.height() - y;

                let app_data = get_data!(app, AppData, as_mut());
                let mouse = &mut app_data.mouse_controller;
                mouse.last_press_position =
                    Point::new(x + gl_offset.dx(), flipped_y + gl_offset.dy());
                mouse.pressed = true;
                mouse.frames_since_pressed = 0;

                log::trace!("{} {:?}", "pressed".white().bold(), mouse);
            }
        ));
        click_controller.connect_released(glib::clone!(
            #[weak(rename_to = app)]
            self.app,
            move |_, _, _, _| {
                let app_data = get_data!(app, AppData, as_mut());
                let mouse = &mut app_data.mouse_controller;
                mouse.last_release_position = mouse.current_position;
                mouse.pressed = false;

                log::trace!("{} {:?}", "released".white().bold(), mouse);
            }
        ));
        widget.add_controller(click_controller);
    }

    /// Produces ShaderToy-compatible mouse uniform data.
    ///
    /// Must be called once per rendered frame.
    ///
    /// Multi-monitor rendering may produce multiple frames per logical click.
    /// Therefore a small frame window is maintained so every monitor observes
    /// the press transition.
    pub fn snapshot(&mut self) -> MouseData {
        let MouseController {
            app,
            current_position: current,
            last_press_position: press,
            last_release_position: release,
            pressed,
            frames_since_pressed,
        } = &mut *self;

        let app_data = get_data!(app, AppData, as_mut());

        let raw = if *pressed {
            if *frames_since_pressed >= app_data.screen_controller.selected_monitors().len() {
                // Button held down
                [current.x(), current.y(), press.x(), -press.y()]
            } else {
                // Initial press frame
                *frames_since_pressed += 1;
                [current.x(), current.y(), press.x(), press.y()]
            }
        } else {
            // Button released
            [release.x(), release.y(), -press.x(), -press.y()]
        };

        MouseData { raw }
    }
}
