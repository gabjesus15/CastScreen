//! The receiver window — the live preview on the streaming laptop.
//!
//! This window has two audiences at once: the person watching it, and the
//! capture software pointed at it. That tension decides its design:
//!
//! - **The picture is the interface.** Chrome frames the video; it never sits
//!   on top of it. Anything drawn inside the video rectangle will be captured
//!   by OBS, so almost nothing is.
//! - **Clean capture mode means clean.** No border, no badge, no permanent
//!   hint. The instruction for leaving fades out on its own, because a hint
//!   that stays would be composited into the broadcast forever.
//! - **Waiting is a state, not an error.** A laptop sitting ready before the
//!   game PC starts is working correctly, so it says what to do next in a calm
//!   voice instead of alarming in red.

use crate::audio_out::AudioOutput;
use castscreen_core::{
    button, chip, configure_dark_studio_theme, draw_castscreen_logo, draw_live_badge,
    draw_vu_meter, fader, launch_launcher, material, prefers_reduced_motion, sheet, sheet_header,
    space, text as ty, AudioSubmixer, ButtonStyle, Dismiss, Layer, ACCENT_BRAND, ACCENT_DANGER,
    ACCENT_LIVE, BG_CANVAS, BORDER_SUBTLE, CURRENT_VERSION, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use castscreen_network::{DiscoveryResponder, MpegTsDemuxer, ReceiverStats, SrtReceiver};
use eframe::egui::{self, Align, Align2, Color32, Layout, Rect, Rounding, Stroke, Vec2};
use openh264::formats::YUVSource;
use std::time::Instant;

/// How long the "press Escape to leave" hint stays up in clean capture mode.
///
/// Long enough to read, short enough that it is gone before anyone goes live.
const CLEAN_HINT_SECONDS: f32 = 4.0;

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
    clean_mode_since: Option<Instant>,
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
    record_file: Option<std::fs::File>,
    /// Set while the window is asking permission to drop a live link.
    show_leave_sheet: bool,
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
            clean_mode_since: None,
            master_volume: 0.85,
            stats: ReceiverStats::default(),
            left_vu: 0.0,
            right_vu: 0.0,
            buffer_drain: Vec::with_capacity(32768),
            frames_in_window: 0,
            fps_window_start: Instant::now(),
            current_fps: 0.0,
            h264_decoder: openh264::decoder::Decoder::new()
                .expect("Failed to initialize OpenH264 decoder"),
            record_file: None,
            show_leave_sheet: false,
        }
    }

    /// Ask first if leaving would drop a live picture or truncate a recording.
    fn request_switch_mode(&mut self, ctx: &egui::Context) {
        if self.is_connected || self.record_file.is_some() {
            self.show_leave_sheet = true;
            return;
        }
        self.switch_mode(ctx);
    }

    fn switch_mode(&mut self, ctx: &egui::Context) {
        // Dropping the handle flushes and closes the recording first.
        self.record_file = None;
        let _ = launch_launcher();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Drain the socket, demux, decode, and drive the meters.
    fn pump_stream(&mut self, ctx: &egui::Context) {
        self.buffer_drain.clear();
        let bytes_read = self.receiver.receive_ts_chunk(&mut self.buffer_drain);
        self.stats = self.receiver.get_stats();

        // Ballistic decay each UI frame; real peaks lift the meters back up.
        self.left_vu *= 0.85;
        self.right_vu *= 0.85;

        if bytes_read > 0 {
            self.demuxer.feed_ts_bytes(&self.buffer_drain);
            if let Some(file) = &mut self.record_file {
                use std::io::Write;
                let _ = file.write_all(&self.buffer_drain);
            }
        }

        // Connection state comes straight from the TCP link, not from guessing
        // at byte flow — a static screen with silent audio sends almost nothing.
        self.is_connected = self.stats.connected;
        if !self.is_connected {
            self.left_vu = 0.0;
            self.right_vu = 0.0;
        }

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

        let window = self.fps_window_start.elapsed().as_secs_f32();
        if window >= 1.0 {
            self.current_fps = self.frames_in_window as f32 / window;
            self.frames_in_window = 0;
            self.fps_window_start = Instant::now();
        }
        if !self.is_connected {
            self.current_fps = 0.0;
        }
    }
}

impl eframe::App for ReceiverGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        configure_dark_studio_theme(ctx);
        self.pump_stream(ctx);

        // Escape is the way out of clean capture mode, and the hint says so
        // while it is still on screen.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.clean_capture_mode {
            self.clean_capture_mode = false;
            self.clean_mode_since = None;
        }

        if !self.clean_capture_mode {
            self.chrome(ctx);
            self.audio_dock(ctx);
        }
        self.viewport(ctx);
        self.leave_sheet(ctx);

        // 60 FPS pacing without uncapped repaint thrashing.
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

