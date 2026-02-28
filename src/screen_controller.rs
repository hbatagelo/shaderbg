// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! Monitor discovery and virtual screen layout management.
//!
//! Determines which monitors participate in rendering and computes the
//! virtual desktop geometry used by the renderer.
//!
//! Converts GDK monitor coordinates into OpenGL coordinate space and
//! reacts to runtime monitor configuration changes (hotplug, resolution,
//! or layout updates).

use function_name::named;
use gtk::{gdk, glib, prelude::*};
use owo_colors::OwoColorize;

use crate::{app::*, geometry::*, preset::*, *};

/// Manages monitor selection and virtual screen layout.
///
/// The controller determines which monitors participate in rendering,
/// how their geometries are combined, and how monitor coordinates map into OpenGL space.
///
/// Depending on `ScreenBoundsPolicy`, monitors may share a single
/// virtual screen or operate independently (clone mode).
#[derive(Default)]
pub struct ScreenController {
    /// Monitors currently selected for rendering.
    selected_monitors: Vec<gdk::Monitor>,

    /// Virtual screen bounds covering all active monitors.
    ///
    /// `None` indicates cloned mode, where each monitor is treated
    /// as an independent screen with its own origin.
    ///
    /// Coordinates use the GDK convention: origin = top-left, +X -> right, +Y -> down.
    screen_bounds: Option<Rectangle>,

    /// Policy used to compute `screen_bounds`.
    screen_bounds_policy: ScreenBoundsPolicy,
}

impl ScreenController {
    /// Builds a controller using the application's active preset.
    ///
    /// Also installs geometry change listeners so monitor layout
    /// updates automatically when displays are reconfigured.
    pub fn new(app: &gtk::Application) -> Self {
        let app_data = get_data!(app, AppData, as_mut());

        let all_monitors = ScreenController::all_monitors();
        ScreenController::connect_geometry_notify(app, &all_monitors);

        // Determine whether preset selects all monitors
        let select_all = app_data
            .cli_config
            .preset
            .monitor_selection
            .iter()
            .any(|s| s == "*");

        // Select monitors based on connector names
        let selected_monitors = all_monitors
            .iter()
            .filter(|monitor| {
                monitor
                    .connector()
                    .map(|connector| {
                        select_all
                            || app_data
                                .cli_config
                                .preset
                                .monitor_selection
                                .contains(&connector.to_string())
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>();

        let screen_bounds_policy = app_data.cli_config.preset.screen_bounds_policy;
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

    /// Returns the monitors participating in rendering.
    pub fn selected_monitors(&self) -> &[gdk::Monitor] {
        &self.selected_monitors
    }

    /// Returns the virtual screen bounds.
    ///
    /// Returns `None` in cloned mode.
    pub fn screen_bounds(&self) -> Option<Rectangle> {
        self.screen_bounds
    }

    /// Computes monitor-local bounds and OpenGL offset.
    ///
    /// Returns a tuple containing the monitor rectangle relative to the virtual screen and
    /// the OpenGL-space offset (origin at bottom-left).
    pub fn bounds_and_gl_offset_of(&self, monitor: &gdk::Monitor) -> (Rectangle, Offset) {
        let screen_bounds = match self.screen_bounds_policy {
            ScreenBoundsPolicy::Cloned => Rectangle::from(monitor.geometry()),
            _ => self.screen_bounds.unwrap(),
        };
        // Monitor rectangle relative to virtual screen origin
        let monitor_bounds =
            Rectangle::from(monitor.geometry()) - Offset::from(screen_bounds.top_left());
        // Convert Y axis from GDK space to OpenGL space
        let monitor_offset = Offset::new(
            monitor_bounds.left(),
            screen_bounds.height() - (monitor_bounds.top() + monitor_bounds.height()),
        );

        (monitor_bounds, monitor_offset)
    }

    /// Returns all monitors available on the default display.
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

    /// Ensures geometry change notifications are connected exactly once.
    ///
    /// Duplicate signal connections are preventd when the controller is recreated.
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

    /// Computes the union rectangle enclosing all provided monitors.
    ///
    /// Returns an empty rectangle if no monitors are supplied.
    fn union_geometry(monitors: &[gdk::Monitor]) -> Rectangle {
        monitors
            .iter()
            .map(|monitor| Rectangle::from(monitor.geometry()))
            .reduce(|acc, rect| acc.union(&rect))
            .unwrap_or(Rectangle::default())
    }
}

#[named]
/// Handles monitor geometry changes.
///
/// Rebuilds the `ScreenController` and notifies the application
/// that monitor configuration has changed.
fn on_geometry_notify(app: gtk::Application) {
    log::debug!("{}", function_name!().white().bold());

    let app_data = get_data!(app, AppData, as_mut());
    app_data.screen_controller = ScreenController::new(&app);

    on_monitor_changed(app);
}
