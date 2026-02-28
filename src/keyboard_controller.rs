// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard input handling and ShaderToy compatibility layer.
//!
//! Captures GTK keyboard events and exposes them as ShaderToy-style
//! keyboard buffers, including keydown state, one-frame press pulses,
//! and toggle semantics.

use gtk::{gdk, glib, prelude::*};
use owo_colors::OwoColorize;

use crate::{app::*, *};

/// Maintains keyboard input state for shader consumption.
///
/// The controller translates GTK key events into ShaderToy-compatible
/// keyboard buffers using JavaScript keycode conventions.
#[derive(Debug)]
pub struct KeyboardController {
    /// Application reference used to access shared state.
    app: gtk::Application,

    /// Internal keyboard state buffers.
    data: KeyboardData,

    /// Indicates whether a new snapshot should be emitted.
    snapshot_ready: bool,

    /// `true` if any [`KeyboardData::keypressed`] entry is currently active.
    /// Used to clear one-frame pulses.
    keypressed: bool,
}

/// Number of keycodes.
const NUM_KEYS: usize = u8::MAX as usize + 1;

/// Shader-visible keyboard buffers.
///
/// Arrays are indexed by JavaScript keycode to match ShaderToy's keyboard texture layout.
#[derive(Clone, Copy, Debug)]
pub struct KeyboardData {
    /// Keys currently held down.
    keydown: [bool; NUM_KEYS],

    /// One-frame pulse generated when a key transitions from released to pressed.
    keypressed: [bool; NUM_KEYS],

    /// Persistent toggle state flipped on every press.
    toggled: [bool; NUM_KEYS],
}

impl KeyboardData {
    pub fn new() -> Self {
        Self {
            keydown: [false; NUM_KEYS],
            keypressed: [false; NUM_KEYS],
            toggled: [false; NUM_KEYS],
        }
    }

    /// Continuous key state (`is_down`).
    pub fn keydown(&self) -> &[bool] {
        &self.keydown
    }

    /// One-frame press pulses.
    pub fn keypressed(&self) -> &[bool] {
        &self.keypressed
    }

    /// Toggle state updated on each press.
    pub fn toggled(&self) -> &[bool] {
        &self.toggled
    }
}

impl KeyboardController {
    pub fn new(app: gtk::Application) -> Self {
        Self {
            app,
            data: KeyboardData::new(),
            snapshot_ready: true,
            keypressed: false,
        }
    }

    /// Installs keyboard event handlers on a GTK widget.
    ///
    /// Events update global keyboard state stored in [`AppData`].
    /// Multiple widgets may share the same controller.
    ///
    /// The widget must be focusable (i.e. `set_focusable(true)`) to
    /// receive key events.
    pub fn setup_widget(&self, widget: &impl gtk::prelude::IsA<gtk::Widget>) {
        let key_controller = gtk::EventControllerKey::new();

        key_controller.connect_key_pressed(glib::clone!(
            #[weak(rename_to = app)]
            self.app,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                let app_data = get_data!(app, AppData, as_mut());
                let keyboard = &mut app_data.keyboard_controller;

                if let Some(js) = keyval_to_js_keycode(key) {
                    // Generate one-frame pulse on rising edge
                    if !keyboard.data.keydown[js as usize] {
                        keyboard.data.keypressed[js as usize] = true;
                        keyboard.keypressed = true;
                        keyboard.data.toggled[js as usize] = !keyboard.data.toggled[js as usize];
                    }

                    keyboard.data.keydown[js as usize] = true;
                    keyboard.snapshot_ready = true;

                    log::debug!("{} key={} js={}", "key-pressed".white().bold(), key, js);
                }

                glib::Propagation::Proceed
            }
        ));

        key_controller.connect_key_released(glib::clone!(
            #[weak(rename_to = app)]
            self.app,
            move |_, key, _, _| {
                let app_data = get_data!(app, AppData, as_mut());
                let keyboard = &mut app_data.keyboard_controller;

                if let Some(js) = keyval_to_js_keycode(key) {
                    keyboard.data.keydown[js as usize] = false;
                    keyboard.snapshot_ready = true;

                    log::debug!("{} key={} js={}", "key-released".white().bold(), key, js);
                }
            }
        ));

        widget.as_ref().add_controller(key_controller);
    }

    /// Produces keyboard data for renderer upload.
    ///
    /// Returns `None` when no state changes occurred since the previous snapshot.
    pub fn snapshot(&mut self) -> Option<KeyboardData> {
        if self.snapshot_ready {
            self.snapshot_ready = false;
            Some(self.data)
        } else {
            None
        }
    }

    /// Finalizes the frame and clears one-frame press pulses.
    ///
    /// Must be called after all monitors have rendered so that
    /// every render target observes the press event.
    pub fn end_frame(&mut self) {
        if self.keypressed {
            self.data.keypressed.fill(false);
            self.keypressed = false;
            self.snapshot_ready = true;
        }
    }
}

