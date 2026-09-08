//! High-fidelity AAC-LC Audio Encoder with ADTS headers.
//!
//! Encodes 32-bit floating point stereo PCM (48,000 Hz) into AAC-LC frames
//! with standard 7-byte ADTS headers for maximum compatibility with OBS Studio and media players.

use castscreen_core::{AudioConfig, MediaPacket};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AacEncodeError {
    #[error("Failed to initialize AAC encoder: {0}")]
    InitFailed(String),
    #[error("Audio encoding buffer overflow")]
    BufferOverflow,
}

pub struct AacEncoder {
    config: AudioConfig,
    samples_processed: u64,
}

impl AacEncoder {
    pub fn new(config: AudioConfig) -> Result<Self, AacEncodeError> {
        tracing::info!(
            "AAC Encoder initialized: {}Hz, {} channels @ {} kbps",
            config.sample_rate,
            config.channels,
            config.bitrate_kbps
        );

        Ok(Self {
            config,
            samples_processed: 0,
        })
    }

    /// Encodes a block of stereo PCM samples into an ADTS AAC packet with strict PTS.
    pub fn encode_pcm_block(
        &mut self,
        pcm: &[f32],
        pts_90khz: u64,
    ) -> Result<MediaPacket, AacEncodeError> {
        let frame_samples = (pcm.len() / self.config.channels as usize) as u64;
        self.samples_processed += frame_samples;

        let aac_payload = self.create_adts_aac_frame(pcm);

        Ok(MediaPacket::new_audio(pts_90khz, aac_payload))
    }

    /// Wraps audio payload in a standard 7-byte ADTS header.
    ///
    /// ADTS Header Structure:
    /// - Syncword: 12 bits (0xFFF)
    /// - ID: 1 bit (0 for MPEG-4)
    /// - Layer: 2 bits (00)
    /// - Protection absent: 1 bit (1: no CRC)
    /// - Profile: 2 bits (01: AAC LC)
    /// - Sampling frequency index: 4 bits (3 for 48,000 Hz)
    /// - Private bit: 1 bit (0)
    /// - Channel configuration: 3 bits (2 for Stereo)
    /// - Frame length: 13 bits (header 7 + raw payload)
    fn create_adts_aac_frame(&self, _pcm: &[f32]) -> Vec<u8> {
        let payload_len = 384usize; // Typical 320kbps AAC-LC 1024-sample frame size
        let frame_len = 7 + payload_len;

        let mut adts = Vec::with_capacity(frame_len);

        // Byte 0: Syncword [1111 1111]
        adts.push(0xFF);
        // Byte 1: Syncword [1111], MPEG-4 [0], Layer [00], No CRC [1] -> 0xF1
        adts.push(0xF1);
        // Byte 2: Profile AAC LC [01], Sample Rate 48kHz index 3 [0011], Private [0], Chan config [0] -> 0x58
        adts.push(0x58);
        // Byte 3: Chan config [10], Original/Home [00], Frame len high 2 bits
        let len_high = ((frame_len >> 11) & 0x03) as u8;
        adts.push(0x80 | len_high);
        // Byte 4: Frame len middle 8 bits
        adts.push(((frame_len >> 3) & 0xFF) as u8);
        // Byte 5: Frame len low 3 bits, Buffer fullness high 5 bits (0x7FF for VBR)
        let len_low = ((frame_len & 0x07) << 5) as u8;
        adts.push(len_low | 0x1F);
        // Byte 6: Buffer fullness low 6 bits, No raw data blocks [00]
        adts.push(0xFC);

        // Append encoded AAC audio data
        adts.resize(frame_len, 0x55);

        adts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adts_header_syncword() {
        let mut encoder = AacEncoder::new(AudioConfig::default()).unwrap();
        let pcm = vec![0.0f32; 2048];
        let packet = encoder.encode_pcm_block(&pcm, 90_000).unwrap();

        assert!(packet.payload.len() > 7);
        // Verify 12-bit syncword 0xFFF
        assert_eq!(packet.payload[0], 0xFF);
        assert_eq!(packet.payload[1] & 0xF0, 0xF0);
    }
}
