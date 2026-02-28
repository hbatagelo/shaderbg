// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK application runtime and window management.
//!
//! Handles application initialization, OpenGL setup,
//! window lifecycle, frame scheduling, monitor changes,
//! rendering orchestration, and input integration.

use function_name::named;
use gtk::{
    cairo, gdk,
    gio::{self, prelude::*},
    glib,
    prelude::*,
};
use gtk4_layer_shell::*;
use owo_colors::OwoColorize;
use std::{path::*, sync::Once, time::Duration};

use crate::{
    cli::CliConfig, drm::*, frame_controller::*, geometry::*, keyboard_controller::*,
    mouse_controller::*, preset::*, renderer::*, screen_controller::*, *,
};

/// Interval for checking monitor state during standby.
const STANDBY_CHECK_INTERVAL: Duration = Duration::from_millis(250);

/// Ensures that GL function pointers are only loaded once.
static LOAD_GL: Once = Once::new();

/// Global application state stored on the [`gtk::Application`].
pub struct AppData {
    /// Active rendering surfaces.
    /// One [`gtk::GLArea`] is created per monitor when using Layer Shell.
    pub areas: Vec<gtk::GLArea>,

    /// Configuration loaded from CLI arguments.
    pub cli_config: CliConfig,

    /// File change monitor.
    pub preset_monitor: Option<gio::FileMonitor>,

    /// Timer driving frame updates when rendering is
    /// throttled  or during crossfade animation.
    /// At most one animation source is active at a time.
    pub animation_timer: Option<glib::SourceId>,

    /// Controls logical frame production, timing statistics,
    ///  and crossfade animation.
    pub frame_controller: FrameController,

    /// Mouse controller.
    pub mouse_controller: MouseController,

    /// Keyboard controller.
    pub keyboard_controller: KeyboardController,

    /// Screen controller.
    pub screen_controller: ScreenController,

    /// Indicates whether the compositor supports the
    /// `zwlr_layer_shell_v1` protocol.
    pub layer_shell_supported: bool,
}

/// Per-window rendering state attached to each `GLArea`.
/// Stores monitor-specific geometry and renderer instance.
#[derive(Default)]
pub struct AreaData {
    /// Renderer.
    pub renderer: Option<Renderer>,

    /// Name of the monitor connector associated to this area.
    pub connector: String,

    /// Geometry of the GL area, in screen space.
    /// The origin of the screen space is at the top-left corner
    /// with x-axis pointing right and y-axis pointing down.
    pub bounds: Rectangle,

    /// Monitor rectangle in global screen space
    /// (origin at top-left, Y increasing downward).
    pub gl_offset: Offset,

    /// Optional widget for displaying shader info,
    /// shown when the area is first rendered.
    pub info_overlay: Option<gtk::Widget>,
}

/// Snapshot of input state supplied to the renderer for one frame.
pub struct InputData {
    pub mouse: MouseData,
    pub keyboard: Option<KeyboardData>,
}

pub fn init_logging() -> Result<(), log::SetLoggerError> {
    let level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    };

    simple_logger::SimpleLogger::new().with_level(level).init()
}

/// Initializes and runs the GTK application.
///
/// Creates global `AppData`, installs signal handlers,
/// and starts the GTK main loop.
pub fn run(cli_config: CliConfig) -> glib::ExitCode {
    let app = gtk::Application::builder().application_id(APP_ID).build();

    set_data!(
        app,
        AppData {
            areas: Vec::default(),
            cli_config,
            preset_monitor: None,
            animation_timer: None,
            frame_controller: FrameController::default(),
            mouse_controller: MouseController::new(app.clone()),
            keyboard_controller: KeyboardController::new(app.clone()),
            screen_controller: ScreenController::default(),
            layer_shell_supported: false,
        }
    );

    let app_data = get_data!(app, AppData, as_ref());

    if let Some(path) = &app_data.cli_config.preset_path {
        setup_preset_monitor(&app, path, on_preset_change);
    }

    app.connect_activate(activate);
    app.run_with_args(&[""])
}

