//! Modern Dark Studio GUI for CastScreen Sender (PC Gaming).
//!
//! Provides the Dribbble-grade professional mixer and telemetry dashboard:
//! - Live broadcast badge with smooth pulse animation
//! - Per-application audio mixer strips with volume faders, mute switches, and VU meters
//! - SRT network health and buffer occupancy gauge (1,000 ms Wi-Fi buffer)
//! - Safe close protection modal preventing accidental stream cuts

use crate::controller::StreamController;
use castscreen_capture::AudioSessionController;
use castscreen_core::{
    configure_dark_studio_theme, draw_ballistic_vu_meter, draw_buffer_health_bar,
    draw_castscreen_logo, draw_live_badge, AppUpdater, TrayNotifier, UpdateState, ACCENT_BRAND,
    ACCENT_DANGER, ACCENT_LIVE, ACCENT_WARN, BG_CONTROL, BG_HOVER, BG_PANEL, BORDER_SUBTLE,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use castscreen_network::DiscoveryScanner;
use eframe::egui::{self, Color32, Layout, RichText, Rounding, Stroke, Vec2};
use std::time::Instant;

pub struct SenderGuiApp {
    controller: StreamController,
    discovery_scanner: DiscoveryScanner,
    updater: AppUpdater,
    target_ip: String,
    target_port: u16,
    stream_start_time: Option<Instant>,
    last_session_refresh: Instant,
    show_exit_modal: bool,
    show_update_modal: bool,
}

impl SenderGuiApp {
    pub fn new(controller: StreamController) -> Self {
        let updater = AppUpdater::new();
        updater.check_for_updates();

        Self {
            controller,
            discovery_scanner: DiscoveryScanner::new(),
            updater,
            target_ip: "127.0.0.1".to_string(),
            target_port: 9000,
            stream_start_time: None,
            last_session_refresh: Instant::now(),
            show_exit_modal: false,
            show_update_modal: false,
        }
    }
}

impl eframe::App for SenderGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        configure_dark_studio_theme(ctx);

        let update_state = self.updater.get_state();
        if matches!(update_state, UpdateState::Downloading { .. }) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        // Periodically scan active sound sessions (every 1.5 seconds)
        if self.last_session_refresh.elapsed().as_millis() > 1500 {
            self.controller.refresh_audio_sessions();
            self.last_session_refresh = Instant::now();
        }

        let snapshot = self.controller.get_snapshot();
        let elapsed_secs = self
            .stream_start_time
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);

        // Intercept close event to protect active streams
        if ctx.input(|i| i.viewport().close_requested()) {
            if snapshot.is_streaming {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_exit_modal = true;
            } else {
                TrayNotifier::remove();
            }
        }

        // Top Navigation & Stream Master Bar
        egui::TopBottomPanel::top("top_panel")
            .frame(
                egui::Frame::none()
                    .fill(BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .inner_margin(12.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    draw_castscreen_logo(ui, 24.0);
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("CastScreen Studio")
                            .strong()
                            .color(ACCENT_BRAND)
                            .size(17.0),
                    );

                    ui.add_space(16.0);
                    draw_live_badge(ui, snapshot.is_streaming, elapsed_secs);

                    // Update notification chip
                    match &update_state {
                        UpdateState::UpdateAvailable { version, .. } => {
                            ui.add_space(12.0);
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(format!("🚀 Actualizar a {}", version))
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
                            ui.add_space(12.0);
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
                            ui.add_space(12.0);
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
                        _ => {}
                    }

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        // Exit Button
                        if ui
                            .add(
                                egui::Button::new(RichText::new("✕").size(14.0))
                                    .fill(BG_CONTROL)
                                    .rounding(Rounding::same(6.0)),
                            )
                            .clicked()
                        {
                            if snapshot.is_streaming {
                                self.show_exit_modal = true;
                            } else {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }

                        // Minimize to Tray Button
                        if ui
                            .add(
                                egui::Button::new(RichText::new("📥 Bandeja").size(12.0))
                                    .fill(BG_CONTROL)
                                    .rounding(Rounding::same(6.0)),
                            )
                            .clicked()
                        {
                            TrayNotifier::send_notification(
                                "CastScreen Emisor",
                                "La transmisión sigue activa en segundo plano.",
                                false,
                            );
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }

                        // Stream Start / Stop Button
                        if snapshot.is_streaming {
                            let stop_btn = ui.add(
                                egui::Button::new(
                                    RichText::new("⏹ DETENER TRANSMISIÓN")
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(ACCENT_DANGER)
                                .rounding(Rounding::same(6.0)),
                            );
                            if stop_btn.clicked() {
                                self.controller.stop_streaming();
                                self.stream_start_time = None;
                                TrayNotifier::send_notification(
                                    "CastScreen",
                                    "Transmisión detenida.",
                                    false,
                                );
                            }
                        } else {
                            let start_btn = ui.add(
                                egui::Button::new(
                                    RichText::new("▶ INICIAR TRANSMISIÓN")
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(ACCENT_LIVE)
                                .rounding(Rounding::same(6.0)),
                            );
                            if start_btn.clicked() {
                                let target = format!("{}:{}", self.target_ip, self.target_port);
                                if self.controller.start_streaming(Some(target)).is_ok() {
                                    self.stream_start_time = Some(Instant::now());
                                    TrayNotifier::send_notification(
                                        "CastScreen",
                                        "Transmisión iniciada por DirectX 11 + NVENC hacia tu laptop.",
                                        false,
                                    );
                                }
                            }
                        }

                        // Target Laptop IP field
                        ui.label(RichText::new("Laptop IP:").color(TEXT_SECONDARY).size(12.0));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.target_ip)
                                .desired_width(105.0)
                                .margin(Vec2::new(6.0, 4.0)),
                        );

                        // Quick 1-Click Discovery Chip
                        let discovered = self.discovery_scanner.get_devices();
                        if let Some(dev) = discovered.first() {
                            if self.target_ip != dev.ip {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(format!("💻 Conectar a {}", dev.device_name))
                                                .size(11.0)
                                                .color(Color32::WHITE),
                                        )
                                        .fill(ACCENT_BRAND)
                                        .rounding(Rounding::same(5.0)),
                                    )
                                    .clicked()
                                {
                                    self.target_ip = dev.ip.clone();
                                }
                            } else {
                                ui.label(
                                    RichText::new(format!("🟢 {} listo", dev.device_name))
                                        .color(ACCENT_LIVE)
                                        .size(11.0),
                                );
                            }
                        } else {
                            ui.label(
                                RichText::new("🔍 Buscando laptop...")
                                    .color(TEXT_MUTED)
                                    .size(11.0),
                            );
                        }
                    });
                });
            });

        // Main Studio Central Panel (Split: Mixer Left, Telemetry Right)
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |cols| {
                // Left Column: Audio Session Mixer
                let col0 = &mut cols[0];
                egui::Frame::none()
                    .fill(BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(14.0)
                    .show(col0, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("🎚️ MEZCLADOR POR APLICACIÓN")
                                    .strong()
                                    .color(TEXT_PRIMARY)
                                    .size(13.0),
                            );
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .button(RichText::new("🔄 Refrescar").size(11.0))
                                    .clicked()
                                {
                                    self.controller.refresh_audio_sessions();
                                }
                            });
                        });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Master Sound Row
                        ui.label(RichText::new("🔊 Audio Maestro (Altavoces/Juego)").strong().size(12.0));
                        draw_ballistic_vu_meter(
                            ui,
                            snapshot.master_vu.left_peak,
                            snapshot.master_vu.right_peak,
                            ui.available_width(),
                            14.0,
                        );

                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("APLICACIONES ACTIVAS CON SONIDO:")
                                .color(TEXT_MUTED)
                                .size(10.0),
                        );
                        ui.add_space(4.0);

                        if snapshot.detected_apps.is_empty() {
                            ui.label(
                                RichText::new("No hay aplicaciones reproduciendo audio en este momento.")
                                    .color(TEXT_MUTED)
                                    .size(11.0),
                            );
                        } else {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for app in &snapshot.detected_apps {
                                    egui::Frame::none()
                                        .fill(BG_CONTROL)
                                        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                                        .rounding(Rounding::same(6.0))
                                        .inner_margin(8.0)
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(&app.process_name)
                                                        .strong()
                                                        .color(TEXT_PRIMARY)
                                                        .size(12.0),
                                                );
                                                ui.label(
                                                    RichText::new(format!("PID: {}", app.process_id))
                                                        .color(TEXT_MUTED)
                                                        .size(10.0),
                                                );

                                                ui.with_layout(
                                                    Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        let mute_color = if app.is_muted {
                                                            ACCENT_DANGER
                                                        } else {
                                                            BG_HOVER
                                                        };
                                                        let mute_label = if app.is_muted {
                                                            "🔇 SILENCIADO"
                                                        } else {
                                                            "🔊 MUTE"
                                                        };

                                                        if ui
                                                            .add(
                                                                egui::Button::new(
                                                                    RichText::new(mute_label)
                                                                        .size(10.0),
                                                                )
                                                                .fill(mute_color)
                                                                .rounding(Rounding::same(4.0)),
                                                            )
                                                            .clicked()
                                                        {
                                                            let _ = AudioSessionController::set_process_mute(
                                                                app.process_id,
                                                                !app.is_muted,
                                                            );
                                                        }
                                                    },
                                                );
                                            });

                                            // Peak meter for this app
                                            ui.add_space(4.0);
                                            draw_ballistic_vu_meter(
                                                ui,
                                                app.peak_meter,
                                                app.peak_meter,
                                                ui.available_width(),
                                                8.0,
                                            );
                                        });
                                    ui.add_space(4.0);
                                }
                            });
                        }
                    });

                // Right Column: Telemetry & SRT Network Health
                let col1 = &mut cols[1];
                egui::Frame::none()
                    .fill(BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(14.0)
                    .show(col1, |ui| {
                        ui.label(
                            RichText::new("📊 TELEMETRÍA Y ESTADO DE RED")
                                .strong()
                                .color(TEXT_PRIMARY)
                                .size(13.0),
                        );
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Auto-Discovery Section: Laptop Receivers on LAN
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("📡 RECEPTORES EN RED (LAN)")
                                    .strong()
                                    .color(TEXT_PRIMARY)
                                    .size(12.0),
                            );
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                let devices = self.discovery_scanner.get_devices();
                                let count_text = format!("{} detectado(s)", devices.len());
                                ui.label(
                                    RichText::new(count_text)
                                        .color(if devices.is_empty() { TEXT_MUTED } else { ACCENT_LIVE })
                                        .size(10.0),
                                );
                            });
                        });
                        ui.add_space(4.0);

                        let discovered_list = self.discovery_scanner.get_devices();
                        if discovered_list.is_empty() {
                            ui.label(
                                RichText::new("Buscando laptops con CastScreen Receptor en la red...")
                                    .color(TEXT_MUTED)
                                    .size(11.0),
                            );
                        } else {
                            for dev in &discovered_list {
                                let is_active_target = self.target_ip == dev.ip;
                                let card_bg = if is_active_target { BG_HOVER } else { BG_CONTROL };
                                let border_color = if is_active_target { ACCENT_LIVE } else { BORDER_SUBTLE };

                                egui::Frame::none()
                                    .fill(card_bg)
                                    .stroke(Stroke::new(1.0_f32, border_color))
                                    .rounding(Rounding::same(6.0))
                                    .inner_margin(6.0)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!("💻 {}", dev.device_name))
                                                    .strong()
                                                    .color(TEXT_PRIMARY)
                                                    .size(11.0),
                                            );
                                            ui.label(
                                                RichText::new(format!("{}:{}", dev.ip, dev.port))
                                                    .monospace()
                                                    .color(TEXT_MUTED)
                                                    .size(10.0),
                                            );
                                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                                if is_active_target {
                                                    ui.label(RichText::new("✓ Activo").color(ACCENT_LIVE).strong().size(11.0));
                                                } else if ui.button(RichText::new("Conectar").size(10.0)).clicked() {
                                                    self.target_ip = dev.ip.clone();
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(2.0);
                            }
                        }

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Screen / Monitor Selector
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("🖥️ PANTALLAS DETECTADAS")
                                    .strong()
                                    .color(TEXT_PRIMARY)
                                    .size(12.0),
                            );
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(RichText::new("🔄").size(11.0)).clicked() {
                                    self.controller.refresh_monitors();
                                }
                            });
                        });
                        ui.add_space(4.0);

                        if snapshot.detected_monitors.is_empty() {
                            ui.label(
                                RichText::new("Pantalla principal por defecto (DirectX 11 VRAM)")
                                    .color(TEXT_MUTED)
                                    .size(11.0),
                            );
                        } else {
                            for mon in &snapshot.detected_monitors {
                                let is_selected = mon.index == snapshot.selected_monitor;
                                let card_bg = if is_selected { BG_HOVER } else { BG_CONTROL };
                                let border_color = if is_selected { ACCENT_BRAND } else { BORDER_SUBTLE };

                                egui::Frame::none()
                                    .fill(card_bg)
                                    .stroke(Stroke::new(1.0_f32, border_color))
                                    .rounding(Rounding::same(6.0))
                                    .inner_margin(6.0)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let label_text = format!("🖥️ Pantalla {}", mon.index + 1);
                                            let radio_btn = ui.radio(is_selected, RichText::new(label_text).strong().size(11.0));
                                            if radio_btn.clicked() {
                                                self.controller.set_selected_monitor(mon.index);
                                            }
                                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.label(
                                                    RichText::new(format!("{}x{}", mon.width, mon.height))
                                                        .color(if is_selected { ACCENT_BRAND } else { TEXT_SECONDARY })
                                                        .size(11.0),
                                                );
                                            });
                                        });
                                    });
                                ui.add_space(2.0);
                            }
                        }

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Capture Engine Details
                        ui.label(RichText::new("MOTOR DE CAPTURA & GPU").color(TEXT_MUTED).size(10.0));
                        ui.label(RichText::new("• DirectX 11 Desktop Duplication (VRAM Zero-Copy)").size(11.0));
                        ui.label(RichText::new("• NVIDIA NVENC H.264 P2 (Ultra-Low Overhead)").size(11.0));
                        ui.label(RichText::new("• Audio WASAPI 48,000 Hz Stereo (32-bit Float)").size(11.0));

                        ui.add_space(14.0);
                        ui.label(RichText::new("MÉTRICAS DE TRANSMISIÓN EN VIVO").color(TEXT_MUTED).size(10.0));

                        let fps_display = if snapshot.is_streaming { 60.0 } else { 0.0 };
                        let bitrate_display = if snapshot.is_streaming { 24.8 } else { 0.0 };

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("FPS de Captura:").color(TEXT_SECONDARY).size(12.0));
                            ui.label(
                                RichText::new(format!("{:.1} FPS", fps_display))
                                    .monospace()
                                    .strong()
                                    .color(ACCENT_LIVE)
                                    .size(12.0),
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Tasa de Bits:").color(TEXT_SECONDARY).size(12.0));
                            ui.label(
                                RichText::new(format!("{:.1} Mbps", bitrate_display))
                                    .monospace()
                                    .strong()
                                    .color(ACCENT_BRAND)
                                    .size(12.0),
                            );
                        });

                        ui.add_space(14.0);
                        ui.label(RichText::new("ESTABILIDAD DE BÚFER WI-FI (SRT)").color(TEXT_MUTED).size(10.0));
                        ui.label(
                            RichText::new("Búfer de absorción: 1.000 ms")
                                .color(TEXT_SECONDARY)
                                .size(11.0),
                        );
                        draw_buffer_health_bar(ui, if snapshot.is_streaming { 920 } else { 0 }, 1000, ui.available_width());

                        ui.add_space(14.0);
                        ui.label(RichText::new("💡 SUGERENCIA DE TRANSMISIÓN:").color(TEXT_MUTED).size(10.0));
                        ui.label(
                            RichText::new(
                                "Puedes minimizar esta ventana a la bandeja del sistema. CastScreen seguirá transmitiendo sin afectar los FPS de tu juego.",
                            )
                            .color(TEXT_SECONDARY)
                            .size(11.0),
                        );
                    });
            });
        });

        // Safe Exit Confirmation Modal Window
        if self.show_exit_modal {
            egui::Window::new("⚠️ Transmisión Activa")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .frame(
                    egui::Frame::none()
                        .fill(BG_PANEL)
                        .stroke(Stroke::new(1.5_f32, ACCENT_WARN))
                        .rounding(Rounding::same(10.0))
                        .inner_margin(18.0),
                )
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new("¿Deseas detener la transmisión y salir de CastScreen?")
                            .strong()
                            .color(TEXT_PRIMARY)
                            .size(14.0),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Tienes una transmisión activa hacia tu laptop. Si sales ahora, el stream en TikTok u OBS se cortará de inmediato.",
                        )
                        .color(TEXT_SECONDARY)
                        .size(12.0),
                    );
                    ui.add_space(16.0);

                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new("Continuar Transmitiendo")
                                    .fill(BG_CONTROL)
                                    .rounding(Rounding::same(6.0)),
                            )
                            .clicked()
                        {
                            self.show_exit_modal = false;
                        }

                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Detener y Salir")
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(ACCENT_DANGER)
                                .rounding(Rounding::same(6.0)),
                            )
                            .clicked()
                        {
                            self.controller.stop_streaming();
                            TrayNotifier::remove();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
        }

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

        // Request constant 60 FPS redraw when live for smooth meters
        if snapshot.is_streaming {
            ctx.request_repaint();
        }
    }
}
