//! Streaming Pipeline Orchestrator for the Gaming PC (CastScreen-Sender).
//!
//! Coordinates zero-copy screen capture, WASAPI loopback audio, the per-application
//! mixer, NVENC hardware encoding, and TCP transmission across isolated lock-free threads.

use anyhow::Result;
use castscreen_capture::{
    AudioAppSession, AudioSessionController, DxgiScreenCapture, MonitorInfo,
    WasapiLoopbackCapture,
};
use castscreen_core::{AudioSubmixer, CastScreenConfig, MediaPacket, QpcClock, VuMeterLevel};
use castscreen_encoder::{NvencEncoder, PcmPacker};
use castscreen_network::{MpegTsMuxer, SrtSender};
use crossbeam_channel::bounded;
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
    pub detected_monitors: Vec<MonitorInfo>,
    pub selected_monitor: u32,
    /// Set when DXGI or NVENC fail to initialize; shown as an error banner in the GUI.
    pub pipeline_error: Option<String>,
    /// True while a live TCP connection to the receiver is established.
    pub network_connected: bool,
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
        let monitors = DxgiScreenCapture::enumerate_monitors().unwrap_or_default();
        let selected = config.video.display_index;
        let mut initial_snapshot = SenderStateSnapshot::default();
        initial_snapshot.detected_monitors = monitors;
        initial_snapshot.selected_monitor = selected;

        Self {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            state_snapshot: Arc::new(RwLock::new(initial_snapshot)),
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
                let mut aac = PcmPacker::new(audio_config).expect("PCM packer init failed");

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

        // Thread: Network Transmission (MPEG-TS Muxer -> TCP socket)
        let is_running_net = self.is_running.clone();
        let net_config = self.config.network.clone();
        let snapshot_net = self.state_snapshot.clone();

        let net_handle = thread::Builder::new()
            .name("ts-tcp-sender".to_string())
            .spawn(move || {
                let mut muxer = MpegTsMuxer::new();
                // The sender is a TCP client: it connects to the receiver, so it
                // binds no local port and cannot collide with a receiver running
                // on the same machine (the 127.0.0.1 loopback test).
                let mut sender = SrtSender::new(net_config).expect("Stream sender init failed");

                if let Some(target) = target_ip {
                    sender.set_target(target);
                }

                while is_running_net.load(Ordering::Relaxed) {
                    if let Ok(packet) = media_rx.recv_timeout(Duration::from_millis(50)) {
                        let ts_bytes = muxer.mux_packet(&packet);
                        sender.send_ts_data(&ts_bytes);
                    }

                    // Update stats on every iteration so connection status is always fresh.
                    let stats = sender.get_stats();
                    {
                        let mut snap = snapshot_net.write();
                        snap.bitrate_mbps = stats.bitrate_mbps;
                        snap.total_bytes_sent = stats.total_bytes_sent;
                        snap.network_connected = stats.connected;
                    }
                }
            })?;
        self.threads.push(net_handle);

        // Thread: Video Capture & NVENC Encoding (DirectX 11 VRAM -> NVENC)
        let is_running_video = self.is_running.clone();
        let video_config = self.config.video.clone();
        let clock_video = clock.clone();
        let media_tx_video = media_tx;
        let snapshot_video = self.state_snapshot.clone();

        let video_handle = thread::Builder::new()
            .name("dxgi-nvenc".to_string())
            .spawn(move || {
                let mut dxgi = match DxgiScreenCapture::new(video_config.display_index) {
                    Ok(d) => d,
                    Err(e) => {
                        let msg = format!("Error captura DXGI (DirectX): {:?}", e);
                        tracing::error!("{}", msg);
                        snapshot_video.write().pipeline_error = Some(msg);
                        return;
                    }
                };

                let mut nvenc = match NvencEncoder::new(video_config.clone()) {
                    Ok(n) => n,
                    Err(e) => {
                        let msg = format!("Error encoder NVENC: {:?}", e);
                        tracing::error!("{}", msg);
                        snapshot_video.write().pipeline_error = Some(msg);
                        return;
                    }
                };

                let frame_interval = Duration::from_micros(1_000_000 / video_config.fps as u64);
                let mut raw_frame = Vec::new();

                // Real captured-FPS measurement over a rolling 1-second window.
                let mut fps_window_start = std::time::Instant::now();
                let mut frames_in_window: u32 = 0;

                while is_running_video.load(Ordering::Relaxed) {
                    let loop_start = std::time::Instant::now();

                    // acquire_frame_rgba returns Timeout when the desktop image is
                    // unchanged; that is normal and simply means no new frame to send.
                    if let Ok((width, height)) = dxgi.acquire_frame_rgba(10, &mut raw_frame) {
                        let pts = clock_video.current_pts_90khz();
                        
                        // Modificación para manejar Option<MediaPacket>
                        match nvenc.encode_rgba_frame(&raw_frame, width, height, pts) {
                            Ok(Some(video_packet)) => {
                                let _ = media_tx_video.try_send(video_packet);
                                frames_in_window += 1;
                            }
                            Ok(None) => {
                                // NVENC absorbió el frame y está esperando el próximo en cola. No hacemos nada.
                            }
                            Err(e) => {
                                tracing::warn!("Error al codificar frame de video: {:?}", e);
                            }
                        }
                    }

                    let window_elapsed = fps_window_start.elapsed();
                    if window_elapsed.as_secs_f32() >= 1.0 {
                        let fps = frames_in_window as f32 / window_elapsed.as_secs_f32();
                        snapshot_video.write().current_fps = fps;
                        frames_in_window = 0;
                        fps_window_start = std::time::Instant::now();
                    }

                    // Precise frame pacing to the configured capture rate.
                    let elapsed = loop_start.elapsed();
                    if elapsed < frame_interval {
                        thread::sleep(frame_interval - elapsed);
                    }
                }
                snapshot_video.write().current_fps = 0.0;
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
        snap.network_connected = false;
        snap.pipeline_error = None;
        tracing::info!("CastScreen streaming pipeline stopped");
    }

    /// Periodically refreshes the list of running sound applications.
    pub fn refresh_audio_sessions(&self) {
        if let Ok(sessions) = AudioSessionController::enumerate_active_sessions() {
            self.state_snapshot.write().detected_apps = sessions;
        }
    }

    /// Updates the target physical monitor to capture via DirectX 11.
    pub fn set_selected_monitor(&mut self, index: u32) {
        self.config.video.display_index = index;
        self.state_snapshot.write().selected_monitor = index;
    }

    /// Re-enumerates connected displays.
    pub fn refresh_monitors(&self) {
        if let Ok(monitors) = DxgiScreenCapture::enumerate_monitors() {
            self.state_snapshot.write().detected_monitors = monitors;
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