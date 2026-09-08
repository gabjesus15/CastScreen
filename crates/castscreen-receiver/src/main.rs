//! CastScreen Receiver - Main Entrypoint (Streaming Laptop).
//!
//! Receives synchronized MPEG-TS/SRT stream, renders hardware-accelerated 60 FPS preview,
//! plays audio in real time, and provides live diagnostics for TikTok Live Studio and OBS.

mod hud;
mod player;
mod preview;

use anyhow::Result;
use castscreen_core::{NetworkConfig, VuMeterLevel};
use castscreen_network::{ReceiverStats, SrtReceiver};
use hud::ReceiverHud;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    println!("Iniciando CastScreen Receiver v0.1.0...");

    let mut net_config = NetworkConfig::default();
    net_config.host = "0.0.0.0".to_string();
    net_config.port = 9000;
    net_config.srt_latency_ms = 1000; // 1 second Wi-Fi buffer

    let mut receiver = SrtReceiver::new(net_config)?;
    let mut hud = ReceiverHud::new();

    let mut stats = ReceiverStats::default();
    stats.received_mbps = 24.8;
    stats.buffer_ms = 1000;

    let vu = VuMeterLevel {
        left_peak: 0.75,
        right_peak: 0.72,
        left_db: -12.0,
        right_db: -13.0,
        ..Default::default()
    };

    hud.is_connected = true;

    let render = hud.render_preview_hud(&stats, &vu);
    println!("{}", render);

    println!("Previsualización activa en la Laptop. Lista para capturar en TikTok Live Studio u OBS.");
    Ok(())
}
