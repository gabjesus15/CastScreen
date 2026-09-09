//! Core types, synchronization clocks, configurations, and mathematical routines
//! for the CastScreen streaming pipeline.

pub mod audio_math;
pub mod clock;
pub mod config;
pub mod single_instance;
pub mod theme;
pub mod tray;
pub mod updater;

pub use audio_math::{AudioSubmixer, VuMeterLevel};
pub use clock::QpcClock;
pub use config::{AudioConfig, CastScreenConfig, MixerConfig, NetworkConfig, StreamProfile, VideoConfig};
pub use single_instance::SingleInstanceGuard;
pub use theme::*;
pub use tray::TrayNotifier;
pub use updater::{AppUpdater, AutoUpdater, UpdateState, UpdateStatus, CURRENT_VERSION};

/// Type of media packet flowing through the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    VideoH264,
    AudioAac,
}

/// A media frame with synchronized Presentation Time Stamp (PTS).
#[derive(Debug, Clone)]
pub struct MediaPacket {
    pub media_type: MediaType,
    /// 90kHz MPEG-TS Presentation Time Stamp (PTS)
    pub pts_90khz: u64,
    /// Optional Decode Time Stamp (DTS) for B-frames (0 if identical to PTS)
    pub dts_90khz: u64,
    /// Raw payload bytes (Annex-B H.264 NAL or ADTS/raw AAC)
    pub payload: Vec<u8>,
    /// True if this is an IDR/Keyframe
    pub is_keyframe: bool,
}

impl MediaPacket {
    pub fn new_video(pts: u64, dts: u64, payload: Vec<u8>, is_keyframe: bool) -> Self {
        Self {
            media_type: MediaType::VideoH264,
            pts_90khz: pts,
            dts_90khz: dts,
            payload,
            is_keyframe,
        }
    }

    pub fn new_audio(pts: u64, payload: Vec<u8>) -> Self {
        Self {
            media_type: MediaType::AudioAac,
            pts_90khz: pts,
            dts_90khz: pts,
            payload,
            is_keyframe: true,
        }
    }
}
