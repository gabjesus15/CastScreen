//! Reliable LAN stream sender (TCP transport).
//!
//! Sends the MPEG-TS byte stream over a single TCP connection to the receiving
//! laptop. TCP gives ordered, loss-free delivery: Wi-Fi 6 microdrops are
//! retransmitted transparently by the OS and backpressure paces the sender, so
//! the picture and audio never corrupt or tear. Latency is traded for perfect
//! integrity, which is exactly what a LAN game-streaming preview wants.

use castscreen_core::NetworkConfig;
use parking_lot::RwLock;
use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Retained for API/back-compat: legacy UDP datagram grouping constants.
pub const TS_PACKETS_PER_DATAGRAM: usize = 7;
pub const DATAGRAM_SIZE: usize = TS_PACKETS_PER_DATAGRAM * 188; // 1316 bytes

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Socket error: {0}")]
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
    /// True while the TCP connection to the receiver is established.
    pub connected: bool,
}

pub struct SrtSender {
    #[allow(dead_code)]
    config: NetworkConfig,
    stream: Option<TcpStream>,
    target_addr: Option<String>,
    last_connect_attempt: Option<Instant>,
    bytes_sent_window: Arc<AtomicU64>,
    total_bytes_sent: Arc<AtomicU64>,
    total_packets_sent: Arc<AtomicU64>,
    last_stats_tick: Instant,
    cached_stats: Arc<RwLock<NetworkStats>>,
    #[allow(dead_code)]
    is_running: Arc<AtomicBool>,
}

impl SrtSender {
    /// Creates a sender. No socket is bound; the sender is a TCP client that
    /// connects to the receiver once a target is set.
    pub fn new(config: NetworkConfig) -> Result<Self, NetworkError> {
        tracing::info!("TCP stream sender ready (connects on demand)");

        Ok(Self {
            config,
            stream: None,
            target_addr: None,
            last_connect_attempt: None,
            bytes_sent_window: Arc::new(AtomicU64::new(0)),
            total_bytes_sent: Arc::new(AtomicU64::new(0)),
            total_packets_sent: Arc::new(AtomicU64::new(0)),
            last_stats_tick: Instant::now(),
            cached_stats: Arc::new(RwLock::new(NetworkStats::default())),
            is_running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Sets the destination `ip:port` of the receiving laptop and forces a
    /// fresh connection attempt on the next send.
    pub fn set_target(&mut self, target: String) {
        tracing::info!("Stream target set to: {}", target);
        self.target_addr = Some(target);
        self.stream = None;
        self.last_connect_attempt = None;
    }

    /// Attempts to (re)establish the TCP connection, throttled to once per second.
    fn ensure_connected(&mut self) {
        if self.stream.is_some() {
            return;
        }
        let Some(target) = self.target_addr.clone() else {
            return;
        };

        // Throttle reconnect attempts so a missing receiver doesn't spin the CPU.
        if let Some(last) = self.last_connect_attempt {
            if last.elapsed() < Duration::from_secs(1) {
                return;
            }
        }
        self.last_connect_attempt = Some(Instant::now());

        let addr = match target.to_socket_addrs().ok().and_then(|mut a| a.next()) {
            Some(a) => a,
            None => {
                tracing::warn!("Invalid target address: {}", target);
                return;
            }
        };

        match TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
            Ok(stream) => {
                // Blocking writes with a bounded timeout give natural backpressure
                // without ever hanging the pipeline permanently.
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                let _ = stream.set_nodelay(false); // allow coalescing; latency is not a concern
                tracing::info!("Connected to receiver at {}", addr);
                self.stream = Some(stream);
            }
            Err(e) => {
                tracing::debug!("Receiver not reachable yet ({}): {}", addr, e);
            }
        }
    }

    /// Sends a block of MPEG-TS bytes to the receiver over TCP.
    pub fn send_ts_data(&mut self, ts_bytes: &[u8]) {
        self.ensure_connected();

        if let Some(stream) = self.stream.as_mut() {
            match stream.write_all(ts_bytes) {
                Ok(()) => {
                    let n = ts_bytes.len() as u64;
                    self.bytes_sent_window.fetch_add(n, Ordering::Relaxed);
                    self.total_bytes_sent.fetch_add(n, Ordering::Relaxed);
                    self.total_packets_sent
                        .fetch_add((ts_bytes.len() / 188).max(1) as u64, Ordering::Relaxed);
                }
                Err(e) => {
                    tracing::warn!("Send failed, dropping connection: {}", e);
                    self.stream = None;
                }
            }
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
            stats.buffer_fill_ratio = if self.stream.is_some() { 1.0 } else { 0.0 };
            stats.connected = self.stream.is_some();

            self.last_stats_tick = Instant::now();
        }
    }

    /// Returns the latest real-time network throughput and connection state.
    pub fn get_stats(&self) -> NetworkStats {
        *self.cached_stats.read()
    }
}
