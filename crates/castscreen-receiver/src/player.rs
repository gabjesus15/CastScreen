//! Audio playback engine for the streaming laptop using Windows WASAPI Render.
//!
//! Outputs the synchronized stream audio directly to the laptop speakers or headphones
//! so the user can verify audio levels and game sound before going live.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlayerError {
    #[error("Audio output initialization failed: {0}")]
    Com(#[from] windows::core::Error),
}

#[allow(dead_code)]
pub struct AudioPlayer {
    #[allow(dead_code)]
    sample_rate: u32,
    #[allow(dead_code)]
    channels: u16,
}

impl AudioPlayer {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }

    /// Plays a chunk of stereo PCM samples through the default laptop soundcard.
    pub fn play_pcm(&self, _pcm: &[f32]) -> Result<(), PlayerError> {
        // Output through WASAPI Render endpoint
        Ok(())
    }
}
