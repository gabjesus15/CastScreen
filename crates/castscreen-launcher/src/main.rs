#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CastScreen Unified Launcher & Hardware Mode Selector (Dark Studio GUI).
//!
//! Provides a modern desktop interface to choose between:
//! - 🎮 Modo Emisor (PC Gaming: DirectX 11 + NVENC + Per-App Audio Mixer)
//! - 💻 Modo Receptor (Streaming Laptop: 60 FPS Live Preview + Master Audio Monitor)

use anyhow::Result;
use castscreen_core::{
    configure_dark_studio_theme, SingleInstanceGuard, ACCENT_BRAND, ACCENT_LIVE, BG_PANEL,
    BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
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
        return launch_sender();
    }
    if args.iter().any(|a| a == "--receiver" || a == "-r") {
        return launch_receiver();
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
            .with_title("CastScreen Launcher"),
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
}

impl LauncherApp {
    fn new() -> Self {
        let local_ip = Self::detect_local_ip();
        Self {
            local_ip,
            status_msg: "Sistema listo para transmitir.".to_string(),
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

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);

            // Top Header: Logo + Version + LAN Status Chip
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("📡 CastScreen")
                        .strong()
                        .color(ACCENT_BRAND)
                        .size(20.0),
                );
                ui.label(
                    RichText::new(format!("v{}", VERSION))
                        .color(TEXT_MUTED)
                        .size(13.0),
                );

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
                            if let Err(e) = launch_sender() {
                                self.status_msg = format!("Error al iniciar emisor: {}", e);
                            } else {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
                            if let Err(e) = launch_receiver() {
                                self.status_msg = format!("Error al iniciar receptor: {}", e);
                            } else {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
    }
}

fn launch_sender() -> Result<()> {
    let current_exe = env::current_exe()?;
    let exe_dir = current_exe.parent().unwrap_or_else(|| std::path::Path::new("."));
    let sender_path = exe_dir.join("castscreen-sender.exe");

    if sender_path.exists() {
        Command::new(&sender_path).spawn()?;
    } else {
        Command::new("cargo")
            .args(["run", "-p", "castscreen-sender", "--release"])
            .spawn()?;
    }
    Ok(())
}

fn launch_receiver() -> Result<()> {
    let current_exe = env::current_exe()?;
    let exe_dir = current_exe.parent().unwrap_or_else(|| std::path::Path::new("."));
    let receiver_path = exe_dir.join("castscreen-receiver.exe");

    if receiver_path.exists() {
        Command::new(&receiver_path).spawn()?;
    } else {
        Command::new("cargo")
            .args(["run", "-p", "castscreen-receiver", "--release"])
            .spawn()?;
    }
    Ok(())
}