/// Reloads preset from the given file and applies it if it has changed.
fn on_preset_change(app: &gtk::Application, preset_path: &Path) {
    match Preset::from_toml_file(preset_path) {
        Ok(new_preset) => {
            let app_data = get_data!(app, AppData, as_mut());

            if new_preset != app_data.cli_config.preset {
                log::info!("Applying updated preset");
                app_data.cli_config.preset = new_preset;
                on_monitor_changed(app.clone());
            } else {
                log::info!("Preset unchanged after reload");
            }
        }
        Err(err) => log::error!("Error reloading preset: {err}"),
    }
}

/// GTK activation handler.
///
/// Detects compositor capabilities, installs monitor listeners,
/// and triggers initial window creation.
#[named]
fn activate(app: &gtk::Application) {
    log::debug!("{}", function_name!().white().bold());

    log::info!(
        "GTK Layer Shell version: {}.{}.{}",
        gtk4_layer_shell::major_version(),
        gtk4_layer_shell::minor_version(),
        gtk4_layer_shell::micro_version()
    );

    let app_data = get_data!(app, AppData, as_mut());
    app_data.layer_shell_supported = gtk4_layer_shell::is_supported();

    if app_data.layer_shell_supported {
        log::info!(
            "Layer Shell Protocol (zwlr_layer_shell_v1) version: {}",
            gtk4_layer_shell::protocol_version()
        );
    }

    if let Some(display) = gdk::Display::default() {
        // Sets up the monitor change handler for the display
        let monitors = display.monitors();
        monitors.connect_items_changed(glib::clone!(
            #[weak]
            app,
            move |_, _, _, _| {
                on_monitor_changed(app.clone());
            }
        ));
        on_monitor_changed(app.clone());
    } else {
        log::error!("No default GdkDisplay");
    }
}

/// Callback for when the monitor configuration changes.
///
/// This function orchestrates the recreation of windows to match the new
/// monitor setup or enters a standby mode if no valid monitors are found.
///
/// Steps after a monitor configuration change:
/// 1. Destroy existing windows.
/// 2. Verify DRM outputs and GDK monitors.
/// 3. Enter standby mode if no usable displays exist.
/// 4. Otherwise recreate rendering windows.
#[named]
pub fn on_monitor_changed(app: gtk::Application) {
    log::debug!("{}", function_name!().white().bold());

    // Destroy existing windows before creating new ones
    app.windows().iter().for_each(|window| window.destroy());

    let has_connected_output = has_connected_drm_output().unwrap_or_else(|err| {
        log::warn!("Could not query DRM: {err}");
        true // Fall back to GDK monitors check
    });

    let monitors = ScreenController::all_monitors();

    if !has_connected_output
        || monitors.is_empty()
        || monitors
            .iter()
            .any(|monitor| !monitor.is_valid() || monitor.connector().is_none())
    {
        start_standby_mode(&app);
    } else {
        create_windows(&app);
    }
}

/// Enters standby mode when no usable monitors are available.
///
/// A hidden window keeps the GTK application alive while
/// periodically rechecking monitor availability.
fn start_standby_mode(app: &gtk::Application) {
    if app.windows().is_empty() {
        let standby_window = gtk::ApplicationWindow::builder()
            .application(app)
            .name(APP_NAME)
            .title(APP_NAME)
            .build();

        standby_window.set_default_size(1, 1);
        standby_window.set_decorated(false);
        standby_window.set_visible(false);
        standby_window.set_opacity(0.0);
    }

    // Schedules a recheck of monitor status
    let app_clone = app.clone();
    glib::timeout_add_local_once(STANDBY_CHECK_INTERVAL, move || {
        log::trace!("Standby check");
        on_monitor_changed(app_clone);
    });
}

