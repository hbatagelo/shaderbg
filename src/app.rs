// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use function_name::named;
use gtk::{
    gdk,
    gio::{self, prelude::*},
    glib,
    prelude::*,
};
use gtk4_layer_shell::*;
use owo_colors::OwoColorize;
use std::{path::*, sync::Once, time::Duration};

use crate::{
    drm::*, frame_controller::*, geometry::*, mouse_controller::*, preset::*, renderer::*,
    screen_controller::*, *,
};

const STANDBY_CHECK_INTERVAL: Duration = Duration::from_millis(250);

static LOAD_GL: Once = Once::new();

pub struct AppData {
    pub areas: Vec<gtk::GLArea>,
    pub preset: Preset,
    pub preset_file: Option<PathBuf>,
    pub show_overlay: bool,
    pub preset_monitor: Option<gio::FileMonitor>,
    pub animation_timer: Option<glib::SourceId>,
    pub frame_controller: FrameController,
    pub mouse_controller: MouseController,
    pub screen_controller: ScreenController,
    pub layer_shell_supported: bool,
}

#[derive(Default)]
pub struct AreaData {
    pub renderer: Option<Renderer>,
    pub connector: String,
    pub bounds: Rectangle,
    pub gl_offset: Offset,
    pub info_overlay: Option<gtk::Widget>,
}

pub fn init_logging() -> Result<(), log::SetLoggerError> {
    let level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    };

    simple_logger::SimpleLogger::new().with_level(level).init()
}

pub fn run(preset: Preset, preset_file: Option<PathBuf>, show_overlay: bool) -> glib::ExitCode {
    let app = gtk::Application::builder().application_id(APP_ID).build();

    set_data!(
        app,
        AppData {
            areas: Vec::default(),
            preset,
            preset_file: None,
            show_overlay,
            preset_monitor: None,
            animation_timer: None,
            frame_controller: FrameController::default(),
            mouse_controller: MouseController::new(app.clone()),
            screen_controller: ScreenController::default(),
            layer_shell_supported: false,
        }
    );

    if let Some(path) = &preset_file {
        setup_preset_monitor(&app, path, on_preset_change);
    }

    app.connect_activate(activate);
    app.run_with_args(&[""])
}

fn on_preset_change(app: &gtk::Application, preset_path: &Path) {
    match Preset::from_toml_file(preset_path) {
        Ok(new_preset) => {
            let app_data = get_data!(app, AppData, as_mut());

            if new_preset != app_data.preset {
                log::info!("Applying updated preset");
                app_data.preset = new_preset;
                on_monitor_changed(app.clone());
            } else {
                log::info!("Preset unchanged after reload");
            }
        }
        Err(err) => log::error!("Error reloading preset: {err}"),
    }
}

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

