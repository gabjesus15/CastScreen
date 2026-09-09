//! Controls that respond the way physical things do.
//!
//! Every control here follows the same three rules:
//!
//! 1. **Feedback on pointer-down, commit on pointer-up.** The moment a control
//!    is pressed it scales down under the finger; the action only fires when the
//!    press is released on the control, so a press can be cancelled by dragging
//!    away. The instant a control waits for the click to acknowledge the press,
//!    directness falls off a cliff.
//! 2. **Feedback is continuous, not terminal.** Hover weight, press scale and
//!    knob position are springs, so they can be grabbed, reversed and
//!    redirected mid-flight without ever jumping.
//! 3. **Mapping is spatial.** A fader tracks the pointer 1:1 from wherever it
//!    was grabbed, and resists rather than stopping dead at its bounds.

use eframe::egui::{
    self, Align2, Color32, Id, Rect, Response, Rounding, Sense, Stroke, TextWrapMode, Vec2,
    WidgetText,
};
use eframe::emath::TSTransform;

use super::material::{
    blend, material, radius, scrim, space, specular_edge, with_alpha, Layer, ACCENT_BRAND,
    BG_CONTROL, BG_HOVER, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY,
};
use super::motion::{hover_weight, press_scale, spring, Motion};
use super::typography as ty;

/// How much presence a control claims.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ButtonStyle {
    /// The primary path. Solid accent, one per region at most.
    Primary(Color32),
    /// A secondary action that still needs colour to name its consequence.
    Tinted(Color32),
    /// The neutral default: a raised material with primary text.
    Plain,
    /// Present but recessive — visible only once the pointer arrives.
    Quiet,
}

impl ButtonStyle {
    fn resting_fill(self) -> Color32 {
        match self {
            Self::Primary(accent) => accent,
            Self::Tinted(accent) => blend(BG_CONTROL, accent, 0.16),
            Self::Plain => BG_CONTROL,
            Self::Quiet => Color32::TRANSPARENT,
        }
    }

    fn hovered_fill(self) -> Color32 {
        match self {
            Self::Primary(accent) => blend(accent, Color32::WHITE, 0.12),
            Self::Tinted(accent) => blend(BG_CONTROL, accent, 0.28),
            Self::Plain => BG_HOVER,
            Self::Quiet => BG_CONTROL,
        }
    }

    fn label_color(self) -> Color32 {
        match self {
            Self::Primary(_) => Color32::WHITE,
            Self::Tinted(accent) => accent,
            Self::Plain => TEXT_PRIMARY,
            Self::Quiet => TEXT_SECONDARY,
        }
    }

    fn stroke(self) -> Color32 {
        match self {
            Self::Primary(accent) => blend(accent, Color32::WHITE, 0.18),
            Self::Tinted(accent) => with_alpha(accent, 0.35),
            Self::Plain => BORDER_SUBTLE,
            Self::Quiet => Color32::TRANSPARENT,
        }
    }
}

/// A button that acknowledges the press itself, not the click.
pub fn button(ui: &mut egui::Ui, text: impl Into<WidgetText>, style: ButtonStyle) -> Response {
    button_sized(ui, text, style, Vec2::ZERO)
}

/// A button with a minimum size — for filling a card's width, or lining up a
/// row of equal-weight actions.
pub fn button_sized(
    ui: &mut egui::Ui,
    text: impl Into<WidgetText>,
    style: ButtonStyle,
    min_size: Vec2,
) -> Response {
    let galley = text
        .into()
        .into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button);

    let padding = Vec2::new(space::MD, space::SM);
    let desired = (galley.size() + padding * 2.0).max(min_size);
    // Hit area is allocated at full size even though the pressed control draws
    // smaller, so shrinking under the finger can never drop the press.
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), galley.text())
    });

    if ui.is_rect_visible(rect) {
        let scale = press_scale(ui, &response);
        let hover = hover_weight(ui, &response);
        let visual = Rect::from_center_size(rect.center(), rect.size() * scale);
        let rounding = Rounding::same(radius::CONTROL);

        let fill = blend(style.resting_fill(), style.hovered_fill(), hover);
        let painter = ui.painter();
        painter.rect_filled(visual, rounding, fill);
        painter.rect_stroke(visual, rounding, Stroke::new(1.0_f32, style.stroke()));
        if !matches!(style, ButtonStyle::Quiet) {
            specular_edge(painter, visual, radius::CONTROL);
        }
        if response.has_focus() {
            painter.rect_stroke(visual.expand(2.0), rounding, Stroke::new(2.0_f32, ACCENT_BRAND));
        }

        let text_pos = visual.center() - galley.size() * 0.5;
        painter.galley(text_pos, galley, style.label_color());
    }

    response
}

