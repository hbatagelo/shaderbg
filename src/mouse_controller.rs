// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use gtk::{glib, prelude::*};
use owo_colors::OwoColorize;

use crate::{app::*, geometry::*, *};

#[derive(Debug)]
pub struct MouseController {
    app: gtk::Application,
    current_position: Point,
    last_press_position: Point,
    last_release_position: Point,
    pressed: bool,
    frames_since_pressed: usize,
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

    pub fn setup_area(&self, area: &gtk::GLArea) {
        let motion_controller = gtk::EventControllerMotion::new();
        motion_controller.connect_motion(glib::clone!(
            #[weak(rename_to = app)]
            self.app,
            #[weak]
            area,
            move |_, x, y| {
                let (x, y) = (x.round() as i32, y.round() as i32);
                let flipped_y = area.height() - y;

                let area_data = get_data!(area, AreaData, as_mut());
                let app_data = get_data!(app, AppData, as_mut());

                let mouse = &mut app_data.mouse_controller;
                mouse.current_position = Point::new(
                    x + area_data.gl_offset.dx(),
                    flipped_y + area_data.gl_offset.dy(),
                );

                log::trace!("{} {:?}", "motion".white().bold(), mouse);
            }
        ));
        area.add_controller(motion_controller);

        let click_controller = gtk::GestureClick::new();
        click_controller.set_button(1);
        click_controller.connect_pressed(glib::clone!(
            #[weak(rename_to = app)]
            self.app,
            #[weak]
            area,
            move |_, _, x, y| {
                let (x, y) = (x.round() as i32, y.round() as i32);
                let flipped_y = area.height() - y;

                let area_data = get_data!(area, AreaData, as_mut());
                let app_data = get_data!(app, AppData, as_mut());

                let mouse = &mut app_data.mouse_controller;
                mouse.last_press_position = Point::new(
                    x + area_data.gl_offset.dx(),
                    flipped_y + area_data.gl_offset.dy(),
                );
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
        area.add_controller(click_controller);
    }

    pub fn i_mouse_data(&mut self) -> [i32; 4] {
        let MouseController {
            app,
            current_position: current,
            last_press_position: press,
            last_release_position: release,
            pressed,
            frames_since_pressed,
        } = &mut *self;

        let app_data = get_data!(app, AppData, as_mut());

        match pressed {
            true => {
                if *frames_since_pressed >= app_data.screen_controller.selected_monitors().len() {
                    [current.x(), current.y(), press.x(), -press.y()]
                } else {
                    *frames_since_pressed += 1;
                    [current.x(), current.y(), press.x(), press.y()]
                }
            }
            false => [release.x(), release.y(), -press.x(), -press.y()],
        }
    }
}