#[named]
pub fn on_monitor_changed(app: gtk::Application) {
    log::debug!("{}", function_name!().white().bold());

    app.windows().iter().for_each(|window| window.destroy());

    let has_connected_output = has_connected_drm_output().unwrap_or_else(|err| {
        log::warn!("Could not query DRM: {err}");
        true
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

    let app_clone = app.clone();
    glib::timeout_add_local_once(STANDBY_CHECK_INTERVAL, move || {
        log::trace!("Standby check");
        on_monitor_changed(app_clone);
    });
}

pub fn create_windows(app: &gtk::Application) {
    let app_data = get_data!(app, AppData, as_mut());
    let old_areas = std::mem::take(&mut app_data.areas);

    app_data.screen_controller = ScreenController::new(app);

    let monitor_count = app_data.screen_controller.selected_monitors().len();
    app_data.frame_controller = FrameController::new(&app_data.preset, monitor_count);

    if app_data.layer_shell_supported {
        create_layer_windows(app);
    } else {
        create_fallback_window(app);
    }

    drop(old_areas);
    setup_animation_driver(app);
}

fn create_layer_windows(app: &gtk::Application) {
    let app_data = get_data!(app, AppData, as_mut());

    for monitor in app_data.screen_controller.selected_monitors() {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .name(APP_NAME)
            .title(APP_NAME)
            .build();

        setup_layer_shell(&window);

        let area = setup_area(app);

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

        if app_data.show_overlay {
            let overlay = gtk::Overlay::new();
            overlay.set_child(Some(&area));

            if gl_offset == Offset::default() {
                let name = &app_data.preset.name;
                let author = &app_data.preset.username;
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
    }
}

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

    let area = setup_area(app);

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

    if app_data.show_overlay {
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&area));

        let name = &app_data.preset.name;
        let author = &app_data.preset.username;
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

        for i in (1..=3).rev() {
            let shadow = create_label(text, font_size, is_bold, "black");
            shadow.set_opacity(0.3 / i as f64);
            fixed.put(&shadow, i as f64, i as f64);
        }

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

fn setup_area(app: &gtk::Application) -> gtk::GLArea {
    let area = gtk::GLArea::new();

    area.set_required_version(GL_VERSION.0, GL_VERSION.1);
    area.set_has_depth_buffer(false);
    area.set_has_stencil_buffer(false);
    area.set_auto_render(false);

    let app_data = get_data!(app, AppData, as_ref());
    app_data.mouse_controller.setup_area(&area);

    area.connect_realize(on_realize);
    area.connect_resize(on_resize);
    area.connect_render(on_render);

    area
}

fn setup_layer_shell(window: &gtk::ApplicationWindow) {
    window.init_layer_shell();
    window.set_layer(Layer::Background);
    [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom]
        .iter()
        .for_each(|&anchor| window.set_anchor(anchor, true));
    window.set_namespace(Some(APP_NAME));
    window.set_exclusive_zone(-1);
}

fn setup_animation_driver(app: &gtk::Application) {
    let app_data = get_data!(app, AppData, as_mut());
    if let Some(source_id) = app_data.animation_timer.take() {
        source_id.remove();
    }

    if app_data.preset.interval_between_frames.is_zero() {
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
    } else if app_data.preset.crossfade_overlap_ratio > 0.0 {
        cross_fade(app);
    } else {
        let tick_callback = glib::clone!(
            #[weak]
            app,
            #[upgrade_or_panic]
            move || {
                areas_queue_render(&app);
                glib::ControlFlow::Continue
            }
        );
        let source_id =
            glib::timeout_add_local(app_data.preset.interval_between_frames, tick_callback);
        app_data.animation_timer = Some(source_id);
    }
}

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

fn cross_fade(app: &gtk::Application) {
    let app_data = get_data!(app, AppData, as_mut());

    app_data.frame_controller.reset_crossfade();

    let crossfade_duration = app_data
        .preset
        .interval_between_frames
        .mul_f64(app_data.preset.crossfade_overlap_ratio);
    let idle_duration = app_data.preset.interval_between_frames - crossfade_duration;

    log::debug!("Crossfade started for {:#?}...", crossfade_duration);

    let tick_callback = glib::clone!(
        #[weak]
        app,
        #[upgrade_or_panic]
        move || {
            areas_queue_render(&app);
            if app_data.frame_controller.crossfade_t() < 1.0 {
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
        app_data.frame_controller = FrameController::new(&app_data.preset, monitor_count);
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

    let renderer = Renderer::new(screen_size, viewport_size, area_size, &app_data.preset);
    if let Err(err) = &renderer {
        log::error!("Failed to create renderer: {err}");
        std::process::exit(1);
    }
    area_data.renderer = renderer.ok();
}

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
            for area in &app_data.areas {
                let area_data = get_data!(area, AreaData, as_mut());

                if app_data.show_overlay && frame_stats.frame_number == 0 {
                    if let Some(widget) = &area_data.info_overlay {
                        setup_fadeout_timer(widget);
                    }
                }

                if let Some(renderer) = area_data.renderer.as_mut() {
                    renderer.render(
                        area_data.gl_offset,
                        app_data.mouse_controller.i_mouse_data(),
                        frame_stats,
                    );
                }
            }
        },
        |crossfade_t| {
            if let Some(renderer) = area_data.renderer.as_ref() {
                renderer.blit(crossfade_t);
            }
        },
    );

    glib::Propagation::Stop
}

fn get_app_from_area(area: &gtk::GLArea) -> gtk::Application {
    area.root()
        .and_downcast::<gtk::Window>()
        .and_then(|window| window.application())
        .unwrap()
}

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

#[macro_export]
macro_rules! has_data {
    ($obj:expr, $ty:ty) => {{
        let key = stringify!($ty);
        let obj = &$obj;
        unsafe { obj.data::<$ty>(key).is_some() }
    }};
}
