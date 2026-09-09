//! The CastScreen type scale.
//!
//! Three rules from Apple's typography guidance drive every value here:
//!
//! 1. **Tracking is size-specific.** Large text reads too loose as it grows, so
//!    display sizes get negative tracking; small text and all-caps labels get
//!    positive tracking to stay legible. A single `letter-spacing` for the whole
//!    UI is wrong somewhere by definition.
//! 2. **Leading tracks size inversely.** Tight on headings, generous on body,
//!    tightened again for dense telemetry rows.
//! 3. **Hierarchy is weight + size + leading as a set**, not size alone. Weight
//!    adds presence without taking more space, which matters in a window that
//!    has to sit next to a game.
//!
//! The platform system face (Segoe UI) is the default, because it already ships
//! the hinting and legibility tuning a bundled fallback does not. Loading is
//! best-effort: if the faces are missing, `egui`'s own fonts are used unchanged.

use eframe::egui::{self, FontFamily, FontId, RichText};
use std::sync::atomic::{AtomicBool, Ordering};

/// Named family for the semibold cut of the system face.
pub const SEMIBOLD: &str = "castscreen-semibold";

/// Weight is a first-class part of a role, not an afterthought applied by colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Semibold,
}

/// One step of the type scale: a size, its own tracking, its own leading,
/// and the weight that belongs with it.
#[derive(Clone, Copy, Debug)]
pub struct TextRole {
    /// Point size.
    pub size: f32,
    /// Extra letter spacing in points. Negative tightens, positive opens up.
    pub tracking: f32,
    /// Line height as a multiple of the size.
    pub leading: f32,
    pub weight: Weight,
}

impl TextRole {
    const fn new(size: f32, tracking: f32, leading: f32, weight: Weight) -> Self {
        Self { size, tracking, leading, weight }
    }

    fn family(&self) -> FontFamily {
        match self.weight {
            Weight::Regular => FontFamily::Proportional,
            // `egui` panics on an unbound family, and new fonts only take
            // effect on the next frame. Install runs before the first frame,
            // but a caller that skips it degrades to the regular cut rather
            // than taking the whole window down over a font weight.
            Weight::Semibold if semibold_is_bound() => FontFamily::Name(SEMIBOLD.into()),
            Weight::Semibold => FontFamily::Proportional,
        }
    }

    /// Text in this role.
    pub fn text(&self, text: impl Into<String>) -> RichText {
        RichText::new(text)
            .font(FontId::new(self.size, self.family()))
            .extra_letter_spacing(self.tracking)
            .line_height(Some(self.size * self.leading))
    }

    /// Text in this role, in a specific colour.
    pub fn colored(&self, text: impl Into<String>, color: egui::Color32) -> RichText {
        self.text(text).color(color)
    }

    /// Numeric readouts. Tabular figures stop a live counter from shivering as
    /// digits change width, which is the whole reason telemetry gets a mono face.
    pub fn mono(&self, text: impl Into<String>) -> RichText {
        RichText::new(text)
            .font(FontId::new(self.size, FontFamily::Monospace))
            // Mono faces are already open; extra tracking only hurts them.
            .extra_letter_spacing(self.tracking.min(0.0))
            .line_height(Some(self.size * self.leading))
    }

    /// Numeric readout in a specific colour.
    pub fn mono_colored(&self, text: impl Into<String>, color: egui::Color32) -> RichText {
        self.mono(text).color(color)
    }

    /// The `FontId` for this role, for direct painter calls.
    pub fn font_id(&self) -> FontId {
        FontId::new(self.size, self.family())
    }
}