/// Recreates rendering windows according to the current
/// monitor configuration and compositor capabilities.
pub fn create_windows(app: &gtk::Application) {
    let app_data = get_data!(app, AppData, as_mut());
    let old_areas = std::mem::take(&mut app_data.areas);

    app_data.screen_controller = ScreenController::new(app);

    let monitor_count = app_data.screen_controller.selected_monitors().len();
    app_data.frame_controller = FrameController::new(&app_data.cli_config.preset, monitor_count);

    if app_data.layer_shell_supported {
        create_layer_windows(app);
    } else {
        create_fallback_window(app);
    }

    drop(old_areas);
    setup_animation_driver(app);
}

/// Creates one background Layer Shell window per selected monitor.
///
/// Each render window is paired with a transparent [`create_input_window`]
/// on [`Layer::Bottom`] that captures mouse and keyboard events without
/// interfering with the composited wallpaper below.
fn create_layer_windows(app: &gtk::Application) {
    let app_data = get_data!(app, AppData, as_mut());

    for monitor in app_data.screen_controller.selected_monitors() {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .name(APP_NAME)
            .title(APP_NAME)
            .build();

        setup_layer_shell(&window);

        // Input is handled by the companion input window; render areas
        // must not consume keyboard events from the compositor.
        let area = setup_area(app, false);

        let connector = monitor
            .connector()
            .map(|connector| connector.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let (bounds, gl_offset) = app_data.screen_controller.bounds_and_gl_offset_of(monitor);

        set_data!(
            area,
            AreaData {
                renderer: None,
                connector,
                bounds,
                gl_offset,
                info_overlay: None,
            }
        );

        if app_data.cli_config.show_overlay {
            let overlay = gtk::Overlay::new();
            overlay.set_child(Some(&area));

            if gl_offset == Offset::default() {
                let name = &app_data.cli_config.preset.name;
                let author = &app_data.cli_config.preset.username;
                let area_data = get_data!(area, AreaData, as_mut());
                area_data.info_overlay = create_info_widget(name, author);
                if let Some(widget) = &area_data.info_overlay {
                    overlay.add_overlay(widget);
                }
            }
            window.set_child(Some(&overlay));
        } else {
            window.set_child(Some(&area));
        }

        window.set_monitor(Some(monitor));
        app_data.areas.push(area);
        window.present();

        // Create the companion transparent input-capture window for this monitor
        create_input_window(app, monitor, gl_offset);
    }
}

/// Creates a single top-level window when Layer Shell is unavailable.
fn create_fallback_window(app: &gtk::Application) {
    log::warn!("Layer Shell protocol not supported. Using top-level window.");

    let app_data = get_data!(app, AppData, as_mut());

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .name(APP_NAME)
        .icon_name(APP_NAME)
        .title(format!("{APP_NAME} {APP_SEMVER}"))
        .default_width(800)
        .default_height(600)
        .width_request(320)
        .height_request(240)
        .build();

    let area = setup_area(app, true);

    set_data!(
        area,
        AreaData {
            renderer: None,
            connector: String::default(),
            bounds: Rectangle::new(
                Point::default(),
                SizeI::new(window.width(), window.height())
            ),
            gl_offset: Offset::default(),
            info_overlay: None,
        }
    );

    if app_data.cli_config.show_overlay {
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&area));

        let name = &app_data.cli_config.preset.name;
        let author = &app_data.cli_config.preset.username;
        let area_data = get_data!(area, AreaData, as_mut());
        area_data.info_overlay = create_info_widget(name, author);
        if let Some(widget) = &area_data.info_overlay {
            overlay.add_overlay(widget);
        }

        window.set_child(Some(&overlay));
    } else {
        window.set_child(Some(&area));
    }
    app_data.areas.push(area);
    window.present();
}

