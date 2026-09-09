#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CastScreen Sender - Main Desktop GUI Application (PC Gaming).
//!
//! Captures screen via DirectX 11 VRAM, loopback audio via WASAPI,
//! manages per-app mixing, and streams MPEG-TS over a TCP link on the LAN.

mod controller;
mod gui;

use anyhow::Result;
use castscreen_core::{CastScreenConfig, SingleInstanceGuard, CURRENT_VERSION};
use controller::StreamController;
use gui::SenderGuiApp;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    // Prevent launching duplicate sender processes
    let _guard = SingleInstanceGuard::new("CastScreen_Sender_Mutex", "CastScreen Studio (Emisor PC Gaming)", true);
    if !_guard.is_primary() {
        return Ok(());
    }

    let mut config = CastScreenConfig::default();
    config.network.port = 9000;
    config.network.srt_latency_ms = 1000; // 1 second Wi-Fi buffer

    let controller = StreamController::new(config);

    let title = format!("CastScreen Studio v{} (Emisor PC Gaming)", CURRENT_VERSION);

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_min_inner_size([880.0, 580.0])
            .with_icon(castscreen_core::theme::load_window_icon())
            .with_title(&title),
        ..Default::default()
    };

    eframe::run_native(
        &title,
        native_options,
        Box::new(|cc| {
            // Fonts take effect on the next frame, so the design system is
            // installed before the first one runs.
            castscreen_core::configure_dark_studio_theme(&cc.egui_ctx);
            Ok(Box::new(SenderGuiApp::new(controller)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe error: {}", e))
}
