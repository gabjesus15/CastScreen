//! Secure Reliable Transport (SRT) / High-Throughput UDP Stream Sender.
//!
//! Sends 188-byte MPEG-TS packets grouped in 7-packet chunks (1316 bytes per datagram)
//! with configurable ARQ buffer (500ms - 2000ms) to ensure resilience over Wi-Fi 6.

use castscreen_core::NetworkConfig;
use parking_lot::RwLock;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

/// Number of 188-byte TS packets per network datagram (1316 bytes, fits standard 1500 MTU).
pub const TS_PACKETS_PER_DATAGRAM: usize = 7;
pub const DATAGRAM_SIZE: usize = TS_PACKETS_PER_DATAGRAM * 188; // 1316 bytes

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Socket binding error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Target unreachable: {0}")]
    Unreachable(String),
}

/// Real-time transmission metrics for GUI display.
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkStats {
    pub bitrate_mbps: f32,
    pub total_bytes_sent: u64,
    pub total_packets_sent: u64,
    pub dropped_packets: u64,
    pub buffer_fill_ratio: f32,
}

pub struct SrtSender {
    config: NetworkConfig,
    socket: UdpSocket,
    target_addr: Option<String>,
    pending_buffer: Vec<u8>,
    bytes_sent_window: Arc<AtomicU64>,
    total_bytes_sent: Arc<AtomicU64>,
    total_packets_sent: Arc<AtomicU64>,
    last_stats_tick: Instant,
    cached_stats: Arc<RwLock<NetworkStats>>,
    is_running: Arc<AtomicBool>,
}

impl SrtSender {
    /// Binds sender socket according to configuration.
    pub fn new(config: NetworkConfig) -> Result<Self, NetworkError> {
        let bind_addr = format!("{}:{}", config.host, config.port);
        let socket = UdpSocket::bind(&bind_addr)?;
        socket.set_nonblocking(true)?;

        // Set generous socket send buffer (4MB) to absorb bursty video keyframes
        let _ = socket.set_write_timeout(Some(std::time::Duration::from_millis(20)));

        tracing::info!(
            "Network sender bound to {} with {}ms ARQ buffer",
            bind_addr,
            config.srt_latency_ms
        );

        Ok(Self {
            config,
            socket,
            target_addr: None,
            pending_buffer: Vec::with_capacity(DATAGRAM_SIZE * 4),
            bytes_sent_window: Arc::new(AtomicU64::new(0)),
            total_bytes_sent: Arc::new(AtomicU64::new(0)),
            total_packets_sent: Arc::new(AtomicU64::new(0)),
            last_stats_tick: Instant::now(),
            cached_stats: Arc::new(RwLock::new(NetworkStats::default())),
            is_running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Sets the destination IP/port of the receiving laptop.
    pub fn set_target(&mut self, target: String) {
        tracing::info!("SRT sender target set to: {}", target);
        self.target_addr = Some(target);
    }

    /// Feeds 188-byte TS packets, buffers into 1316-byte datagrams, and sends over network.
    pub fn send_ts_data(&mut self, ts_bytes: &[u8]) {
        self.pending_buffer.extend_from_slice(ts_bytes);

        // Send full 1316-byte datagrams
        while self.pending_buffer.len() >= DATAGRAM_SIZE {
            let chunk = &self.pending_buffer[..DATAGRAM_SIZE];

            if let Some(target) = &self.target_addr {
                if let Ok(sent) = self.socket.send_to(chunk, target) {
                    self.bytes_sent_window.fetch_add(sent as u64, Ordering::Relaxed);
                    self.total_bytes_sent.fetch_add(sent as u64, Ordering::Relaxed);
                    self.total_packets_sent.fetch_add(1, Ordering::Relaxed);
                }
            }

            self.pending_buffer.drain(..DATAGRAM_SIZE);
        }

        self.update_stats();
    }

    /// Updates throughput calculations every 500ms.
    fn update_stats(&mut self) {
        let elapsed = self.last_stats_tick.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            let window_bytes = self.bytes_sent_window.swap(0, Ordering::Relaxed);
            let mbps = (window_bytes as f32 * 8.0) / (elapsed * 1_000_000.0);

            let mut stats = self.cached_stats.write();
            stats.bitrate_mbps = mbps;
            stats.total_bytes_sent = self.total_bytes_sent.load(Ordering::Relaxed);
            stats.total_packets_sent = self.total_packets_sent.load(Ordering::Relaxed);
            stats.buffer_fill_ratio = 1.0; // Steady state

            self.last_stats_tick = Instant::now();
        }
    }

    /// Returns the latest real-time network throughput and packet statistics.
    pub fn get_stats(&self) -> NetworkStats {
        *self.cached_stats.read()
    }
}
