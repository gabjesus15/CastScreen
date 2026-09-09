#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CastScreen Unified Launcher & Hardware Mode Selector (Dark Studio GUI).
//!
//! Provides a modern desktop interface to choose between:
//! - 🎮 Modo Emisor (PC Gaming: DirectX 11 + NVENC + Per-App Audio Mixer)
//! - 💻 Modo Receptor (Streaming Laptop: 60 FPS Live Preview + Master Audio Monitor)

use anyhow::Result;
use castscreen_core::{
    configure_dark_studio_theme, draw_castscreen_logo, AppUpdater, SingleInstanceGuard,
    UpdateState, ACCENT_BRAND, ACCENT_LIVE, BG_CONTROL, BG_PANEL, BORDER_SUBTLE, TEXT_MUTED,
    TEXT_PRIMARY, TEXT_SECONDARY,
};
use eframe::egui::{self, Color32, RichText, Rounding, Stroke, Vec2};
use std::env;
use std::process::Command;

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
            .with_inner_size([660.0, 460.0])
            .with_min_inner_size([640.0, 440.0])
            .with_resizable(false)
            .with_icon(castscreen_core::theme::load_window_icon())
            .with_title(format!("CastScreen Launcher v{}", VERSION)),
        ..Default::default()
    };

    eframe::run_native(
        "CastScreen Launcher",
        native_options,
        Box::new(|_cc| Ok(Box::new(LauncherApp::new()))),
    )
    .map_err(|e| anyhow::anyhow!("Eframe error: {}", e))
}

struct LauncherApp {
    local_ip: String,
    status_msg: String,
    updater: AppUpdater,
    show_update_modal: bool,
}

