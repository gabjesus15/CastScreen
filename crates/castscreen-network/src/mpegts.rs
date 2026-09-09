//! ISO/IEC 13818-1 MPEG-2 Transport Stream (MPEG-TS) Multiplexer.
//!
//! Packs H.264 video and AAC audio PES packets into standard 188-byte TS packets
//! with Program Association Table (PAT), Program Map Table (PMT), and PCR clock references.

use castscreen_core::{MediaPacket, MediaType};

pub const TS_PACKET_SIZE: usize = 188;
pub const TS_SYNC_BYTE: u8 = 0x47;

pub const PID_PAT: u16 = 0x0000;
pub const PID_PMT: u16 = 0x1000;
pub const PID_VIDEO: u16 = 0x0100;
pub const PID_AUDIO: u16 = 0x0101;

/// Computes the MPEG-2 systems CRC-32 (polynomial 0x04C11DB7, no final XOR)
/// used to terminate PSI sections (PAT / PMT). Players validate this checksum,
/// so it must be real, not a placeholder.
pub(crate) fn crc32_mpeg(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= (byte as u32) << 24;
        for _ in 0..8 {
            if crc & 0x8000_0000 != 0 {
                crc = (crc << 1) ^ 0x04C1_1DB7;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

pub struct MpegTsMuxer {
    pat_continuity: u8,
    pmt_continuity: u8,
    video_continuity: u8,
    audio_continuity: u8,
    frames_since_pat: u32,
}

impl MpegTsMuxer {
    pub fn new() -> Self {
        Self {
            pat_continuity: 0,
            pmt_continuity: 0,
            video_continuity: 0,
            audio_continuity: 0,
            frames_since_pat: 0,
        }
    }

    /// Multiplexes a MediaPacket (Video or Audio) into a vector of 188-byte TS packets.
    pub fn mux_packet(&mut self, packet: &MediaPacket) -> Vec<u8> {
        let mut output = Vec::with_capacity(packet.payload.len() + 512);

        // Periodically inject PAT & PMT (every 30 frames or on keyframes)
        if self.frames_since_pat == 0 || packet.is_keyframe {
            self.inject_pat(&mut output);
            self.inject_pmt(&mut output);
            self.frames_since_pat = 30;
        } else {
            self.frames_since_pat = self.frames_since_pat.saturating_sub(1);
        }

        // Build PES Packet
        let (pid, stream_id, is_video) = match packet.media_type {
            MediaType::VideoH264 => (PID_VIDEO, 0xE0u8, true),
            MediaType::AudioAac => (PID_AUDIO, 0xC0u8, false),
        };

        let pes_data = Self::create_pes_packet(
            stream_id,
            packet.pts_90khz,
            packet.dts_90khz,
            &packet.payload,
        );

        // Packetize PES into 188-byte TS packets
        self.packetize_pes(
            pid,
            &pes_data,
            is_video && packet.is_keyframe,
            packet.pts_90khz,
            &mut output,
        );

        output
    }

    /// Constructs a PES (Packetized Elementary Stream) packet with PTS/DTS headers.
    fn create_pes_packet(
        stream_id: u8,
        pts: u64,
        dts: u64,
        payload: &[u8],
    ) -> Vec<u8> {
        let has_dts = pts != dts && dts > 0;
        let header_data_len = if has_dts { 10 } else { 5 };
        let pes_packet_length = if payload.len() + 3 + header_data_len > 65535 {
            0 // 0 is allowed for unbounded video streams in TS
        } else {
            payload.len() + 3 + header_data_len
        };

        let mut pes = Vec::with_capacity(14 + payload.len());

        // PES start code prefix: 0x000001
        pes.extend_from_slice(&[0x00, 0x00, 0x01]);
        pes.push(stream_id);

        // PES packet length (16 bits)
        pes.push(((pes_packet_length >> 8) & 0xFF) as u8);
        pes.push((pes_packet_length & 0xFF) as u8);

        // Flags: [1000 0000] = 0x80
        pes.push(0x80);

        // PTS/DTS flags: [0010 0000] for PTS only (0x80), [0011 0000] for PTS+DTS (0xC0)
        let pts_dts_flags = if has_dts { 0xC0 } else { 0x80 };
        pes.push(pts_dts_flags);

        // PES header data length
        pes.push(header_data_len as u8);

        // Encode 33-bit PTS into 5 bytes
        Self::encode_pts(&mut pes, pts, if has_dts { 0x03 } else { 0x02 });

        if has_dts {
            // Encode DTS
            Self::encode_pts(&mut pes, dts, 0x01);
        }

        pes.extend_from_slice(payload);
        pes
    }

    /// Formats a 33-bit timestamp into MPEG-TS 5-byte representation.
    fn encode_pts(dest: &mut Vec<u8>, pts: u64, prefix: u8) {
        let pts_32_30 = ((pts >> 30) & 0x07) as u8;
        let pts_29_15 = ((pts >> 15) & 0x7FFF) as u16;
        let pts_14_0 = (pts & 0x7FFF) as u16;

        dest.push((prefix << 4) | (pts_32_30 << 1) | 0x01);
        dest.push(((pts_29_15 >> 7) & 0xFF) as u8);
        dest.push((((pts_29_15 & 0x7F) << 1) | 0x01) as u8);
        dest.push(((pts_14_0 >> 7) & 0xFF) as u8);
        dest.push((((pts_14_0 & 0x7F) << 1) | 0x01) as u8);
    }

    /// Slices PES payload into contiguous 188-byte TS packets.
    fn packetize_pes(
        &mut self,
        pid: u16,
        pes_data: &[u8],
        insert_pcr: bool,
        pts: u64,
        output: &mut Vec<u8>,
    ) {
        let mut offset = 0;
        let total = pes_data.len();
        let mut is_first = true;

        while offset < total {
            let mut packet = [0xFFu8; TS_PACKET_SIZE];
            packet[0] = TS_SYNC_BYTE;

            let continuity = match pid {
                PID_VIDEO => &mut self.video_continuity,
                _ => &mut self.audio_continuity,
            };

            // Byte 1-2: Transport error [0], Payload unit start indicator [is_first], Transport priority [0], PID [13 bits]
            let pusi = if is_first { 0x40 } else { 0x00 };
            packet[1] = pusi | (((pid >> 8) & 0x1F) as u8);
            packet[2] = (pid & 0xFF) as u8;

            // Byte 3: Scrambling [00], Adaptation field control [01 or 11], Continuity counter [4 bits]
            let needs_pcr = is_first && insert_pcr;
            let remaining = total - offset;

            let chunk_len;
            if needs_pcr {
                // Header (4) + Adapt Len (1) + Flags (1) + PCR (6) = 12 bytes minimum
                // Max payload with PCR is 188 - 12 = 176
                chunk_len = remaining.min(176);
                let adapt_len = 183 - chunk_len;

                packet[3] = 0x30 | (*continuity & 0x0F); // Adaptation + payload
                packet[4] = adapt_len as u8;
                packet[5] = 0x10; // PCR flag

                // Write 42-bit PCR: pts in 90kHz scale
                let pcr_base = pts;
                packet[6] = ((pcr_base >> 25) & 0xFF) as u8;
                packet[7] = ((pcr_base >> 17) & 0xFF) as u8;
                packet[8] = ((pcr_base >> 9) & 0xFF) as u8;
                packet[9] = ((pcr_base >> 1) & 0xFF) as u8;
                packet[10] = (((pcr_base & 0x01) << 7) | 0x7E) as u8;
                packet[11] = 0x00; // PCR extension

                // Copy payload at the end of the packet (exactly chunk_len bytes)
                let payload_start = 5 + adapt_len;
                packet[payload_start..188]
                    .copy_from_slice(&pes_data[offset..offset + chunk_len]);
            } else if remaining >= 184 {
                // Pure payload: 184 bytes
                chunk_len = 184;
                packet[3] = 0x10 | (*continuity & 0x0F); // Payload only
                packet[4..188].copy_from_slice(&pes_data[offset..offset + chunk_len]);
            } else {
                // Padding required via adaptation field
                chunk_len = remaining;
                packet[3] = 0x30 | (*continuity & 0x0F); // Adaptation + payload

                if chunk_len == 183 {
                    packet[4] = 0x00; // Adaptation field length 0
                    packet[5..188].copy_from_slice(&pes_data[offset..offset + chunk_len]);
                } else {
                    let adapt_len = 183 - chunk_len;
                    packet[4] = adapt_len as u8;
                    packet[5] = 0x00; // Flags: no optional fields, remaining bytes are stuffing (0xFF)
                    let payload_start = 5 + adapt_len;
                    packet[payload_start..188]
                        .copy_from_slice(&pes_data[offset..offset + chunk_len]);
                }
            }

            *continuity = (*continuity + 1) & 0x0F;
            output.extend_from_slice(&packet);

            offset += chunk_len;
            is_first = false;
        }
    }

    /// Injects standard Program Association Table (PAT).
    fn inject_pat(&mut self, output: &mut Vec<u8>) {
        let mut pat_packet = [0xFFu8; TS_PACKET_SIZE];
        pat_packet[0] = TS_SYNC_BYTE;
        pat_packet[1] = 0x40 | (((PID_PAT >> 8) & 0x1F) as u8);
        pat_packet[2] = (PID_PAT & 0xFF) as u8;
        pat_packet[3] = 0x10 | (self.pat_continuity & 0x0F);
        self.pat_continuity = (self.pat_continuity + 1) & 0x0F;

        // Pointer field
        pat_packet[4] = 0x00;

        // Table ID: 0x00 (PAT)
        pat_packet[5] = 0x00;
        // Section syntax indicator [1], '0' [0], Reserved [11], Section length [13 = 0x00D]
        pat_packet[6] = 0xB0;
        pat_packet[7] = 0x0D;

        // Transport Stream ID: 0x0001
        pat_packet[8] = 0x00;
        pat_packet[9] = 0x01;
        // Version [0], Current/Next [1]
        pat_packet[10] = 0xC1;
        // Section number [0], Last section number [0]
        pat_packet[11] = 0x00;
        pat_packet[12] = 0x00;

        // Program 1 -> PMT PID 0x1000
        pat_packet[13] = 0x00;
        pat_packet[14] = 0x01;
        pat_packet[15] = 0xE0 | (((PID_PMT >> 8) & 0x1F) as u8);
        pat_packet[16] = (PID_PMT & 0xFF) as u8;

        // CRC-32 over the section (table_id .. last program byte), big-endian.
        let crc = crc32_mpeg(&pat_packet[5..17]);
        pat_packet[17] = ((crc >> 24) & 0xFF) as u8;
        pat_packet[18] = ((crc >> 16) & 0xFF) as u8;
        pat_packet[19] = ((crc >> 8) & 0xFF) as u8;
        pat_packet[20] = (crc & 0xFF) as u8;

        output.extend_from_slice(&pat_packet);
    }

    /// Injects standard Program Map Table (PMT) declaring Video H.264 (0x1B) and Audio AAC (0x0F).
    fn inject_pmt(&mut self, output: &mut Vec<u8>) {
        let mut pmt_packet = [0xFFu8; TS_PACKET_SIZE];
        pmt_packet[0] = TS_SYNC_BYTE;
        pmt_packet[1] = 0x40 | (((PID_PMT >> 8) & 0x1F) as u8);
        pmt_packet[2] = (PID_PMT & 0xFF) as u8;
        pmt_packet[3] = 0x10 | (self.pmt_continuity & 0x0F);
        self.pmt_continuity = (self.pmt_continuity + 1) & 0x0F;

        // Pointer field
        pmt_packet[4] = 0x00;
        // Table ID: 0x02 (PMT)
        pmt_packet[5] = 0x02;
        // Section length: 23 bytes (0x017)
        pmt_packet[6] = 0xB0;
        pmt_packet[7] = 0x17;

        // Program number: 0x0001
        pmt_packet[8] = 0x00;
        pmt_packet[9] = 0x01;
        // Version 0, Current 1
        pmt_packet[10] = 0xC1;
        pmt_packet[11] = 0x00;
        pmt_packet[12] = 0x00;

        // PCR PID: Video PID (0x0100)
        pmt_packet[13] = 0xE0 | (((PID_VIDEO >> 8) & 0x1F) as u8);
        pmt_packet[14] = (PID_VIDEO & 0xFF) as u8;
        // Program info length: 0
        pmt_packet[15] = 0xF0;
        pmt_packet[16] = 0x00;

        // Stream 1: Video H.264 (Stream type 0x1B)
        pmt_packet[17] = 0x1B;
        pmt_packet[18] = 0xE0 | (((PID_VIDEO >> 8) & 0x1F) as u8);
        pmt_packet[19] = (PID_VIDEO & 0xFF) as u8;
        pmt_packet[20] = 0xF0;
        pmt_packet[21] = 0x00;

        // Stream 2: Audio AAC ADTS (Stream type 0x0F)
        pmt_packet[22] = 0x0F;
        pmt_packet[23] = 0xE0 | (((PID_AUDIO >> 8) & 0x1F) as u8);
        pmt_packet[24] = (PID_AUDIO & 0xFF) as u8;
        pmt_packet[25] = 0xF0;
        pmt_packet[26] = 0x00;

        // CRC-32 over the section (table_id .. last stream-info byte), big-endian.
        let crc = crc32_mpeg(&pmt_packet[5..27]);
        pmt_packet[27] = ((crc >> 24) & 0xFF) as u8;
        pmt_packet[28] = ((crc >> 16) & 0xFF) as u8;
        pmt_packet[29] = ((crc >> 8) & 0xFF) as u8;
        pmt_packet[30] = (crc & 0xFF) as u8;

        output.extend_from_slice(&pmt_packet);
    }
}

impl Default for MpegTsMuxer {
    fn default() -> Self {
        Self::new()
    }
}

/// MPEG-2 Transport Stream Demultiplexer.
///
/// Reassembles 188-byte TS packets from the network, demultiplexes video PES streams
/// (PID 0x0100), and outputs complete frame payloads ready for decoding.
pub struct MpegTsDemuxer {
    video_pes_buffer: Vec<u8>,
    completed_video_frames: Vec<Vec<u8>>,
    audio_pes_buffer: Vec<u8>,
    completed_audio_frames: Vec<Vec<u8>>,
    leftover: Vec<u8>,
}

impl Default for MpegTsDemuxer {
    fn default() -> Self {
        Self::new()
    }
}

impl MpegTsDemuxer {
    pub fn new() -> Self {
        Self {
            video_pes_buffer: Vec::with_capacity(262144),
            completed_video_frames: Vec::new(),
            audio_pes_buffer: Vec::with_capacity(65536),
            completed_audio_frames: Vec::new(),
            leftover: Vec::with_capacity(TS_PACKET_SIZE * 2),
        }
    }

    /// Feeds incoming raw network data into the demultiplexer.
    pub fn feed_ts_bytes(&mut self, data: &[u8]) {
        let mut stream = Vec::with_capacity(self.leftover.len() + data.len());
        stream.extend_from_slice(&self.leftover);
        stream.extend_from_slice(data);
        self.leftover.clear();

        let mut offset = 0;
        while offset + TS_PACKET_SIZE <= stream.len() {
            // Synchronize on TS Sync Byte (0x47)
            if stream[offset] != TS_SYNC_BYTE {
                offset += 1;
                continue;
            }

            let packet = &stream[offset..offset + TS_PACKET_SIZE];
            self.process_ts_packet(packet);
            offset += TS_PACKET_SIZE;
        }

        if offset < stream.len() {
            self.leftover.extend_from_slice(&stream[offset..]);
        }
    }

    fn process_ts_packet(&mut self, packet: &[u8]) {
        let pusi = (packet[1] & 0x40) != 0;
        let pid = (((packet[1] & 0x1F) as u16) << 8) | (packet[2] as u16);
        let adapt_ctrl = (packet[3] >> 4) & 0x03;

        if pid != PID_VIDEO && pid != PID_AUDIO {
            return;
        }

        let mut payload_offset = 4;

        // Handle adaptation field
        if adapt_ctrl == 0b10 {
            // Adaptation field only, no payload
            return;
        } else if adapt_ctrl == 0b11 {
            // Adaptation field followed by payload
            let adapt_len = packet[4] as usize;
            payload_offset = 5 + adapt_len;
            if payload_offset >= TS_PACKET_SIZE {
                return;
            }
        } else if adapt_ctrl == 0b00 {
            // Reserved
            return;
        }

        let ts_payload = &packet[payload_offset..TS_PACKET_SIZE];

        let (pes_buffer, completed) = if pid == PID_VIDEO {
            (&mut self.video_pes_buffer, &mut self.completed_video_frames)
        } else {
            (&mut self.audio_pes_buffer, &mut self.completed_audio_frames)
        };

        if pusi {
            // Start of a new PES packet: finalize the previous elementary frame.
            if !pes_buffer.is_empty() {
                if let Some(frame) = Self::extract_pes_payload(pes_buffer) {
                    completed.push(frame);
                }
                pes_buffer.clear();
            }
        }

        pes_buffer.extend_from_slice(ts_payload);
    }

    fn extract_pes_payload(pes: &[u8]) -> Option<Vec<u8>> {
        if pes.len() < 9 {
            return None;
        }
        if pes[0] != 0x00 || pes[1] != 0x00 || pes[2] != 0x01 {
            return None;
        }

        let pes_packet_len = ((pes[4] as usize) << 8) | (pes[5] as usize);
        let header_data_len = pes[8] as usize;
        let payload_start = 9 + header_data_len;

        if payload_start > pes.len() {
            return None;
        }

        if pes_packet_len > 0 && pes_packet_len + 6 <= pes.len() {
            Some(pes[payload_start..6 + pes_packet_len].to_vec())
        } else {
            Some(pes[payload_start..].to_vec())
        }
    }

    /// Retrieves the next available decoded video frame payload.
    pub fn next_video_frame(&mut self) -> Option<Vec<u8>> {
        if !self.completed_video_frames.is_empty() {
            Some(self.completed_video_frames.remove(0))
        } else {
            None
        }
    }

    /// Retrieves the next available audio frame payload (interleaved i16 LE PCM).
    pub fn next_audio_frame(&mut self) -> Option<Vec<u8>> {
        if !self.completed_audio_frames.is_empty() {
            Some(self.completed_audio_frames.remove(0))
        } else {
            None
        }
    }

    /// Flushes any pending PES packets currently in the buffers.
    pub fn flush(&mut self) {
        if !self.video_pes_buffer.is_empty() {
            if let Some(frame) = Self::extract_pes_payload(&self.video_pes_buffer) {
                self.completed_video_frames.push(frame);
            }
            self.video_pes_buffer.clear();
        }
        if !self.audio_pes_buffer.is_empty() {
            if let Some(frame) = Self::extract_pes_payload(&self.audio_pes_buffer) {
                self.completed_audio_frames.push(frame);
            }
            self.audio_pes_buffer.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ts_packets_alignment() {
        let mut muxer = MpegTsMuxer::new();
        let payload = vec![0x12; 500];
        let packet = MediaPacket::new_video(90_000, 90_000, payload, true);
        let ts_data = muxer.mux_packet(&packet);

        // Every TS packet must be exactly 188 bytes long
        assert_eq!(ts_data.len() % TS_PACKET_SIZE, 0);

        // Verify that every packet begins with 0x47 sync byte
        for chunk in ts_data.chunks_exact(TS_PACKET_SIZE) {
            assert_eq!(chunk[0], TS_SYNC_BYTE);
        }
    }

    #[test]
    fn test_muxer_demuxer_roundtrip() {
        let mut muxer = MpegTsMuxer::new();
        let mut demuxer = MpegTsDemuxer::new();

        let original_frame_1 = vec![0xAA; 4096];
        let original_frame_2 = vec![0xBB; 8192];

        let p1 = MediaPacket::new_video(90_000, 90_000, original_frame_1.clone(), true);
        let p2 = MediaPacket::new_video(91_500, 91_500, original_frame_2.clone(), false);

        let ts1 = muxer.mux_packet(&p1);
        let ts2 = muxer.mux_packet(&p2);

        demuxer.feed_ts_bytes(&ts1);
        demuxer.feed_ts_bytes(&ts2);
        demuxer.flush();

        let frame1 = demuxer.next_video_frame().expect("Frame 1 missing");
        let frame2 = demuxer.next_video_frame().expect("Frame 2 missing");

        assert_eq!(frame1, original_frame_1);
        assert_eq!(frame2, original_frame_2);
    }

    #[test]
    fn test_audio_roundtrip() {
        let mut muxer = MpegTsMuxer::new();
        let mut demuxer = MpegTsDemuxer::new();

        // Two audio PCM payloads interleaved with a video keyframe (which triggers
        // PAT/PMT injection) to exercise multi-PID demultiplexing.
        let audio_1 = vec![0x11u8; 3072];
        let audio_2 = vec![0x22u8; 3072];

        let v = MediaPacket::new_video(90_000, 90_000, vec![0x33; 2048], true);
        let a1 = MediaPacket::new_audio(90_000, audio_1.clone());
        let a2 = MediaPacket::new_audio(91_920, audio_2.clone());

        demuxer.feed_ts_bytes(&muxer.mux_packet(&v));
        demuxer.feed_ts_bytes(&muxer.mux_packet(&a1));
        demuxer.feed_ts_bytes(&muxer.mux_packet(&a2));
        demuxer.flush();

        let got1 = demuxer.next_audio_frame().expect("Audio 1 missing");
        let got2 = demuxer.next_audio_frame().expect("Audio 2 missing");
        assert_eq!(got1, audio_1);
        assert_eq!(got2, audio_2);
    }

    #[test]
    fn test_psi_crc_matches_section() {
        let mut muxer = MpegTsMuxer::new();
        let ts = muxer.mux_packet(&MediaPacket::new_video(0, 0, vec![0x00; 32], true));

        // Packet 0 = PAT (CRC over bytes 5..17, stored at 17..21).
        let pat = &ts[0..TS_PACKET_SIZE];
        let pat_crc = crc32_mpeg(&pat[5..17]).to_be_bytes();
        assert_eq!(&pat[17..21], &pat_crc);

        // Packet 1 = PMT (CRC over bytes 5..27, stored at 27..31).
        let pmt = &ts[TS_PACKET_SIZE..TS_PACKET_SIZE * 2];
        let pmt_crc = crc32_mpeg(&pmt[5..27]).to_be_bytes();
        assert_eq!(&pmt[27..31], &pmt_crc);
    }

    #[test]
    fn test_crc32_mpeg_known_vector() {
        // "123456789" under CRC-32/MPEG-2 => 0x0376E6E7.
        assert_eq!(crc32_mpeg(b"123456789"), 0x0376_E6E7);
    }
}

