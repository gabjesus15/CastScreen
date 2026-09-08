//! Dark Studio Design System & Theme for CastScreen.
//!
//! Provides Dribbble-grade professional aesthetics:
//! - Deep OLED carbon backgrounds (#0B0E14)
//! - Crisp 1px borders (#22283A)
//! - Ballistic stereo VU meters with peak-hold
//! - Pulsing live broadcast badges
//! - Wi-Fi buffer stability gauges

use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Stroke, Vec2, Visuals};

pub const BG_CANVAS: Color32 = Color32::from_rgb(11, 14, 20); // #0B0E14
pub const BG_PANEL: Color32 = Color32::from_rgb(19, 23, 34); // #131722
pub const BG_CONTROL: Color32 = Color32::from_rgb(27, 32, 48); // #1B2030
pub const BG_HOVER: Color32 = Color32::from_rgb(35, 41, 61); // #23293D
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(34, 40, 58); // #22283A

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(249, 250, 251); // #F9FAFB
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(156, 163, 175); // #9CA3AF
pub const TEXT_MUTED: Color32 = Color32::from_rgb(99, 107, 126); // #636B7E

pub const ACCENT_BRAND: Color32 = Color32::from_rgb(99, 102, 241); // #6366F1
pub const ACCENT_LIVE: Color32 = Color32::from_rgb(16, 185, 129); // #10B981
pub const ACCENT_WARN: Color32 = Color32::from_rgb(245, 158, 11); // #F59E0B
pub const ACCENT_DANGER: Color32 = Color32::from_rgb(239, 68, 68); // #EF4444

/// Configures the Dark Studio theme on the egui context.
pub fn configure_dark_studio_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.panel_fill = BG_CANVAS;
    visuals.window_fill = BG_PANEL;
    visuals.faint_bg_color = BG_PANEL;
    visuals.extreme_bg_color = BG_CANVAS;

    // Window & Card styling
    visuals.window_rounding = Rounding::same(10.0);
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);

    // Widget styling (Buttons, Sliders)
    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

    visuals.widgets.inactive.bg_fill = BG_CONTROL;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.inactive.rounding = Rounding::same(6.0);

    visuals.widgets.hovered.bg_fill = BG_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT_BRAND);
    visuals.widgets.hovered.rounding = Rounding::same(6.0);

    visuals.widgets.active.bg_fill = ACCENT_BRAND;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT_BRAND);
    visuals.widgets.active.rounding = Rounding::same(6.0);

    ctx.set_visuals(visuals);
}

/// Draws an animated "LIVE" broadcast status badge.
pub fn draw_live_badge(ui: &mut egui::Ui, is_live: bool, elapsed_seconds: u64) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
        let center = rect.center();

        if is_live {
            let time = ui.input(|i| i.time);
            let pulse = (time * 3.0).sin().abs() as f32; // 0.0 to 1.0 pulse
            let alpha = (140.0 + pulse * 115.0) as u8;

            // Outer glow
            ui.painter().circle_filled(
                center,
                6.5 + pulse * 1.5,
                Color32::from_rgba_unmultiplied(16, 185, 129, (40.0 + pulse * 40.0) as u8),
            );
            // Core circle
            ui.painter().circle_filled(
                center,
                4.5,
                Color32::from_rgba_unmultiplied(16, 185, 129, alpha),
            );

            let hours = elapsed_seconds / 3600;
            let mins = (elapsed_seconds % 3600) / 60;
            let secs = elapsed_seconds % 60;
            let time_str = format!("{:02}:{:02}:{:02}", hours, mins, secs);

            ui.colored_label(ACCENT_LIVE, egui::RichText::new("EN VIVO").strong().size(12.0));
            ui.colored_label(
                TEXT_SECONDARY,
                egui::RichText::new(format!("[ {} ]", time_str))
                    .monospace()
                    .size(12.0),
            );
        } else {
            ui.painter().circle_filled(center, 4.0, TEXT_MUTED);
            ui.colored_label(
                TEXT_MUTED,
                egui::RichText::new("LISTO PARA TRANSMITIR").size(12.0),
            );
        }
    });
}

/// Draws a studio-grade segmented ballistic stereo VU meter.
pub fn draw_ballistic_vu_meter(
    ui: &mut egui::Ui,
    left_peak: f32,
    right_peak: f32,
    width: f32,
    height: f32,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter();

    // Background track
    painter.rect_filled(rect, Rounding::same(3.0), BG_CONTROL);
    painter.rect_stroke(rect, Rounding::same(3.0), Stroke::new(1.0_f32, BORDER_SUBTLE));

    let bar_height = (height - 3.0) / 2.0;
    let max_fill_width = width - 4.0;

    let draw_channel = |y_offset: f32, peak: f32| {
        let clamped_peak = peak.clamp(0.0, 1.0);
        let fill_w = max_fill_width * clamped_peak;
        let channel_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 2.0, rect.min.y + y_offset),
            Vec2::new(fill_w, bar_height),
        );

        if fill_w > 0.0 {
            // Gradient color based on intensity
            let fill_color = if clamped_peak > 0.90 {
                ACCENT_DANGER // > -1 dB clip
            } else if clamped_peak > 0.70 {
                ACCENT_WARN // -6 to -1 dB
            } else {
                ACCENT_LIVE // Nominal safe zone
            };
            painter.rect_filled(channel_rect, Rounding::same(2.0), fill_color);
        }
    };

    // Left Channel (Top)
    draw_channel(1.0, left_peak);
    // Right Channel (Bottom)
    draw_channel(bar_height + 2.0, right_peak);
}

/// Draws a Wi-Fi buffer stability bar (e.g. 0 to 1,000 ms).
pub fn draw_buffer_health_bar(ui: &mut egui::Ui, current_ms: u32, max_ms: u32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 10.0), egui::Sense::hover());
    let painter = ui.painter();

    painter.rect_filled(rect, Rounding::same(3.0), BG_CONTROL);
    painter.rect_stroke(rect, Rounding::same(3.0), Stroke::new(1.0_f32, BORDER_SUBTLE));

    let ratio = (current_ms as f32 / max_ms as f32).clamp(0.0, 1.0);
    let fill_w = (width - 2.0) * ratio;

    let fill_color = if ratio > 0.85 {
        ACCENT_LIVE // Buffer healthy
    } else if ratio > 0.50 {
        ACCENT_WARN // Re-transmitting packets
    } else {
        ACCENT_DANGER // Low buffer margin
    };

    if fill_w > 0.0 {
        let fill_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 1.0, rect.min.y + 1.0),
            Vec2::new(fill_w, 8.0),
        );
        painter.rect_filled(fill_rect, Rounding::same(2.0), fill_color);
    }
}