impl LauncherApp {
    fn new() -> Self {
        let local_ip = Self::detect_local_ip();
        let updater = AppUpdater::new();
        updater.check_for_updates();

        Self {
            local_ip,
            status_msg: "Sistema listo para transmitir.".to_string(),
            updater,
            show_update_modal: false,
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
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        configure_dark_studio_theme(ctx);

        let update_state = self.updater.get_state();
        if matches!(update_state, UpdateState::Downloading { .. }) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);

            // Top Header: Logo + Version + Update Badge + LAN Status Chip
            ui.horizontal(|ui| {
                draw_castscreen_logo(ui, 26.0);
                ui.add_space(8.0);
                ui.label(
                    RichText::new("CastScreen")
                        .strong()
                        .color(ACCENT_BRAND)
                        .size(20.0),
                );
                ui.label(
                    RichText::new(format!("v{}", VERSION))
                        .color(TEXT_MUTED)
                        .size(13.0),
                );

                ui.add_space(8.0);

                // Update notification chip
                match &update_state {
                    UpdateState::UpdateAvailable { version, .. } => {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!("🚀 Actualización {} disponible", version))
                                        .size(11.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(ACCENT_LIVE)
                                .rounding(Rounding::same(5.0)),
                            )
                            .clicked()
                        {
                            self.show_update_modal = true;
                        }
                    }
                    UpdateState::Downloading { progress_pct, .. } => {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!("⬇️ Descargando ({:.0}%)", progress_pct * 100.0))
                                        .size(11.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(ACCENT_BRAND)
                                .rounding(Rounding::same(5.0)),
                            )
                            .clicked()
                        {
                            self.show_update_modal = true;
                        }
                    }
                    UpdateState::ReadyToInstall { version, .. } => {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!("✨ Instalar {}", version))
                                        .size(11.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(ACCENT_LIVE)
                                .rounding(Rounding::same(5.0)),
                            )
                            .clicked()
                        {
                            self.show_update_modal = true;
                        }
                    }
                    UpdateState::Checking => {
                        ui.label(RichText::new("🔄 Buscando actualizaciones...").color(TEXT_MUTED).size(11.0));
                    }
                    UpdateState::UpToDate => {
                        ui.label(RichText::new("✓ Versión al día").color(ACCENT_LIVE).size(11.0));
                    }
                    UpdateState::Error(_) => {
                        ui.label(RichText::new("⚠️ Error comprobando versión").color(TEXT_MUTED).size(11.0));
                    }
                    _ => {}
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.painter().circle_filled(
                        ui.cursor().min + Vec2::new(-8.0, 9.0),
                        4.0,
                        ACCENT_LIVE,
                    );
                    ui.label(
                        RichText::new(format!("IP: {}", self.local_ip))
                            .monospace()
                            .color(TEXT_SECONDARY)
                            .size(12.0),
                    );
                });
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(14.0);

            // Subtitle
            ui.label(
                RichText::new("Selecciona el rol que cumplirá esta computadora:")
                    .color(TEXT_PRIMARY)
                    .size(14.0),
            );

            ui.add_space(14.0);

            // Main Selection Cards (Side-by-Side)
            ui.columns(2, |columns| {
                // Card 1: PC Gaming (Sender)
                let col0 = &mut columns[0];
                let card_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);

                egui::Frame::none()
                    .fill(BG_PANEL)
                    .stroke(card_stroke)
                    .rounding(Rounding::same(10.0))
                    .inner_margin(16.0)
                    .show(col0, |ui| {
                        ui.label(
                            RichText::new("🎮 MODO EMISOR")
                                .strong()
                                .color(ACCENT_LIVE)
                                .size(15.0),
                        );
                        ui.label(
                            RichText::new("Para tu PC Gaming principal")
                                .color(TEXT_SECONDARY)
                                .size(12.0),
                        );

                        ui.add_space(12.0);
                        ui.label(RichText::new("• Captura DirectX 11 VRAM (Zero-Copy)").size(11.0).color(TEXT_PRIMARY));
                        ui.label(RichText::new("• Codificación por Hardware NVENC").size(11.0).color(TEXT_PRIMARY));
                        ui.label(RichText::new("• Mezclador de audio por aplicación").size(11.0).color(TEXT_PRIMARY));
                        ui.label(RichText::new("• Búfer Wi-Fi 6 de 1.000 ms (SRT)").size(11.0).color(TEXT_PRIMARY));

                        ui.add_space(18.0);
                        let btn = ui.add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new(
                                RichText::new("▶ Iniciar como Emisor")
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(ACCENT_BRAND)
                            .rounding(Rounding::same(6.0)),
                        );

                        if btn.clicked() {
                            match launch_sender() {
                                Ok(true) => {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                Ok(false) => {
                                    self.status_msg = "Compilando e iniciando emisor... Por favor espera unos segundos.".to_string();
                                }
                                Err(e) => {
                                    self.status_msg = format!("Error al iniciar emisor: {}", e);
                                }
                            }
                        }
                    });

                // Card 2: Laptop Streamer (Receiver)
                let col1 = &mut columns[1];
                egui::Frame::none()
                    .fill(BG_PANEL)
                    .stroke(card_stroke)
                    .rounding(Rounding::same(10.0))
                    .inner_margin(16.0)
                    .show(col1, |ui| {
                        ui.label(
                            RichText::new("💻 MODO RECEPTOR")
                                .strong()
                                .color(ACCENT_BRAND)
                                .size(15.0),
                        );
                        ui.label(
                            RichText::new("Para tu Laptop de Streaming")
                                .color(TEXT_SECONDARY)
                                .size(12.0),
                        );

                        ui.add_space(12.0);
                        ui.label(RichText::new("• Previsualización fluida a 60 FPS").size(11.0).color(TEXT_PRIMARY));
                        ui.label(RichText::new("• Monitoreo de sonido maestro en audífonos").size(11.0).color(TEXT_PRIMARY));
                        ui.label(RichText::new("• Modo captura limpia para OBS y TikTok").size(11.0).color(TEXT_PRIMARY));
                        ui.label(RichText::new("• Telemetría de paquetes y sincronización").size(11.0).color(TEXT_PRIMARY));

                        ui.add_space(18.0);
                        let btn = ui.add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new(
                                RichText::new("👁️ Iniciar como Receptor")
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(ACCENT_LIVE)
                            .rounding(Rounding::same(6.0)),
                        );

                        if btn.clicked() {
                            match launch_receiver() {
                                Ok(true) => {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                Ok(false) => {
                                    self.status_msg = "Compilando e iniciando receptor... Por favor espera unos segundos.".to_string();
                                }
                                Err(e) => {
                                    self.status_msg = format!("Error al iniciar receptor: {}", e);
                                }
                            }
                        }
                    });
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // Bottom System Status Bar
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Estado: {}", self.status_msg))
                        .color(TEXT_MUTED)
                        .size(11.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("✓ Direct3D 11  ✓ WASAPI Loopback  ✓ SRT MPEG-TS")
                            .color(TEXT_MUTED)
                            .size(11.0),
                    );
                });
            });
        });

        // In-App Auto-Update Modal Window
        if self.show_update_modal {
            egui::Window::new("🚀 Actualización de CastScreen")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .frame(
                    egui::Frame::none()
                        .fill(BG_PANEL)
                        .stroke(Stroke::new(1.5_f32, ACCENT_BRAND))
                        .rounding(Rounding::same(10.0))
                        .inner_margin(18.0),
                )
                .show(ctx, |ui| {
                    ui.set_width(420.0);
                    match &update_state {
                        UpdateState::UpdateAvailable { version, title, notes, asset_size, .. } => {
                            ui.label(
                                RichText::new(format!("Nueva versión disponible: {}", version))
                                    .strong()
                                    .color(ACCENT_BRAND)
                                    .size(15.0),
                            );
                            ui.add_space(4.0);
                            ui.label(RichText::new(title).strong().color(TEXT_PRIMARY).size(13.0));
                            ui.add_space(6.0);
                            let mb = (*asset_size as f64) / (1024.0 * 1024.0);
                            ui.label(
                                RichText::new(format!("Tamaño del instalador: {:.1} MB", mb))
                                    .color(TEXT_MUTED)
                                    .size(11.0),
                            );
                            ui.add_space(8.0);

                            ui.label(RichText::new("Novedades:").color(TEXT_SECONDARY).size(12.0));
                            egui::Frame::none()
                                .fill(BG_CONTROL)
                                .rounding(Rounding::same(6.0))
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    egui::ScrollArea::vertical().max_height(130.0).show(ui, |ui| {
                                        ui.label(RichText::new(notes).color(TEXT_PRIMARY).size(11.0));
                                    });
                                });
                            ui.add_space(14.0);

                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("⬇️ Descargar e Instalar")
                                                .strong()
                                                .color(Color32::WHITE),
                                        )
                                        .fill(ACCENT_LIVE)
                                        .rounding(Rounding::same(6.0)),
                                    )
                                    .clicked()
                                {
                                    self.updater.start_download();
                                }
                                if ui.button("Cancelar").clicked() {
                                    self.show_update_modal = false;
                                }
                            });
                        }
                        UpdateState::Downloading { version, downloaded_bytes, total_bytes, progress_pct } => {
                            ui.label(
                                RichText::new(format!("Descargando versión {}...", version))
                                    .strong()
                                    .color(ACCENT_BRAND)
                                    .size(14.0),
                            );
                            ui.add_space(10.0);

                            let dl_mb = (*downloaded_bytes as f64) / (1024.0 * 1024.0);
                            let tot_mb = (*total_bytes as f64) / (1024.0 * 1024.0);
                            ui.add(egui::ProgressBar::new(*progress_pct).show_percentage().animate(true));
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(format!("{:.1} MB / {:.1} MB descargados", dl_mb, tot_mb))
                                    .color(TEXT_MUTED)
                                    .size(11.0),
                            );
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new("Descargando actualización en segundo plano...")
                                    .color(TEXT_SECONDARY)
                                    .size(11.0),
                            );
                        }
                        UpdateState::ReadyToInstall { version, .. } => {
                            ui.label(RichText::new("✅ ¡Descarga completada!").strong().color(ACCENT_LIVE).size(15.0));
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(format!("La versión {} está lista para instalar.", version))
                                    .color(TEXT_PRIMARY)
                                    .size(13.0),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Al hacer clic, CastScreen se cerrará e iniciará el instalador automáticamente para actualizar el programa sin necesidad de abrir el navegador.",
                                )
                                .color(TEXT_SECONDARY)
                                .size(11.0),
                            );
                            ui.add_space(16.0);

                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("🔄 Instalar y Reiniciar")
                                                .strong()
                                                .color(Color32::WHITE),
                                        )
                                        .fill(ACCENT_LIVE)
                                        .rounding(Rounding::same(6.0)),
                                    )
                                    .clicked()
                                {
                                    let _ = self.updater.install_and_restart();
                                }
                                if ui.button("Más tarde").clicked() {
                                    self.show_update_modal = false;
                                }
                            });
                        }
                        UpdateState::Error(err) => {
                            ui.label(
                                RichText::new("⚠️ Error al actualizar")
                                    .strong()
                                    .color(Color32::from_rgb(239, 68, 68))
                                    .size(14.0),
                            );
                            ui.add_space(8.0);
                            ui.label(RichText::new(err).color(TEXT_SECONDARY).size(11.0));
                            ui.add_space(14.0);
                            ui.horizontal(|ui| {
                                if ui.button("Reintentar").clicked() {
                                    self.updater.check_for_updates();
                                }
                                if ui.button("Cerrar").clicked() {
                                    self.show_update_modal = false;
                                }
                            });
                        }
                        _ => {
                            ui.label(RichText::new("Buscando actualizaciones en GitHub...").color(TEXT_MUTED).size(12.0));
                            ui.add_space(8.0);
                            if ui.button("Cerrar").clicked() {
                                self.show_update_modal = false;
                            }
                        }
                    }
                });
        }
    }
}

