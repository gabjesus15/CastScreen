#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CastScreen Sender - Main Desktop GUI Application (PC Gaming).
//!
//! Captures screen via DirectX 11 VRAM, loopback audio via WASAPI,
//! manages per-app mixing, and streams via SRT with an ARQ jitter buffer.

mod controller;
mod gui;

use anyhow::Result;
use castscreen_core::{CastScreenConfig, SingleInstanceGuard};
use controller::StreamController;
use gui::SenderGuiApp;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    // Prevent launching duplicate sender processes
    let _guard = SingleInstanceGuard::new("CastScreen_Sender_Mutex", "CastScreen Studio (Emisor)", true);
    if !_guard.is_primary() {
        return Ok(());
    }

    let mut config = CastScreenConfig::default();
    config.network.port = 9000;
    config.network.srt_latency_ms = 1000; // 1 second Wi-Fi buffer

    let mut controller = StreamController::new(config);
    controller.refresh_audio_sessions();

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([940.0, 640.0])
            .with_min_inner_size([880.0, 580.0])
            .with_title("CastScreen Studio (Emisor PC Gaming)"),
        ..Default::default()
    };

    eframe::run_native(
        "CastScreen Studio",
        native_options,
        Box::new(|_cc| Ok(Box::new(SenderGuiApp::new(controller)))),
    )
    .map_err(|e| anyhow::anyhow!("Eframe error: {}", e))
}
