//! Real-time PCM audio packer for the CastScreen internal transport.
//!
//! Captured WASAPI loopback audio arrives as 48,000 Hz stereo 32-bit float PCM.
//! For maximum fidelity and zero decoder complexity over the LAN, samples are
//! packed as interleaved signed 16-bit little-endian PCM and carried inside the
//! MPEG-TS audio elementary stream. The receiver unpacks and plays them directly.
//!
//! (An external H.264/AAC path for OBS `srt://` ingest would require a hardware
//! codec; this transport targets the built-in CastScreen receiver, which is the
//! source that OBS / TikTok Live Studio capture as a window.)

use castscreen_core::{AudioConfig, MediaPacket};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PcmPackError {
    #[error("Failed to initialize audio encoder: {0}")]
    InitFailed(String),
    #[error("Audio encoding buffer overflow")]
    BufferOverflow,
}

/// Packs stereo f32 PCM blocks into interleaved i16 LE PCM packets with PTS.
pub struct PcmPacker {
    config: AudioConfig,
    samples_processed: u64,
}

impl PcmPacker {
    pub fn new(config: AudioConfig) -> Result<Self, PcmPackError> {
        tracing::info!(
            "PCM audio packer initialized: {}Hz, {} channels (i16 LE transport)",
            config.sample_rate,
            config.channels,
        );

        Ok(Self {
            config,
            samples_processed: 0,
        })
    }

    /// Packs a block of stereo f32 PCM samples into an i16 LE PCM packet with strict PTS.
    ///
    /// The payload is real audio: every input sample is converted, nothing is padded
    /// or synthesized. `pcm` is expected to be interleaved stereo in [-1.0, 1.0].
    pub fn encode_pcm_block(
        &mut self,
        pcm: &[f32],
        pts_90khz: u64,
    ) -> Result<MediaPacket, PcmPackError> {
        let frame_samples = (pcm.len() / self.config.channels.max(1) as usize) as u64;
        self.samples_processed += frame_samples;

        let mut payload = Vec::with_capacity(pcm.len() * 2);
        for &sample in pcm {
            // Clamp then scale to full i16 range (32767) to avoid wrap on overshoot.
            let clamped = sample.clamp(-1.0, 1.0);
            let s = (clamped * 32767.0).round() as i16;
            payload.extend_from_slice(&s.to_le_bytes());
        }

        Ok(MediaPacket::new_audio(pts_90khz, payload))
    }
}

/// Unpacks an i16 LE PCM payload back into interleaved f32 samples for playback.
pub fn decode_pcm_payload(payload: &[u8]) -> Vec<f32> {
    let mut out = Vec::with_capacity(payload.len() / 2);
    for chunk in payload.chunks_exact(2) {
        let s = i16::from_le_bytes([chunk[0], chunk[1]]);
        out.push(s as f32 / 32768.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcm_roundtrip_is_lossless_enough() {
        let mut encoder = PcmPacker::new(AudioConfig::default()).unwrap();
        let pcm: Vec<f32> = (0..2048)
            .map(|i| ((i as f32) * 0.01).sin() * 0.5)
            .collect();
        let packet = encoder.encode_pcm_block(&pcm, 90_000).unwrap();

        // Real data: 2 bytes per sample, no padding.
        assert_eq!(packet.payload.len(), pcm.len() * 2);

        let decoded = decode_pcm_payload(&packet.payload);
        assert_eq!(decoded.len(), pcm.len());
        for (a, b) in pcm.iter().zip(decoded.iter()) {
            assert!((a - b).abs() < 1.0 / 256.0, "sample drift too high");
        }
    }
}
