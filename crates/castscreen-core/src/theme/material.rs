//! Surfaces, depth and spacing — the material system.
//!
//! `egui` has no backdrop blur, so translucency is expressed the way it reads
//! anyway: as **material weight**. Structural regions sit heavier and darker;
//! interactive surfaces sit lighter and lifted. The rules that survive the port
//! from glass to flat layers are the ones that actually carry the hierarchy:
//!
//! - **Bigger surfaces read as thicker** — a sheet gets a deeper, softer shadow
//!   than a chip, because a thicker material sits further off the page.
//! - **A bright top edge is light catching the material.** One hairline along
//!   the top of a raised surface does more for depth than a full 1px box.
//! - **Never stack a lighter surface on a lighter surface.** [`Layer::Raised`]
//!   belongs inside [`Layer::Surface`], never inside another `Raised`.
//! - **Dim to focus, separate to keep flow.** A modal task pairs its surface
//!   with a scrim; a parallel panel uses offset and weight without one.
//! - **Scroll edge effects, not hard dividers.** Where content slides under
//!   floating chrome, fade it — a 1px rule there is a seam, not a boundary.

use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Stroke, Vec2};
use eframe::epaint::{Mesh, Shadow};

// ── Ground and materials ────────────────────────────────────────────────────

/// The deepest ground. Everything else floats above it.
pub const BG_CANVAS: Color32 = Color32::from_rgb(11, 14, 20);
/// Structural chrome: title bars and docks that frame the whole window.
pub const BG_CHROME: Color32 = Color32::from_rgb(16, 20, 30);
/// Content surfaces: cards and panels.
pub const BG_PANEL: Color32 = Color32::from_rgb(19, 23, 34);
/// Raised interactive surfaces: rows, fields, controls.
pub const BG_CONTROL: Color32 = Color32::from_rgb(27, 32, 48);
/// A raised surface under the pointer.
pub const BG_HOVER: Color32 = Color32::from_rgb(35, 41, 61);
/// Sheets and popovers — the topmost material.
pub const BG_OVERLAY: Color32 = Color32::from_rgb(26, 31, 46);

/// The hairline that separates materials of similar weight.
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(34, 40, 58);
/// Light catching the top edge of a raised material.
pub const EDGE_SPECULAR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 12);

// ── Text ────────────────────────────────────────────────────────────────────

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(249, 250, 251);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(156, 163, 175);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(99, 107, 126);

// ── Accents ─────────────────────────────────────────────────────────────────

/// The product's own colour. Reserved for identity and the primary path.
pub const ACCENT_BRAND: Color32 = Color32::from_rgb(99, 102, 241);
/// Healthy, live, connected.
pub const ACCENT_LIVE: Color32 = Color32::from_rgb(16, 185, 129);
/// Degraded but working — a warning, not a failure.
pub const ACCENT_WARN: Color32 = Color32::from_rgb(245, 158, 11);
/// Failure, or an action that cuts a live stream.
pub const ACCENT_DANGER: Color32 = Color32::from_rgb(239, 68, 68);

// ── Spacing and radius ──────────────────────────────────────────────────────

/// A single spacing scale. Every gap in the app is one of these values, so no
/// spacing is ever an arbitrary number someone typed once.
pub mod space {
    /// Hairline separation inside a row.
    pub const XXS: f32 = 2.0;
    /// Between tightly related items (label and its value).
    pub const XS: f32 = 4.0;
    /// Between items in a group.
    pub const SM: f32 = 8.0;
    /// Default padding inside a control.
    pub const MD: f32 = 12.0;
    /// Padding inside a card; gap between groups.
    pub const LG: f32 = 16.0;
    /// Between major sections.
    pub const XL: f32 = 24.0;
    /// Padding inside a sheet.
    pub const XXL: f32 = 32.0;
}

/// Corner radii, scaled to the surface. A big surface with a small radius reads
/// as thin; a small control with a large one reads as a pill.
pub mod radius {
    pub const CHIP: f32 = 999.0;
    pub const CONTROL: f32 = 7.0;
    pub const ROW: f32 = 9.0;
    pub const CARD: f32 = 12.0;
    pub const SHEET: f32 = 16.0;
}

/// The material a surface is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Structural chrome framing the window: top bar, bottom dock.
    Chrome,
    /// A content card or panel.
    Surface,
    /// A raised interactive surface inside a `Surface`.
    Raised,
    /// A sheet or popover above everything.
    Overlay,
}

impl Layer {
    pub fn fill(self) -> Color32 {
        match self {
            Self::Chrome => BG_CHROME,
            Self::Surface => BG_PANEL,
            Self::Raised => BG_CONTROL,
            Self::Overlay => BG_OVERLAY,
        }
    }

    pub fn rounding(self) -> f32 {
        match self {
            Self::Chrome => 0.0,
            Self::Surface => radius::CARD,
            Self::Raised => radius::ROW,
            Self::Overlay => radius::SHEET,
        }
    }

    pub fn padding(self) -> f32 {
        match self {
            Self::Chrome => space::MD,
            Self::Surface => space::LG,
            Self::Raised => space::MD,
            Self::Overlay => space::XL,
        }
    }

