//! Live readouts: meters, gauges, the broadcast badge, the mark.
//!
//! These are the parts of the interface that move on their own, so restraint
//! matters most here.
//!
//! - **A meter must rise instantly and fall ballistically.** Smoothing the rise
//!   would be lying about a transient; smoothing the fall is what makes a level
//!   readable. Peak-hold keeps the loudest moment visible long enough to catch.
//! - **A gauge is not a meter.** Buffer health changes slowly and is read at a
//!   glance, so it is spring-smoothed — a strobing bar is unreadable and reads
//!   as instability that is not there.
//! - **Status is a fact, not an animation.** The live dot is solid; only a
//!   faint halo breathes, and only when the system allows animation at all.

use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};

use super::material::{
    with_alpha, ACCENT_BRAND, ACCENT_DANGER, ACCENT_LIVE, ACCENT_WARN, BG_CANVAS, BG_CONTROL,
    BORDER_SUBTLE, TEXT_MUTED, TEXT_SECONDARY,
};
use super::motion::{prefers_reduced_motion, spring, Motion};
use super::typography as ty;

/// Level above which the signal is about to clip (~-1 dBFS).
const ZONE_CLIP: f32 = 0.90;
/// Level above which the signal is hot but safe (~-6 dBFS).
const ZONE_HOT: f32 = 0.70;
/// How long the loudest recent peak stays pinned, in seconds.
const PEAK_HOLD_SECONDS: f32 = 1.2;
/// How fast the held peak falls once it lets go, in units per second.
const PEAK_FALL_RATE: f32 = 0.5;

#[derive(Clone, Copy, Default)]
struct PeakHold {
    level: f32,
    held_for: f32,
}

impl PeakHold {
    fn update(&mut self, peak: f32, dt: f32) -> f32 {
        if peak >= self.level {
            // Rise is instantaneous: a transient the meter smooths is a
            // transient the engineer never sees.
            self.level = peak;
            self.held_for = 0.0;
        } else {
            self.held_for += dt;
            if self.held_for > PEAK_HOLD_SECONDS {
                self.level = (self.level - PEAK_FALL_RATE * dt).max(peak);
            }
        }
        self.level
    }
}

fn zone_color(level: f32) -> Color32 {
    if level > ZONE_CLIP {
        ACCENT_DANGER
    } else if level > ZONE_HOT {
        ACCENT_WARN
    } else {
        ACCENT_LIVE
    }
}

/// A stereo meter with instant rise, ballistic fall and peak-hold.
///
/// The zone boundaries are drawn as ticks, so the colour change is mapped to
/// something visible rather than happening for reasons the user has to guess.
pub fn draw_vu_meter(ui: &mut egui::Ui, left_peak: f32, right_peak: f32, width: f32, height: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let dt = ui.input(|i| i.stable_dt).clamp(1.0 / 240.0, 1.0 / 15.0);
    let painter = ui.painter();
    let rounding = Rounding::same(3.0);

    painter.rect_filled(rect, rounding, BG_CANVAS);
    painter.rect_stroke(rect, rounding, Stroke::new(1.0_f32, BORDER_SUBTLE));

    let inset = 1.5;
    let track = rect.shrink(inset);
    let channel_h = (track.height() - 1.0) / 2.0;

    for (index, peak) in [left_peak, right_peak].into_iter().enumerate() {
        let peak = peak.clamp(0.0, 1.0);
        let id = response.id.with(("vu-peak", index));
        let mut hold = ui.ctx().data_mut(|d| d.get_temp::<PeakHold>(id)).unwrap_or_default();
        let held = hold.update(peak, dt);
        ui.ctx().data_mut(|d| d.insert_temp(id, hold));

        let y = track.min.y + index as f32 * (channel_h + 1.0);
        let bar = Rect::from_min_size(
            Pos2::new(track.min.x, y),
            Vec2::new(track.width() * peak, channel_h),
        );
        if bar.width() > 0.5 {
            painter.rect_filled(bar, Rounding::same(2.0), zone_color(peak));
        }

        // The held peak is a thin marker, not a second bar: it reports history,
        // so it must never be mistaken for the current level.
        if held > 0.01 {
            let x = track.min.x + track.width() * held;
            painter.line_segment(
                [Pos2::new(x, y), Pos2::new(x, y + channel_h)],
                Stroke::new(1.5_f32, with_alpha(zone_color(held), 0.85)),
            );
        }
    }

    for boundary in [ZONE_HOT, ZONE_CLIP] {
        let x = track.min.x + track.width() * boundary;
        painter.line_segment(
            [Pos2::new(x, rect.min.y + 1.0), Pos2::new(x, rect.max.y - 1.0)],
            Stroke::new(1.0_f32, with_alpha(Color32::BLACK, 0.45)),
        );
    }

    if left_peak > 0.0 || right_peak > 0.0 {
        ui.ctx().request_repaint();
    }
}

