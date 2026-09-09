use castscreen_core::{MediaPacket, VideoConfig};
use thiserror::Error;
use std::mem::ManuallyDrop;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Variant::VT_BOOL;
use windows::core::VARIANT;
use windows::Win32::Foundation::VARIANT_BOOL;
use windows::core::Interface;

#[derive(Error, Debug)]
pub enum NvencError {
    #[error("NVENC initialization failed: {0}")]
    InitFailed(String),
    #[error("Hardware encode error: {0}")]
    EncodeError(String),
    #[error("Windows API error: {0}")]
    WindowsError(#[from] windows::core::Error),
    #[error("Not supported")]
    NotSupported,
}

/// Hardware NVENC encoder session using Windows Media Foundation (MFT).
pub struct NvencEncoder {
    config: VideoConfig,
    encoder: IMFTransform,
    nv12_buffer: Vec<u8>,
    output_bytes: Vec<u8>,
    frame_count: u64,
}

impl NvencEncoder {
    /// Initializes an NVENC hardware session for the specified resolution and bitrate.
    pub fn new(config: VideoConfig) -> Result<Self, NvencError> {
        tracing::info!(
            "Initializing Media Foundation H.264 Encoder: {}x{} @ {} FPS, {} kbps",
            config.width,
            config.height,
            config.fps,
            config.bitrate_kbps
        );

        let encoder = unsafe { Self::init_mft(&config)? };

        // NV12 buffer size: Y = width * height, UV = width * height / 2. Total = width * height * 1.5
        let nv12_size = (config.width * config.height + (config.width * config.height / 2)) as usize;

        Ok(Self {
            config,
            encoder,
            nv12_buffer: vec![0; nv12_size],
            output_bytes: Vec::with_capacity(1024 * 1024),
            frame_count: 0,
        })
    }

    unsafe fn init_mft(config: &VideoConfig) -> Result<IMFTransform, NvencError> {
        // Ensure COM and MF are initialized
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED); // Ignored if already initialized
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET)?;

        // Fix 1: Correct GUID constant name
        let encoder: IMFTransform = CoCreateInstance(&CLSID_MSH264EncoderMFT, None, CLSCTX_INPROC_SERVER)?;

        // Fix 2: Encode dimensions/rates into UINT64 as expected by Media Foundation
        let frame_size = ((config.width as u64) << 32) | (config.height as u64);
        let frame_rate = ((config.fps as u64) << 32) | 1;

        // Output type (H.264)
        let out_type: IMFMediaType = MFCreateMediaType()?;
        out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        out_type.SetUINT32(&MF_MT_AVG_BITRATE, config.bitrate_kbps * 1000)?;
        out_type.SetUINT64(&MF_MT_FRAME_SIZE, frame_size)?;
        out_type.SetUINT64(&MF_MT_FRAME_RATE, frame_rate)?;
        out_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        encoder.SetOutputType(0, &out_type, 0)?;

        // Input type (NV12)
        let in_type: IMFMediaType = MFCreateMediaType()?;
        in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        in_type.SetUINT64(&MF_MT_FRAME_SIZE, frame_size)?;
        in_type.SetUINT64(&MF_MT_FRAME_RATE, frame_rate)?;
        in_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        encoder.SetInputType(0, &in_type, 0)?;

        // Setup Low Latency via ICodecAPI (Ignoramos el error si el MFT no lo soporta)
        if let Ok(codec_api) = encoder.cast::<ICodecAPI>() {
            let low_latency = VARIANT::from(true);
            let _ = codec_api.SetValue(&CODECAPI_AVEncCommonLowLatency, &low_latency);
        }

