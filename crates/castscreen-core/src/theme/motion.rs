//! Fluid motion primitives for CastScreen.
//!
//! Apple's fluid-interface model, ported to `egui`'s immediate-mode loop:
//!
//! - Motion is described as **damping ratio + response**, not as a duration.
//!   A spring has no fixed duration; its settle time emerges from the physics.
//! - Every animation starts from the **presentation value** (what is currently
//!   on screen), never from the logical target. That is what makes an animation
//!   interruptible: retargeting mid-flight keeps position *and* velocity, so
//!   there is no jump and no velocity "brick wall".
//! - Overshoot is reserved for motion that carried momentum. UI that simply
//!   appears is critically damped.
//!
//! Springs live in `egui`'s temporary memory keyed by widget `Id`, so callers
//! stay stateless and any widget can animate without owning a field.

use eframe::egui::{self, Id};
use std::sync::OnceLock;

/// A motion character, in Apple's two designer-facing parameters.
///
/// * `damping` — 1.0 is critically damped (no overshoot). Below 1.0 overshoots;
///   lower is bouncier.
/// * `response` — roughly how long, in seconds, the value takes to reach the
///   target. Lower is snappier. **Not** a duration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub damping: f32,
    pub response: f32,
}

impl Motion {
    /// House default: critically damped, graceful, never distracting.
    pub const UI: Self = Self { damping: 1.0, response: 0.35 };

    /// Press / release feedback. Must feel instantaneous under the finger.
    pub const PRESS: Self = Self { damping: 1.0, response: 0.16 };

    /// Repositioning an element (Apple ships 1.0 / 0.4 for PiP).
    pub const MOVE: Self = Self { damping: 1.0, response: 0.4 };

    /// A sheet or drawer arriving. Slight overshoot: it was thrown into place.
    pub const SHEET: Self = Self { damping: 0.8, response: 0.3 };

    /// Continuous telemetry (meters, gauges). Fast enough to read as live,
    /// damped enough not to strobe between frames.
    pub const READOUT: Self = Self { damping: 1.0, response: 0.22 };
}

/// A single-axis spring holding its own presentation value and velocity.
///
/// Decompose 2D motion into two independent springs — one spring driven by a
/// 2D distance desynchronises as soon as the axes have different velocities.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spring {
    /// The value currently on screen.
    pub value: f32,
    /// Current rate of change, in units per second.
    pub velocity: f32,
}

impl Spring {
    pub fn at(value: f32) -> Self {
        Self { value, velocity: 0.0 }
    }

    /// Integrate one frame toward `target`, starting from the presentation value.
    ///
    /// Retargeting between calls is free and continuous: the new force is
    /// applied to the velocity the element already had.
    pub fn advance(&mut self, target: f32, motion: Motion, dt: f32) -> f32 {
        let omega = std::f32::consts::TAU / motion.response.max(1.0e-4);
        let stiffness = omega * omega;
        let friction = 2.0 * motion.damping * omega;

        // Sub-step at 240 Hz so a dropped frame cannot destabilise the integrator.
        let steps = ((dt * 240.0).ceil() as u32).clamp(1, 8);
        let h = dt / steps as f32;
        for _ in 0..steps {
            let accel = -stiffness * (self.value - target) - friction * self.velocity;
            self.velocity += accel * h;
            self.value += self.velocity * h;
        }

        if self.is_settled(target) {
            self.value = target;
            self.velocity = 0.0;
        }
        self.value
    }

    /// True once the spring is within perceptual noise of the target.
    pub fn is_settled(&self, target: f32) -> bool {
        (self.value - target).abs() < 0.001 && self.velocity.abs() < 0.01
    }

    /// Hand a gesture's release velocity to the spring so the animation
    /// continues at exactly the speed the pointer was moving — no seam between
    /// dragging and animating.
    pub fn hand_off(&mut self, gesture_velocity: f32) {
        self.velocity = gesture_velocity;
    }
}

/// Whether Windows is asking for reduced motion
/// (Settings -> Accessibility -> Visual effects -> Animation effects).
///
/// Reduced motion is not *no* feedback: springs collapse to their settled value
/// and looping ambient motion stops, while colour and opacity cues that carry
/// meaning stay.
pub fn prefers_reduced_motion() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        use windows::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        };
        let mut animations_enabled: i32 = 1;
        let queried = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some(&mut animations_enabled as *mut i32 as *mut core::ffi::c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        // If the query fails, assume the user wants the full experience.
        queried.is_ok() && animations_enabled == 0
    })
}

