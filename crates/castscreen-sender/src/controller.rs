//! Streaming Pipeline Orchestrator for the Gaming PC (CastScreen-Sender).
//!
//! Coordinates zero-copy screen capture, WASAPI loopback audio, the per-application
//! mixer, NVENC hardware encoding, and SRT transmission across isolated lock-free threads.

use anyhow::Result;
use castscreen_capture::{AudioAppSession, AudioSessionController, DxgiScreenCapture, WasapiLoopbackCapture};
use castscreen_core::{
    AudioConfig, AudioSubmixer, CastScreenConfig, MediaPacket, NetworkConfig, QpcClock,
    VideoConfig, VuMeterLevel,
};
use castscreen_encoder::{AacEncoder, NvencEncoder};
use castscreen_network::{MpegTsMuxer, NetworkStats, SrtSender};
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Real-time dashboard state reported to the GUI.
#[derive(Debug, Clone, Default)]
pub struct SenderStateSnapshot {
    pub is_streaming: bool,
    pub current_fps: f32,
    pub bitrate_mbps: f32,
    pub total_bytes_sent: u64,
    pub master_vu: VuMeterLevel,
    pub detected_apps: Vec<AudioAppSession>,
}

pub struct StreamController {
    config: CastScreenConfig,
    is_running: Arc<AtomicBool>,
    state_snapshot: Arc<RwLock<SenderStateSnapshot>>,
    threads: Vec<JoinHandle<()>>,
    wasapi_capture: Option<WasapiLoopbackCapture>,
}

impl StreamController {
    pub fn new(config: CastScreenConfig) -> Self {
        Self {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            state_snapshot: Arc::new(RwLock::new(SenderStateSnapshot::default())),
            threads: Vec::new(),
            wasapi_capture: None,
        }
    }

    /// Starts the end-to-end streaming engine.
    pub fn start_streaming(&mut self, target_ip: Option<String>) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.is_running.store(true, Ordering::SeqCst);
        let clock = QpcClock::new();

        // 1. Channel for raw audio PCM blocks from WASAPI loopback
        let (audio_pcm_tx, audio_pcm_rx) = bounded::<Vec<f32>>(64);

        // 2. Channel for encoded MediaPackets (Video + Audio) heading to the TS Multiplexer
        let (media_tx, media_rx) = bounded::<MediaPacket>(128);

        // Start WASAPI Loopback Capture
        let wasapi = WasapiLoopbackCapture::start(audio_pcm_tx, self.config.audio.sample_rate)?;
        self.wasapi_capture = Some(wasapi);

        // Thread: Audio Processing & Encoding (Mixer -> AAC)
        let is_running_audio = self.is_running.clone();
        let audio_config = self.config.audio.clone();
        let clock_audio = clock.clone();
        let media_tx_audio = media_tx.clone();
        let snapshot_audio = self.state_snapshot.clone();

        let audio_handle = thread::Builder::new()
            .name("audio-encoder".to_string())
            .spawn(move || {
                let mut aac = AacEncoder::new(audio_config).expect("AAC encoder init failed");

                while is_running_audio.load(Ordering::Relaxed) {
                    if let Ok(pcm_chunk) = audio_pcm_rx.recv_timeout(Duration::from_millis(50)) {
                        // Calculate real-time VU meter levels
                        let vu = AudioSubmixer::calculate_vu_meter(&pcm_chunk);
                        snapshot_audio.write().master_vu = vu;

                        let pts = clock_audio.current_pts_90khz();
                        if let Ok(packet) = aac.encode_pcm_block(&pcm_chunk, pts) {
                            let _ = media_tx_audio.try_send(packet);
                        }
                    }
                }
            })?;
        self.threads.push(audio_handle);

        // Thread: Network Transmission (MPEG-TS Muxer -> SRT Socket)
        let is_running_net = self.is_running.clone();
        let net_config = self.config.network.clone();
        let snapshot_net = self.state_snapshot.clone();

        let net_handle = thread::Builder::new()
            .name("ts-srt-sender".to_string())
            .spawn(move || {
                let mut muxer = MpegTsMuxer::new();
                let mut sender = SrtSender::new(net_config).expect("SRT sender socket bind failed");

                if let Some(target) = target_ip {
                    sender.set_target(target);
                }

                while is_running_net.load(Ordering::Relaxed) {
                    if let Ok(packet) = media_rx.recv_timeout(Duration::from_millis(50)) {
                        let ts_bytes = muxer.mux_packet(&packet);
                        sender.send_ts_data(&ts_bytes);

                        let stats = sender.get_stats();
                        let mut snap = snapshot_net.write();
                        snap.bitrate_mbps = stats.bitrate_mbps;
                        snap.total_bytes_sent = stats.total_bytes_sent;
                    }
                }
            })?;
        self.threads.push(net_handle);

        // Thread: Video Capture & NVENC Encoding (DirectX 11 VRAM -> NVENC)
        let is_running_video = self.is_running.clone();
        let video_config = self.config.video.clone();
        let clock_video = clock.clone();
        let media_tx_video = media_tx;

        let video_handle = thread::Builder::new()
            .name("dxgi-nvenc".to_string())
            .spawn(move || {
                let mut dxgi = match DxgiScreenCapture::new(video_config.display_index) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::error!("DXGI init failed: {:?}", e);
                        return;
                    }
                };

                let mut nvenc = match NvencEncoder::new(video_config.clone()) {
                    Ok(n) => n,
                    Err(e) => {
                        tracing::error!("NVENC init failed: {:?}", e);
                        return;
                    }
                };

                let frame_interval = Duration::from_micros(1_000_000 / video_config.fps as u64);

                while is_running_video.load(Ordering::Relaxed) {
                    let loop_start = std::time::Instant::now();

                    if let Ok(texture) = dxgi.acquire_frame_texture(10) {
                        let pts = clock_video.current_pts_90khz();
                        if let Ok(video_packet) = nvenc.encode_frame(&texture, pts) {
                            let _ = media_tx_video.try_send(video_packet);
                        }
                        let _ = dxgi.release_frame();
                    }

                    // Precise frame pacing to exact 60.0 FPS
                    let elapsed = loop_start.elapsed();
                    if elapsed < frame_interval {
                        thread::sleep(frame_interval - elapsed);
                    }
                }
            })?;
        self.threads.push(video_handle);

        self.state_snapshot.write().is_streaming = true;
        tracing::info!("CastScreen streaming pipeline active");
        Ok(())
    }

    /// Stops all running media threads cleanly.
    pub fn stop_streaming(&mut self) {
        if !self.is_running.load(Ordering::SeqCst) {
            return;
        }

        self.is_running.store(false, Ordering::SeqCst);
        if let Some(mut wasapi) = self.wasapi_capture.take() {
            wasapi.stop();
        }
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }

        let mut snap = self.state_snapshot.write();
        snap.is_streaming = false;
        snap.bitrate_mbps = 0.0;
        snap.current_fps = 0.0;
        tracing::info!("CastScreen streaming pipeline stopped");
    }

    /// Periodically refreshes the list of running sound applications.
    pub fn refresh_audio_sessions(&self) {
        if let Ok(sessions) = AudioSessionController::enumerate_active_sessions() {
            self.state_snapshot.write().detected_apps = sessions;
        }
    }

    /// Returns a snapshot of the live streaming stats and audio meter levels.
    pub fn get_snapshot(&self) -> SenderStateSnapshot {
        self.state_snapshot.read().clone()
    }
}

impl Drop for StreamController {
    fn drop(&mut self) {
        self.stop_streaming();
    }
}