/// Creates a text widget for displaying shader info.
fn create_info_widget(name: &str, author: &str) -> Option<gtk::Widget> {
    const NAME_FONT_SIZE_PT: i32 = 18;
    const AUTHOR_FONT_SIZE_PT: i32 = 14;
    const MARGIN: i32 = 25;

    let has_name = !name.is_empty();
    let has_author = !author.is_empty();

    if !has_name && !has_author {
        return None;
    }

    let container = gtk::Box::new(gtk::Orientation::Vertical, 2);
    container.set_opacity(1.0);

    fn create_text_element(text: &str, font_size: i32, is_bold: bool) -> gtk::Widget {
        let fixed = gtk::Fixed::new();

        fn create_label(text: &str, font_size: i32, is_bold: bool, color: &str) -> gtk::Label {
            let label = gtk::Label::new(None);
            let weight = if is_bold { "bold" } else { "normal" };
            label.set_markup(&format!(
                r#"<span font="{}" font_weight="{}" foreground="{}">{}</span>"#,
                font_size,
                weight,
                color,
                glib::markup_escape_text(text)
            ));
            label
        }

        // Shadow layers
        for i in (1..=3).rev() {
            let shadow = create_label(text, font_size, is_bold, "black");
            shadow.set_opacity(0.3 / i as f64);
            fixed.put(&shadow, i as f64, i as f64);
        }

        // Foreground text
        let foreground = create_label(text, font_size, is_bold, "white");
        fixed.put(&foreground, 0.0, 0.0);

        fixed.upcast()
    }

    if has_name {
        let name_widget = create_text_element(name, NAME_FONT_SIZE_PT, true);
        container.append(&name_widget);
    }

    if has_author {
        let author_widget =
            create_text_element(&format!("by {author}"), AUTHOR_FONT_SIZE_PT, false);
        container.append(&author_widget);
    }

    container.set_halign(gtk::Align::Start);
    container.set_valign(gtk::Align::End);
    container.set_margin_start(MARGIN);
    container.set_margin_bottom(MARGIN);
    container.set_hexpand(false);
    container.set_vexpand(false);

    Some(container.upcast())
}

/// Sets up a fade-out animation for the given widget.
fn setup_fadeout_timer(widget: &gtk::Widget) {
    const AFTER_SECS: u32 = 10;
    const FADE_SECS: u32 = 2;
    const FADE_FPS: u64 = 60;

    let widget_clone = widget.clone();
    glib::timeout_add_seconds_local(AFTER_SECS, move || {
        let start_time = std::time::Instant::now();
        let container_clone = widget_clone.clone();

        glib::timeout_add_local(Duration::from_millis(1000 / FADE_FPS), move || {
            let elapsed = start_time.elapsed().as_secs_f64();

            if elapsed >= FADE_SECS as f64 {
                // Animation complete, hide the widget
                container_clone.set_visible(false);
                return glib::ControlFlow::Break;
            }

            let opacity = 1.0 - (elapsed / FADE_SECS as f64);
            container_clone.set_opacity(opacity);

            glib::ControlFlow::Continue
        });

        glib::ControlFlow::Break
    });
}

/// Creates a GL area and configures its OpenGL settings and signal handlers.
///
/// When `with_input` is `true` (used for the non-layer-shell fallback window)
/// mouse and keyboard controllers are also attached to the area directly.
/// In layer-shell mode pass `false` to make the companion transparent
/// window handle input.
fn setup_area(app: &gtk::Application, with_input: bool) -> gtk::GLArea {
    let area = gtk::GLArea::new();

    area.set_required_version(GL_VERSION.0, GL_VERSION.1);
    area.set_has_depth_buffer(false);
    area.set_has_stencil_buffer(false);
    area.set_auto_render(false);
    area.set_focusable(true);

    if with_input {
        let app_data = get_data!(app, AppData, as_ref());
        app_data
            .mouse_controller
            .setup_widget(&area, Offset::default());
        app_data.keyboard_controller.setup_widget(&area);
    }

    area.connect_realize(on_realize);
    area.connect_resize(on_resize);
    area.connect_render(on_render);

    area
}

/// Applies Layer Shell configuration to a render window.
///
/// Render windows sit on [`Layer::Background`], span the full monitor,
/// claim an exclusive zone so the compositor reserves the entire output,
/// and intentionally opt out of keyboard focus ([`KeyboardMode::None`]).
/// Keyboard input is handled instead by the companion transparent input
/// window created by [`create_input_window`].
fn setup_layer_shell(window: &gtk::ApplicationWindow) {
    window.init_layer_shell();
    window.set_layer(Layer::Background);

    [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom]
        .iter()
        .for_each(|&anchor| window.set_anchor(anchor, true));

    window.set_namespace(Some(APP_NAME));
    window.set_exclusive_zone(-1);
    window.set_keyboard_mode(KeyboardMode::None);
}