/// A switch. The knob is spring-driven, so flipping it twice quickly reverses
/// from wherever the knob actually is rather than snapping and restarting.
pub fn switch(ui: &mut egui::Ui, on: &mut bool, accent: Color32) -> Response {
    let size = Vec2::new(38.0, 22.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());

    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, "")
    });

    if ui.is_rect_visible(rect) {
        let t = spring(ui.ctx(), response.id.with("switch"), *on as u32 as f32, Motion::UI);
        let scale = press_scale(ui, &response);
        let visual = Rect::from_center_size(rect.center(), rect.size() * scale);
        let painter = ui.painter();

        let track = blend(BG_CONTROL, accent, t);
        painter.rect_filled(visual, Rounding::same(visual.height() * 0.5), track);
        painter.rect_stroke(
            visual,
            Rounding::same(visual.height() * 0.5),
            Stroke::new(1.0_f32, blend(BORDER_SUBTLE, accent, t)),
        );

        let knob_r = visual.height() * 0.5 - 3.0;
        let travel = visual.width() - visual.height();
        let knob_x = visual.min.x + visual.height() * 0.5 + travel * t;
        painter.circle_filled(
            egui::Pos2::new(knob_x, visual.center().y),
            knob_r,
            Color32::WHITE,
        );
    }

    response
}

/// A fader that stays under the pointer.
///
/// Grabbing the knob preserves the offset from where it was grabbed, so the
/// knob never teleports to centre itself on the pointer. Dragging past either
/// end resists progressively instead of stopping dead.
pub fn fader(ui: &mut egui::Ui, value: &mut f32, width: f32, accent: Color32) -> Response {
    let height = 20.0;
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());

    let knob_r = 7.0;
    let track = Rect::from_min_max(
        egui::Pos2::new(rect.min.x + knob_r, rect.center().y - 2.5),
        egui::Pos2::new(rect.max.x - knob_r, rect.center().y + 2.5),
    );
    let grab_id = response.id.with("grab-offset");

    if let Some(pointer) = response.interact_pointer_pos() {
        let knob_x = track.min.x + track.width() * value.clamp(0.0, 1.0);

        if response.drag_started() || response.clicked() {
            // Grabbing the knob keeps the offset; grabbing the track jumps to
            // the pointer and then tracks from there.
            let offset = if (pointer.x - knob_x).abs() <= knob_r + 2.0 {
                pointer.x - knob_x
            } else {
                0.0
            };
            ui.ctx().data_mut(|d| d.insert_temp(grab_id, offset));
        }

        let offset = ui.ctx().data_mut(|d| d.get_temp::<f32>(grab_id)).unwrap_or(0.0);
        let raw = (pointer.x - offset - track.min.x) / track.width().max(1.0);
        let clamped = raw.clamp(0.0, 1.0);
        if (clamped - *value).abs() > f32::EPSILON {
            *value = clamped;
            response.mark_changed();
        }

        // Past the end the fader resists — the value is already pinned, so the
        // resistance shows up as the knob easing to a stop rather than freezing.
        let overshoot = if raw < 0.0 { raw } else { (raw - 1.0).max(0.0) };
        ui.ctx()
            .data_mut(|d| d.insert_temp(response.id.with("overshoot"), overshoot));
    }

    response.widget_info(|| {
        egui::WidgetInfo::slider(ui.is_enabled(), *value as f64, "")
    });

    if ui.is_rect_visible(rect) {
        let hover = hover_weight(ui, &response);
        let painter = ui.painter();
        let rounding = Rounding::same(track.height() * 0.5);

        painter.rect_filled(track, rounding, BG_CONTROL);
        let filled = Rect::from_min_max(
            track.min,
            egui::Pos2::new(track.min.x + track.width() * value.clamp(0.0, 1.0), track.max.y),
        );
        painter.rect_filled(filled, rounding, accent);

        let overshoot = ui
            .ctx()
            .data_mut(|d| d.get_temp::<f32>(response.id.with("overshoot")))
            .unwrap_or(0.0);
        let resist = super::motion::rubberband(overshoot, track.width(), 0.55);
        let knob_x = track.min.x + track.width() * value.clamp(0.0, 1.0) + resist;

        // The knob grows a little as the pointer approaches — the control
        // telegraphs that it is grabbable before it is grabbed.
        let r = knob_r + hover * 1.5;
        painter.circle_filled(egui::Pos2::new(knob_x, rect.center().y), r, Color32::WHITE);
        painter.circle_stroke(
            egui::Pos2::new(knob_x, rect.center().y),
            r,
            Stroke::new(1.0_f32, with_alpha(Color32::BLACK, 0.25)),
        );
    }

    response
}

