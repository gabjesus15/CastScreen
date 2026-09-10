//! The sender window — the mixing desk on the gaming PC.
//!
//! This window is used while doing something else, which decides almost every
//! choice in it:
//!
//! - **One primary action.** Starting and stopping the stream is the only
//!   filled button on screen, always in the same corner, always named after
//!   what it will do. Everything else is quieter than it.
//! - **Controls sit next to what they change.** The destination address lives
//!   with the receivers found on the network, not up in the window chrome
//!   beside the close button; the monitor picker lives with the screens.
//! - **Status is continuous, not announced.** Meters, throughput and link state
//!   are always visible at a glance, so nothing has to interrupt to be seen.
//! - **The stream is protected, once.** Closing mid-broadcast asks — because it
//!   is genuinely irreversible and cuts a live audience — and nothing else does,
//!   so the question still means something when it appears.

use crate::controller::StreamController;
use castscreen_capture::AudioSessionController;
use castscreen_core::{
    button, chip, configure_dark_studio_theme, draw_buffer_health_bar, draw_castscreen_logo,
    draw_live_badge, draw_vu_meter, fill_screen, material, material_accented, scroll_edge, section_label, sheet,
    sheet_header, space, spring, stat_row, switch, text as ty, update_chip, update_sheet, AppUpdater,
    launch_launcher, ButtonStyle, Dismiss, Fill, Layer, Motion, FILL_SCREEN_FRAMES, TrayNotifier, UpdateState, ACCENT_BRAND,
    ACCENT_DANGER, ACCENT_LIVE, ACCENT_WARN, CURRENT_VERSION, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use castscreen_network::DiscoveryScanner;
use eframe::egui::{self, Align, Layout, Vec2};
use std::collections::HashMap;
use std::time::Instant;

/// How often the active sound sessions are re-enumerated.
const SESSION_REFRESH: std::time::Duration = std::time::Duration::from_millis(1500);

/// What the pending confirmation is actually for.
///
/// Both ways out of this window end the broadcast, so both ask the same
/// question — but each says what will happen next, because "are you sure" with
/// no consequence named is a question nobody can answer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leaving {
    /// Back to the mode picker.
    SwitchMode,
    /// Close CastScreen on this PC.
    Quit,
}

pub struct SenderGuiApp {
    controller: StreamController,
    discovery_scanner: DiscoveryScanner,
    updater: AppUpdater,
    target_ip: String,
    target_port: u16,
    stream_start_time: Option<Instant>,
    last_session_refresh: Instant,
    /// Per-application volume the user has set by hand this session.
    ///
    /// The session list is re-enumerated every 1.5 s, so without this a fader
    /// would spring back to the system's value the moment it was let go.
    volume_overrides: HashMap<u32, f32>,
    /// Set while the window is asking permission to end a live broadcast.
    leaving: Option<Leaving>,
    show_leave_sheet: bool,
    show_update_sheet: bool,
    /// Counts down while the window is still being asked to fill the screen.
    fill_frames: u32,
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
            volume_overrides: HashMap::new(),
            leaving: None,
            show_leave_sheet: false,
            show_update_sheet: false,
            fill_frames: FILL_SCREEN_FRAMES,
        }
    }

    fn start_stream(&mut self) {
        let target = format!("{}:{}", self.target_ip, self.target_port);
        if self.controller.start_streaming(Some(target)).is_ok() {
            self.stream_start_time = Some(Instant::now());
            TrayNotifier::send_notification(
                "CastScreen",
                "Transmitiendo hacia tu laptop por DirectX 11 y NVENC.",
                false,
            );
        }
    }

    /// Ask first if there is a live audience to lose; otherwise just go.
    fn request_leave(&mut self, ctx: &egui::Context, intent: Leaving, streaming: bool) {
        if streaming {
            self.leaving = Some(intent);
            self.show_leave_sheet = true;
            return;
        }
        self.perform_leave(ctx, intent);
    }

    fn perform_leave(&mut self, ctx: &egui::Context, intent: Leaving) {
        self.controller.stop_streaming();
        self.stream_start_time = None;
        TrayNotifier::remove();
        if intent == Leaving::SwitchMode {
            let _ = launch_launcher();
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn stop_stream(&mut self) {
        self.controller.stop_streaming();
        self.stream_start_time = None;
        TrayNotifier::send_notification("CastScreen", "Transmisión detenida.", false);
    }
}

impl eframe::App for SenderGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        configure_dark_studio_theme(ctx);

        fill_screen(ctx, &mut self.fill_frames, Fill::Maximized);

        let update_state = self.updater.get_state();
        if matches!(update_state, UpdateState::Downloading { .. }) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        if self.last_session_refresh.elapsed() > SESSION_REFRESH {
            self.controller.refresh_audio_sessions();
            self.last_session_refresh = Instant::now();
        }

        let snapshot = self.controller.get_snapshot();
        let elapsed_secs = self
            .stream_start_time
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);

        // Closing mid-stream is the one irreversible thing this window can do,
        // so it is the one thing that asks.
        if ctx.input(|i| i.viewport().close_requested()) {
            if snapshot.is_streaming {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.leaving = Some(Leaving::Quit);
                self.show_leave_sheet = true;
            } else {
                TrayNotifier::remove();
            }
        }

        self.chrome(ctx, &snapshot, elapsed_secs, &update_state);
        self.body(ctx, &snapshot);
        self.leave_sheet(ctx);
        update_sheet(ctx, &self.updater, &mut self.show_update_sheet);

        // A live window repaints continuously so the meters read as live; an
        // idle one repaints only when something asks, so it costs the game
        // nothing while it sits in the background.
        if snapshot.is_streaming {
            ctx.request_repaint();
        }
    }
}

