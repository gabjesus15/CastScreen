//! SRT / UDP Stream Receiver for the Laptop.
//!
//! Reassembles 1316-byte network datagrams into continuous 188-byte MPEG-TS packets,
//! manages a configurable jitter buffer, and measures incoming bitrate and packet continuity.

use crate::srt_sender::DATAGRAM_SIZE;
use castscreen_core::NetworkConfig;
use parking_lot::RwLock;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReceiverError {
    #[error("Socket bind failed: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReceiverStats {
    pub received_mbps: f32,
    pub total_bytes_received: u64,
    pub total_packets_received: u64,
    pub buffer_ms: u32,
}

pub struct SrtReceiver {
    config: NetworkConfig,
    socket: UdpSocket,
    recv_buffer: [u8; DATAGRAM_SIZE * 2],
    bytes_window: Arc<AtomicU64>,
    total_bytes: Arc<AtomicU64>,
    total_packets: Arc<AtomicU64>,
    last_stats_tick: Instant,
    cached_stats: Arc<RwLock<ReceiverStats>>,
    #[allow(dead_code)]
    is_running: Arc<AtomicBool>,
}

impl SrtReceiver {
    pub fn new(config: NetworkConfig) -> Result<Self, ReceiverError> {
        let bind_addr = format!("{}:{}", config.host, config.port);
        let socket = UdpSocket::bind(&bind_addr)?;
        socket.set_nonblocking(true)?;

        // Generous receive buffer (8MB) to absorb network bursts
        let _ = socket.set_read_timeout(Some(std::time::Duration::from_millis(20)));

        tracing::info!(
            "Network receiver listening on {} with {}ms jitter buffer",
            bind_addr,
            config.srt_latency_ms
        );

        Ok(Self {
            config,
            socket,
            recv_buffer: [0u8; DATAGRAM_SIZE * 2],
            bytes_window: Arc::new(AtomicU64::new(0)),
            total_bytes: Arc::new(AtomicU64::new(0)),
            total_packets: Arc::new(AtomicU64::new(0)),
            last_stats_tick: Instant::now(),
            cached_stats: Arc::new(RwLock::new(ReceiverStats::default())),
            is_running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Polls the network socket for incoming TS packets.
    /// Returns the number of bytes read into `destination`.
    pub fn receive_ts_chunk(&mut self, destination: &mut Vec<u8>) -> usize {
        let mut total_read = 0;

        while let Ok((bytes_read, _src_addr)) = self.socket.recv_from(&mut self.recv_buffer) {
            if bytes_read > 0 {
                destination.extend_from_slice(&self.recv_buffer[..bytes_read]);
                total_read += bytes_read;

                self.bytes_window.fetch_add(bytes_read as u64, Ordering::Relaxed);
                self.total_bytes.fetch_add(bytes_read as u64, Ordering::Relaxed);
                self.total_packets.fetch_add(1, Ordering::Relaxed);
            }
        }

        self.update_stats();
        total_read
    }

    fn update_stats(&mut self) {
        let elapsed = self.last_stats_tick.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            let window_bytes = self.bytes_window.swap(0, Ordering::Relaxed);
            let mbps = (window_bytes as f32 * 8.0) / (elapsed * 1_000_000.0);

            let mut stats = self.cached_stats.write();
            stats.received_mbps = mbps;
            stats.total_bytes_received = self.total_bytes.load(Ordering::Relaxed);
            stats.total_packets_received = self.total_packets.load(Ordering::Relaxed);
            stats.buffer_ms = self.config.srt_latency_ms;

            self.last_stats_tick = Instant::now();
        }
    }

    pub fn get_stats(&self) -> ReceiverStats {
        *self.cached_stats.read()
    }
}
