// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frame scheduling, timing, and presentation control.
//!
//! Coordinates logical frame production across multiple monitors,
//! maintains animation timing statistics, and drives crossfade-based
//! frame presentation.

use std::{collections::VecDeque, time::*};

use crate::preset::Preset;

/// Number of warm-up frames ignored before timing becomes valid.
///
/// Prevents startup artifacts from polluting frame timing statistics.
const INITIAL_FRAMES_TO_SKIP: u32 = 2;

/// Time window used for smoothed FPS calculation.
const FRAME_RATE_WINDOW: Duration = Duration::from_secs(1);

/// Coordinates frame production, presentation timing, and crossfade blending.
pub struct FrameController {
    /// Animation time multiplier.
    time_scale: f64,

    /// Constant offset added to shader time.
    time_offset: Duration,

    /// Reference start time for animation clock.
    start_time: Instant,

    /// Timestamp of previous logical frame.
    previous_frame_time: Instant,

    /// Logical frame counter (independent of monitor count).
    frame_number: u32,

    /// Frame timestamps used for FPS smoothing.
    frame_times: VecDeque<Instant>,

    /// Index of monitor currently rendering.
    current_monitor: usize,

    /// Number of monitors composing one logical frame.
    monitor_count: usize,

    /// Warm-up frames already skipped.
    frames_skipped: u32,

    /// Timestamp when the last frame content was rendered.
    last_frame_render_time: Instant,

    /// Timestamp when the last frame content was rendered.
    crossfade: CrossfadeState,

    /// Idle delay between crossfade cycles.
    /// `interval_between_frames - crossfade_duration`
    idle_duration: Duration,
}

#[derive(Debug, Clone)]
struct CrossfadeState {
    /// Total duration of the transition.
    duration: Duration,

    /// Normalized progress in range `[0,1]`.
    t: f32,
}

impl CrossfadeState {
    fn new(duration: Duration) -> Self {
        Self { duration, t: 0.0 }
    }

    /// Returns true if crossfade animation is active.
    fn is_enabled(&self) -> bool {
        !self.duration.is_zero()
    }

    /// Advances normalized crossfade progress.
    fn update(&mut self, elapsed: Duration) {
        if self.is_enabled() {
            self.t = elapsed.div_duration_f32(self.duration).clamp(0.0, 1.0);
        }
    }

    fn reset(&mut self) {
        self.t = 0.0;
    }

    /// Smoothstep easing used for perceptually smooth blending.
    fn smoothstep(t: f32) -> f32 {
        t * t * (3.0 - 2.0 * t)
    }

    /// Returns eased interpolation parameter.
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
            idle_duration: Duration::ZERO,
        }
    }
}

/// Runtime timing information supplied to render callbacks.
#[derive(Debug, Clone)]
pub struct FrameStats {
    /// Scaled elapsed time since animation start (with offset applied).
    /// Corresponds to ShaderToy's `iTime` uniform.
    pub time: Duration,

    /// Scaled time since previous logical frame.
    /// Corresponds to ShaderToy's `iTimeDelta` uniform.
    pub time_delta: Duration,

    /// Smoothed frames-per-second measurement.
    pub frame_rate: f64,

    /// Zero-based logical frame index.
    pub frame_number: u32,
}