/// Applies Layer Shell configuration to a transparent input-capture window.
///
/// Input windows sit on [`Layer::Bottom`] above the render background but
/// below all normal application windows so they receive pointer and
/// keyboard events only when no regular window is focused over the desktop.
///
/// `exclusive_zone(0)` means the window does not push any panel or dock away.
/// [`KeyboardMode::OnDemand`] grants keyboard focus when the surface is clicked.
fn setup_input_layer_shell(window: &gtk::ApplicationWindow) {
    window.init_layer_shell();
    window.set_layer(Layer::Bottom);

    [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom]
        .iter()
        .for_each(|&anchor| window.set_anchor(anchor, true));

    window.set_namespace(Some(APP_NAME));
    window.set_exclusive_zone(0);
    window.set_keyboard_mode(KeyboardMode::OnDemand);
    window.set_decorated(false);

    window.connect_is_active_notify(|w| {
        log::debug!("Window active: {}", w.is_active());
    });
}

/// Creates a transparent [`Layer::Bottom`] window on `monitor` that captures
/// mouse and keyboard events on behalf of the paired render window.
///
/// The window contains a single [`gtk::DrawingArea`] that draws nothing
/// (transparent) so the wallpaper rendered by the [`Layer::Background`] window
/// below is fully visible to the user. Mouse and keyboard controllers are
/// wired to this area so input state in [`AppData`] is updated identically
/// to how it worked when the controllers were on the render [`gtk::GLArea`]
/// itself.
///
/// The window is registered with the GTK application and is therefore
/// destroyed automatically by [`on_monitor_changed`] when the monitor
/// configuration changes.
fn create_input_window(app: &gtk::Application, monitor: &gdk::Monitor, gl_offset: Offset) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .name(APP_NAME)
        .title(format!("{APP_NAME}-input"))
        .build();

    setup_input_layer_shell(&window);
    window.set_monitor(Some(monitor));

    // A DrawingArea that explicitly paints fully transparent.
    // This is intentionally not left empty: if the surface has no rendered
    // content, GTK/GDK clears the Wayland input region and the compositor
    // routes pointer and keyboard events around the window instead of to it.
    // Painting alpha=0 keeps the surface visually invisible while preserving
    // the full-surface input region so events are delivered normally.
    //
    // Note: just setting the window opacity to 0.0 does not work because it
    // also disables the input region.
    let da = gtk::DrawingArea::new();
    da.set_focusable(true);
    da.set_draw_func(|_, cr, _, _| {
        cr.set_operator(cairo::Operator::Source);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = cr.paint();
    });
    window.set_child(Some(&da));

    ensure_transparent_css();
    window.add_css_class("shaderbg-input");

    // Wire input controllers. gl_offset is the same value used by the
    // sibling render GLArea so coordinate spaces match exactly.
    let app_data = get_data!(app, AppData, as_ref());
    app_data.mouse_controller.setup_widget(&da, gl_offset);
    app_data.keyboard_controller.setup_widget(&da);

    log::debug!(
        "Input window created for monitor {:?} gl_offset={:?}",
        monitor.connector(),
        gl_offset,
    );

    window.present();
}

/// Installs a CSS rule that strips the GTK/GSK background from
/// `.shaderbg-input` windows.
///
/// In GTK4 the GSK renderer draws widget backgrounds before the Cairo draw
/// function runs, so the draw function alone cannot suppress the window
/// background. This CSS rule removes it at the GSK level.
///
/// The draw function on the [`gtk::DrawingArea`] is still required separately
/// to keep the Wayland input region intact (see [`create_input_window`]).
fn ensure_transparent_css() {
    static CSS_APPLIED: std::sync::Once = std::sync::Once::new();
    CSS_APPLIED.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            ".shaderbg-input, .shaderbg-input * { background: transparent; box-shadow: none; }",
        );
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
    });
}

