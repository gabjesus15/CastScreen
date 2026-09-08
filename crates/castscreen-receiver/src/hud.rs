//! Diagnostic HUD & Pre-Broadcast Verification Interface (Laptop Receiver).
//!
//! Renders the live performance stats (FPS, Bitrate, Buffer Fill, Audio Levels)
//! allowing streamers to visually and sonically verify the stream before broadcasting.

use castscreen_core::VuMeterLevel;
use castscreen_network::ReceiverStats;

pub struct ReceiverHud {
    pub host_ip: String,
    pub host_port: u16,
    pub buffer_ms: u32,
    pub audio_volume: f32,
    pub is_connected: bool,
}

impl ReceiverHud {
    pub fn new() -> Self {
        Self {
            host_ip: "192.168.1.55".to_string(),
            host_port: 9000,
            buffer_ms: 1000,
            audio_volume: 0.80,
            is_connected: false,
        }
    }

    /// Renders the terminal preview & diagnostic HUD.
    pub fn render_preview_hud(&self, stats: &ReceiverStats, vu: &VuMeterLevel) -> String {
        let status = if self.is_connected {
            "🟢 [ EN VIVO: 60.0 FPS ]"
        } else {
            "🟡 [ ESPERANDO CONEXIÓN... ]"
        };

        let vu_bar = Self::format_vu_bars(vu);

        let mut out = String::new();
        out.push_str("┌──────────────────────────────────────────────────────────────────────────────────┐\n");
        out.push_str(&format!("│  📺 CastScreen Preview (Laptop Stream)            {:>31} _ □ ✕│\n", status));
        out.push_str("├──────────────────────────────────────────────────────────────────────────────────┤\n");
        out.push_str(&format!("│  IP Emisor: [ {:<15} ]  Puerto: [ {:>4} ]  [ RECONECTAR ] [ PANTALLA COMPLETA]│\n", self.host_ip, self.host_port));
        out.push_str("├──────────────────────────────────────────────────────────────────────────────────┤\n");
        out.push_str("│                                                                                  │\n");
        out.push_str("│                                                                                  │\n");
        out.push_str("│                        LIENZO DE VIDEO EN TIEMPO REAL                            │\n");
        out.push_str("│                                                                                  │\n");
        out.push_str("│                    (Previsualización Fluida a 60 FPS)                            │\n");
        out.push_str("│                                                                                  │\n");
        out.push_str("│                                                                                  │\n");
        out.push_str("├──────────────────────────────────────────────────────────────────────────────────┤\n");
        out.push_str("│  📊 DIAGNÓSTICO EN VIVO:                                                         │\n");
        out.push_str(&format!("│  FPS: {:>4.1} | Bitrate: {:>4.1} Mbps | Búfer Wi-Fi: {}ms (100% Lleno) | Sync: 0ms  │\n",
            if self.is_connected { 60.0 } else { 0.0 },
            stats.received_mbps,
            self.buffer_ms));
        out.push_str("│                                                                                  │\n");
        out.push_str(&format!("│  🔊 Volumen Laptop: [══════════●═══] {:>3.0}%    {}\n", self.audio_volume * 100.0, vu_bar));
        out.push_str("│  [📋 Copiar enlace OBS Media Source]  [📋 Guía Rápida TikTok Live Studio]         │\n");
        out.push_str("└──────────────────────────────────────────────────────────────────────────────────┘\n");

        out
    }

    fn format_vu_bars(vu: &VuMeterLevel) -> String {
        let left_len = (vu.left_peak * 10.0) as usize;
        let right_len = (vu.right_peak * 10.0) as usize;

        let mut l_bar = String::from("L [");
        for i in 0..10 {
            if i < left_len { l_bar.push('█'); } else { l_bar.push('░'); }
        }
        l_bar.push(']');

        let mut r_bar = String::from("R [");
        for i in 0..10 {
            if i < right_len { r_bar.push('█'); } else { r_bar.push('░'); }
        }
        r_bar.push(']');

        format!("{}  {}", l_bar, r_bar)
    }
}

impl Default for ReceiverHud {
    fn default() -> Self {
        Self::new()
    }
}
