// SPDX-License-Identifier: GPL-3.0-only

//! Spring and motion switch for the menu's open/close animation. The same
//! math as the taskbar's `anim.rs` (copied: the two are separate crates).

use std::sync::OnceLock;

/// Longest slice a spring integrates in one go; longer frames are split.
const SPRING_SUBSTEP_S: f32 = 1.0 / 120.0;
/// A frame longer than this (a stalled menu) is treated as this long.
const SPRING_MAX_FRAME_S: f32 = 0.1;

#[derive(Debug, Clone, Copy)]
pub struct Spring {
    pub pos: f32,
    pub vel: f32,
}

impl Spring {
    pub fn new(pos: f32) -> Self {
        Self { pos, vel: 0.0 }
    }

    /// Semi-implicit Euler, split into fixed substeps so a long frame stays stable.
    pub fn step(&mut self, target: f32, dt_s: f32, stiffness: f32, damping: f32) {
        let mut left = dt_s.clamp(0.0, SPRING_MAX_FRAME_S);
        while left > 0.0 {
            let dt = left.min(SPRING_SUBSTEP_S);
            let accel = stiffness * (target - self.pos) - damping * self.vel;
            self.vel += accel * dt;
            self.pos += self.vel * dt;
            left -= dt;
        }
    }

    pub fn settled(&self, target: f32) -> bool {
        (self.pos - target).abs() < 0.002 && self.vel.abs() < 0.01
    }
}

/// 0.0 when motion is off (reduced motion or `animation_duration_ms` = 0), else 1.0.
/// Read once: `COSMETICS_REDUCE_MOTION` and the compositor's `animation_duration_ms`.
pub fn motion_scale() -> f32 {
    static SCALE: OnceLock<f32> = OnceLock::new();
    *SCALE.get_or_init(|| {
        let reduce = std::env::var("COSMETICS_REDUCE_MOTION").ok();
        let duration = std::env::var_os("HOME")
            .map(|home| {
                std::path::Path::new(&home)
                    .join(".config/cosmic/com.system76.CosmicComp/v1/animation_duration_ms")
            })
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| text.trim().trim_start_matches("Some(").trim_end_matches(')').parse().ok());
        motion_scale_from(reduce.as_deref(), duration)
    })
}

fn motion_scale_from(reduce: Option<&str>, duration_ms: Option<u32>) -> f32 {
    if reduce == Some("1") || duration_ms == Some(0) {
        0.0
    } else {
        1.0
    }
}

/// How far below its place the menu starts, in px.
pub const SLIDE_PX: f32 = 36.0;

/// Spring constants: the opening gives a little overshoot, the closing none.
pub const OPEN: (f32, f32) = (380.0, 26.0);
pub const CLOSE: (f32, f32) = (520.0, 46.0);

/// The menu's offset below its place at spring position `pos` (0 = hidden, 1 = open).
pub fn slide_offset(pos: f32) -> f32 {
    (1.0 - pos) * SLIDE_PX
}

/// Opacity of the menu's glass at spring position `pos`.
pub fn glass_alpha(pos: f32) -> f32 {
    0.55 + 0.45 * pos.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_converges_without_big_overshoot() {
        let mut s = Spring::new(0.0);
        let mut peak = 0.0f32;
        let mut t = 0.0;
        while t < 1.5 {
            s.step(1.0, 1.0 / 120.0, OPEN.0, OPEN.1);
            peak = peak.max(s.pos);
            t += 1.0 / 120.0;
        }
        assert!(s.settled(1.0), "pos={} vel={}", s.pos, s.vel);
        assert!(peak <= 1.1, "peak={peak}");
    }

    #[test]
    fn closing_goes_down_fast_without_going_below_zero_much() {
        let mut s = Spring::new(1.0);
        let mut t = 0.0;
        let mut low = 1.0f32;
        while t < 0.3 {
            s.step(0.0, 1.0 / 120.0, CLOSE.0, CLOSE.1);
            low = low.min(s.pos);
            t += 1.0 / 120.0;
        }
        assert!(s.pos < 0.05, "pos={}", s.pos);
        assert!(low > -0.05, "low={low}");
    }

    #[test]
    fn a_long_frame_is_stable() {
        let mut s = Spring::new(0.0);
        s.step(1.0, 5.0, OPEN.0, OPEN.1);
        assert!(s.pos.is_finite() && s.pos.abs() < 3.0);
    }

    #[test]
    fn slide_and_glass_follow_the_position() {
        assert_eq!(slide_offset(1.0), 0.0);
        assert_eq!(slide_offset(0.0), SLIDE_PX);
        assert!(glass_alpha(0.0) < glass_alpha(1.0));
        assert_eq!(glass_alpha(1.0), 1.0);
        assert_eq!(glass_alpha(1.4), 1.0);
    }

    #[test]
    fn motion_is_off_for_reduced_motion_or_zero_duration() {
        assert_eq!(motion_scale_from(Some("1"), Some(200)), 0.0);
        assert_eq!(motion_scale_from(None, Some(0)), 0.0);
        assert_eq!(motion_scale_from(None, Some(200)), 1.0);
        assert_eq!(motion_scale_from(None, None), 1.0);
    }
}
