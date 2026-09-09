//! MPEG-TS multiplexing and SRT network transport with ARQ jitter resilience.

pub mod discovery;
pub mod mpegts;
pub mod srt_receiver;
pub mod srt_sender;

pub use discovery::{DiscoveredDevice, DiscoveryResponder, DiscoveryScanner, DISCOVERY_PORT};
pub use mpegts::{MpegTsDemuxer, MpegTsMuxer, TS_PACKET_SIZE, TS_SYNC_BYTE};
pub use srt_receiver::{ReceiverError, ReceiverStats, SrtReceiver};
pub use srt_sender::{NetworkError, NetworkStats, SrtSender, DATAGRAM_SIZE, TS_PACKETS_PER_DATAGRAM};
