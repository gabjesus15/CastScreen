//! The update affordance, in one place.
//!
//! Both the launcher and the sender surface exactly the same thing — "there is
//! a newer CastScreen, here is what changed, install it" — so it is built once.
//! Things that look the same must behave the same and live in the same place;
//! two copies of this drift apart within one release.
//!
//! The chip states are deliberately four different kinds of feedback rather
//! than four colours of the same one: **status** (up to date), **status with an
//! offer** (a version is available), **ongoing** (downloading, with progress),
//! and **completion** (ready to install).

use eframe::egui;

use crate::theme::{
    button, chip, material, sheet, sheet_header, space, text as ty, ButtonStyle, Dismiss, Layer,
    ACCENT_BRAND, ACCENT_DANGER, ACCENT_LIVE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use crate::updater::{AppUpdater, UpdateState};

/// The update chip for a top bar. Returns `true` if the user asked to see more.
///
/// When nothing is happening this is a quiet status line, not a button — an
/// affordance that leads nowhere is noise.
pub fn update_chip(ui: &mut egui::Ui, state: &UpdateState) -> bool {
    match state {
        UpdateState::UpdateAvailable { version, .. } => button(
            ui,
            ty::CAPTION.text(format!("Actualizar a {version}")),
            ButtonStyle::Tinted(ACCENT_LIVE),
        )
        .clicked(),
        UpdateState::Downloading { progress_pct, .. } => button(
            ui,
            ty::CAPTION.text(format!("Descargando {:.0} %", progress_pct * 100.0)),
            ButtonStyle::Tinted(ACCENT_BRAND),
        )
        .clicked(),
        UpdateState::ReadyToInstall { version, .. } => button(
            ui,
            ty::CAPTION.text(format!("Instalar {version}")),
            ButtonStyle::Primary(ACCENT_LIVE),
        )
        .clicked(),
        UpdateState::UpToDate => {
            chip(ui, "Versión al día", ACCENT_LIVE);
            false
        }
        UpdateState::Checking => {
            ui.label(ty::CAPTION.colored("Buscando actualizaciones…", TEXT_MUTED));
            false
        }
        UpdateState::Error(_) => {
            chip(ui, "No se pudo comprobar la versión", TEXT_MUTED);
            false
        }
        _ => false,
    }
}

/// The update sheet.
///
/// Installing replaces the running program, so this sheet is
/// [`Dismiss::Deliberate`]: it closes through its own buttons, never by a stray
/// click outside. Every state still offers a way out, because a dialog with no
/// exit is a trap.
pub fn update_sheet(ctx: &egui::Context, updater: &AppUpdater, open: &mut bool) {
    let state = updater.get_state();

    sheet(
        ctx,
        "castscreen-update-sheet",
        open,
        |ui| match &state {
            UpdateState::UpdateAvailable { version, title, notes, asset_size, .. } => {
                sheet_header(
                    ui,
                    &format!("CastScreen {version}"),
                    "Se descargará e instalará desde GitHub. Tu configuración se conserva.",
                );
                ui.label(ty::BODY_EMPHASIS.colored(title, TEXT_PRIMARY));
                ui.add_space(space::SM);

                ui.label(ty::OVERLINE.colored("NOVEDADES", TEXT_MUTED));
                ui.add_space(space::XS);
                material(Layer::Raised).show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                        ui.label(ty::CALLOUT.colored(notes, TEXT_SECONDARY));
                    });
                });

                ui.add_space(space::MD);
                ui.label(ty::CAPTION.colored(
                    format!("Instalador: {:.1} MB", *asset_size as f64 / (1024.0 * 1024.0)),
                    TEXT_MUTED,
                ));
                ui.add_space(space::LG);

                ui.horizontal(|ui| {
                    if button(ui, "Descargar e instalar", ButtonStyle::Primary(ACCENT_LIVE)).clicked() {
                        updater.start_download();
                    }
                    button(ui, "Ahora no", ButtonStyle::Quiet).clicked()
                })
                .inner
            }

            UpdateState::Downloading { version, downloaded_bytes, total_bytes, progress_pct } => {
                sheet_header(
                    ui,
                    &format!("Descargando {version}"),
                    "Puedes seguir usando CastScreen mientras tanto.",
                );
                ui.add(egui::ProgressBar::new(*progress_pct).show_percentage().animate(true));
                ui.add_space(space::SM);
                ui.label(ty::CAPTION.mono_colored(
                    format!(
                        "{:.1} MB de {:.1} MB",
                        *downloaded_bytes as f64 / (1024.0 * 1024.0),
                        *total_bytes as f64 / (1024.0 * 1024.0)
                    ),
                    TEXT_SECONDARY,
                ));
                ui.add_space(space::LG);
                button(ui, "Seguir en segundo plano", ButtonStyle::Plain).clicked()
            }

            UpdateState::ReadyToInstall { version, .. } => {
                sheet_header(
                    ui,
                    &format!("{version} está lista"),
                    "CastScreen se cerrará y el instalador se abrirá solo. No necesitas el navegador.",
                );
                ui.horizontal(|ui| {
                    if button(ui, "Instalar y reiniciar", ButtonStyle::Primary(ACCENT_LIVE)).clicked() {
                        let _ = updater.install_and_restart();
                    }
                    button(ui, "Más tarde", ButtonStyle::Quiet).clicked()
                })
                .inner
            }

            UpdateState::Error(err) => {
                sheet_header(ui, "No se pudo actualizar", err);
                ui.horizontal(|ui| {
                    if button(ui, "Reintentar", ButtonStyle::Tinted(ACCENT_DANGER)).clicked() {
                        updater.check_for_updates();
                    }
                    button(ui, "Cerrar", ButtonStyle::Quiet).clicked()
                })
                .inner
            }

            _ => {
                sheet_header(ui, "Buscando actualizaciones", "Consultando las versiones publicadas en GitHub.");
                button(ui, "Cerrar", ButtonStyle::Quiet).clicked()
            }
        },
        Dismiss::Deliberate,
    );
}
