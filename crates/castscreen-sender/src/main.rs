//! CastScreen Sender - Main Entrypoint (PC Gaming).
//!
//! Captures screen via DirectX 11 VRAM, loopback audio via WASAPI,
//! manages per-app mixing, and streams via SRT with an ARQ jitter buffer.

mod controller;
mod gui;

use anyhow::Result;
use castscreen_core::CastScreenConfig;
use controller::StreamController;
use gui::DashboardUi;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    println!("Iniciando CastScreen Sender v0.1.0...");

    let mut config = CastScreenConfig::default();
    config.network.port = 9000;
    config.network.srt_latency_ms = 1000; // 1 second Wi-Fi buffer

    let mut controller = StreamController::new(config);
    let mut ui = DashboardUi::new();

    // Initial scan of active sound sessions (Discord, Spotify, games)
    controller.refresh_audio_sessions();

    let snapshot = controller.get_snapshot();
    let render = ui.render_console_view(&snapshot);
    println!("{}", render);

    println!("CastScreen Sender listo. Conecta tu laptop de streaming a esta PC.");
    Ok(())
}
