//! Modern Dark Studio GUI & Live Viewport for CastScreen Receiver (Laptop).
//!
//! Provides the 60 FPS hardware-accelerated preview window, audio master monitor,
//! and clean window mode for pixel-perfect capture in TikTok Live Studio and OBS Studio.

use crate::audio_out::AudioOutput;
use castscreen_core::{
    configure_dark_studio_theme, draw_ballistic_vu_meter, draw_castscreen_logo, draw_live_badge,
    AudioSubmixer, ACCENT_BRAND, ACCENT_LIVE, BG_CANVAS, BG_CONTROL, BG_PANEL, BORDER_SUBTLE,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, CURRENT_VERSION,
};
use castscreen_network::{DiscoveryResponder, MpegTsDemuxer, ReceiverStats, SrtReceiver};
use eframe::egui::{self, Color32, Layout, Rect, RichText, Rounding, Stroke, Vec2};
use openh264::formats::YUVSource;
use std::time::Instant;

/// Decodes an interleaved i16 LE PCM payload into f32 samples in [-1.0, 1.0].
fn decode_pcm_i16(payload: &[u8]) -> Vec<f32> {
    let mut out = Vec::with_capacity(payload.len() / 2);
    for chunk in payload.chunks_exact(2) {
        let s = i16::from_le_bytes([chunk[0], chunk[1]]);
        out.push(s as f32 / 32768.0);
    }
    out
}

pub struct ReceiverGuiApp {
    receiver: SrtReceiver,
    _discovery_responder: DiscoveryResponder,
    demuxer: MpegTsDemuxer,
    audio_out: AudioOutput,
    video_texture: Option<egui::TextureHandle>,
    frame_dimensions: Option<(usize, usize)>,
    is_connected: bool,
    clean_capture_mode: bool,
    master_volume: f32,
    stats: ReceiverStats,
    left_vu: f32,
    right_vu: f32,
    buffer_drain: Vec<u8>,
    // Real decoded-frame rate measured over a rolling 1-second window.
    frames_in_window: u32,
    fps_window_start: Instant,
    current_fps: f32,
    h264_decoder: openh264::decoder::Decoder,
}

impl ReceiverGuiApp {
    pub fn new(receiver: SrtReceiver) -> Self {
        let device_name = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "Laptop-Stream".to_string());
        let discovery_responder = DiscoveryResponder::start(device_name, 9000);

        let audio_out = AudioOutput::start();
        audio_out.set_volume(0.85);

        Self {
            receiver,
            _discovery_responder: discovery_responder,
            demuxer: MpegTsDemuxer::new(),
            audio_out,
            video_texture: None,
            frame_dimensions: None,
            is_connected: false, // Disconnected until incoming packets arrive
            clean_capture_mode: false,
            master_volume: 0.85,
            stats: ReceiverStats::default(),
            left_vu: 0.0,
            right_vu: 0.0,
            buffer_drain: Vec::with_capacity(32768),
            frames_in_window: 0,
            fps_window_start: Instant::now(),
            current_fps: 0.0,
            h264_decoder: openh264::decoder::Decoder::new().expect("Failed to initialize OpenH264 decoder"),
        }
    }
}