/// Configures frame scheduling based on preset timing parameters.
fn setup_animation_driver(app: &gtk::Application) {
    let app_data = get_data!(app, AppData, as_mut());
    if let Some(source_id) = app_data.animation_timer.take() {
        source_id.remove();
    }

    if app_data.cli_config.preset.interval_between_frames.is_zero() {
        // Continuous
        let areas = &app_data.areas;
        if app_data.frame_controller.current_monitor() == 0
            && areas.iter().all(|area| area.is_realized())
        {
            for area in areas {
                area.add_tick_callback(glib::clone!(
                    #[strong]
                    area,
                    move |_, _| {
                        area.queue_render();
                        glib::ControlFlow::Continue
                    }
                ));
            }
        }
    } else if app_data.cli_config.preset.crossfade_overlap_ratio > 0.0 {
        // Continuous during crossfade, throttled otherwise
        cross_fade(app);
    } else {
        // Throttled
        let tick_callback = glib::clone!(
            #[weak]
            app,
            #[upgrade_or_panic]
            move || {
                areas_queue_render(&app);
                glib::ControlFlow::Continue
            }
        );
        let source_id = glib::timeout_add_local(
            app_data.cli_config.preset.interval_between_frames,
            tick_callback,
        );
        app_data.animation_timer = Some(source_id);
    }
}

/// Requests rendering for all active GL areas.
///
/// Rendering is gated to preserve logical frame synchronization:
///
/// - When Layer Shell is unsupported, all areas render immediately.
/// - When Layer Shell is active, rendering is triggered only on the
///   first monitor once all areas are realized.
///
/// This prevents multiple monitors from independently driving frame
/// production, ensuring that a single logical frame is rendered and
/// presented consistently across displays.
fn areas_queue_render(app: &gtk::Application) {
    let app_data = get_data!(app, AppData, as_mut());
    if !app_data.layer_shell_supported
        || (app_data.frame_controller.current_monitor() == 0
            && app_data.areas.iter().all(|area| area.is_realized()))
    {
        for area in &app_data.areas {
            area.queue_render();
        }
    }
}

/// Drives crossfade animation between frames.
///
/// Rendering runs continuously during the transition,
/// then schedules the next cycle after the idle interval.
fn cross_fade(app: &gtk::Application) {
    let app_data = get_data!(app, AppData, as_mut());

    app_data.frame_controller.reset_crossfade();

    let crossfade_duration = app_data.frame_controller.crossfade_duration();
    let idle_duration = app_data.frame_controller.idle_duration();

    log::debug!("Crossfade started for {:#?}...", crossfade_duration);

    let tick_callback = glib::clone!(
        #[weak]
        app,
        #[upgrade_or_panic]
        move || {
            let app_data = get_data!(app, AppData, as_mut());

            areas_queue_render(&app);
            if !app_data.frame_controller.is_crossfade_complete() {
                glib::ControlFlow::Continue
            } else {
                let source_id = glib::timeout_add_local_once(idle_duration, move || {
                    cross_fade(&app);
                });
                app_data.animation_timer = Some(source_id);

                log::debug!("Crossfade ended. Next one starting in {:#?}", idle_duration);

                glib::ControlFlow::Break
            }
        }
    );
    const CROSSFADE_FPS: u64 = 60;
    let source_id =
        glib::timeout_add_local(Duration::from_millis(1000 / CROSSFADE_FPS), tick_callback);
    let app_data = get_data!(app, AppData, as_mut());
    app_data.animation_timer = Some(source_id);
}

/// Initializes OpenGL for a newly realized [`gtk::GLArea`].
///
/// Loads GL function pointers once and logs driver information.
#[named]
fn on_realize(area: &gtk::GLArea) {
    log::debug!("{}", function_name!().white().bold());

    if let Some(err) = area.error() {
        log::error!("{err}");
        let (minor, major) = area.required_version();
        log::error!("OpenGL {minor}.{major} required");
        std::process::exit(1);
    }

    let gl_context = area.context().expect("Failed to get GL context");
    gl_context.make_current();

    LOAD_GL.call_once(|| {
        log_gl_version(&gl_context);
        if let Err(err) = load_gl_functions() {
            log::error!("Failed to load GL functions: {err}");
            std::process::exit(1);
        }
        log_glsl_version();
    });
}