/// Window and screen titles. Tightest tracking in the scale.
pub const DISPLAY: TextRole = TextRole::new(24.0, -0.55, 1.10, Weight::Semibold);
/// Section titles inside a window.
pub const TITLE: TextRole = TextRole::new(18.0, -0.32, 1.18, Weight::Semibold);
/// Card and panel headings.
pub const HEADLINE: TextRole = TextRole::new(14.5, -0.14, 1.28, Weight::Semibold);
/// Default reading size. Tracking sits at zero — body text is already tuned.
pub const BODY: TextRole = TextRole::new(13.0, 0.0, 1.45, Weight::Regular);
/// Body text that carries emphasis.
pub const BODY_EMPHASIS: TextRole = TextRole::new(13.0, 0.0, 1.45, Weight::Semibold);
/// Secondary lines, list rows, hints.
pub const CALLOUT: TextRole = TextRole::new(12.0, 0.06, 1.38, Weight::Regular);
/// Dense metadata and helper text.
pub const CAPTION: TextRole = TextRole::new(11.0, 0.14, 1.34, Weight::Regular);
/// All-caps section labels. Capitals need the widest tracking in the scale.
pub const OVERLINE: TextRole = TextRole::new(10.0, 0.95, 1.30, Weight::Semibold);

/// Set once [`install`] has bound the semibold family on this process's fonts.
static SEMIBOLD_BOUND: AtomicBool = AtomicBool::new(false);

fn semibold_is_bound() -> bool {
    SEMIBOLD_BOUND.load(Ordering::Relaxed)
}

/// Install the system face and bind `egui`'s built-in text styles to the scale,
/// so a bare `ui.label()` already lands in the system rather than outside it.
pub fn install(ctx: &egui::Context) {
    install_system_fonts(ctx);
    SEMIBOLD_BOUND.store(true, Ordering::Relaxed);

    ctx.style_mut(|style| {
        use egui::TextStyle;
        style.text_styles = [
            (TextStyle::Heading, HEADLINE.font_id()),
            (TextStyle::Body, BODY.font_id()),
            (TextStyle::Button, TextRole::new(13.0, 0.0, 1.2, Weight::Semibold).font_id()),
            (TextStyle::Small, CAPTION.font_id()),
            (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
        ]
        .into();
    });
}

/// Best-effort load of Segoe UI (regular + semibold) and Cascadia/Consolas.
///
/// Loaded faces are placed *in front of* `egui`'s defaults rather than replacing
/// them, so the bundled emoji fallback keeps working.
fn install_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let fonts_dir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());
    let fonts_dir = std::path::Path::new(&fonts_dir).join("Fonts");

    let mut load = |key: &str, files: &[&str]| -> bool {
        for file in files {
            if let Ok(bytes) = std::fs::read(fonts_dir.join(file)) {
                fonts
                    .font_data
                    .insert(key.to_owned(), egui::FontData::from_owned(bytes));
                return true;
            }
        }
        false
    };

    let has_regular = load("system-regular", &["segoeui.ttf"]);
    let has_semibold = load("system-semibold", &["seguisb.ttf", "segoeuib.ttf"]);
    let has_mono = load("system-mono", &["CascadiaMono.ttf", "consola.ttf"]);

    if has_regular {
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "system-regular".to_owned());
    }
    if has_mono {
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "system-mono".to_owned());
    }

    // The semibold family always exists, so `Weight::Semibold` is never a
    // missing-font hole; without the real cut it simply falls back to regular.
    let mut semibold_stack = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if has_semibold {
        semibold_stack.insert(0, "system-semibold".to_owned());
    }
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), semibold_stack);

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_opens_up_as_text_gets_smaller() {
        // Large text tightens, body sits at zero, small text and caps open up.
        assert!(DISPLAY.tracking < TITLE.tracking);
        assert!(TITLE.tracking < HEADLINE.tracking);
        assert!(HEADLINE.tracking < BODY.tracking);
        assert_eq!(BODY.tracking, 0.0);
        assert!(CAPTION.tracking > BODY.tracking);
        assert!(OVERLINE.tracking > CAPTION.tracking);
    }

    #[test]
    fn leading_loosens_as_text_gets_smaller() {
        assert!(DISPLAY.leading < TITLE.leading);
        assert!(TITLE.leading < HEADLINE.leading);
        assert!(HEADLINE.leading < BODY.leading);
    }

    #[test]
    fn hierarchy_is_carried_by_weight_as_well_as_size() {
        assert_eq!(HEADLINE.weight, Weight::Semibold);
        assert_eq!(BODY.weight, Weight::Regular);
        assert_eq!(BODY_EMPHASIS.size, BODY.size);
        assert_eq!(BODY_EMPHASIS.weight, Weight::Semibold);
    }
}