/// A read-only status pill. Proximity does the work: put it beside the thing
/// whose state it reports.
pub fn chip(ui: &mut egui::Ui, text: impl Into<String>, accent: Color32) -> Response {
    let galley = WidgetText::from(ty::CAPTION.text(text.into()))
        .into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Small);
    let padding = Vec2::new(space::SM + 2.0, space::XS);
    let (rect, response) = ui.allocate_exact_size(galley.size() + padding * 2.0, Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, Rounding::same(radius::CHIP), with_alpha(accent, 0.14));
        painter.rect_stroke(
            rect,
            Rounding::same(radius::CHIP),
            Stroke::new(1.0_f32, with_alpha(accent, 0.32)),
        );
        let pos = rect.center() - galley.size() * 0.5;
        painter.galley(pos, galley, accent);
    }
    response
}

/// An all-caps section label. Grouping starts with naming the group.
pub fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(ty::OVERLINE.colored(text.to_uppercase(), super::material::TEXT_MUTED));
    ui.add_space(space::XS);
}

/// How a sheet may be dismissed, which is really a question about consequences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dismiss {
    /// Escape or a click outside closes it. The default: slips should be cheap.
    Casual,
    /// Only the sheet's own buttons close it — for a decision that cuts a live
    /// stream or replaces the running program.
    Deliberate,
}

/// A modal sheet that materialises instead of appearing.
///
/// The sheet scales up from 94% as it fades in and returns along exactly the
/// same path on the way out, so it is obviously the same object arriving and
/// leaving rather than two unrelated events. The background is dimmed by the
/// same spring, so the whole transition is one motion.
///
/// `add_contents` returns `true` to ask the sheet to close, so a button inside
/// the sheet never has to reach back out and borrow the caller's flag.
///
/// Returns `true` while any part of the sheet is still on screen.
pub fn sheet(
    ctx: &egui::Context,
    id: &str,
    open: &mut bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> bool,
    dismiss: Dismiss,
) -> bool {
    let id = Id::new(id);
    let presence = spring(ctx, id.with("presence"), *open as u32 as f32, Motion::SHEET);

    if !*open && presence <= 0.001 {
        return false;
    }

    if dismiss == Dismiss::Casual && *open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
    }

    scrim(ctx, presence);

    let area = egui::Area::new(id.with("area"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_opacity(presence.clamp(0.0, 1.0));
            material(Layer::Overlay)
                .show(ui, |ui| {
                    ui.set_max_width(430.0);
                    add_contents(ui)
                })
                .inner
        });

    let close_requested = area.inner;

    // Scale about the sheet's own centre, from the live spring value, so an
    // interrupted open reverses from wherever the sheet actually is.
    // Scaling happens around the origin, so translate by c·(1−s) to pin the centre.
    let scale = 0.94 + 0.06 * presence;
    let centre = area.response.rect.center().to_vec2();
    ctx.transform_layer_shapes(
        area.response.layer_id,
        TSTransform { scaling: scale, translation: centre * (1.0 - scale) },
    );

    if close_requested {
        *open = false;
    }

    if dismiss == Dismiss::Casual
        && *open
        && ctx.input(|i| i.pointer.any_click())
        && ctx
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|p| !area.response.rect.contains(p))
    {
        *open = false;
    }

    true
}