/// Loads OpenGL function pointers via libepoxy.
///
/// Required because GTK does not expose GL symbol loading.
fn load_gl_functions() -> Result<(), Box<dyn std::error::Error>> {
    let library = unsafe {
        libloading::os::unix::Library::new("libepoxy.so.0")
            .map_err(|err| format!("Failed to load libepoxy.so.0: {}", err))?
    };

    epoxy::load_with(|name| {
        unsafe { library.get::<_>(name.as_bytes()) }
            .map(|symbol| *symbol)
            .unwrap_or(std::ptr::null())
    });

    gl::load_with(epoxy::get_proc_addr);

    // Verify GL loaded correctly by testing a basic function
    let version = unsafe { gl::GetString(gl::VERSION) };
    if version.is_null() {
        return Err("GL functions not loaded properly".into());
    }

    Ok(())
}

fn log_gl_version(gl_context: &gdk::GLContext) {
    let (major, minor) = gl_context.version();
    log::debug!("GL version: {major}.{minor}");
}

fn log_glsl_version() {
    let glsl_version = glsl_version().unwrap_or_else(|err| {
        log::warn!("Failed to get GLSL version: {}", err);
        "Unknown".to_string()
    });
    log::debug!("GLSL version: {glsl_version}");
}

fn glsl_version() -> Result<String, &'static str> {
    unsafe {
        let ptr = gl::GetString(gl::SHADING_LANGUAGE_VERSION);
        if ptr.is_null() {
            return Err("Failed to get GLSL version string");
        }

        std::ffi::CStr::from_ptr(ptr as *const i8)
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_| "Invalid UTF-8 in GLSL version string")
    }
}

/// Recreates the renderer when the drawing surface changes size
/// or monitor layout requires reconfiguration.
#[named]
fn on_resize(area: &gtk::GLArea, width: i32, height: i32) {
    log::debug!("{}", function_name!().white().bold());

    let gl_context = area.context().expect("Failed to get GL context");
    gl_context.make_current();

    unsafe { gl::ClearColor(0.0, 0.0, 0.0, 1.0) };

    let area_data = get_data!(area, AreaData, as_mut());
    let app = get_app_from_area(area);
    let app_data = get_data!(app, AppData, as_mut());

    if !app_data.layer_shell_supported {
        let monitor_count = app_data.screen_controller.selected_monitors().len();
        app_data.frame_controller =
            FrameController::new(&app_data.cli_config.preset, monitor_count);
    }

    let viewport_size = Size::new(width as u32, height as u32);

    if !area_data.connector.is_empty() {
        log::debug!(
            "{:?}, {:?}, {:?}",
            area_data.connector,
            area_data.bounds,
            area_data.gl_offset
        );
    }

    let area_size = if app_data.layer_shell_supported {
        Size::new(
            area_data.bounds.width() as u32,
            area_data.bounds.height() as u32,
        )
    } else {
        Size::new(viewport_size.width(), viewport_size.height())
    };
    let screen_size = match app_data.screen_controller.screen_bounds() {
        Some(screen_bounds) if app_data.layer_shell_supported => {
            Size::new(screen_bounds.width() as u32, screen_bounds.height() as u32)
        }
        _ => area_size,
    };

    let renderer = Renderer::new(
        screen_size,
        viewport_size,
        area_size,
        &app_data.cli_config.preset,
    );
    if let Err(err) = &renderer {
        log::error!("Failed to create renderer: {err}");
        std::process::exit(1);
    }
    area_data.renderer = renderer.ok();
}

