//! Dark Studio Modern User Interface & Dashboard for CastScreen Sender.
//!
//! Provides the visual control panel with the live per-application audio mixer,
//! ballistic VU-meters, display selector, and stream status indicators.

use crate::controller::SenderStateSnapshot;
use castscreen_capture::AudioAppSession;
use castscreen_core::VuMeterLevel;

pub struct DashboardUi {
    pub selected_display: u32,
    pub target_ip: String,
    pub target_port: u16,
    pub srt_latency_ms: u32,
    pub bitrate_mbps: u32,
    pub master_volume: f32,
    pub master_muted: bool,
}

impl DashboardUi {
    pub fn new() -> Self {
        Self {
            selected_display: 0,
            target_ip: "192.168.1.55".to_string(),
            target_port: 9000,
            srt_latency_ms: 1000,
            bitrate_mbps: 25,
            master_volume: 0.85,
            master_muted: false,
        }
    }

    /// Generates a live formatted terminal / dashboard view of the Dark Studio interface.
    pub fn render_console_view(&self, snapshot: &SenderStateSnapshot) -> String {
        let status_badge = if snapshot.is_streaming {
            "🟢 [ EN VIVO - 60.0 FPS ]"
        } else {
            "⚪ [ LISTO PARA TRANSMITIR ]"
        };

        let master_meter_bar = Self::format_vu_bar(&snapshot.master_vu);

        let mut output = String::new();
        output.push_str("┌──────────────────────────────────────────────────────────────────────────────────┐\n");
        output.push_str(&format!("│  📡 CastScreen Sender v1.0.0                      {:>31} _ □ ✕│\n", status_badge));
        output.push_str("├───────────────────────────────────────┬──────────────────────────────────────────┤\n");
        output.push_str("│ 🖥️ CONFIGURACIÓN DE VIDEO & RED       │ 🎚️ MEZCLADOR DE AUDIO POR APLICACIÓN     │\n");
        output.push_str("│                                       │                                          │\n");
        output.push_str(&format!("│  Pantalla: Monitor {} [1080p @ 60 FPS]  │  🔊 Audio Maestro (Desktop)              │\n", self.selected_display + 1));
        output.push_str(&format!("│  Bitrate LAN: {:>2} Mbps                │  {} {:>3.0}% 🔉 │\n", self.bitrate_mbps, master_meter_bar, self.master_volume * 100.0));
        output.push_str("│                                       │                                          │\n");
        output.push_str(&format!("│  Búfer Wi-Fi 6: {:>4} ms               │  🎤 Micrófono                            │\n", self.srt_latency_ms));
        output.push_str("│  💡 \"1000ms absorbe cortes en Wi-Fi\"  │  [ON] [████████░░░░░░░░░░] -24dB  70% 🎙️ │\n");
        output.push_str(&format!("│  Laptop Destino: {:<15}:{:>4}  │  ──────────────────────────────────────  │\n", self.target_ip, self.target_port));
        output.push_str("│                                       │  APLICACIONES DETECTADAS EN TIEMPO REAL: │\n");

        if snapshot.detected_apps.is_empty() {
            output.push_str("│                                       │  (Ninguna app reproduciendo audio)       │\n");
        } else {
            for app in snapshot.detected_apps.iter().take(3) {
                let app_meter = Self::format_app_vu(app);
                output.push_str(&format!("│                                       │  🎮 {:<20}         │\n", app.process_name));
                output.push_str(&format!("│                                       │  {}     │\n", app_meter));
            }
        }

        output.push_str("├───────────────────────────────────────┴──────────────────────────────────────────┤\n");
        if snapshot.is_streaming {
            output.push_str(&format!("│  📊 EN TRANSMISIÓN: Bitrate: {:.1} Mbps | Bytes: {} MB | Cero Desincronización     │\n",
                snapshot.bitrate_mbps, snapshot.total_bytes_sent / (1024 * 1024)));
            output.push_str("│   [  ⏹  DETENER TRANSMISIÓN  ]                   [⚙ Ajustes] [📁 Logs]           │\n");
        } else {
            output.push_str("│  💡 Presiona 'Iniciar' para transmitir pantalla y audio sincronizados a tu Laptop│\n");
            output.push_str("│   [  ▶  INICIAR TRANSMISIÓN AL STREAM  ]         [⚙ Ajustes] [📁 Logs]           │\n");
        }
        output.push_str("└──────────────────────────────────────────────────────────────────────────────────┘\n");

        output
    }

    /// Formats a 20-character ballistic visual VU meter bar.
    fn format_vu_bar(vu: &VuMeterLevel) -> String {
        let level = (vu.left_peak.max(vu.right_peak) * 16.0) as usize;
        let mut bar = String::from("[");
        for i in 0..16 {
            if i < level {
                if i >= 14 {
                    bar.push('█'); // Red / Peak zone
                } else if i >= 10 {
                    bar.push('█'); // Yellow zone
                } else {
                    bar.push('█'); // Green zone
                }
            } else {
                bar.push('░');
            }
        }
        bar.push(']');
        format!("{} {:>3.0}dB", bar, vu.left_db.max(vu.right_db))
    }

    fn format_app_vu(app: &AudioAppSession) -> String {
        let status = if app.is_muted { "[OFF]" } else { "[ON] " };
        let level = (app.peak_meter * 12.0) as usize;
        let mut bar = String::from("[");
        for i in 0..12 {
            if i < level && !app.is_muted {
                bar.push('█');
            } else {
                bar.push('░');
            }
        }
        bar.push(']');
        format!("{} {} {:>3.0}%", status, bar, app.volume * 100.0)
    }
}

impl Default for DashboardUi {
    fn default() -> Self {
        Self::new()
    }
}
