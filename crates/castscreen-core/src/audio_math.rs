//! High-performance audio mathematical routines for mixing, volume scaling,
//! soft-knee limiting, and real-time RMS/Peak VU metering.

/// Represents stereo audio levels in decibels and normalized float (0.0 to 1.0).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct VuMeterLevel {
    pub left_rms: f32,
    pub right_rms: f32,
    pub left_peak: f32,
    pub right_peak: f32,
    pub left_db: f32,
    pub right_db: f32,
    pub is_clipping: bool,
}

/// Software mixer for 32-bit floating point stereo PCM audio (48,000 Hz standard).
#[derive(Debug, Clone)]
pub struct AudioSubmixer {
    sample_rate: u32,
    channels: u16,
}

impl AudioSubmixer {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }

    /// Mixes multiple input stereo PCM streams into a single master output buffer.
    ///
    /// Each input is scaled by its individual linear gain.
    /// If `is_muted` is true, the track contributes zero.
    /// A soft limiter is applied to prevent harsh digital clipping.
    pub fn mix_tracks(
        output: &mut [f32],
        tracks: &[(&[f32], f32, bool)], // (samples, gain, is_muted)
    ) {
        // Zero out output
        for sample in output.iter_mut() {
            *sample = 0.0;
        }

        // Accumulate active tracks
        for &(samples, gain, is_muted) in tracks {
            if is_muted || gain <= 0.0 {
                continue;
            }

            let len = output.len().min(samples.len());
            for i in 0..len {
                output[i] += samples[i] * gain;
            }
        }

        // Apply transparent soft-limiting
        Self::apply_soft_limiter(output);
    }

    /// Transparent soft-knee limiter using hyperbolic tangent approximation.
    /// Keeps signals strictly within [-1.0, 1.0] without hard clipping harmonics.
    #[inline]
    pub fn apply_soft_limiter(buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            let s = *sample;
            if s > 0.95 {
                // Soft compress positive peak
                *sample = 0.95 + (0.05 * ((s - 0.95) / 0.05).tanh());
            } else if s < -0.95 {
                // Soft compress negative peak
                *sample = -0.95 + (0.05 * ((s + 0.95) / 0.05).tanh());
            }
        }
    }

    /// Computes RMS and Peak levels from a stereo interleaved buffer.
    pub fn calculate_vu_meter(buffer: &[f32]) -> VuMeterLevel {
        if buffer.is_empty() {
            return VuMeterLevel {
                left_db: -100.0,
                right_db: -100.0,
                ..Default::default()
            };
        }

        let mut sum_sq_l = 0.0f32;
        let mut sum_sq_r = 0.0f32;
        let mut peak_l = 0.0f32;
        let mut peak_r = 0.0f32;
        let mut count_l = 0usize;
        let mut count_r = 0usize;

        for chunk in buffer.chunks_exact(2) {
            let l = chunk[0].abs();
            let r = chunk[1].abs();

            sum_sq_l += l * l;
            sum_sq_r += r * r;

            if l > peak_l {
                peak_l = l;
            }
            if r > peak_r {
                peak_r = r;
            }

            count_l += 1;
            count_r += 1;
        }

        let rms_l = if count_l > 0 { (sum_sq_l / count_l as f32).sqrt() } else { 0.0 };
        let rms_r = if count_r > 0 { (sum_sq_r / count_r as f32).sqrt() } else { 0.0 };

        let left_db = Self::linear_to_db(rms_l);
        let right_db = Self::linear_to_db(rms_r);

        VuMeterLevel {
            left_rms: rms_l.clamp(0.0, 1.0),
            right_rms: rms_r.clamp(0.0, 1.0),
            left_peak: peak_l.clamp(0.0, 1.0),
            right_peak: peak_r.clamp(0.0, 1.0),
            left_db,
            right_db,
            is_clipping: peak_l >= 0.999 || peak_r >= 0.999,
        }
    }

    /// Converts linear amplitude [0.0, 1.0] to Decibels Full Scale (dBFS).
    #[inline]
    pub fn linear_to_db(linear: f32) -> f32 {
        if linear <= 0.00001 {
            -100.0
        } else {
            (20.0 * linear.log10()).max(-100.0)
        }
    }

    /// Converts Decibels Full Scale (dBFS) back to linear gain multiplier.
    #[inline]
    pub fn db_to_linear(db: f32) -> f32 {
        if db <= -100.0 {
            0.0
        } else {
            10.0f32.powf(db / 20.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixer_accumulation_and_mute() {
        let track1 = [0.5, 0.5, 0.5, 0.5];
        let track2 = [0.2, 0.2, 0.2, 0.2];
        let mut output = [0.0; 4];

        // Track 1 active at 1.0 gain, Track 2 muted
        AudioSubmixer::mix_tracks(
            &mut output,
            &[(&track1, 1.0, false), (&track2, 1.0, true)],
        );

        assert_eq!(output, [0.5, 0.5, 0.5, 0.5]);

        // Both active
        AudioSubmixer::mix_tracks(
            &mut output,
            &[(&track1, 1.0, false), (&track2, 1.0, false)],
        );

        assert!((output[0] - 0.7).abs() < 1e-5);
    }

    #[test]
    fn test_soft_limiter() {
        let mut loud_buffer = [2.0, -3.0, 0.5, -0.5];
        AudioSubmixer::apply_soft_limiter(&mut loud_buffer);

        for &sample in &loud_buffer {
            assert!(sample <= 1.0 && sample >= -1.0);
        }
    }

    #[test]
    fn test_vu_meter_silence() {
        let silence = [0.0; 128];
        let meter = AudioSubmixer::calculate_vu_meter(&silence);
        assert_eq!(meter.left_rms, 0.0);
        assert_eq!(meter.right_rms, 0.0);
        assert_eq!(meter.left_db, -100.0);
        assert!(!meter.is_clipping);
    }
}