impl SenderGuiApp {
    /// Identity and state on the left, the one primary action on the right.
    fn chrome(
        &mut self,
        ctx: &egui::Context,
        snapshot: &crate::controller::SenderStateSnapshot,
        elapsed_secs: u64,
        update_state: &UpdateState,
    ) {
        egui::TopBottomPanel::top("sender_chrome")
            .frame(
                material(Layer::Chrome)
                    .inner_margin(egui::Margin::symmetric(space::LG, space::MD)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Top-left, in both roles, always meaning the same thing:
                    // one step back out of whatever you are in.
                    if button(ui, ty::CAPTION.text("←  Cambiar de modo"), ButtonStyle::Quiet)
                        .on_hover_text("Vuelve al selector para usar esta PC como receptor")
                        .clicked()
                    {
                        self.request_leave(ctx, Leaving::SwitchMode, snapshot.is_streaming);
                    }
                    ui.add_space(space::SM);
                    draw_castscreen_logo(ui, 24.0);
                    ui.add_space(space::SM);
                    ui.label(ty::HEADLINE.colored("CastScreen", TEXT_PRIMARY));
                    chip(ui, format!("v{CURRENT_VERSION}"), TEXT_MUTED);
                    ui.add_space(space::MD);
                    draw_live_badge(ui, snapshot.is_streaming, elapsed_secs);

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // The primary action names its consequence, so nobody
                        // has to remember what state the stream is in to know
                        // what this button will do.
                        if snapshot.is_streaming {
                            if button(
                                ui,
                                ty::BODY_EMPHASIS.text("Detener transmisión"),
                                ButtonStyle::Tinted(ACCENT_DANGER),
                            )
                            .clicked()
                            {
                                self.stop_stream();
                            }
                        } else if button(
                            ui,
                            ty::BODY_EMPHASIS.text("Iniciar transmisión"),
                            ButtonStyle::Primary(ACCENT_LIVE),
                        )
                        .clicked()
                        {
                            self.start_stream();
                        }

                        if button(ui, ty::CAPTION.text("Minimizar a bandeja"), ButtonStyle::Quiet)
                            .on_hover_text("La transmisión sigue activa en segundo plano")
                            .clicked()
                        {
                            TrayNotifier::send_notification(
                                "CastScreen Emisor",
                                "La transmisión sigue activa en segundo plano.",
                                false,
                            );
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }

                        if update_chip(ui, update_state) {
                            self.show_update_sheet = true;
                        }

                        if button(ui, ty::CAPTION.text("Salir"), ButtonStyle::Quiet).clicked() {
                            self.request_leave(ctx, Leaving::Quit, snapshot.is_streaming);
                        }
                    });
                });
            });
    }

    fn body(&mut self, ctx: &egui::Context, snapshot: &crate::controller::SenderStateSnapshot) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(space::LG))
            .show(ctx, |ui| {
                ui.columns(2, |columns| {
                    self.mixer_panel(&mut columns[0], snapshot);
                    self.network_panel(&mut columns[1], snapshot);
                });
            });
    }

    /// Everything about sound, in one place.
    fn mixer_panel(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &crate::controller::SenderStateSnapshot,
    ) {
        material(Layer::Surface).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(ty::HEADLINE.colored("Mezcla de audio", TEXT_PRIMARY));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if button(ui, ty::CAPTION.text("Volver a buscar"), ButtonStyle::Quiet).clicked() {
                        self.controller.refresh_audio_sessions();
                    }
                });
            });
            ui.add_space(space::MD);

            section_label(ui, "Salida maestra");
            draw_vu_meter(
                ui,
                snapshot.master_vu.left_peak,
                snapshot.master_vu.right_peak,
                ui.available_width(),
                16.0,
            );
            ui.add_space(space::XS);
            ui.label(ty::CAPTION.colored(
                "Todo lo que suena en esta PC, tal como lo oirá tu audiencia.",
                TEXT_MUTED,
            ));

            ui.add_space(space::LG);
            section_label(ui, "Aplicaciones con sonido");

            if snapshot.detected_apps.is_empty() {
                // An empty state that says what would fill it beats one that
                // only reports that it is empty.
                ui.label(ty::CALLOUT.colored(
                    "Nada está sonando ahora mismo. En cuanto una aplicación reproduzca audio aparecerá aquí con su propio control.",
                    TEXT_MUTED,
                ));
                return;
            }

            let list_top = ui.cursor().min;
            egui::ScrollArea::vertical().id_source("mixer-apps").show(ui, |ui| {
                for app in &snapshot.detected_apps {
                    self.app_strip(ui, app);
                    ui.add_space(space::XS);
                }
            });
            // Rows dissolve into the section heading instead of being cut by it.
            scroll_edge(
                ui.painter(),
                egui::Rect::from_min_size(list_top, Vec2::new(ui.available_width(), 12.0)),
                Layer::Surface.fill(),
                12.0,
            );
        });
    }

    /// One application's strip: who it is, how loud it is, and two controls.
    fn app_strip(&mut self, ui: &mut egui::Ui, app: &castscreen_capture::AudioAppSession) {
        material(Layer::Raised).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(ty::BODY_EMPHASIS.colored(&app.process_name, TEXT_PRIMARY));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // The switch answers "is this in the stream?", which is the
                    // question the mixer exists to answer — so it is phrased
                    // that way round rather than as a mute toggle.
                    let mut included = !app.is_muted;
                    if switch(ui, &mut included, ACCENT_LIVE).changed() {
                        let _ = AudioSessionController::set_process_mute(app.process_id, !included);
                    }
                    ui.label(ty::CAPTION.colored(
                        if app.is_muted { "Fuera del stream" } else { "En el stream" },
                        if app.is_muted { TEXT_MUTED } else { ACCENT_LIVE },
                    ));
                });
            });

            ui.add_space(space::SM);
            ui.horizontal(|ui| {
                let mut volume = self
                    .volume_overrides
                    .get(&app.process_id)
                    .copied()
                    .unwrap_or(app.volume);

                let fader_width = (ui.available_width() - 110.0).max(80.0);
                if castscreen_core::fader(ui, &mut volume, fader_width, ACCENT_BRAND).changed() {
                    self.volume_overrides.insert(app.process_id, volume);
                    let _ = AudioSessionController::set_process_volume(app.process_id, volume);
                }
                ui.label(ty::CAPTION.mono_colored(
                    format!("{:>3.0} %", volume * 100.0),
                    TEXT_SECONDARY,
                ));
            });

            ui.add_space(space::XS);
            draw_vu_meter(ui, app.peak_meter, app.peak_meter, ui.available_width(), 8.0);
        });
    }

    /// Where the stream is going, what it is capturing, and how it is doing.
    fn network_panel(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &crate::controller::SenderStateSnapshot,
    ) {
        material(Layer::Surface).show(ui, |ui| {
            egui::ScrollArea::vertical().id_source("network-panel").show(ui, |ui| {
                let error_visible = snapshot.pipeline_error.is_some();
                let error_t = spring(ui.ctx(), egui::Id::new("network_error_spring"), if error_visible { 1.0 } else { 0.0 }, Motion::SHEET);
                
                if error_t > 0.001 {
                    if let Some(error) = &snapshot.pipeline_error {
                        // Smoothly fade the error in/out
                        ui.set_opacity(error_t);
                        material_accented(Layer::Raised, ACCENT_DANGER).show(ui, |ui| {
                            ui.label(ty::BODY_EMPHASIS.colored("La captura se detuvo", ACCENT_DANGER));
                            ui.add_space(space::XS);
                            ui.label(ty::CALLOUT.colored(error.as_str(), TEXT_SECONDARY));
                        });
                        ui.add_space(space::MD);
                        ui.set_opacity(1.0); // Reset for the rest of the panel
                    }
                }

                ui.label(ty::HEADLINE.colored("Destino", TEXT_PRIMARY));
                ui.add_space(space::MD);

                // The address and the devices that could fill it sit together,
                // because they are one decision.
                ui.horizontal(|ui| {
                    ui.label(ty::CALLOUT.colored("Laptop", TEXT_SECONDARY));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.target_ip)
                            .desired_width(120.0)
                            .margin(Vec2::new(space::SM, space::XS)),
                    );
                    ui.label(ty::CAPTION.mono_colored(format!(":{}", self.target_port), TEXT_MUTED));
                });
                ui.add_space(space::SM);

                let devices = self.discovery_scanner.get_devices();
                if devices.is_empty() {
                    ui.label(ty::CAPTION.colored(
                        "Buscando laptops con el receptor abierto en esta red…",
                        TEXT_MUTED,
                    ));
                } else {
                    for device in &devices {
                        let selected = self.target_ip == device.ip;
                        let accent = if selected { ACCENT_LIVE } else { ACCENT_BRAND };
                        material(Layer::Raised)
                            .inner_margin(space::SM)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        ty::CALLOUT.colored(&device.device_name, TEXT_PRIMARY),
                                    );
                                    ui.label(ty::CAPTION.mono_colored(&device.ip, TEXT_MUTED));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if selected {
                                            chip(ui, "Destino actual", accent);
                                        } else if button(
                                            ui,
                                            ty::CAPTION.text("Usar esta"),
                                            ButtonStyle::Tinted(accent),
                                        )
                                        .clicked()
                                        {
                                            self.target_ip = device.ip.clone();
                                        }
                                    });
                                });
                            });
                        ui.add_space(space::XXS);
                    }
                }

                if snapshot.is_streaming {
                    ui.add_space(space::SM);
                    if snapshot.network_connected {
                        chip(ui, format!("Conectado a {}", self.target_ip), ACCENT_LIVE);
                    } else {
                        chip(ui, format!("Conectando a {}…", self.target_ip), ACCENT_WARN);
                    }
                }

                ui.add_space(space::XL);
                ui.label(ty::HEADLINE.colored("Pantalla", TEXT_PRIMARY));
                ui.add_space(space::MD);
                self.monitor_picker(ui, snapshot);

                ui.add_space(space::XL);
                ui.label(ty::HEADLINE.colored("Estado del envío", TEXT_PRIMARY));
                ui.add_space(space::MD);
                self.telemetry(ui, snapshot);
            });
        });
    }

    fn monitor_picker(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &crate::controller::SenderStateSnapshot,
    ) {
        if snapshot.detected_monitors.is_empty() {
            ui.horizontal(|ui| {
                ui.label(ty::CALLOUT.colored("Pantalla principal", TEXT_SECONDARY));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if button(ui, ty::CAPTION.text("Buscar pantallas"), ButtonStyle::Quiet).clicked()
                    {
                        self.controller.refresh_monitors();
                    }
                });
            });
            // We still want to show the Virtual Monitor option if no extra screens are found
        }
        
        // Virtual monitor button
        ui.add_space(space::SM);
        if !castscreen_virtual_monitor::is_installed() {
            ui.horizontal(|ui| {
                if button(
                    ui,
                    ty::CAPTION.text("Instalar Monitor Virtual (Beta)"),
                    ButtonStyle::Tinted(ACCENT_BRAND),
                ).on_hover_text("Extiende el escritorio usando un driver IDD (requiere permisos de Administrador)").clicked()
                {
                    if let Err(e) = castscreen_virtual_monitor::install() {
                        tracing::error!("Failed to install virtual monitor: {e}");
                    }
                }
            });
            ui.add_space(space::SM);
        }

        for monitor in &snapshot.detected_monitors {
            let selected = monitor.index == snapshot.selected_monitor;
            let frame = if selected {
                material_accented(Layer::Raised, ACCENT_BRAND)
            } else {
                material(Layer::Raised)
            };
            let clicked = frame
                .inner_margin(space::SM)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(ty::CALLOUT.colored(
                            format!("Pantalla {}", monitor.index + 1),
                            if selected { TEXT_PRIMARY } else { TEXT_SECONDARY },
                        ));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if selected {
                                chip(ui, "Capturando", ACCENT_BRAND);
                            }
                            ui.label(ty::CAPTION.mono_colored(
                                format!("{}×{}", monitor.width, monitor.height),
                                TEXT_MUTED,
                            ));
                        });
                    });
                })
                .response
                .interact(egui::Sense::click())
                .clicked();

            if clicked && !selected {
                self.controller.set_selected_monitor(monitor.index);
            }
            ui.add_space(space::XXS);
        }
    }

    fn telemetry(&self, ui: &mut egui::Ui, snapshot: &crate::controller::SenderStateSnapshot) {
        let live = snapshot.is_streaming;
        let sending = snapshot.bitrate_mbps > 0.05;

        stat_row(
            ui,
            "Captura",
            format!("{:.0} FPS", snapshot.current_fps),
            if live { ACCENT_LIVE } else { TEXT_MUTED },
        );
        stat_row(
            ui,
            "Tasa de bits",
            format!("{:.1} Mbps", snapshot.bitrate_mbps),
            if sending { ACCENT_BRAND } else { TEXT_MUTED },
        );
        stat_row(
            ui,
            "Enviado",
            format!("{:.1} MB", snapshot.total_bytes_sent as f64 / (1024.0 * 1024.0)),
            TEXT_SECONDARY,
        );

        ui.add_space(space::MD);
        // The gauge is derived from real throughput, so an idle sender reads as
        // idle rather than as a healthy link with nothing on it.
        let health = (snapshot.bitrate_mbps.max(0.0) * 40.0).min(1000.0) as u32;
        draw_buffer_health_bar(ui, health, 1000, ui.available_width());
        ui.add_space(space::XS);
        ui.label(ty::CAPTION.colored(
            if !live {
                "Sin transmitir."
            } else if sending {
                "Enviando a la laptop."
            } else {
                "Esperando que la laptop acepte la conexión."
            },
            TEXT_MUTED,
        ));
    }

    /// The only confirmation in this window, for the only irreversible thing.
    fn leave_sheet(&mut self, ctx: &egui::Context) {
        let Some(intent) = self.leaving else {
            return;
        };
        let (detail, confirm) = match intent {
            Leaving::SwitchMode => (
                "Si cambias de modo ahora, lo que ve tu audiencia en TikTok u OBS se corta y esta PC pasa a recibir.",
                "Detener y cambiar",
            ),
            Leaving::Quit => (
                "Si sales ahora, lo que ve tu audiencia en TikTok u OBS se corta de inmediato.",
                "Detener y salir",
            ),
        };

        let mut confirmed = false;
        sheet(
            ctx,
            "sender-leave",
            &mut self.show_leave_sheet,
            |ui| {
                sheet_header(ui, "Hay una transmisión en vivo", detail);
                ui.horizontal(|ui| {
                    // The safe path is the filled one, and it comes first.
                    let keep = button(
                        ui,
                        ty::BODY_EMPHASIS.text("Seguir transmitiendo"),
                        ButtonStyle::Primary(ACCENT_LIVE),
                    )
                    .clicked();
                    if button(ui, confirm, ButtonStyle::Tinted(ACCENT_DANGER)).clicked() {
                        confirmed = true;
                    }
                    keep
                })
                .inner
            },
            Dismiss::Deliberate,
        );

        if confirmed {
            self.perform_leave(ctx, intent);
        }
        if !self.show_leave_sheet {
            self.leaving = None;
        }
    }
}
