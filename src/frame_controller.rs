// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::VecDeque, time::*};

use crate::preset::Preset;

const INITIAL_FRAMES_TO_SKIP: u32 = 2;
const FRAME_RATE_WINDOW: Duration = Duration::from_secs(1);

pub struct FrameController {
    time_scale: f64,
    time_offset: Duration,
    start_time: Instant,
    previous_frame_time: Instant,
    frame_number: u32,
    frame_times: VecDeque<Instant>,
    current_monitor: usize,
    monitor_count: usize,
    frames_skipped: u32,
    last_frame_render_time: Instant,
    crossfade: CrossfadeState,
}

#[derive(Debug, Clone)]
struct CrossfadeState {
    duration: Duration,
    t: f32,
}

impl CrossfadeState {
    fn new(duration: Duration) -> Self {
        Self { duration, t: 0.0 }
    }

    fn is_enabled(&self) -> bool {
        !self.duration.is_zero()
    }

    fn update(&mut self, elapsed: Duration) {
        if self.is_enabled() {
            self.t = elapsed.div_duration_f32(self.duration).clamp(0.0, 1.0);
        }
    }

    fn reset(&mut self) {
        self.t = 0.0;
    }

    fn smoothstep(t: f32) -> f32 {
        t * t * (3.0 - 2.0 * t)
    }

    fn value(&self) -> f32 {
        Self::smoothstep(self.t)
    }
}

impl Default for FrameController {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            time_scale: 1.0,
            time_offset: Duration::ZERO,
            start_time: now,
            previous_frame_time: now,
            frame_number: 0,
            frame_times: VecDeque::new(),
            current_monitor: 0,
            monitor_count: 1,
            frames_skipped: 0,
            last_frame_render_time: now,
            crossfade: CrossfadeState::new(Duration::ZERO),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameStats {
    pub time: Duration,
    pub time_delta: Duration,
    pub frame_rate: f64,
    pub frame_number: u32,
}

impl FrameController {
    pub fn new(preset: &Preset, monitor_count: usize) -> Self {
        let now = Instant::now();
        let crossfade_duration = preset
            .interval_between_frames
            .mul_f64(preset.crossfade_overlap_ratio);

        Self {
            time_scale: preset.time_scale.max(0.0),
            time_offset: preset.time_offset,
            start_time: now,
            previous_frame_time: now,
            frame_number: 0,
            frame_times: VecDeque::new(),
            current_monitor: 0,
            monitor_count,
            frames_skipped: 0,
            last_frame_render_time: now,
            crossfade: CrossfadeState::new(crossfade_duration),
        }
    }

    pub fn render<F, G>(&mut self, mut render_callback: F, blit_callback: G)
    where
        F: FnMut(&FrameStats),
        G: Fn(f32),
    {
        if self.is_first_monitor() && self.should_render_new_frame() {
            self.render_new_frame(&mut render_callback);
        }

        self.advance_monitor();

        if self.frame_number >= INITIAL_FRAMES_TO_SKIP {
            self.perform_crossfade_blit(&blit_callback);
        } else {
            unsafe { gl::Clear(gl::COLOR_BUFFER_BIT) };
        }
    }

    pub fn current_monitor(&self) -> usize {
        self.current_monitor
    }

    pub fn crossfade_t(&self) -> f32 {
        self.crossfade.t
    }

    pub fn reset_crossfade(&mut self) {
        self.crossfade.reset();
    }

    fn is_first_monitor(&self) -> bool {
        self.current_monitor == 0
    }

    fn should_render_new_frame(&self) -> bool {
        if self.frames_skipped < INITIAL_FRAMES_TO_SKIP {
            true
        } else {
            self.crossfade.t == 0.0 || !self.crossfade.is_enabled()
        }
    }

    fn render_new_frame<F>(&mut self, render_callback: &mut F)
    where
        F: FnMut(&FrameStats),
    {
        if self.frames_skipped < INITIAL_FRAMES_TO_SKIP {
            self.handle_frame_skip();
        } else {
            let frame_stats = self.update_frame_stats();
            render_callback(&frame_stats);
        }

        self.last_frame_render_time = Instant::now();
    }

    fn handle_frame_skip(&mut self) {
        self.frames_skipped += 1;
        if self.frames_skipped == INITIAL_FRAMES_TO_SKIP {
            self.reset_timing();
        }
    }

    fn advance_monitor(&mut self) {
        self.current_monitor = (self.current_monitor + 1) % self.monitor_count;
    }

    fn perform_crossfade_blit<G>(&mut self, blit_callback: &G)
    where
        G: Fn(f32),
    {
        let elapsed_since_render = Instant::now().duration_since(self.last_frame_render_time);
        self.crossfade.update(elapsed_since_render);

        let crossfade_t = if self.frame_number.is_multiple_of(2) {
            1.0 - self.crossfade.value()
        } else {
            self.crossfade.value()
        };

        blit_callback(crossfade_t);
    }

    fn reset_timing(&mut self) {
        let now = Instant::now();
        self.start_time = now;
        self.previous_frame_time = now;
        self.frame_number = 0;
        self.frame_times.clear();
    }

    fn update_frame_stats(&mut self) -> FrameStats {
        let now = Instant::now();
        let elapsed_time = now.duration_since(self.start_time);
        let delta_time = now.duration_since(self.previous_frame_time);

        self.record_frame_time(now);
        let frame_rate = self.calculate_frame_rate(now);

        let stats = FrameStats {
            time: elapsed_time.mul_f64(self.time_scale) + self.time_offset,
            time_delta: delta_time.mul_f64(self.time_scale),
            frame_rate,
            frame_number: self.frame_number,
        };

        self.previous_frame_time = now;
        self.frame_number = self.frame_number.wrapping_add(1);

        stats
    }

    fn record_frame_time(&mut self, time: Instant) {
        self.frame_times.push_back(time);
        self.remove_old_frame_times(time);
    }

    fn remove_old_frame_times(&mut self, current_time: Instant) {
        while let Some(&oldest_time) = self.frame_times.front() {
            if current_time.duration_since(oldest_time) > FRAME_RATE_WINDOW {
                self.frame_times.pop_front();
            } else {
                break;
            }
        }
    }

    fn calculate_frame_rate(&self, current_time: Instant) -> f64 {
        match self.frame_times.len() {
            0 => 0.0,
            1 => {
                let delta = current_time.duration_since(self.previous_frame_time);
                if delta.is_zero() {
                    0.0
                } else {
                    1.0 / delta.as_secs_f64()
                }
            }
            len => {
                let window_start = *self.frame_times.front().unwrap();
                let total_time = current_time.duration_since(window_start).as_secs_f64();
                let frame_intervals = len - 1;

                if total_time > 0.0 {
                    frame_intervals as f64 / total_time
                } else {
                    0.0
                }
            }
        }
    }
}