/// Kept for callers that still use the previous name.
#[deprecated(note = "renamed to draw_vu_meter")]
pub fn draw_ballistic_vu_meter(
    ui: &mut egui::Ui,
    left_peak: f32,
    right_peak: f32,
    width: f32,
    height: f32,
) {
    draw_vu_meter(ui, left_peak, right_peak, width, height);
}

/// A slow gauge — buffer occupancy, throughput headroom.
///
/// Spring-smoothed rather than ballistic: this is read at a glance, and a bar
/// that flickers between frames reads as instability the link does not have.
pub fn draw_buffer_health_bar(ui: &mut egui::Ui, current_ms: u32, max_ms: u32, width: f32) {
    let height = 8.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let target = (current_ms as f32 / max_ms.max(1) as f32).clamp(0.0, 1.0);
    let ratio = spring(ui.ctx(), response.id.with("gauge"), target, Motion::READOUT);

    let rounding = Rounding::same(height * 0.5);
    let painter = ui.painter();
    painter.rect_filled(rect, rounding, BG_CONTROL);

    let fill_color = if ratio > 0.85 {
        ACCENT_LIVE
    } else if ratio > 0.50 {
        ACCENT_WARN
    } else {
        ACCENT_DANGER
    };

    if ratio > 0.01 {
        let fill = Rect::from_min_size(rect.min, Vec2::new(rect.width() * ratio, height));
        painter.rect_filled(fill, rounding, fill_color);
    }
}

/// The broadcast status badge.
///
/// Live or not live is a fact, so the dot itself is solid at full strength. The
/// only motion is a slow halo breath that tells you the readout is live rather
/// than frozen — and it stops entirely under reduced motion, where the solid
/// dot and its label already carry the whole message.
pub fn draw_live_badge(ui: &mut egui::Ui, is_live: bool, elapsed_seconds: u64) {
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
        let centre = rect.center();

        // Springing the transition means going live is one continuous motion,
        // and going off air reverses along the same path.
        let live = spring(
            ui.ctx(),
            response.id.with("live"),
            is_live as u32 as f32,
            Motion::UI,
        );
        let color = super::material::blend(TEXT_MUTED, ACCENT_LIVE, live);

        if is_live && !prefers_reduced_motion() {
            let breath = 0.5 + 0.5 * (ui.input(|i| i.time) * 1.9).sin() as f32;
            ui.painter().circle_filled(
                centre,
                5.0 + 2.5 * breath,
                with_alpha(ACCENT_LIVE, 0.10 + 0.10 * breath),
            );
            ui.ctx().request_repaint();
        }
        ui.painter().circle_filled(centre, 4.0, color);

        if is_live {
            let hours = elapsed_seconds / 3600;
            let mins = (elapsed_seconds % 3600) / 60;
            let secs = elapsed_seconds % 60;
            ui.label(ty::CAPTION.colored("EN VIVO", ACCENT_LIVE));
            ui.label(ty::CAPTION.mono_colored(
                format!("{hours:02}:{mins:02}:{secs:02}"),
                TEXT_SECONDARY,
            ));
        } else {
            ui.label(ty::CAPTION.colored("Listo para transmitir", TEXT_MUTED));
        }
    });
}

