//! The CastScreen design system.
//!
//! One place decides how this product looks and how it moves, because the two
//! are the same decision — you should not be able to tell where the visuals end
//! and the interaction begins.
//!
//! | Module | Owns |
//! | --- | --- |
//! | [`motion`] | Springs, reduced motion, press and hover feedback |
//! | [`typography`] | The type scale: size, tracking, leading and weight as a set |
//! | [`material`] | Surfaces, depth, spacing, radii, scrims and scroll edges |
//! | [`controls`] | Buttons, switches, faders, chips and sheets |
//! | [`meters`] | Meters, gauges, the broadcast badge and the mark |
//!
//! The through-line is that nothing here is arbitrary. Every spacing value is a
//! step on one scale, every radius is chosen by the size of the surface it
//! rounds, every animation is a spring with a named character, and every
//! tracking value belongs to exactly one text size.

pub mod controls;
pub mod material;
pub mod meters;
pub mod motion;
pub mod typography;

pub use controls::{
    button, button_sized, card, chip, fader, feature_row, section_label, sheet, sheet_header,
    stat_row, switch, ButtonStyle, Dismiss,
};
pub use material::{
    blend, material, material_accented, radius, scroll_edge, scrim, space, specular_edge,
    with_alpha, Layer, ACCENT_BRAND, ACCENT_DANGER, ACCENT_LIVE, ACCENT_WARN, BG_CANVAS, BG_CHROME,
    BG_CONTROL, BG_HOVER, BG_OVERLAY, BG_PANEL, BORDER_SUBTLE, EDGE_SPECULAR, TEXT_MUTED,
    TEXT_PRIMARY, TEXT_SECONDARY,
};
#[allow(deprecated)]
pub use meters::draw_ballistic_vu_meter;
pub use meters::{
    draw_buffer_health_bar, draw_castscreen_logo, draw_live_badge, draw_vu_meter, load_window_icon,
};
pub use motion::{
    hover_weight, peek_spring, prefers_reduced_motion, press_scale, rubberband, spring, Motion, Spring,
};
pub use typography as text;

use eframe::egui::{self, Rounding, Stroke, Vec2, Visuals};

/// Apply the design system to an `egui` context.
///
/// Safe to call every frame: the expensive half (loading the system face and
/// binding the type scale) runs once and is then skipped.
pub fn configure_dark_studio_theme(ctx: &egui::Context) {
    let install_id = egui::Id::new("castscreen-design-system-installed");
    let already = ctx.data_mut(|d| d.get_temp::<bool>(install_id)).unwrap_or(false);
    if !already {
        typography::install(ctx);
        ctx.data_mut(|d| d.insert_temp(install_id, true));
    }

    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.panel_fill = BG_CANVAS;
    visuals.window_fill = BG_OVERLAY;
    visuals.faint_bg_color = BG_PANEL;
    visuals.extreme_bg_color = BG_CANVAS;
    visuals.selection.bg_fill = with_alpha(ACCENT_BRAND, 0.35);
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT_BRAND);
    visuals.hyperlink_color = ACCENT_BRAND;

    visuals.window_rounding = Rounding::same(radius::SHEET);
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.window_shadow = Layer::Overlay.shadow();
    visuals.popup_shadow = Layer::Surface.shadow();

    // Widgets that are still stock `egui` inherit the same material language,
    // so a control we have not replaced yet does not look like a different app.
    let control_rounding = Rounding::same(radius::CONTROL);
    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.weak_bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.noninteractive.rounding = control_rounding;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_SECONDARY);

    visuals.widgets.inactive.bg_fill = BG_CONTROL;
    visuals.widgets.inactive.weak_bg_fill = BG_CONTROL;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.inactive.rounding = control_rounding;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);

    visuals.widgets.hovered.bg_fill = BG_HOVER;
    visuals.widgets.hovered.weak_bg_fill = BG_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, with_alpha(ACCENT_BRAND, 0.55));
    visuals.widgets.hovered.rounding = control_rounding;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.hovered.expansion = 0.0;

    visuals.widgets.active.bg_fill = blend(BG_HOVER, ACCENT_BRAND, 0.5);
    visuals.widgets.active.weak_bg_fill = blend(BG_HOVER, ACCENT_BRAND, 0.5);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT_BRAND);
    visuals.widgets.active.rounding = control_rounding;
    // Stock egui grows a pressed widget. Ours shrink under the finger, the way
    // a physical button gives, so the expansion is removed here too.
    visuals.widgets.active.expansion = 0.0;

    visuals.widgets.open.bg_fill = BG_CONTROL;
    visuals.widgets.open.weak_bg_fill = BG_CONTROL;
    visuals.widgets.open.rounding = control_rounding;

    ctx.set_visuals(visuals);

    ctx.style_mut(|style| {
        style.spacing.item_spacing = Vec2::new(space::SM, space::SM);
        style.spacing.button_padding = Vec2::new(space::MD, space::SM);
        style.spacing.menu_margin = egui::Margin::same(space::SM);
        style.spacing.indent = space::LG;
        style.spacing.scroll.bar_width = 6.0;
        style.spacing.scroll.floating = true;

        // Latency is the enemy of directness, and a tooltip that waits a third
        // of a second before answering is latency the user can feel.
        style.interaction.tooltip_delay = 0.1;
        style.interaction.show_tooltips_only_when_still = false;

        // egui's own cross-fades sit alongside our springs; under reduced motion
        // they collapse to a static transition like everything else.
        style.animation_time = if motion::prefers_reduced_motion() { 0.0 } else { 0.08 };
    });
}
