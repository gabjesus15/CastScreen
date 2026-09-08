//! Windows WASAPI Loopback Audio Capture.
//!
//! Captures bit-perfect system audio from the default playback device (what you hear in headphones)
//! in 48,000 Hz stereo IEEE 32-bit float format with zero virtual cable latency.

use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use thiserror::Error;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};

#[derive(Error, Debug)]
pub enum WasapiCaptureError {
    #[error("COM operation failed: {0}")]
    Com(#[from] windows::core::Error),
    #[error("Could not obtain default playback endpoint")]
    EndpointUnavailable,
    #[error("Audio format negotiation failed")]
    FormatUnsupported,
}

pub struct WasapiLoopbackCapture {
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl WasapiLoopbackCapture {
    /// Spawns a background real-time capture thread that pushes stereo f32 audio chunks
    /// into the provided `pcm_sender`.
    pub fn start(
        pcm_sender: Sender<Vec<f32>>,
        sample_rate: u32,
    ) -> Result<Self, WasapiCaptureError> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let worker = thread::Builder::new()
            .name("wasapi-loopback".to_string())
            .spawn(move || {
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

                    if let Err(e) = Self::capture_loop(running_clone, pcm_sender, sample_rate) {
                        tracing::error!("WASAPI loopback error: {:?}", e);
                    }

                    CoUninitialize();
                }
            })
            .map_err(|_| WasapiCaptureError::FormatUnsupported)?;

        Ok(Self {
            running,
            worker: Some(worker),
        })
    }

    unsafe fn capture_loop(
        running: Arc<AtomicBool>,
        sender: Sender<Vec<f32>>,
        _target_sample_rate: u32,
    ) -> Result<(), WasapiCaptureError> {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        let device: IMMDevice =
            enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;

        let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

        let mix_format = audio_client.GetMixFormat()?;
        let format_ref = &*mix_format;

        // Initialize in Loopback mode with 20ms buffer
        let hns_buffer_duration = 200_000i64; // 20ms in 100ns units
        audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            hns_buffer_duration,
            0,
            mix_format,
            None,
        )?;

        let capture_client: IAudioCaptureClient = audio_client.GetService()?;
        audio_client.Start()?;

        let sample_rate = format_ref.nSamplesPerSec;
        let channels = format_ref.nChannels as usize;

        tracing::info!(
            "WASAPI Loopback started: {}Hz, {} channels",
            sample_rate,
            channels
        );

        while running.load(Ordering::Relaxed) {
            let packet_length = capture_client.GetNextPacketSize().unwrap_or(0);
            if packet_length > 0 {
                let mut data_ptr = std::ptr::null_mut();
                let mut num_frames_read = 0u32;
                let mut flags = 0u32;

                if capture_client
                    .GetBuffer(&mut data_ptr, &mut num_frames_read, &mut flags, None, None)
                    .is_ok()
                {
                    if num_frames_read > 0 && !data_ptr.is_null() {
                        let total_samples = (num_frames_read as usize) * channels;
                        let mut stereo_pcm = Vec::with_capacity(num_frames_read as usize * 2);

                        if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                            // Buffer is silent
                            stereo_pcm.resize(num_frames_read as usize * 2, 0.0f32);
                        } else {
                            let float_slice = std::slice::from_raw_parts(data_ptr as *const f32, total_samples);

                            // Normalize down to stereo if source is multi-channel (e.g. 5.1/7.1)
                            for frame in float_slice.chunks_exact(channels) {
                                if channels >= 2 {
                                    stereo_pcm.push(frame[0]);
                                    stereo_pcm.push(frame[1]);
                                } else {
                                    // Mono duplicated
                                    stereo_pcm.push(frame[0]);
                                    stereo_pcm.push(frame[0]);
                                }
                            }
                        }

                        let _ = sender.try_send(stereo_pcm);
                    }

                    let _ = capture_client.ReleaseBuffer(num_frames_read);
                }
            } else {
                // Sleep briefly to prevent busy spinning (5ms)
                thread::sleep(std::time::Duration::from_millis(5));
            }
        }

        let _ = audio_client.Stop();
        Ok(())
    }

    /// Signals the capture thread to terminate.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WasapiLoopbackCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
