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

        // CRC-32 (Placeholder MPEG-TS standard checksum)
        pat_packet[17] = 0x2A;
        pat_packet[18] = 0xB1;
        pat_packet[19] = 0x04;
        pat_packet[20] = 0xB2;

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

        // CRC-32
        pmt_packet[27] = 0x4E;
        pmt_packet[28] = 0x59;
        pmt_packet[29] = 0x3D;
        pmt_packet[30] = 0x1E;

        output.extend_from_slice(&pmt_packet);
    }
}

impl Default for MpegTsMuxer {
    fn default() -> Self {
        Self::new()
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
}