/// The CastScreen mark: a display, and a signal leaving it.
///
/// Drawn as vectors so it stays sharp at any DPI, and built from the same
/// stroke weight throughout so it reads as one drawn object rather than a
/// collection of shapes that happen to sit together.
pub fn draw_castscreen_logo(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();

    let cyan = Color32::from_rgb(0, 242, 254);
    let mint = Color32::from_rgb(0, 245, 160);
    let stroke_w = size * 0.065;

    painter.rect_filled(rect, Rounding::same(size * 0.24), Color32::from_rgb(14, 17, 24));
    painter.rect_stroke(
        rect,
        Rounding::same(size * 0.24),
        Stroke::new(1.0_f32, BORDER_SUBTLE),
    );

    let screen = Rect::from_min_max(
        Pos2::new(rect.min.x + size * 0.16, rect.min.y + size * 0.22),
        Pos2::new(rect.min.x + size * 0.72, rect.min.y + size * 0.74),
    );
    painter.rect_stroke(screen, Rounding::same(size * 0.10), Stroke::new(stroke_w, cyan));

    let stand_x = rect.min.x + size * 0.44;
    painter.line_segment(
        [
            Pos2::new(stand_x, rect.min.y + size * 0.74),
            Pos2::new(stand_x, rect.min.y + size * 0.85),
        ],
        Stroke::new(stroke_w, ACCENT_BRAND),
    );
    painter.line_segment(
        [
            Pos2::new(rect.min.x + size * 0.32, rect.min.y + size * 0.85),
            Pos2::new(rect.min.x + size * 0.56, rect.min.y + size * 0.85),
        ],
        Stroke::new(stroke_w, ACCENT_BRAND),
    );

    let origin = Pos2::new(rect.min.x + size * 0.64, rect.min.y + size * 0.38);
    for (index, radius) in [size * 0.13, size * 0.20, size * 0.27].into_iter().enumerate() {
        let color = super::material::blend(cyan, mint, index as f32 / 2.0);
        let points: Vec<Pos2> = (0..=10)
            .map(|step| {
                let angle = -std::f32::consts::FRAC_PI_2
                    + (step as f32 / 10.0) * std::f32::consts::FRAC_PI_2;
                Pos2::new(
                    origin.x + radius * angle.cos(),
                    origin.y + radius * angle.sin(),
                )
            })
            .collect();
        for pair in points.windows(2) {
            painter.line_segment([pair[0], pair[1]], Stroke::new(stroke_w * 0.85, color));
        }
    }
}

/// The window icon, compiled into the binary.
pub fn load_window_icon() -> egui::IconData {
    let bytes = include_bytes!("../../../../assets/logo.jpg");
    let image = image::load_from_memory(bytes).expect("Failed to load logo.jpg");
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_rises_instantly_and_falls_only_after_the_hold() {
        let mut hold = PeakHold::default();
        assert_eq!(hold.update(0.9, 1.0 / 60.0), 0.9, "a transient must be shown at once");

        // Silence right after the peak: the marker stays pinned through the hold.
        let mut elapsed = 0.0;
        while elapsed < PEAK_HOLD_SECONDS - 0.05 {
            hold.update(0.0, 1.0 / 60.0);
            elapsed += 1.0 / 60.0;
        }
        assert_eq!(hold.level, 0.9, "the held peak fell before its hold expired");

        for _ in 0..120 {
            hold.update(0.0, 1.0 / 60.0);
        }
        assert!(hold.level < 0.9, "the held peak never let go");
    }

    #[test]
    fn zones_map_to_the_right_colours() {
        assert_eq!(zone_color(0.5), ACCENT_LIVE);
        assert_eq!(zone_color(0.8), ACCENT_WARN);
        assert_eq!(zone_color(0.95), ACCENT_DANGER);
    }
}