impl FrameController {
    /// Creates a controller using preset timing configuration.
    ///
    /// Crossfade duration is derived from `interval_between_frames * crossfade_overlap_ratio`.
    pub fn new(preset: &Preset, monitor_count: usize) -> Self {
        let now = Instant::now();
        let crossfade_duration = preset
            .interval_between_frames
            .mul_f64(preset.crossfade_overlap_ratio);
        let idle_duration = preset
            .interval_between_frames
            .saturating_sub(crossfade_duration);

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
            idle_duration,
        }
    }

    /// Executes rendering for one monitor.
    ///
    /// A single logical frame may span multiple monitors. Only the first monitor renders new content, while the
    /// remaining monitors reuse the result via blitting. This avoids presentation skew between displays.
    pub fn render<F, G>(&mut self, mut render_callback: F, blit_callback: G)
    where
        F: FnMut(&FrameStats),
        G: Fn(f32),
    {
        // Only render new frame content on the first monitor
        if self.is_first_monitor() && self.should_render_new_frame() {
            self.render_new_frame(&mut render_callback);
        }

        self.advance_monitor();

        // Always perform blitting for monitors after initial frames
        if self.frame_number >= INITIAL_FRAMES_TO_SKIP {
            self.perform_crossfade_blit(&blit_callback);
        } else {
            unsafe { gl::Clear(gl::COLOR_BUFFER_BIT) };
        }
    }

    /// Gets the current monitor index being rendered (0 to monitor_count-1).
    pub fn current_monitor(&self) -> usize {
        self.current_monitor
    }

    /// Returns the duration of the crossfade effect.
    pub fn crossfade_duration(&self) -> Duration {
        self.crossfade.duration
    }

    /// Returns the idle duration between crossfade cycles (interval - crossfade duration).
    pub fn idle_duration(&self) -> Duration {
        self.idle_duration
    }

    /// Returns true once the crossfade animation has completed.
    pub fn is_crossfade_complete(&self) -> bool {
        self.crossfade.t >= 1.0
    }

    /// Resets the crossfade parameter to 0, beginning a new crossfade cycle.
    pub fn reset_crossfade(&mut self) {
        self.crossfade.reset();
    }

    /// Returns `true` when rendering the first monitor of the logical frame.
    ///
    /// Only the first monitor is allowed to generate new frame content.
    /// Remaining monitors reuse the result through blitting to avoid
    /// multi-display presentation skew.
    fn is_first_monitor(&self) -> bool {
        self.current_monitor == 0
    }

    /// Determines whether a new logical frame should be rendered.
    ///
    /// A frame is rendered when the warm-up phase is still active, or
    /// the crossfade cycle has completed (or is disabled).
    ///
    /// This ensures rendering occurs once per crossfade period rather
    /// than once per monitor pass.
    fn should_render_new_frame(&self) -> bool {
        if self.frames_skipped < INITIAL_FRAMES_TO_SKIP {
            return true;
        }

        // Render only when crossfade cycle restarts.
        self.crossfade.t == 0.0 || !self.crossfade.is_enabled()
    }

    /// Produces a new logical frame.
    ///
    /// During warm-up, frame statistics are suppressed until a stable
    /// timing baseline is established. Afterward, timing metrics are
    /// updated and passed to the renderer callback.
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

    /// Advances the warm-up phase.
    ///
    /// Initial frames are ignored to avoid unstable timing caused by
    /// GPU initialization, shader compilation, or window realization.
    /// Once warm-up completes, timing statistics are reset.
    fn handle_frame_skip(&mut self) {
        self.frames_skipped += 1;
        if self.frames_skipped == INITIAL_FRAMES_TO_SKIP {
            self.reset_timing();
        }
    }

    /// Advances rendering to the next monitor.
    ///
    /// A full cycle corresponds to one logical frame presentation.
    fn advance_monitor(&mut self) {
        self.current_monitor = (self.current_monitor + 1) % self.monitor_count;
    }

    /// Applies crossfade blending for presentation.
    ///
    /// Alternates blend direction every frame to ping-pong between source and destination framebuffers.
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

    /// Resets all timing measurements after warm-up frames.
    fn reset_timing(&mut self) {
        let now = Instant::now();
        self.start_time = now;
        self.previous_frame_time = now;
        self.frame_number = 0;
        self.frame_times.clear();
    }

    /// Updates frame statistics and returns current measurements.
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

    /// Records the timestamp of a rendered frame.
    ///
    /// The timestamp is added to the FPS averaging window and
    /// outdated samples are removed to maintain a fixed-duration
    /// smoothing interval.
    fn record_frame_time(&mut self, time: Instant) {
        self.frame_times.push_back(time);
        self.remove_old_frame_times(time);
    }

    /// Removes frame times outside the averaging window.
    fn remove_old_frame_times(&mut self, current_time: Instant) {
        while let Some(&oldest_time) = self.frame_times.front() {
            if current_time.duration_since(oldest_time) > FRAME_RATE_WINDOW {
                self.frame_times.pop_front();
            } else {
                break;
            }
        }
    }

    /// Calculates smoothed FPS based on frames within the time window.
    fn calculate_frame_rate(&self, current_time: Instant) -> f64 {
        match self.frame_times.len() {
            0 => 0.0,
            1 => {
                // Only one frame recorded, calculate instantaneous rate
                let delta = current_time.duration_since(self.previous_frame_time);
                if delta.is_zero() {
                    0.0
                } else {
                    1.0 / delta.as_secs_f64()
                }
            }
            len => {
                // Calculate average rate over the window
                let window_start = *self.frame_times.front().unwrap();
                let total_time = current_time.duration_since(window_start).as_secs_f64();
                let frame_intervals = len - 1; // N frames -> N-1 intervals

                if total_time > 0.0 {
                    frame_intervals as f64 / total_time
                } else {
                    0.0
                }
            }
        }
    }
}