/// Spring a named value toward `target`, returning what should be drawn now.
///
/// The spring state is keyed by `id`, so calling this every frame from an
/// immediate-mode widget is all that is required. Repaints are requested only
/// while the value is still moving, so a settled UI costs nothing.
pub fn spring(ctx: &egui::Context, id: Id, target: f32, motion: Motion) -> f32 {
    if prefers_reduced_motion() {
        return target;
    }

    let dt = ctx.input(|i| i.stable_dt).clamp(1.0 / 240.0, 1.0 / 15.0);
    let mut state = ctx
        .data_mut(|d| d.get_temp::<Spring>(id))
        .unwrap_or_else(|| Spring::at(target));

    let value = state.advance(target, motion, dt);
    let settled = state.is_settled(target);
    ctx.data_mut(|d| d.insert_temp(id, state));

    if !settled {
        ctx.request_repaint();
    }
    value
}

/// Read a spring's current presentation value without advancing it.
///
/// Immediate mode builds a container's contents before it knows whether the
/// container is hovered, so a card that wants to nudge something as the pointer
/// arrives reads last frame's value. One frame of lag is below perception; a
/// frame of missing feedback is not.
pub fn peek_spring(ctx: &egui::Context, id: Id) -> f32 {
    ctx.data_mut(|d| d.get_temp::<Spring>(id)).map_or(0.0, |s| s.value)
}

/// Progressive resistance past a boundary.
///
/// A hard stop reads as "frozen". Continuous resistance reads as "responsive,
/// but there is nothing more here" — real things slow before they stop.
pub fn rubberband(overshoot: f32, dimension: f32, constant: f32) -> f32 {
    (overshoot * dimension * constant) / (dimension + constant * overshoot.abs())
}

/// The scale a control should be drawn at right now, given its press state.
///
/// Feedback lives on pointer-*down* and stays continuous for as long as the
/// button is held; waiting for the click to land feels dead.
pub fn press_scale(ui: &egui::Ui, response: &egui::Response) -> f32 {
    let target = if response.is_pointer_button_down_on() { 0.97 } else { 1.0 };
    spring(ui.ctx(), response.id.with("press"), target, Motion::PRESS)
}

/// A 0..1 hover weight for tinting and lifting a control under the pointer.
pub fn hover_weight(ui: &egui::Ui, response: &egui::Response) -> f32 {
    let target = if response.hovered() { 1.0 } else { 0.0 };
    spring(ui.ctx(), response.id.with("hover"), target, Motion::UI)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critically_damped_spring_never_overshoots() {
        let mut s = Spring::at(0.0);
        let mut max = 0.0_f32;
        for _ in 0..600 {
            max = max.max(s.advance(1.0, Motion::UI, 1.0 / 120.0));
        }
        assert!(max <= 1.0 + 1.0e-3, "critically damped spring overshot to {max}");
        assert!(s.is_settled(1.0), "spring failed to settle");
    }

    #[test]
    fn underdamped_sheet_spring_carries_momentum_past_the_target() {
        let mut s = Spring::at(0.0);
        let mut max = 0.0_f32;
        for _ in 0..600 {
            max = max.max(s.advance(1.0, Motion::SHEET, 1.0 / 120.0));
        }
        assert!(max > 1.0, "sheet spring should overshoot a little");
        assert!(max < 1.15, "overshoot should be a hint of bounce, not a bounce");
    }

    #[test]
    fn retargeting_preserves_the_presentation_value() {
        let mut s = Spring::at(0.0);
        for _ in 0..10 {
            s.advance(1.0, Motion::UI, 1.0 / 120.0);
        }
        let mid = s.value;
        // Reversing mid-flight must continue from where the element actually is.
        let next = s.advance(0.0, Motion::UI, 1.0 / 120.0);
        assert!((next - mid).abs() < 0.05, "interrupting the spring caused a visible jump");
    }

    #[test]
    fn rubberband_resists_progressively() {
        let near = rubberband(20.0, 300.0, 0.55);
        let far = rubberband(200.0, 300.0, 0.55);
        assert!(near < 20.0 && far < 200.0, "a boundary must resist, not follow 1:1");
        assert!(far / 200.0 < near / 20.0, "resistance must grow with overshoot");
    }
}