/// Main render callback executed for each [`gtk::GLArea`].
///
/// Delegates frame production to [`FrameController`],
/// updates input snapshots, and performs presentation.
#[named]
fn on_render(area: &gtk::GLArea, gl_context: &gdk::GLContext) -> glib::Propagation {
    gl_context.make_current();

    let area_data = get_data!(area, AreaData, as_ref());
    let app_data = get_data!(get_app_from_area(area), AppData, as_mut());

    log::trace!(
        "{} {}: frame_hw={}",
        function_name!().white().bold(),
        area_data.connector,
        area.frame_clock().unwrap().frame_counter(),
    );

    app_data.frame_controller.render(
        |frame_stats| {
            let input = InputData {
                mouse: app_data.mouse_controller.snapshot(),
                keyboard: app_data.keyboard_controller.snapshot(),
            };

            // Render all areas
            for area in &app_data.areas {
                let area_data = get_data!(area, AreaData, as_mut());

                if app_data.cli_config.show_overlay && frame_stats.frame_number == 0 {
                    if let Some(widget) = &area_data.info_overlay {
                        setup_fadeout_timer(widget);
                    }
                }

                if let Some(renderer) = area_data.renderer.as_mut() {
                    renderer.render(area_data.gl_offset, &input, frame_stats);
                }
            }

            app_data.keyboard_controller.end_frame();
        },
        |crossfade_t| {
            // Blit current area
            if let Some(renderer) = area_data.renderer.as_ref() {
                renderer.blit(crossfade_t);
            }
        },
    );

    glib::Propagation::Stop
}

/// Gets the application from a [`gtk::GLArea`], assuming
/// it's contained in a [`gtk::Window`].
/// Panics if the [`gtk::GLArea`] isn't in a [`gtk::Window`] or
/// the [`gtk::Window`] has no [`gtk::Application`].
fn get_app_from_area(area: &gtk::GLArea) -> gtk::Application {
    area.root()
        .and_downcast::<gtk::Window>()
        .and_then(|window| window.application())
        .unwrap()
}

/// Sets typed user data on a [`glib::Object`], using the type name as the key.
///
/// # Example
/// ```
/// set_data!(area, AreaData { ... });
/// ```
/// expands to:
/// ```
/// unsafe { area.set_data("AreaData", AreaData { ... }) };
/// ```
/// Note: The second argument must be in the form `TypeName { ... }`,
/// as `TypeName` is used both as the type and the key.
/// Therefore, the following is not valid:
/// ```
/// let area_data = AreaData { ... };
/// set_data!(area, area_data);
/// ```
#[macro_export]
macro_rules! set_data {
    ($obj:expr, $ty:ident { $($fields:tt)* }) => {{
        let key = stringify!($ty);
        let obj = &$obj;
        let data = $ty { $($fields)* };
        unsafe {
            obj.set_data(key, data)
        }
    }};
}

/// Retrieves typed user data from a [`glib::Object`], using the type name
/// as the key.
///
/// # Example
/// ```
/// let app_data = get_data!(app, AppData, as_ref());
/// ```
/// expands to:
/// ```
/// let app_data = unsafe { app.data::<AppData>("AppData").unwrap().as_ref() };
/// ```
///
/// Panics if the data is missing. Use `.as_ref()`, `.as_mut()`, etc.
/// as needed.
#[macro_export]
macro_rules! get_data {
    ($obj:expr, $ty:ty, $($tail:tt)+) => {{
        let key = stringify!($ty);
        let obj = &$obj;
        unsafe {
            obj.data::<$ty>(key)
                .expect(concat!("Missing data: ", stringify!($ty))).$($tail)+
        }
    }};
}

/// Checks if there is typed user data associated with a [`glib::Object`],
/// using the type name as the key.
///
/// # Example
/// ```
/// if has_data!(app, AppData) {
///     // Handle existing data
/// }
/// ```
/// expands to:
/// ```
/// unsafe { app.data::<AppData>("AppData").is_some() }
/// ```
///
/// Returns `true` if data exists for the given type, `false` otherwise.
#[macro_export]
macro_rules! has_data {
    ($obj:expr, $ty:ty) => {{
        let key = stringify!($ty);
        let obj = &$obj;
        unsafe { obj.data::<$ty>(key).is_some() }
    }};
}
