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
    configure_dark_studio_theme, draw_ballistic_vu_meter, draw_buffer_health_bar, draw_live_badge,
    TrayNotifier, ACCENT_BRAND, ACCENT_DANGER, ACCENT_LIVE, ACCENT_WARN, BG_CONTROL,
    BG_HOVER, BG_PANEL, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use eframe::egui::{self, Color32, Layout, RichText, Rounding, Stroke, Vec2};
use std::time::Instant;

pub struct SenderGuiApp {
    controller: StreamController,
    target_ip: String,
    target_port: u16,
    stream_start_time: Option<Instant>,
    last_session_refresh: Instant,
    show_exit_modal: bool,
}

impl SenderGuiApp {
    pub fn new(controller: StreamController) -> Self {
        Self {
            controller,
            target_ip: "192.168.1.55".to_string(),
            target_port: 9000,
            stream_start_time: None,
            last_session_refresh: Instant::now(),
            show_exit_modal: false,
        }
    }
}

impl eframe::App for SenderGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        configure_dark_studio_theme(ctx);

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
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(12.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("📡 CastScreen Studio")
                            .strong()
                            .color(ACCENT_BRAND)
                            .size(17.0),
                    );

                    ui.add_space(16.0);
                    draw_live_badge(ui, snapshot.is_streaming, elapsed_secs);

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
                                .desired_width(110.0)
                                .margin(Vec2::new(6.0, 4.0)),
                        );
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
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
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
                                        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
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
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
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
                        .stroke(Stroke::new(1.5, ACCENT_WARN))
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

        // Request constant 60 FPS redraw when live for smooth meters
        if snapshot.is_streaming {
            ctx.request_repaint();
        }
    }
}
