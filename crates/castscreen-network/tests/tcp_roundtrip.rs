//! End-to-end transport test: real bytes over a real TCP loopback connection,
//! muxed on the sender and demuxed on the receiver, video and audio interleaved.

use castscreen_core::{MediaPacket, NetworkConfig};
use castscreen_network::{MpegTsDemuxer, MpegTsMuxer, SrtReceiver, SrtSender};
use std::thread::sleep;
use std::time::{Duration, Instant};

#[test]
fn tcp_pipeline_delivers_video_and_audio() {
    let port = 34_567u16;

    let mut rx_cfg = NetworkConfig::default();
    rx_cfg.host = "127.0.0.1".to_string();
    rx_cfg.port = port;
    let mut receiver = SrtReceiver::new(rx_cfg).expect("receiver bind");

    let mut sender = SrtSender::new(NetworkConfig::default()).expect("sender init");
    sender.set_target(format!("127.0.0.1:{}", port));

    let mut muxer = MpegTsMuxer::new();
    let mut demuxer = MpegTsDemuxer::new();

    let video_payload = vec![0xABu8; 6000];
    let audio_payload = vec![0x5Au8; 3072];

    let video = MediaPacket::new_video(90_000, 90_000, video_payload.clone(), true);
    let audio = MediaPacket::new_audio(90_000, audio_payload.clone());

    // The sender connects lazily on first send; retry until the receiver accepts.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut got_video: Option<Vec<u8>> = None;
    let mut got_audio: Option<Vec<u8>> = None;
    let mut drain = Vec::new();

    while Instant::now() < deadline && (got_video.is_none() || got_audio.is_none()) {
        sender.send_ts_data(&muxer.mux_packet(&video));
        sender.send_ts_data(&muxer.mux_packet(&audio));

        drain.clear();
        if receiver.receive_ts_chunk(&mut drain) > 0 {
            demuxer.feed_ts_bytes(&drain);
            demuxer.flush();
            if got_video.is_none() {
                got_video = demuxer.next_video_frame();
            }
            if got_audio.is_none() {
                got_audio = demuxer.next_audio_frame();
            }
        }
        sleep(Duration::from_millis(20));
    }

    assert_eq!(got_video.expect("no video received"), video_payload);
    assert_eq!(got_audio.expect("no audio received"), audio_payload);
}