impl ReceiverGuiApp {
    fn chrome(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("receiver_chrome")
            .frame(
                material(Layer::Chrome)
                    .inner_margin(egui::Margin::symmetric(space::LG, space::SM)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Same control, same corner, same meaning as in the sender.
                    if button(ui, ty::CAPTION.text("←  Cambiar de modo"), ButtonStyle::Quiet)
                        .on_hover_text("Vuelve al selector para usar esta PC como emisor")
                        .clicked()
                    {
                        self.request_switch_mode(ctx);
                    }
                    ui.add_space(space::SM);
                    draw_castscreen_logo(ui, 24.0);
                    ui.add_space(space::SM);
                    ui.label(ty::HEADLINE.colored("Vista previa", TEXT_PRIMARY));
                    chip(ui, format!("v{CURRENT_VERSION}"), TEXT_MUTED);
                    ui.add_space(space::MD);
                    draw_live_badge(ui, self.is_connected, 0);

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // The primary action of this window is handing a clean
                        // picture to OBS, so it is the only filled button.
                        if button(
                            ui,
                            ty::BODY_EMPHASIS.text("Modo captura limpia"),
                            ButtonStyle::Primary(ACCENT_BRAND),
                        )
                        .on_hover_text("Oculta toda la interfaz. Escape para volver.")
                        .clicked()
                        {
                            self.clean_capture_mode = true;
                            self.clean_mode_since = Some(Instant::now());
                        }

                        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                        let label = if fullscreen { "Salir de pantalla completa" } else { "Pantalla completa" };
                        if button(ui, ty::CAPTION.text(label), ButtonStyle::Quiet).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
                        }

                        let recording = self.record_file.is_some();
                        let (label, style) = if recording {
                            ("Detener grabación", ButtonStyle::Tinted(ACCENT_DANGER))
                        } else {
                            ("Grabar a disco", ButtonStyle::Quiet)
                        };
                        if button(ui, ty::CAPTION.text(label), style).clicked() {
                            if recording {
                                // Dropping the handle closes the file.
                                self.record_file = None;
                            } else {
                                let stamp = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                self.record_file =
                                    std::fs::File::create(format!("CastScreen_{stamp}.ts")).ok();
                            }
                        }

                        chip(ui, "Puerto 9000", TEXT_MUTED);
                    });
                });
            });
    }

    fn audio_dock(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("receiver_dock")
            .frame(
                material(Layer::Chrome)
                    .inner_margin(egui::Margin::symmetric(space::LG, space::SM)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(ty::OVERLINE.colored("AUDIO", TEXT_MUTED));
                    ui.add_space(space::SM);
                    draw_vu_meter(ui, self.left_vu, self.right_vu, 150.0, 16.0);

                    ui.add_space(space::LG);
                    // The fader sits right beside the meter it moves, so the
                    // effect of dragging it is visible in the same glance.
                    fader(ui, &mut self.master_volume, 110.0, ACCENT_BRAND);
                    ui.label(ty::CAPTION.mono_colored(
                        format!("{:>3.0} %", self.master_volume * 100.0),
                        TEXT_SECONDARY,
                    ));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let resolution = self
                            .frame_dimensions
                            .map(|(w, h)| format!("{w}×{h}"))
                            .unwrap_or_else(|| "—".to_string());
                        ui.label(ty::CAPTION.mono_colored(
                            format!("{:.1} Mbps", self.stats.received_mbps),
                            TEXT_PRIMARY,
                        ));
                        ui.label(ty::CAPTION.colored("·", TEXT_MUTED));
                        ui.label(ty::CAPTION.mono_colored(
                            format!("{resolution} a {:.0} FPS", self.current_fps),
                            if self.is_connected { ACCENT_LIVE } else { TEXT_MUTED },
                        ));
                        ui.label(ty::CAPTION.colored("·", TEXT_MUTED));
                        ui.label(ty::CAPTION.mono_colored(
                            format!("búfer {} ms", self.stats.buffer_ms),
                            TEXT_SECONDARY,
                        ));
                    });
                });
            });
    }

    fn viewport(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG_CANVAS))
            .show(ctx, |ui| {
                let available = ui.available_rect_before_wrap();
                let (frame_w, frame_h) = self.frame_dimensions.unwrap_or((1920, 1080));
                let aspect = frame_w as f32 / frame_h as f32;

                let (target_w, target_h) = if available.width() / available.height() > aspect {
                    (available.height() * aspect, available.height())
                } else {
                    (available.width(), available.width() / aspect)
                };
                let video_rect = Rect::from_center_size(
                    available.center(),
                    Vec2::new(target_w, target_h),
                );

                // Allocate the exact rect so egui's layout solver cannot
                // oscillate between two sizes and flicker.
                ui.allocate_rect(video_rect, egui::Sense::hover());
                let painter = ui.painter().clone();

                if let Some(texture) = &self.video_texture {
                    painter.image(
                        texture.id(),
                        video_rect,
                        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    painter.rect_filled(video_rect, Rounding::same(6.0), BG_CANVAS);
                    self.draw_placeholder(ui, &painter, video_rect);
                }

                // Everything below is chrome, and chrome must never end up
                // inside somebody's broadcast.
                if self.clean_capture_mode {
                    self.draw_clean_mode_hint(ui, &painter, video_rect);
                    return;
                }

                let border = if self.is_connected { ACCENT_LIVE } else { BORDER_SUBTLE };
                painter.rect_stroke(
                    video_rect,
                    Rounding::same(6.0),
                    Stroke::new(1.0_f32, border),
                );
            });
    }

    /// What to show when there is no picture yet.
    fn draw_placeholder(&self, ui: &egui::Ui, painter: &egui::Painter, rect: Rect) {
        let centre = rect.center();

        if self.is_connected {
            painter.text(
                centre - Vec2::new(0.0, 10.0),
                Align2::CENTER_CENTER,
                "Recibiendo señal",
                ty::TITLE.font_id(),
                TEXT_PRIMARY,
            );
            painter.text(
                centre + Vec2::new(0.0, 16.0),
                Align2::CENTER_CENTER,
                format!(
                    "{:.1} Mbps · esperando el primer fotograma completo",
                    self.stats.received_mbps
                ),
                ty::CALLOUT.font_id(),
                TEXT_SECONDARY,
            );
            return;
        }

        // Waiting is normal. One slow, low-contrast ring says the laptop is
        // listening; it stops entirely when the system asks for less motion.
        if !prefers_reduced_motion() {
            let phase = (ui.input(|i| i.time) * 0.5).fract() as f32;
            let radius = 26.0 + phase * 46.0;
            painter.circle_stroke(
                centre - Vec2::new(0.0, 42.0),
                radius,
                Stroke::new(
                    1.0_f32,
                    Color32::from_rgba_unmultiplied(99, 102, 241, ((1.0 - phase) * 90.0) as u8),
                ),
            );
        } else {
            painter.circle_stroke(
                centre - Vec2::new(0.0, 42.0),
                34.0,
                Stroke::new(1.0_f32, BORDER_SUBTLE),
            );
        }

        painter.text(
            centre + Vec2::new(0.0, 22.0),
            Align2::CENTER_CENTER,
            "Esta laptop está lista y escuchando",
            ty::TITLE.font_id(),
            TEXT_PRIMARY,
        );
        painter.text(
            centre + Vec2::new(0.0, 50.0),
            Align2::CENTER_CENTER,
            "En tu PC de juego abre CastScreen y pulsa «Iniciar transmisión».",
            ty::CALLOUT.font_id(),
            TEXT_SECONDARY,
        );
        painter.text(
            centre + Vec2::new(0.0, 72.0),
            Align2::CENTER_CENTER,
            "Si pruebas en esta misma PC, usa 127.0.0.1 como destino.",
            ty::CAPTION.font_id(),
            TEXT_MUTED,
        );
    }

    /// The way out of clean capture mode, shown once and then gone.
    fn draw_clean_mode_hint(&self, ui: &egui::Ui, painter: &egui::Painter, rect: Rect) {
        let Some(since) = self.clean_mode_since else {
            return;
        };
        let age = since.elapsed().as_secs_f32();
        if age > CLEAN_HINT_SECONDS {
            return;
        }

        // Fades over the last second so it leaves the way it arrived, and the
        // repaint keeps ticking until it is actually gone.
        let alpha = ((CLEAN_HINT_SECONDS - age).min(1.0)).clamp(0.0, 1.0);
        ui.ctx().request_repaint();

        let hint = Rect::from_min_size(
            rect.min + Vec2::new(space::MD, space::MD),
            Vec2::new(258.0, 28.0),
        );
        painter.rect_filled(
            hint,
            Rounding::same(6.0),
            Color32::from_black_alpha((150.0 * alpha) as u8),
        );
        painter.text(
            hint.center(),
            Align2::CENTER_CENTER,
            "Modo captura limpia · Escape para salir",
            ty::CAPTION.font_id(),
            Color32::from_rgba_unmultiplied(255, 255, 255, (220.0 * alpha) as u8),
        );
    }

    /// Confirmation, shown only when leaving would actually cost something.
    fn leave_sheet(&mut self, ctx: &egui::Context) {
        let recording = self.record_file.is_some();
        let mut confirmed = false;

        sheet(
            ctx,
            "receiver-leave",
            &mut self.show_leave_sheet,
            |ui| {
                sheet_header(
                    ui,
                    "Estás recibiendo señal",
                    if recording {
                        "Si cambias de modo ahora se cierra la grabación en curso y la vista previa deja de alimentar a OBS."
                    } else {
                        "Si cambias de modo ahora, la vista previa que está capturando OBS desaparece."
                    },
                );
                ui.horizontal(|ui| {
                    let keep = button(
                        ui,
                        ty::BODY_EMPHASIS.text("Seguir recibiendo"),
                        ButtonStyle::Primary(ACCENT_LIVE),
                    )
                    .clicked();
                    if button(ui, "Cambiar de modo", ButtonStyle::Tinted(ACCENT_DANGER)).clicked() {
                        confirmed = true;
                    }
                    keep
                })
                .inner
            },
            Dismiss::Casual,
        );

        if confirmed {
            self.switch_mode(ctx);
        }
    }
}
