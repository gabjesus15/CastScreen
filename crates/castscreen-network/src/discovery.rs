//! LAN Zero-Configuration Device Discovery for CastScreen (UDP Broadcast Beacon).
//!
//! Enables automatic detection between PC Gaming (Sender) and Streaming Laptop (Receiver)
//! on the local Wi-Fi / Ethernet network without typing manual IP addresses.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const DISCOVERY_PORT: u16 = 9001;
pub const PROBE_HEADER: &str = "CASTSCREEN_PROBE_V1";
pub const BEACON_HEADER: &str = "CASTSCREEN_BEACON_V1:";

/// Information about an active CastScreen instance discovered on the local network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredDevice {
    pub device_name: String,
    pub ip: String,
    pub port: u16,
    pub role: String, // "receiver" | "sender"
    pub version: String,
}

impl DiscoveredDevice {
    pub fn name(&self) -> &str {
        &self.device_name
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BeaconPayload {
    pub device_name: String,
    pub port: u16,
    pub role: String,
    pub version: String,
}

/// Broadcasts presence on the LAN (used by Receiver / Laptop).
pub struct DiscoveryResponder {
    is_running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DiscoveryResponder {
    /// Starts broadcasting availability as a receiver on UDP port 9001.
    pub fn start(device_name: String, streaming_port: u16) -> Self {
        let is_running = Arc::new(AtomicBool::new(true));
        let running_clone = is_running.clone();

        let thread = thread::Builder::new()
            .name("discovery-responder".to_string())
            .spawn(move || {
                let socket = match UdpSocket::bind(format!("0.0.0.0:{}", DISCOVERY_PORT)) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("Could not bind discovery responder on port {}: {:?}", DISCOVERY_PORT, e);
                        return;
                    }
                };

                let _ = socket.set_broadcast(true);
                let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));

                let payload = BeaconPayload {
                    device_name,
                    port: streaming_port,
                    role: "receiver".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                };
                let json = serde_json::to_string(&payload).unwrap_or_default();
                let announce_msg = format!("{}{}", BEACON_HEADER, json);

                let mut last_broadcast = Instant::now() - Duration::from_secs(10);
                let broadcast_addr: SocketAddr = format!("255.255.255.255:{}", DISCOVERY_PORT).parse().unwrap();
                let mut buf = [0u8; 1024];

                while running_clone.load(Ordering::Relaxed) {
                    // Periodically send broadcast beacon every 1.5 seconds
                    if last_broadcast.elapsed() > Duration::from_millis(1500) {
                        let _ = socket.send_to(announce_msg.as_bytes(), broadcast_addr);
                        last_broadcast = Instant::now();
                    }

                    // Also respond directly to active probes from senders
                    if let Ok((len, src)) = socket.recv_from(&mut buf) {
                        if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                            if msg.starts_with(PROBE_HEADER) {
                                let _ = socket.send_to(announce_msg.as_bytes(), src);
                            }
                        }
                    }
                }
            })
            .ok();

        Self { is_running, thread }
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for DiscoveryResponder {
    fn drop(&mut self) {
        self.stop();
    }
}

struct DeviceEntry {
    device: DiscoveredDevice,
    last_seen: Instant,
}

/// Actively listens for and discovers CastScreen devices on the local network (used by Sender / PC Gaming).
pub struct DiscoveryScanner {
    devices: Arc<RwLock<Vec<DeviceEntry>>>,
    is_running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DiscoveryScanner {
    pub fn new() -> Self {
        let devices: Arc<RwLock<Vec<DeviceEntry>>> = Arc::new(RwLock::new(Vec::new()));
        let is_running = Arc::new(AtomicBool::new(true));

        let dev_clone = devices.clone();
        let running_clone = is_running.clone();

        let thread = thread::Builder::new()
            .name("discovery-scanner".to_string())
            .spawn(move || {
                let socket = match UdpSocket::bind("0.0.0.0:0") {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("Could not bind discovery scanner socket: {:?}", e);
                        return;
                    }
                };

                let _ = socket.set_broadcast(true);
                let _ = socket.set_read_timeout(Some(Duration::from_millis(400)));

                let target_broadcast: SocketAddr = format!("255.255.255.255:{}", DISCOVERY_PORT).parse().unwrap();
                let mut last_probe = Instant::now() - Duration::from_secs(10);
                let mut buf = [0u8; 1024];

                while running_clone.load(Ordering::Relaxed) {
                    // Send probe every 2.0 seconds
                    if last_probe.elapsed() > Duration::from_millis(2000) {
                        let _ = socket.send_to(PROBE_HEADER.as_bytes(), target_broadcast);
                        last_probe = Instant::now();
                    }

                    // Receive beacons from responders
                    if let Ok((len, src)) = socket.recv_from(&mut buf) {
                        if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                            if msg.starts_with(BEACON_HEADER) {
                                let json_part = &msg[BEACON_HEADER.len()..];
                                if let Ok(payload) = serde_json::from_str::<BeaconPayload>(json_part) {
                                    let ip = src.ip().to_string();
                                    let discovered = DiscoveredDevice {
                                        device_name: payload.device_name,
                                        ip,
                                        port: payload.port,
                                        role: payload.role,
                                        version: payload.version,
                                    };

                                    let mut list = dev_clone.write();
                                    if let Some(existing) = list.iter_mut().find(|d| d.device.ip == discovered.ip && d.device.port == discovered.port) {
                                        existing.device = discovered;
                                        existing.last_seen = Instant::now();
                                    } else {
                                        list.push(DeviceEntry {
                                            device: discovered,
                                            last_seen: Instant::now(),
                                        });
                                    }
                                }
                            }
                        }
                    }

                    // Prune stale devices not seen in the last 4.5 seconds
                    let mut list = dev_clone.write();
                    list.retain(|d| d.last_seen.elapsed() < Duration::from_millis(4500));
                }
            })
            .ok();

        Self {
            devices,
            is_running,
            thread,
        }
    }

    /// Returns a list of all currently active CastScreen devices detected on LAN.
    pub fn get_devices(&self) -> Vec<DiscoveredDevice> {
        self.devices.read().iter().map(|d| d.device.clone()).collect()
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Default for DiscoveryScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DiscoveryScanner {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beacon_serialization_and_device() {
        let payload = BeaconPayload {
            device_name: "Streaming-Laptop".to_string(),
            port: 9000,
            role: "receiver".to_string(),
            version: "0.2.0".to_string(),
        };

        let json = serde_json::to_string(&payload).expect("Serialization failed");
        let parsed: BeaconPayload = serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(parsed.device_name, "Streaming-Laptop");
        assert_eq!(parsed.port, 9000);
        assert_eq!(parsed.role, "receiver");
        assert_eq!(parsed.version, "0.2.0");

        let dev = DiscoveredDevice {
            device_name: parsed.device_name.clone(),
            ip: "192.168.1.50".to_string(),
            port: parsed.port,
            role: parsed.role,
            version: parsed.version,
        };

        assert_eq!(dev.name(), "Streaming-Laptop");
        assert_eq!(dev.ip, "192.168.1.50");
        assert_eq!(dev.port, 9000);
    }
}