/// Converts a GTK key value into a JavaScript keycode.
fn keyval_to_js_keycode(key: gdk::Key) -> Option<u8> {
    use gdk::Key;
    match key {
        // Control keys
        Key::BackSpace => Some(8),
        Key::Tab => Some(9),
        Key::Return | Key::KP_Enter => Some(13),
        Key::Shift_L | Key::Shift_R => Some(16),
        Key::Control_L => Some(17),
        Key::Control_R => Some(191),
        Key::Alt_L => Some(18),
        Key::Alt_R | Key::ISO_Level3_Shift | Key::Mode_switch => Some(225),
        Key::Caps_Lock => Some(20),
        Key::Escape => Some(27),
        Key::space => Some(32),

        // Navigation
        Key::Page_Up | Key::KP_Page_Up => Some(33),
        Key::Page_Down | Key::KP_Page_Down => Some(34),
        Key::End | Key::KP_End => Some(35),
        Key::Home | Key::KP_Home => Some(36),
        Key::Left | Key::KP_Left => Some(37),
        Key::Up | Key::KP_Up => Some(38),
        Key::Right | Key::KP_Right => Some(39),
        Key::Down | Key::KP_Down => Some(40),
        Key::Insert | Key::KP_Insert => Some(45),
        Key::Delete | Key::KP_Delete => Some(46),

        // Digits
        Key::_0 => Some(48),
        Key::_1 => Some(49),
        Key::_2 => Some(50),
        Key::_3 => Some(51),
        Key::_4 => Some(52),
        Key::_5 => Some(53),
        Key::_6 => Some(54),
        Key::_7 => Some(55),
        Key::_8 => Some(56),
        Key::_9 => Some(57),

        // Letters
        Key::a | Key::A => Some(65),
        Key::b | Key::B => Some(66),
        Key::c | Key::C => Some(67),
        Key::d | Key::D => Some(68),
        Key::e | Key::E => Some(69),
        Key::f | Key::F => Some(70),
        Key::g | Key::G => Some(71),
        Key::h | Key::H => Some(72),
        Key::i | Key::I => Some(73),
        Key::j | Key::J => Some(74),
        Key::k | Key::K => Some(75),
        Key::l | Key::L => Some(76),
        Key::m | Key::M => Some(77),
        Key::n | Key::N => Some(78),
        Key::o | Key::O => Some(79),
        Key::p | Key::P => Some(80),
        Key::q | Key::Q => Some(81),
        Key::r | Key::R => Some(82),
        Key::s | Key::S => Some(83),
        Key::t | Key::T => Some(84),
        Key::u | Key::U => Some(85),
        Key::v | Key::V => Some(86),
        Key::w | Key::W => Some(87),
        Key::x | Key::X => Some(88),
        Key::y | Key::Y => Some(89),
        Key::z | Key::Z => Some(90),

        // System
        Key::Meta_L | Key::Super_L => Some(91),
        Key::Meta_R | Key::Super_R => Some(92),
        Key::Menu => Some(93),

        // Numpad with numlock on
        Key::KP_0 => Some(96),
        Key::KP_1 => Some(97),
        Key::KP_2 => Some(98),
        Key::KP_3 => Some(99),
        Key::KP_4 => Some(100),
        Key::KP_5 => Some(101),
        Key::KP_6 => Some(102),
        Key::KP_7 => Some(103),
        Key::KP_8 => Some(104),
        Key::KP_9 => Some(105),

        // Numpad ops
        Key::KP_Multiply => Some(106),
        Key::KP_Add => Some(107),
        Key::KP_Separator => Some(108),
        Key::KP_Subtract => Some(109),
        Key::KP_Decimal => Some(110),
        Key::KP_Divide => Some(111),

        // Function keys
        Key::F1 => Some(112),
        Key::F2 => Some(113),
        Key::F3 => Some(114),
        Key::F4 => Some(115),
        Key::F5 => Some(116),
        Key::F6 => Some(117),
        Key::F7 => Some(118),
        Key::F8 => Some(119),
        Key::F9 => Some(120),
        Key::F10 => Some(121),
        Key::F11 => Some(122),
        Key::F12 => Some(123),

        Key::Num_Lock => Some(144),
        Key::Scroll_Lock => Some(145),
        Key::Pause => Some(19),

        // Punctuation and other symbols
        Key::semicolon | Key::colon => Some(59),
        Key::equal | Key::plus => Some(61),
        Key::less | Key::comma => Some(188),
        Key::minus | Key::underscore => Some(189),
        Key::period | Key::greater => Some(190),
        Key::slash | Key::question => Some(191),
        Key::grave | Key::asciitilde => Some(192),
        Key::bracketleft | Key::braceleft => Some(219),
        Key::backslash | Key::bar => Some(220),
        Key::bracketright | Key::braceright => Some(221),
        Key::apostrophe | Key::quotedbl => Some(222),

        _ => None,
    }
}
