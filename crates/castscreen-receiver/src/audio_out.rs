//! Real-time WASAPI audio renderer for the CastScreen receiver.
//!
//! Plays the incoming stereo PCM stream through the laptop's default playback
//! device in shared mode. Samples arrive as interleaved 48,000 Hz stereo f32
//! and are linearly resampled to the device mix rate when they differ.

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioClient, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};

/// Source sample rate of the incoming CastScreen audio stream.
const SOURCE_RATE: u32 = 48_000;
/// Hard cap on queued audio (~0.4 s) to bound latency; older samples are dropped.
const MAX_QUEUED_SAMPLES: usize = (SOURCE_RATE as usize) * 2 * 4 / 10;

/// Handle to the background WASAPI render thread.
pub struct AudioOutput {
    queue: Arc<Mutex<VecDeque<f32>>>,
    /// Playback gain in fixed point (volume * 1000) for lock-free updates.
    volume_milli: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl AudioOutput {
    /// Starts the renderer. Returns a silent-but-valid handle even if no audio
    /// device is available, so the receiver never fails to open.
    pub fn start() -> Self {
        let queue: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::with_capacity(
            MAX_QUEUED_SAMPLES,
        )));
        let volume_milli = Arc::new(AtomicU32::new(850)); // 0.85 default
        let running = Arc::new(AtomicBool::new(true));

        let queue_thread = queue.clone();
        let volume_thread = volume_milli.clone();
        let running_thread = running.clone();

        let worker = thread::Builder::new()
            .name("wasapi-render".to_string())
            .spawn(move || unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                if let Err(e) = render_loop(queue_thread, volume_thread, running_thread) {
                    tracing::error!("WASAPI render thread stopped: {:?}", e);
                }
            })
            .ok();

        Self {
            queue,
            volume_milli,
            running,
            worker,
        }
    }

    /// Queues interleaved stereo f32 samples for playback.
    pub fn push_samples(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        let mut q = self.queue.lock();
        q.extend(samples.iter().copied());
        // Bound latency: if we fell behind, drop the oldest audio.
        while q.len() > MAX_QUEUED_SAMPLES {
            let overflow = q.len() - MAX_QUEUED_SAMPLES;
            q.drain(..overflow);
        }
    }

    /// Sets playback volume in [0.0, 1.0].
    pub fn set_volume(&self, volume: f32) {
        let milli = (volume.clamp(0.0, 1.0) * 1000.0) as u32;
        self.volume_milli.store(milli, Ordering::Relaxed);
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

unsafe fn render_loop(
    queue: Arc<Mutex<VecDeque<f32>>>,
    volume_milli: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
) -> windows::core::Result<()> {
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let device: IMMDevice = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
    let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

    let mix_format_ptr = client.GetMixFormat()?;
    let format = &*mix_format_ptr;
    let dev_rate = format.nSamplesPerSec;
    let dev_channels = format.nChannels as usize;
    let bits = format.wBitsPerSample;
    // Shared-mode mix formats are essentially always 32-bit float; 16-bit int is
    // handled as a defensive fallback.
    let is_float = bits == 32;

    // 40 ms shared-mode buffer.
    let buffer_duration_hns: i64 = 400_000;
    let init = client.Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        0,
        buffer_duration_hns,
        0,
        mix_format_ptr,
        None,
    );
    // The mix format pointer is owned by us; free it after Initialize copied it.
    CoTaskMemFree(Some(mix_format_ptr as *const _));
    init?;

    let buffer_frames = client.GetBufferSize()?;
    let render_client: IAudioRenderClient = client.GetService()?;
    client.Start()?;

    tracing::info!(
        "WASAPI render started: {} Hz, {} ch, {}-bit ({})",
        dev_rate,
        dev_channels,
        bits,
        if is_float { "float" } else { "int" }
    );

    // Fractional resampler state (source frame index within the pulled pair).
    let ratio = SOURCE_RATE as f64 / dev_rate as f64;
    let mut frac: f64 = 0.0;
    let mut cur = (0.0f32, 0.0f32);
    let mut nxt = (0.0f32, 0.0f32);
    let mut primed = false;

    while running.load(Ordering::Relaxed) {
        let padding = client.GetCurrentPadding()?;
        let available = buffer_frames.saturating_sub(padding);

        if available == 0 {
            thread::sleep(std::time::Duration::from_millis(3));
            continue;
        }

        let gain = volume_milli.load(Ordering::Relaxed) as f32 / 1000.0;

        // Determine how many source frames we could need and try to pull them.
        let buffer_ptr = render_client.GetBuffer(available)?;
        let out = std::slice::from_raw_parts_mut(
            buffer_ptr,
            available as usize * dev_channels * (bits as usize / 8),
        );

        let mut wrote_silence = true;
        {
            let mut q = queue.lock();

            // Prime interpolation endpoints once enough data exists.
            if !primed {
                if q.len() >= 4 {
                    cur = (pop_or_zero(&mut q), pop_or_zero(&mut q));
                    nxt = (pop_or_zero(&mut q), pop_or_zero(&mut q));
                    primed = true;
                    frac = 0.0;
                }
            }

            for frame_idx in 0..available as usize {
                let (mut l, mut r) = (0.0f32, 0.0f32);

                if primed {
                    let t = frac as f32;
                    l = (cur.0 + (nxt.0 - cur.0) * t) * gain;
                    r = (cur.1 + (nxt.1 - cur.1) * t) * gain;
                    wrote_silence = false;

                    frac += ratio;
                    while frac >= 1.0 {
                        frac -= 1.0;
                        cur = nxt;
                        if q.len() >= 2 {
                            nxt = (pop_or_zero(&mut q), pop_or_zero(&mut q));
                        } else {
                            // Ran dry: hold last sample and mark for re-priming.
                            nxt = cur;
                            primed = false;
                            break;
                        }
                    }
                }

                write_frame(out, frame_idx, dev_channels, bits, is_float, l, r);
            }
        }

        let flags = if wrote_silence {
            AUDCLNT_BUFFERFLAGS_SILENT.0 as u32
        } else {
            0
        };
        render_client.ReleaseBuffer(available, flags)?;
    }

    let _ = client.Stop();
    Ok(())
}

#[inline]
fn pop_or_zero(q: &mut VecDeque<f32>) -> f32 {
    q.pop_front().unwrap_or(0.0)
}

/// Writes one output frame into the device buffer at the correct sample format.
/// Stereo content is placed in the first two channels; any extra channels are
/// zeroed (safe for surround endpoints).
#[inline]
unsafe fn write_frame(
    out: &mut [u8],
    frame_idx: usize,
    dev_channels: usize,
    bits: u16,
    is_float: bool,
    l: f32,
    r: f32,
) {
    let bytes_per_sample = bits as usize / 8;
    let base = frame_idx * dev_channels * bytes_per_sample;

    for ch in 0..dev_channels {
        let sample = match ch {
            0 => l,
            1 => r,
            _ => 0.0,
        }
        .clamp(-1.0, 1.0);

        let offset = base + ch * bytes_per_sample;
        if is_float && bytes_per_sample == 4 {
            let bytes = sample.to_le_bytes();
            out[offset..offset + 4].copy_from_slice(&bytes);
        } else if bytes_per_sample == 2 {
            let s = (sample * 32767.0) as i16;
            out[offset..offset + 2].copy_from_slice(&s.to_le_bytes());
        } else if bytes_per_sample == 4 {
            // 32-bit integer PCM fallback.
            let s = (sample as f64 * 2_147_483_647.0) as i32;
            out[offset..offset + 4].copy_from_slice(&s.to_le_bytes());
        }
    }
}