/// The title block of a sheet: what this is, and what it is about to do.
pub fn sheet_header(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.label(ty::TITLE.colored(title, TEXT_PRIMARY));
    if !detail.is_empty() {
        ui.add_space(space::SM);
        ui.label(ty::BODY.colored(detail, TEXT_SECONDARY));
    }
    ui.add_space(space::LG);
}

/// An interactive card: a whole surface that is the target, not a surface with
/// a target inside it.
///
/// A card that only responds on a small button inside it wastes the obvious
/// affordance — the card *looks* pressable, so it should be. The surface
/// deepens and picks up its accent on pointer-down rather than on click, and
/// the transition is a spring, so moving the pointer across a row of cards is
/// one continuous motion rather than a sequence of snaps.
pub fn card(
    ui: &mut egui::Ui,
    id_source: &str,
    accent: Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) -> Response {
    let id = ui.id().with(id_source);
    let mut prepared = material(Layer::Surface).begin(ui);
    add_contents(&mut prepared.content_ui);

    let rect = prepared
        .content_ui
        .min_rect()
        .expand2(prepared.frame.inner_margin.sum() * 0.5);
    let response = ui.interact(rect, id, Sense::click());

    let hover = hover_weight(ui, &response);
    let engaged = if response.is_pointer_button_down_on() { 1.0 } else { hover };

    prepared.frame.fill = blend(super::material::BG_PANEL, accent, engaged * 0.10);
    prepared.frame.stroke = Stroke::new(1.0_f32, blend(BORDER_SUBTLE, accent, engaged));
    // A lifted surface casts further; a pressed one settles back toward the page.
    let mut shadow = Layer::Surface.shadow();
    shadow.blur += 10.0 * hover;
    shadow.offset.y += 2.0 * hover;
    if response.is_pointer_button_down_on() {
        shadow.blur *= 0.6;
        shadow.offset.y *= 0.4;
    }
    prepared.frame.shadow = shadow;
    prepared.end(ui);

    response
}

/// One line of a feature list inside a card.
///
/// The tick is the accent so the eye can scan capability without reading, and
/// the text stays secondary so the list never competes with the card's title.
pub fn feature_row(ui: &mut egui::Ui, text: &str, accent: Color32) {
    ui.horizontal(|ui| {
        // Painted rather than typed: a tick glyph depends on whichever face
        // the system happens to have, and a missing one shows as tofu.
        let (rect, _) = ui.allocate_exact_size(Vec2::new(9.0, ty::CALLOUT.size), Sense::hover());
        ui.painter().circle_filled(rect.center(), 2.5, accent);
        ui.label(ty::CALLOUT.colored(text, TEXT_SECONDARY));
    });
}

/// A label and its value on one line, value pinned to the right.
///
/// Telemetry is set in mono with tabular figures so a changing number does not
/// make the row shiver, and the pairing puts the value next to the thing it
/// describes rather than in a separate column the eye has to bridge.
pub fn stat_row(ui: &mut egui::Ui, label: &str, value: impl Into<String>, accent: Color32) {
    ui.horizontal(|ui| {
        ui.label(ty::CALLOUT.colored(label, TEXT_SECONDARY));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(ty::CALLOUT.mono_colored(value.into(), accent));
        });
    });
}
