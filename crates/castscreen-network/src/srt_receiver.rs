//! Reliable LAN stream receiver (TCP transport).
//!
//! Listens for the sender's TCP connection and reads the MPEG-TS byte stream.
//! Because TCP guarantees ordered, loss-free delivery, the demuxer never sees
//! gaps or corruption from Wi-Fi packet loss — the cause of the tearing and
//! audio drops seen with UDP-based tools. Reads are non-blocking so the egui
//! render loop that polls this receiver is never stalled.

use castscreen_core::NetworkConfig;
use parking_lot::RwLock;
use std::io::{ErrorKind, Read};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

const RECV_CHUNK: usize = 65536;

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
    /// True while a sender is connected over TCP.
    pub connected: bool,
}

pub struct SrtReceiver {
    config: NetworkConfig,
    listener: TcpListener,
    stream: Option<TcpStream>,
    recv_buffer: Vec<u8>,
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
        let listener = TcpListener::bind(&bind_addr)?;
        listener.set_nonblocking(true)?;

        tracing::info!("TCP stream receiver listening on {}", bind_addr);

        Ok(Self {
            config,
            listener,
            stream: None,
            recv_buffer: vec![0u8; RECV_CHUNK],
            bytes_window: Arc::new(AtomicU64::new(0)),
            total_bytes: Arc::new(AtomicU64::new(0)),
            total_packets: Arc::new(AtomicU64::new(0)),
            last_stats_tick: Instant::now(),
            cached_stats: Arc::new(RwLock::new(ReceiverStats::default())),
            is_running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Polls for incoming stream bytes. Non-blocking: returns however many bytes
    /// were available this call (0 if none / not connected).
    pub fn receive_ts_chunk(&mut self, destination: &mut Vec<u8>) -> usize {
        // Accept a new connection if we don't have one.
        if self.stream.is_none() {
            match self.listener.accept() {
                Ok((stream, peer)) => {
                    let _ = stream.set_nonblocking(true);
                    tracing::info!("Sender connected from {}", peer);
                    self.stream = Some(stream);
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => tracing::debug!("Accept error: {}", e),
            }
        }

        let mut total_read = 0;

        if let Some(stream) = self.stream.as_mut() {
            loop {
                match stream.read(&mut self.recv_buffer) {
                    Ok(0) => {
                        // Peer closed the connection cleanly.
                        tracing::info!("Sender disconnected");
                        self.stream = None;
                        break;
                    }
                    Ok(n) => {
                        destination.extend_from_slice(&self.recv_buffer[..n]);
                        total_read += n;
                        self.bytes_window.fetch_add(n as u64, Ordering::Relaxed);
                        self.total_bytes.fetch_add(n as u64, Ordering::Relaxed);
                        self.total_packets
                            .fetch_add((n / 188).max(1) as u64, Ordering::Relaxed);
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) => {
                        tracing::warn!("Read error, dropping connection: {}", e);
                        self.stream = None;
                        break;
                    }
                }
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
            stats.connected = self.stream.is_some();

            self.last_stats_tick = Instant::now();
        }
    }

    pub fn get_stats(&self) -> ReceiverStats {
        *self.cached_stats.read()
    }
}
