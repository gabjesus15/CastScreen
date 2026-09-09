#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CastScreen Receiver - Main Desktop GUI Application (Streaming Laptop).
//!
//! Receives the synchronized MPEG-TS stream over TCP, renders a 60 FPS preview,
//! plays audio in real time, and provides clean window capture for TikTok Live Studio and OBS.

mod audio_out;
mod hud;

use anyhow::Result;
use castscreen_core::{NetworkConfig, SingleInstanceGuard, CURRENT_VERSION};
use castscreen_network::SrtReceiver;
use hud::ReceiverGuiApp;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    // Prevent launching duplicate receiver processes
    let _guard = SingleInstanceGuard::new("CastScreen_Receiver_Mutex", "CastScreen Receiver (Laptop)", true);
    if !_guard.is_primary() {
        return Ok(());
    }

    let mut net_config = NetworkConfig::default();
    net_config.host = "0.0.0.0".to_string();
    net_config.port = 9000;
    net_config.srt_latency_ms = 1000; // 1 second Wi-Fi buffer

    let receiver = SrtReceiver::new(net_config)?;

    let title = format!("CastScreen Receiver v{} (Live Preview 60 FPS)", CURRENT_VERSION);

    let native_options = eframe::NativeOptions {
        vsync: true,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_fullscreen(true)
            .with_min_inner_size([800.0, 500.0])
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
            Ok(Box::new(ReceiverGuiApp::new(receiver)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe error: {}", e))
}
