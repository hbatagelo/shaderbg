// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use function_name::named;
use gtk::{gdk, glib, prelude::*};
use owo_colors::OwoColorize;

use crate::{app::*, geometry::*, preset::*, *};

#[derive(Default)]
pub struct ScreenController {
    selected_monitors: Vec<gdk::Monitor>,
    screen_bounds: Option<Rectangle>,
    screen_bounds_policy: ScreenBoundsPolicy,
}

impl ScreenController {
    pub fn new(app: &gtk::Application) -> Self {
        let app_data = get_data!(app, AppData, as_mut());

        let all_monitors = ScreenController::all_monitors();
        ScreenController::connect_geometry_notify(app, &all_monitors);

        let selected_monitors = all_monitors
            .iter()
            .filter(|monitor| {
                monitor
                    .connector()
                    .map(|connector| {
                        app_data.preset.monitor_selection.contains(&"*".to_string())
                            || app_data
                                .preset
                                .monitor_selection
                                .contains(&connector.to_string())
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>();

        let screen_bounds_policy = app_data.preset.screen_bounds_policy;
        let screen_bounds = match screen_bounds_policy {
            ScreenBoundsPolicy::AllMonitors => Some(Self::union_geometry(&all_monitors)),
            ScreenBoundsPolicy::SelectedMonitors => Some(Self::union_geometry(&selected_monitors)),
            ScreenBoundsPolicy::Cloned => None,
        };

        log::debug!("Screen bounds: {:?}", screen_bounds);

        Self {
            selected_monitors,
            screen_bounds,
            screen_bounds_policy,
        }
    }

    pub fn selected_monitors(&self) -> &Vec<gdk::Monitor> {
        &self.selected_monitors
    }

    pub fn screen_bounds(&self) -> Option<Rectangle> {
        self.screen_bounds
    }

    pub fn bounds_and_gl_offset_of(&self, monitor: &gdk::Monitor) -> (Rectangle, Offset) {
        let screen_bounds = match self.screen_bounds_policy {
            ScreenBoundsPolicy::Cloned => Rectangle::from(monitor.geometry()),
            _ => self.screen_bounds.unwrap(),
        };
        let monitor_bounds =
            Rectangle::from(monitor.geometry()) - Offset::from(screen_bounds.top_left());
        let monitor_offset = Offset::new(
            monitor_bounds.left(),
            screen_bounds.height() - (monitor_bounds.top() + monitor_bounds.height()),
        );

        (monitor_bounds, monitor_offset)
    }

    pub fn all_monitors() -> Vec<gdk::Monitor> {
        gdk::Display::default()
            .map(|display| {
                display
                    .monitors()
                    .into_iter()
                    .filter_map(|res| res.ok()?.downcast::<gdk::Monitor>().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn connect_geometry_notify(app: &gtk::Application, monitors: &[gdk::Monitor]) {
        struct GeometryNotifyConnected;

        monitors
            .iter()
            .filter(|monitor| !has_data!(monitor, GeometryNotifyConnected))
            .for_each(|monitor| {
                set_data!(monitor, GeometryNotifyConnected {});
                monitor.connect_geometry_notify(glib::clone!(
                    #[weak]
                    app,
                    move |_| {
                        on_geometry_notify(app);
                    }
                ));
            });
    }

    fn union_geometry(monitors: &[gdk::Monitor]) -> Rectangle {
        monitors
            .iter()
            .map(|monitor| Rectangle::from(monitor.geometry()))
            .reduce(|acc, rect| acc.union(&rect))
            .unwrap_or(Rectangle::default())
    }
}

#[named]
fn on_geometry_notify(app: gtk::Application) {
    log::debug!("{}", function_name!().white().bold());

    let app_data = get_data!(app, AppData, as_mut());
    app_data.screen_controller = ScreenController::new(&app);

    on_monitor_changed(app);
}
