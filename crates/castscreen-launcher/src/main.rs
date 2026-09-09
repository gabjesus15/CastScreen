#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The launcher: one question, two answers.
//!
//! This window exists to resolve a single ambiguity — which half of CastScreen
//! is this computer? Everything on screen serves that question, and anything
//! that does not answer it has been taken out.
//!
//! Two things make the answer easy rather than merely possible:
//!
//! - **The whole card is the target.** A card that looks pressable is
//!   pressable, and it acknowledges the press the instant the pointer goes
//!   down, not when it comes back up.
//! - **The pairing is stated, not assumed.** The reason a launcher is
//!   confusing is that nobody says the two roles are two computers running at
//!   once. So it says so, above the choice.

use anyhow::Result;
use castscreen_core::{
    card, chip, configure_dark_studio_theme, fill_screen, draw_castscreen_logo, feature_row, launch_receiver,
    launch_sender, material, peek_spring, space, text as ty, update_chip, update_sheet, AppUpdater,
    Layer, Fill, SingleInstanceGuard, UpdateState, FILL_SCREEN_FRAMES, ACCENT_BRAND, ACCENT_LIVE, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use eframe::egui::{self, Align, Layout};
use std::env;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    let args: Vec<String> = env::args().collect();

    // Direct mode execution via CLI arguments
    if args.iter().any(|a| a == "--sender" || a == "-s") {
        launch_sender()?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--receiver" || a == "-r") {
        launch_receiver()?;
        return Ok(());
    }

    // Single Instance Guard: Prevent launching multiple launcher windows
    let _guard = SingleInstanceGuard::new("CastScreen_Launcher_Mutex", "CastScreen Launcher", true);
    if !_guard.is_primary() {
        return Ok(());
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([680.0, 500.0])
            .with_maximized(true)
            .with_icon(castscreen_core::theme::load_window_icon())
            .with_title(format!("CastScreen {VERSION}")),
        ..Default::default()
    };

    eframe::run_native(
        "CastScreen Launcher",
        native_options,
        Box::new(|cc| {
            // The type scale binds a named font family, and egui applies new
            // fonts at the start of the next frame — so the design system is
            // installed here, before any frame can reference it.
            configure_dark_studio_theme(&cc.egui_ctx);
            Ok(Box::new(LauncherApp::new()))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe error: {}", e))
}

/// Which role the user picked, if any. Resolved in one place so the two ways of
/// choosing — the card and the keyboard shortcut — cannot drift apart.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Sender,
    Receiver,
}

struct LauncherApp {
    local_ip: String,
    status_msg: String,
    updater: AppUpdater,
    show_update_sheet: bool,
    /// Counts down while the window is still being asked to fill the screen.
    fill_frames: u32,
}

impl LauncherApp {
    fn new() -> Self {
        let updater = AppUpdater::new();
        updater.check_for_updates();

        Self {
            local_ip: Self::detect_local_ip(),
            status_msg: String::new(),
            updater,
            show_update_sheet: false,
            fill_frames: FILL_SCREEN_FRAMES,
        }
    }

    fn detect_local_ip() -> String {
        // Simple heuristic to discover active LAN IP
        if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
            if socket.connect("8.8.8.8:80").is_ok() {
                if let Ok(local_addr) = socket.local_addr() {
                    return local_addr.ip().to_string();
                }
            }
        }
        "192.168.1.x".to_string()
    }

    fn start(&mut self, ctx: &egui::Context, role: Role) {
        let launched = match role {
            Role::Sender => launch_sender(),
            Role::Receiver => launch_receiver(),
        };
        match launched {
            Ok(true) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Ok(false) => {
                self.status_msg =
                    "Compilando por primera vez. Esto tarda un momento y sólo pasa una vez."
                        .to_string();
            }
            Err(e) => self.status_msg = format!("No se pudo iniciar: {e}"),
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        configure_dark_studio_theme(ctx);

        fill_screen(ctx, &mut self.fill_frames, Fill::Maximized);

        let update_state = self.updater.get_state();
        if matches!(update_state, UpdateState::Downloading { .. }) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        // Keyboard is a first-class path to the same two actions, not a
        // shortcut bolted on: some people never reach for the mouse.
        let mut picked = None;
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Num1) || i.key_pressed(egui::Key::E) {
                picked = Some(Role::Sender);
            }
            if i.key_pressed(egui::Key::Num2) || i.key_pressed(egui::Key::R) {
                picked = Some(Role::Receiver);
            }
        });

        egui::TopBottomPanel::top("launcher_chrome")
            .frame(material(Layer::Chrome).inner_margin(egui::Margin::symmetric(space::LG, space::MD)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    draw_castscreen_logo(ui, 26.0);
                    ui.add_space(space::SM);
                    ui.label(ty::TITLE.colored("CastScreen", TEXT_PRIMARY));
                    chip(ui, format!("v{VERSION}"), TEXT_MUTED);

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        chip(ui, format!("Esta PC: {}", self.local_ip), ACCENT_LIVE);
                        if update_chip(ui, &update_state) {
                            self.show_update_sheet = true;
                        }
                    });
                });
            });

        egui::TopBottomPanel::bottom("launcher_status")
            .frame(material(Layer::Chrome).inner_margin(egui::Margin::symmetric(space::LG, space::SM)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let status = if self.status_msg.is_empty() {
                        "Atajos: 1 emisor · 2 receptor".to_string()
                    } else {
                        self.status_msg.clone()
                    };
                    ui.label(ty::CAPTION.colored(status, TEXT_MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(ty::CAPTION.colored(
                            "Direct3D 11 · WASAPI · MPEG-TS sobre TCP",
                            TEXT_MUTED,
                        ));
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(space::XL))
            .show(ctx, |ui| {
                // The window fills the screen, but a line of text does not get
                // easier to read by getting longer. The choice stays in a
                // measured column, centred, and grows with the display instead
                // of sitting small in the middle of it.
                let full_width = ui.available_width();
                let scale = (full_width / 1000.0).clamp(1.0, 1.5);
                let column = (860.0 * scale).min(full_width);
                let gutter = ((full_width - column) * 0.5).max(0.0);
                let block_height = 360.0 * scale;
                ui.add_space(((ui.available_height() - block_height) * 0.5).max(0.0));

                ui.horizontal(|ui| {
                    ui.add_space(gutter);
                    ui.vertical(|ui| {
                    ui.set_max_width(column);
                ui.label(ty::DISPLAY.scaled(scale).colored("¿Qué papel cumple esta computadora?", TEXT_PRIMARY));
                ui.add_space(space::SM * scale);
                ui.label(ty::BODY.scaled(scale).colored(
                    "CastScreen corre en las dos a la vez: la PC que juega envía, la laptop recibe y transmite.",
                    TEXT_SECONDARY,
                ));
                ui.add_space(space::XL * scale);

                ui.columns(2, |columns| {
                    let sender = role_card(
                        &mut columns[0],
                        scale,
                        "sender",
                        ACCENT_LIVE,
                        "PC DE JUEGO",
                        "Enviar esta pantalla",
                        "Captura por GPU sin costar FPS y manda vídeo y sonido a la laptop.",
                        &[
                            "Captura DirectX 11 en VRAM, sin copias",
                            "Codificación NVENC por hardware",
                            "Mezclador de audio por aplicación",
                            "Entrega sin pérdidas por TCP en LAN",
                        ],
                        "Iniciar emisor",
                    );

                    let receiver = role_card(
                        &mut columns[1],
                        scale,
                        "receiver",
                        ACCENT_BRAND,
                        "LAPTOP DE STREAMING",
                        "Recibir y previsualizar",
                        "Muestra el juego a 60 FPS y entrega una ventana limpia a OBS o TikTok.",
                        &[
                            "Previsualización a 60 FPS sin jitter",
                            "Monitor de audio maestro en audífonos",
                            "Modo captura limpia en un clic",
                            "Reensamblado MPEG-TS con PTS común",
                        ],
                        "Iniciar receptor",
                    );

                    if sender {
                        picked = Some(Role::Sender);
                    }
                    if receiver {
                        picked = Some(Role::Receiver);
                    }
                });
                    });
                });
            });

        if let Some(role) = picked {
            self.start(ctx, role);
        }

        update_sheet(ctx, &self.updater, &mut self.show_update_sheet);
    }
}

/// One of the two answers.
///
/// The overline names the machine, the headline names what it does, and the
/// list says what that buys — three levels of the same answer, so the choice
/// can be made from any of them. The call to action nudges toward the pointer
/// as it arrives, telegraphing that the whole card is going to move.
#[allow(clippy::too_many_arguments)]
fn role_card(
    ui: &mut egui::Ui,
    scale: f32,
    id: &str,
    accent: egui::Color32,
    machine: &str,
    headline: &str,
    purpose: &str,
    features: &[&str],
    action: &str,
) -> bool {
    let nudge = peek_spring(ui.ctx(), ui.id().with(id).with("hover")) * 4.0;

    card(ui, id, accent, |ui| {
        ui.label(ty::OVERLINE.scaled(scale).colored(machine, accent));
        ui.add_space(space::XS * scale);
        ui.label(ty::HEADLINE.scaled(scale).colored(headline, TEXT_PRIMARY));
        ui.add_space(space::XS * scale);
        ui.label(ty::CALLOUT.scaled(scale).colored(purpose, TEXT_SECONDARY));

        ui.add_space(space::LG * scale);
        for feature in features {
            feature_row(ui, feature, accent, scale);
            ui.add_space(space::XXS * scale);
        }

        ui.add_space(space::LG * scale);
        ui.horizontal(|ui| {
            ui.add_space(nudge);
            ui.label(ty::BODY_EMPHASIS.scaled(scale).colored(format!("{action}  →"), accent));
        });
    })
    .clicked()
}