        // Start Streaming (Es normal y esperado que devuelvan E_NOTIMPL, no usamos `?`)
        let _ = encoder.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
        let _ = encoder.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0);
        let _ = encoder.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0);

        Ok(encoder)
    }

    /// Converts RGBA to NV12 using the formula provided
    fn rgba_to_nv12(&mut self, rgba: &[u8], width: u32, height: u32) {
        let y_plane_size = (width * height) as usize;
        let mut y_idx = 0;
        let mut uv_idx = y_plane_size;

        for y in 0..height {
            for x in 0..width {
                let i = ((y * width + x) * 4) as usize;

                let r = rgba[i] as f32;
                let g = rgba[i + 1] as f32;
                let b = rgba[i + 2] as f32;

                let y_val = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
                self.nv12_buffer[y_idx] = y_val;
                y_idx += 1;

                if y % 2 == 0 && x % 2 == 0 {
                    let cb = (-0.169 * r - 0.331 * g + 0.5 * b + 128.0).clamp(0.0, 255.0) as u8;
                    let cr = (0.5 * r - 0.419 * g - 0.081 * b + 128.0).clamp(0.0, 255.0) as u8;
                    self.nv12_buffer[uv_idx] = cb;
                    self.nv12_buffer[uv_idx + 1] = cr;
                    uv_idx += 2;
                }
            }
        }
    }

    /// Encodes an RGBA pixel buffer into an H.264 Annex-B packet.
    pub fn encode_rgba_frame(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        pts_90khz: u64,
    ) -> Result<MediaPacket, NvencError> {
        self.frame_count += 1;
        let is_keyframe = (self.frame_count % self.config.gop_size as u64) == 1;

        // 1. Convert RGBA -> NV12
        self.rgba_to_nv12(rgba, width, height);

        let pts_100ns = (pts_90khz as u64 * 100_000) / 90_000;
        let duration_100ns = 10_000_000 / self.config.fps as u64;

        unsafe {
            // 2. Create Input Sample
            let nv12_size = self.nv12_buffer.len() as u32;
            let buffer = MFCreateMemoryBuffer(nv12_size)?;

            let mut p_data = std::ptr::null_mut();
            let mut max_len = 0;
            let mut cur_len = 0;
            buffer.Lock(&mut p_data, Some(&mut max_len), Some(&mut cur_len))?;
            std::ptr::copy_nonoverlapping(self.nv12_buffer.as_ptr(), p_data as *mut u8, nv12_size as usize);
            buffer.Unlock()?;
            buffer.SetCurrentLength(nv12_size)?;

            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(pts_100ns as i64)?;
            sample.SetSampleDuration(duration_100ns as i64)?;

            // 3. Process Input
            self.encoder.ProcessInput(0, &sample, 0)?;

            // 4. Get Output
            self.output_bytes.clear();
            
            let mut status = 0u32;
            loop {
                // Fix 5: GetOutputStreamInfo takes 0 arguments and returns the struct directly
                let stream_info = self.encoder.GetOutputStreamInfo(0)?;
                
                let out_sample = MFCreateSample()?;
                let out_buffer = MFCreateMemoryBuffer(stream_info.cbSize)?;
                out_sample.AddBuffer(&out_buffer)?;
                
                // Fix 6: Wrap COM pointers in ManuallyDrop for MFT_OUTPUT_DATA_BUFFER
                let mut output_data_arr = [MFT_OUTPUT_DATA_BUFFER {
                    dwStreamID: 0,
                    pSample: ManuallyDrop::new(Some(out_sample.clone())),
                    dwStatus: 0,
                    pEvents: ManuallyDrop::new(None),
                }];

                // Fix 7: Handle Result rather than checking HRESULT directly
                match self.encoder.ProcessOutput(0, &mut output_data_arr, &mut status) {
                    Ok(_) => {
                        // Dereference ManuallyDrop to access Option
                        if let Some(valid_sample) = &*output_data_arr[0].pSample {
                            let final_buffer = valid_sample.ConvertToContiguousBuffer()?;
                            let mut p_out_data = std::ptr::null_mut();
                            let mut out_len = 0;
                            final_buffer.Lock(&mut p_out_data, None, Some(&mut out_len))?;

                            let slice = std::slice::from_raw_parts(p_out_data as *const u8, out_len as usize);
                            self.output_bytes.extend_from_slice(slice);

                            final_buffer.Unlock()?;
                        }
                    }
                    Err(e) => {
                        if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT {
                            break;
                        }
                        return Err(NvencError::EncodeError(format!("ProcessOutput failed: {:?}", e)));
                    }
                }
            }
        }

        Ok(MediaPacket::new_video(
            pts_90khz,
            pts_90khz,
            self.output_bytes.clone(),
            is_keyframe,
        ))
    }
}

impl Drop for NvencEncoder {
    fn drop(&mut self) {
        unsafe {
            let _ = self.encoder.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = self.encoder.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
            let _ = MFShutdown();
        }
    }
}