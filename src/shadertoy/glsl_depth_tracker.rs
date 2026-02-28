// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! GLSL block-depth tracker used during shader preprocessing.
//!
//! Tracks lexical scope depth while scanning GLSL source code,
//! allowing transformation passes to understand where declarations
//! and statements occur within nested blocks.
//!
//! Used internally by the GLSL preprocessing pipeline.
//! Not intended for direct external use.

#[derive(Debug, Clone, Copy, Default)]
pub struct GlslDepthTracker {
    brace: i32,
    bracket: i32,
    paren: i32,
    for_loop_depth: i32,
    for_loop_part: i32, // 0 = not in for-loop, 1 = initialization, 2 = condition, 3 = increment
}

impl PartialEq for GlslDepthTracker {
    fn eq(&self, other: &Self) -> bool {
        self.brace == other.brace && self.bracket == other.bracket && self.paren == other.paren
    }
}
impl Eq for GlslDepthTracker {}

impl GlslDepthTracker {
    /// Updates brackets depth counters based on the given byte character.
    pub fn update_brackets(&mut self, byte: u8) {
        match byte {
            b'(' => self.paren += 1,
            b')' => self.paren = self.paren.saturating_sub(1),
            b'{' => self.brace += 1,
            b'}' => self.brace = self.brace.saturating_sub(1),
            b'[' => self.bracket += 1,
            b']' => self.bracket = self.bracket.saturating_sub(1),
            _ => {}
        }
    }

    /// Updates for-loop depth counters based on the given byte character.
    pub fn update_for_loop(&mut self, byte: u8) {
        if self.for_loop_depth > 0 {
            match byte {
                b';' if self.for_loop_part < 3 => self.for_loop_part += 1,
                b')' if self.paren == 0 => {
                    self.for_loop_depth = 0;
                    self.for_loop_part = 0;
                }
                _ => {}
            }
        }
    }

    /// Returns true if we're inside parentheses.
    pub fn in_parentheses(&self) -> bool {
        self.paren > 0
    }

    /// Returns true if we're inside square brackets.
    pub fn in_square_brackets(&self) -> bool {
        self.bracket > 0
    }

    /// Returns true if we're at global scope.
    pub fn at_global_scope(&self) -> bool {
        self.brace == 0 && self.paren == 0 && self.bracket == 0
    }

    /// Returns true if we're in for-loop initialization.
    pub fn in_for_loop_initialization(&self) -> bool {
        self.for_loop_depth > 0 && self.for_loop_part == 1
    }

    /// Begins tracking the parts of a for-loop signature.
    pub fn start_for_loop_tracking(&mut self) {
        self.for_loop_depth = 1;
        self.for_loop_part = 1;
    }
}
