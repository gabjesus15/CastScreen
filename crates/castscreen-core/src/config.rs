//! Strongly-typed configurations for Video, Audio, Network, and the Application Mixer.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Predefined profiles optimized for LAN game streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamProfile {
    /// 1080p 60 FPS @ 25 Mbps (Default - Recommended for RTX 3070 to Wi-Fi 6)
    FullHd60,
    /// 1440p 60 FPS @ 35 Mbps (Ultra fidelity)
    QuadHd60,
    /// 1080p 120 FPS @ 40 Mbps (High refresh rate)
    FullHd120,
    /// User defined custom settings
    Custom,
}

/// Video capture and NVENC encoding parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoConfig {
    pub display_index: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub gop_size: u32,
    pub nvenc_preset: String,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            display_index: 0,
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate_kbps: 25_000,
            gop_size: 60, // 1 second keyframe interval
            nvenc_preset: "p1".to_string(), // Low latency NVENC
        }
    }
}

/// Audio capture, mixing, and AAC encoding parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate_kbps: u32,
    pub chunk_size: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 2,
            bitrate_kbps: 320,
            chunk_size: 1024, // 21.33 ms audio blocks
        }
    }
}

/// SRT transport configuration with ARQ jitter buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub host: String,
    pub port: u16,
    /// SRT latency buffer in milliseconds (500 to 2000 ms).
    /// Generous buffer prevents packet drops over Wi-Fi 6.
    pub srt_latency_ms: u32,
    pub is_listener: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 9000,
            srt_latency_ms: 1000, // 1.0 second buffer
            is_listener: true,    // Sender listens on port 9000 by default
        }
    }
}

/// Per-application audio mixer state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MixerConfig {
    pub master_gain: f32,
    pub master_muted: bool,
    pub mic_gain: f32,
    pub mic_muted: bool,
    /// Map of executable name -> (gain, is_muted)
    pub app_rules: HashMap<String, (f32, bool)>,
}

/// Root CastScreen configuration file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CastScreenConfig {
    pub profile: Option<StreamProfile>,
    pub video: VideoConfig,
    pub audio: AudioConfig,
    pub network: NetworkConfig,
    pub mixer: MixerConfig,
}

impl CastScreenConfig {
    pub fn load_from_toml_str(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}