fn find_binary(binary_name: &str) -> Option<std::path::PathBuf> {
    if let Ok(current_exe) = env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            // 1. Same directory (installed or target)
            let direct = exe_dir.join(format!("{}.exe", binary_name));
            if direct.exists() {
                return Some(direct);
            }

            // 2. Sibling release or debug directory
            let sibling_release = exe_dir.join("../release").join(format!("{}.exe", binary_name));
            if sibling_release.exists() {
                return Some(sibling_release);
            }
            let sibling_debug = exe_dir.join("../debug").join(format!("{}.exe", binary_name));
            if sibling_debug.exists() {
                return Some(sibling_debug);
            }
        }
    }

    // 3. Workspace target folders relative to current working directory
    let ws_release = std::path::Path::new("target/release").join(format!("{}.exe", binary_name));
    if ws_release.exists() {
        return Some(ws_release);
    }
    let ws_debug = std::path::Path::new("target/debug").join(format!("{}.exe", binary_name));
    if ws_debug.exists() {
        return Some(ws_debug);
    }

    None
}

fn launch_sender() -> Result<bool> {
    if let Some(sender_path) = find_binary("castscreen-sender") {
        Command::new(&sender_path).spawn()?;
        Ok(true)
    } else {
        let is_release = !cfg!(debug_assertions);
        let mut cmd = Command::new("cargo");
        cmd.args(["run", "-p", "castscreen-sender"]);
        if is_release {
            cmd.arg("--release");
        }
        cmd.spawn()?;
        Ok(false)
    }
}

fn launch_receiver() -> Result<bool> {
    if let Some(receiver_path) = find_binary("castscreen-receiver") {
        Command::new(&receiver_path).spawn()?;
        Ok(true)
    } else {
        let is_release = !cfg!(debug_assertions);
        let mut cmd = Command::new("cargo");
        cmd.args(["run", "-p", "castscreen-receiver"]);
        if is_release {
            cmd.arg("--release");
        }
        cmd.spawn()?;
        Ok(false)
    }
}