impl eframe::App for ReceiverGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        configure_dark_studio_theme(ctx);

        // 0. Poll incoming network stream from SRT socket
        self.buffer_drain.clear();
        let bytes_read = self.receiver.receive_ts_chunk(&mut self.buffer_drain);
        self.stats = self.receiver.get_stats();

        // Ballistic decay of the VU meters each UI frame; real peaks lift them below.
        self.left_vu *= 0.85;
        self.right_vu *= 0.85;

        if bytes_read > 0 {
            self.demuxer.feed_ts_bytes(&self.buffer_drain);
        }

        // Connection state comes straight from the TCP link, not from guessing at
        // byte flow (a static screen with silent audio sends almost nothing).
        self.is_connected = self.stats.connected;
        if !self.is_connected {
            self.left_vu = 0.0;
            self.right_vu = 0.0;
        }

        // 0.05 Play any ready audio frames and drive the VU meters from real levels.
        self.audio_out.set_volume(self.master_volume);
        while let Some(audio_bytes) = self.demuxer.next_audio_frame() {
            let pcm = decode_pcm_i16(&audio_bytes);
            if pcm.is_empty() {
                continue;
            }
            let vu = AudioSubmixer::calculate_vu_meter(&pcm);
            self.left_vu = self.left_vu.max(vu.left_peak);
            self.right_vu = self.right_vu.max(vu.right_peak);
            self.audio_out.push_samples(&pcm);
        }

        // 0.1 Decode any ready video frames from demuxer into the GPU texture
        while let Some(frame_bytes) = self.demuxer.next_video_frame() {
            if let Ok(Some(yuv)) = self.h264_decoder.decode(&frame_bytes) {
                let width = yuv.dimensions().0 as usize;
                let height = yuv.dimensions().1 as usize;
                let mut rgba = vec![0u8; width * height * 4];
                yuv.write_rgba8(&mut rgba);

                let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba);

                if let Some(texture) = &mut self.video_texture {
                    texture.set(color_image, egui::TextureOptions::LINEAR);
                } else {
                    self.video_texture = Some(ctx.load_texture(
                        "castscreen_live_video",
                        color_image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
                self.frame_dimensions = Some((width, height));
                self.frames_in_window += 1;
            }
        }

        // Real measured display frame rate over a rolling 1-second window.
        let win = self.fps_window_start.elapsed().as_secs_f32();
        if win >= 1.0 {
            self.current_fps = self.frames_in_window as f32 / win;
            self.frames_in_window = 0;
            self.fps_window_start = Instant::now();
        }
        if !self.is_connected {
            self.current_fps = 0.0;
        }

        // Keep 60 FPS continuous repaint while connected
        if self.is_connected {
            ctx.request_repaint();
        }

        // Escape key exits clean capture mode
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.clean_capture_mode = false;
        }

        // 1. Top Bar (Hidden in Clean Capture Mode for OBS / TikTok)
        if !self.clean_capture_mode {
            egui::TopBottomPanel::top("receiver_top")
                .frame(
                    egui::Frame::none()
                        .fill(BG_PANEL)
                        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                        .inner_margin(10.0),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        draw_castscreen_logo(ui, 24.0);
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("CastScreen Preview")
                                .strong()
                                .color(ACCENT_BRAND)
                                .size(16.0),
                        );
                        ui.label(
                            RichText::new(format!("v{}", CURRENT_VERSION))
                                .color(TEXT_MUTED)
                                .size(12.0),
                        );

                        ui.add_space(16.0);
                        draw_live_badge(ui, self.is_connected, 0);

                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            // Clean Capture Mode Toggle (For OBS / TikTok)
                            let clean_btn = ui.add(
                                egui::Button::new(
                                    RichText::new("👁️ Modo Captura Limpia (OBS/TikTok)")
                                        .color(Color32::WHITE)
                                        .size(11.0),
                                )
                                .fill(ACCENT_BRAND)
                                .rounding(Rounding::same(6.0)),
                            );
                            if clean_btn.clicked() {
                                self.clean_capture_mode = true;
                            }

                            // Fullscreen Toggle
                            let is_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                            let fs_text = if is_fullscreen { "⛶ Salir Fullscreen" } else { "⛶ Pantalla Completa" };
                            if ui
                                .add(
                                    egui::Button::new(RichText::new(fs_text).size(11.0))
                                        .fill(BG_CONTROL)
                                        .rounding(Rounding::same(6.0)),
                                 )
                                .clicked()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
                            }

                            ui.label(
                                RichText::new("Puerto: 9000 (SRT)")
                                    .monospace()
                                    .color(TEXT_SECONDARY)
                                    .size(11.0),
                            );
                        });
                    });
                });

            // 2. Bottom Dock: Master Audio Monitor & Diagnostics
            egui::TopBottomPanel::bottom("receiver_bottom")
                .frame(
                    egui::Frame::none()
                        .fill(BG_PANEL)
                        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                        .inner_margin(12.0),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        // Left Section: Audio Master VU Meter
                        ui.label(RichText::new("🔊 AUDIO MASTER:").color(TEXT_MUTED).size(10.0));
                        ui.add_space(6.0);
                        draw_ballistic_vu_meter(ui, self.left_vu, self.right_vu, 160.0, 16.0);

                        ui.add_space(14.0);
                        ui.label(RichText::new("Volumen:").color(TEXT_SECONDARY).size(11.0));
                        ui.add_sized(
                            [90.0, 18.0],
                            egui::Slider::new(&mut self.master_volume, 0.0_f32..=1.0_f32)
                                .show_value(false),
                        );
                        ui.label(
                            RichText::new(format!("{:.0}%", self.master_volume * 100.0))
                                .monospace()
                                .color(TEXT_PRIMARY)
                                .size(11.0),
                        );

                        // Right Section: Telemetry Chips (real measured values)
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            let res_text = self
                                .frame_dimensions
                                .map(|(w, h)| format!("{}x{}", w, h))
                                .unwrap_or_else(|| "—".to_string());
                            ui.label(
                                RichText::new(format!(
                                    "Búfer: {} ms  |  {}  @ {:.0} FPS",
                                    self.stats.buffer_ms, res_text, self.current_fps
                                ))
                                .monospace()
                                .color(ACCENT_LIVE)
                                .size(11.0),
                            );
                            ui.label(
                                RichText::new(format!("{:.1} Mbps", self.stats.received_mbps))
                                    .monospace()
                                    .strong()
                                    .color(TEXT_PRIMARY)
                                    .size(12.0),
                            );
                        });
                    });
                });
        }

        // 3. Central Canvas: 60 FPS Live Viewport
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG_CANVAS))
            .show(ctx, |ui| {
                let available_rect = ui.available_rect_before_wrap();
                let avail_w = available_rect.width();
                let avail_h = available_rect.height();

                // Compute centered aspect ratio frame (from incoming stream or 16:9 default)
                let (frame_w, frame_h) = self.frame_dimensions.unwrap_or((1920, 1080));
                let aspect = frame_w as f32 / frame_h as f32;

                let (target_w, target_h) = if avail_w / avail_h > aspect {
                    (avail_h * aspect, avail_h)
                } else {
                    (avail_w, avail_w / aspect)
                };

                let offset_x = available_rect.min.x + (avail_w - target_w) / 2.0;
                let offset_y = available_rect.min.y + (avail_h - target_h) / 2.0;
                let video_rect = Rect::from_min_size(
                    egui::Pos2::new(offset_x, offset_y),
                    Vec2::new(target_w, target_h),
                );

                // CRITICAL FIX: Allocate the exact rect so egui's layout solver does not oscillate and flicker
                ui.allocate_rect(video_rect, egui::Sense::hover());
                let painter = ui.painter();

                // Draw video viewport container
                painter.rect_filled(video_rect, Rounding::same(6.0), Color32::from_rgb(11, 14, 20));
                let border_color = if self.is_connected { ACCENT_LIVE } else { BORDER_SUBTLE };
                painter.rect_stroke(video_rect, Rounding::same(6.0), Stroke::new(1.5_f32, border_color));

                if let Some(texture) = &self.video_texture {
                    // Paint the live video frame directly into the GPU canvas
                    painter.image(
                        texture.id(),
                        video_rect,
                        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );

                    // If not in Clean Capture Mode, show subtle live status in top-right
                    if !self.clean_capture_mode {
                        let badge_pos = egui::pos2(video_rect.max.x - 12.0, video_rect.min.y + 12.0);
                        painter.text(
                            badge_pos,
                            egui::Align2::RIGHT_TOP,
                            format!("🟢 {:.0} FPS · {:.1} Mbps", self.current_fps, self.stats.received_mbps),
                            egui::FontId::proportional(11.0),
                            ACCENT_LIVE,
                        );
                    }
                } else if self.is_connected {
                    // Active Video Signal indicator while waiting for first keyframe
                    painter.text(
                        video_rect.center() - Vec2::new(0.0, 15.0),
                        egui::Align2::CENTER_CENTER,
                        "🟢 SEÑAL DE FLUJO ACTIVA",
                        egui::FontId::proportional(16.0),
                        ACCENT_LIVE,
                    );

                    painter.text(
                        video_rect.center() + Vec2::new(0.0, 15.0),
                        egui::Align2::CENTER_CENTER,
                        format!(
                            "MPEG-TS recibiendo · Tasa: {:.1} Mbps · Sincronizando fotogramas...",
                            self.stats.received_mbps
                        ),
                        egui::FontId::proportional(12.0),
                        TEXT_SECONDARY,
                    );
                } else {
                    // Radar Waiting animation
                    let time = ui.input(|i| i.time);
                    let pulse_radius = 20.0 + (time * 40.0 % 50.0) as f32;
                    let alpha = (255.0 - (pulse_radius / 70.0 * 255.0)) as u8;

                    painter.circle_stroke(
                        video_rect.center() - Vec2::new(0.0, 20.0),
                        pulse_radius,
                        Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(99, 102, 241, alpha)),
                    );

                    painter.text(
                        video_rect.center() + Vec2::new(0.0, 35.0),
                        egui::Align2::CENTER_CENTER,
                        "🔴 ESPERANDO FLUJO EN EL PUERTO 9000 (SRT)",
                        egui::FontId::proportional(14.0),
                        Color32::from_rgb(255, 75, 110),
                    );

                    painter.text(
                        video_rect.center() + Vec2::new(0.0, 60.0),
                        egui::Align2::CENTER_CENTER,
                        "En tu PC Gaming, abre el Emisor y pulsa '▶ INICIAR TRANSMISIÓN'\n(Si pruebas en la misma PC, pon IP: 127.0.0.1)",
                        egui::FontId::proportional(11.0),
                        TEXT_MUTED,
                    );
                }

                // If in Clean Capture Mode, show a small floating hint to exit
                if self.clean_capture_mode {
                    let hint_rect = Rect::from_min_size(
                        egui::Pos2::new(video_rect.min.x + 10.0, video_rect.min.y + 10.0),
                        Vec2::new(260.0, 26.0),
                    );
                    painter.rect_filled(hint_rect, Rounding::same(4.0), Color32::from_rgba_unmultiplied(0, 0, 0, 160));
                    painter.text(
                        hint_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Modo Captura Limpia Activo - Presiona [ESC] para salir",
                        egui::FontId::proportional(10.0),
                        TEXT_SECONDARY,
                    );
                }
            });

        // Smooth 60 FPS pacing without uncapped repaint thrashing (eliminates windowed flicker)
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}