    /// Bigger surfaces are thicker, so they cast further and softer.
    pub fn shadow(self) -> Shadow {
        match self {
            Self::Chrome => Shadow {
                offset: Vec2::new(0.0, 2.0),
                blur: 14.0,
                spread: 0.0,
                color: Color32::from_black_alpha(70),
            },
            Self::Surface => Shadow {
                offset: Vec2::new(0.0, 4.0),
                blur: 22.0,
                spread: 0.0,
                color: Color32::from_black_alpha(60),
            },
            Self::Raised => Shadow {
                offset: Vec2::new(0.0, 1.0),
                blur: 6.0,
                spread: 0.0,
                color: Color32::from_black_alpha(40),
            },
            Self::Overlay => Shadow {
                offset: Vec2::new(0.0, 16.0),
                blur: 52.0,
                spread: 0.0,
                color: Color32::from_black_alpha(130),
            },
        }
    }
}

/// A frame carrying this material's fill, hairline, radius, padding and depth.
pub fn material(layer: Layer) -> egui::Frame {
    egui::Frame::none()
        .fill(layer.fill())
        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
        .rounding(Rounding::same(layer.rounding()))
        .inner_margin(layer.padding())
        .shadow(layer.shadow())
}

/// The same material, outlined in an accent — for a surface whose *state* is
/// the point (connected, selected, erroring) rather than its content.
pub fn material_accented(layer: Layer, accent: Color32) -> egui::Frame {
    material(layer).stroke(Stroke::new(1.0_f32, accent))
}

/// Draw the specular hairline along the top edge of a raised surface.
///
/// One bright edge reads as a real material catching light; a full bright box
/// reads as an outline.
pub fn specular_edge(painter: &egui::Painter, rect: Rect, rounding: f32) {
    let inset = rounding * 0.5;
    painter.line_segment(
        [
            Pos2::new(rect.min.x + inset, rect.min.y + 0.5),
            Pos2::new(rect.max.x - inset, rect.min.y + 0.5),
        ],
        Stroke::new(1.0_f32, EDGE_SPECULAR),
    );
}

/// Blend `tint` into `base` by `weight` (0..1).
pub fn blend(base: Color32, tint: Color32, weight: f32) -> Color32 {
    let w = weight.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * w).round() as u8;
    Color32::from_rgb(
        mix(base.r(), tint.r()),
        mix(base.g(), tint.g()),
        mix(base.b(), tint.b()),
    )
}

/// `color` at `alpha` (0..1), for tinted fills over a known material.
pub fn with_alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (alpha.clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// A soft fade where scrolling content passes beneath floating chrome.
///
/// This replaces the 1px divider under a sticky header: content dissolves into
/// the chrome instead of being cut off by it.
pub fn scroll_edge(painter: &egui::Painter, rect: Rect, from: Color32, height: f32) {
    let height = height.min(rect.height());
    let top = rect.min.y;
    let bottom = top + height;
    let transparent = Color32::from_rgba_unmultiplied(from.r(), from.g(), from.b(), 0);

    let mut mesh = Mesh::default();
    mesh.colored_vertex(Pos2::new(rect.min.x, top), from);
    mesh.colored_vertex(Pos2::new(rect.max.x, top), from);
    mesh.colored_vertex(Pos2::new(rect.min.x, bottom), transparent);
    mesh.colored_vertex(Pos2::new(rect.max.x, bottom), transparent);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(mesh);
}

/// Dim everything behind a modal task, so focus lands on the sheet.
///
/// Use only for tasks that genuinely block. A parallel, non-blocking panel gets
/// weight and offset instead — a scrim there would break the flow.
pub fn scrim(ctx: &egui::Context, opacity: f32) {
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("castscreen-scrim"),
    ));
    painter.rect_filled(
        screen,
        Rounding::ZERO,
        Color32::from_black_alpha((opacity.clamp(0.0, 1.0) * 165.0) as u8),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f32 {
        0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32
    }

    #[test]
    fn materials_get_lighter_as_they_rise_off_the_ground() {
        assert!(luminance(BG_CANVAS) < luminance(BG_CHROME));
        assert!(luminance(BG_CHROME) < luminance(BG_PANEL));
        assert!(luminance(BG_PANEL) < luminance(BG_CONTROL));
        assert!(luminance(BG_CONTROL) < luminance(BG_HOVER));
    }

    #[test]
    fn bigger_surfaces_read_as_thicker() {
        let raised = Layer::Raised.shadow();
        let surface = Layer::Surface.shadow();
        let overlay = Layer::Overlay.shadow();
        assert!(raised.blur < surface.blur);
        assert!(surface.blur < overlay.blur);
        assert!(raised.offset.y < overlay.offset.y);
    }

    #[test]
    fn radius_scales_with_the_surface() {
        assert!(Layer::Raised.rounding() < Layer::Surface.rounding());
        assert!(Layer::Surface.rounding() < Layer::Overlay.rounding());
    }

    #[test]
    fn blend_moves_toward_the_tint() {
        assert_eq!(blend(BG_CONTROL, ACCENT_LIVE, 0.0), BG_CONTROL);
        assert_eq!(blend(BG_CONTROL, ACCENT_LIVE, 1.0), ACCENT_LIVE);
        let half = blend(BG_CONTROL, ACCENT_LIVE, 0.5);
        assert!(half.g() > BG_CONTROL.g() && half.g() < ACCENT_LIVE.g());
    }
}
