//! High-precision monotonic master clock based on Windows QueryPerformanceCounter (QPC).
//!
//! Provides strict Presentation Time Stamp (PTS) calculations for both video
//! and audio frames at the standard MPEG-TS timebase of 90,000 Hz.

use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

/// High-resolution clock calibrated against the hardware counter.
#[derive(Debug, Clone)]
pub struct QpcClock {
    frequency: i64,
    start_ticks: i64,
}

impl QpcClock {
    /// Initializes a new master clock, establishing the baseline $T_0$.
    pub fn new() -> Self {
        let mut freq = 0i64;
        let mut start = 0i64;

        unsafe {
            let _ = QueryPerformanceFrequency(&mut freq);
            let _ = QueryPerformanceCounter(&mut start);
        }

        // Fallback safety if QPC fails (should never happen on modern Windows)
        if freq <= 0 {
            freq = 10_000_000;
        }

        Self {
            frequency: freq,
            start_ticks: start,
        }
    }

    /// Returns current monotonic ticks from QPC.
    #[inline]
    pub fn now_ticks(&self) -> i64 {
        let mut now = 0i64;
        unsafe {
            let _ = QueryPerformanceCounter(&mut now);
        }
        now
    }

    /// Calculates elapsed duration in seconds as f64.
    #[inline]
    pub fn elapsed_seconds(&self) -> f64 {
        let now = self.now_ticks();
        let delta = now.saturating_sub(self.start_ticks);
        delta as f64 / self.frequency as f64
    }

    /// Calculates elapsed duration in milliseconds.
    #[inline]
    pub fn elapsed_millis(&self) -> u64 {
        (self.elapsed_seconds() * 1000.0) as u64
    }

    /// Computes MPEG-TS compatible 90kHz Presentation Time Stamp (PTS)
    /// from the baseline start time.
    ///
    /// Equation:
    /// $$PTS = \frac{(T_{\text{now}} - T_0) \times 90000}{\text{Frequency}_{\text{QPC}}} \pmod{2^{33}}$$
    #[inline]
    pub fn current_pts_90khz(&self) -> u64 {
        self.ticks_to_pts_90khz(self.now_ticks())
    }

    /// Converts arbitrary QPC ticks into a 90kHz PTS value modulo 2^33 (33-bit MPEG-TS standard).
    #[inline]
    pub fn ticks_to_pts_90khz(&self, ticks: i64) -> u64 {
        let delta = ticks.saturating_sub(self.start_ticks).max(0) as u128;
        let pts = (delta * 90_000) / (self.frequency as u128);
        // MPEG-TS PTS wraps at 33 bits (8589934592)
        (pts % 8_589_934_592) as u64
    }

    /// Computes the PTS for an audio packet given the total number of audio samples
    /// transmitted so far at a specific sample rate (e.g., 48,000 Hz).
    ///
    /// This ensures mathematical alignment between audio sample counts and video frames.
    #[inline]
    pub fn audio_samples_to_pts_90khz(sample_count: u64, sample_rate: u32) -> u64 {
        let pts = (sample_count as u128 * 90_000) / (sample_rate as u128);
        (pts % 8_589_934_592) as u64
    }

    /// Resets the baseline $T_0$ to current time.
    pub fn reset_baseline(&mut self) {
        self.start_ticks = self.now_ticks();
    }
}

impl Default for QpcClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qpc_clock_ticks() {
        let clock = QpcClock::new();
        assert!(clock.frequency > 0);
        let t1 = clock.now_ticks();
        let t2 = clock.now_ticks();
        assert!(t2 >= t1);
    }

    #[test]
    fn test_audio_samples_to_pts() {
        // 48000 samples at 48kHz = exactly 1 second = 90,000 PTS ticks
        let pts = QpcClock::audio_samples_to_pts_90khz(48_000, 48_000);
        assert_eq!(pts, 90_000);

        // 1024 samples at 48kHz = 1920 PTS ticks
        let pts_frame = QpcClock::audio_samples_to_pts_90khz(1024, 48_000);
        assert_eq!(pts_frame, 1920);
    }
}
