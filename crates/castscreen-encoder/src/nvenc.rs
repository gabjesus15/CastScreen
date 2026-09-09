use castscreen_core::{MediaPacket, VideoConfig};
use thiserror::Error;
use std::mem::ManuallyDrop;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::*;
use windows::core::VARIANT;
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

pub struct NvencEncoder {
    config: VideoConfig,
    encoder: IMFTransform,
    nv12_buffer: Vec<u8>,
    output_bytes: Vec<u8>,
    frame_count: u64,
    needs_input: bool, 
}

impl NvencEncoder {
    pub fn new(config: VideoConfig) -> Result<Self, NvencError> {
        tracing::info!(
            "Initializing Media Foundation H.264 Hardware Encoder: {}x{} @ {} FPS, {} kbps",
            config.width,
            config.height,
            config.fps,
            config.bitrate_kbps
        );

        let encoder = unsafe { Self::init_mft(&config)? };
        let nv12_size = (config.width * config.height + (config.width * config.height / 2)) as usize;

        Ok(Self {
            config,
            encoder,
            nv12_buffer: vec![0; nv12_size],
            output_bytes: Vec::with_capacity(1024 * 1024),
            frame_count: 0,
            needs_input: false,
        })
    }

    unsafe fn init_mft(config: &VideoConfig) -> Result<IMFTransform, NvencError> {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET)?;

        // 1. Buscar Hardware NVENC
        let mut ppp_mft_activate: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut mft_count: u32 = 0;

        let out_info = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_H264,
        };

        let hr = MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG(MFT_ENUM_FLAG_HARDWARE.0 | MFT_ENUM_FLAG_SORTANDFILTER.0),
            None,
            Some(&out_info as *const _),
            &mut ppp_mft_activate,
            &mut mft_count,
        );

        if hr.is_err() || mft_count == 0 || ppp_mft_activate.is_null() {
            if !ppp_mft_activate.is_null() {
                CoTaskMemFree(Some(ppp_mft_activate as *const _));
            }
            return Err(NvencError::InitFailed("No se encontró un codificador H.264 por hardware (NVENC)".into()));
        }

        let activates = std::slice::from_raw_parts(ppp_mft_activate, mft_count as usize);
        let first_activate = activates[0].as_ref().ok_or_else(|| {
            CoTaskMemFree(Some(ppp_mft_activate as *const _));
            NvencError::InitFailed("IMFActivate devuelto estaba vacío".into())
        })?;

        let encoder_result: Result<IMFTransform, windows::core::Error> = first_activate.ActivateObject();
        CoTaskMemFree(Some(ppp_mft_activate as *const _));
        let encoder = encoder_result?;

        // ==========================================
        // DESBLOQUEAR MODO ASÍNCRONO
        // ==========================================
        if let Ok(attributes) = encoder.GetAttributes() {
            let _ = attributes.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1);
        }

        let frame_size = ((config.width as u64) << 32) | (config.height as u64);
        let frame_rate = ((config.fps as u64) << 32) | 1;

        let out_type: IMFMediaType = MFCreateMediaType()?;
        out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        out_type.SetUINT32(&MF_MT_AVG_BITRATE, config.bitrate_kbps * 1000)?;
        out_type.SetUINT64(&MF_MT_FRAME_SIZE, frame_size)?;
        out_type.SetUINT64(&MF_MT_FRAME_RATE, frame_rate)?;
        out_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        encoder.SetOutputType(0, &out_type, 0)?;

        let in_type: IMFMediaType = MFCreateMediaType()?;
        in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        in_type.SetUINT64(&MF_MT_FRAME_SIZE, frame_size)?;
        in_type.SetUINT64(&MF_MT_FRAME_RATE, frame_rate)?;
        in_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        encoder.SetInputType(0, &in_type, 0)?;

        if let Ok(codec_api) = encoder.cast::<ICodecAPI>() {
            let low_latency = VARIANT::from(true);
            let _ = codec_api.SetValue(&CODECAPI_AVEncCommonLowLatency, &low_latency);
        }

        let _ = encoder.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
        let _ = encoder.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0);
        let _ = encoder.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0);

        Ok(encoder)
    }

    /// Conversión hiper optimizada de RGBA a NV12 utilizando iteradores en bloque
    fn rgba_to_nv12(&mut self, rgba: &[u8], width: u32, height: u32) {
        let w = width as usize;
        let h = height as usize;
        let y_plane_size = w * h;

        // Dividimos el buffer en los planos Y y UV para acceso directo
        let (y_plane, uv_plane) = self.nv12_buffer.split_at_mut(y_plane_size);

        let mut y_idx = 0;
        let mut uv_idx = 0;

        for row in 0..h {
            let row_start = row * w;
            // Extraemos solo la fila actual del buffer RGBA
            let row_rgba = &rgba[(row_start * 4)..((row_start + w) * 4)];

            // chunks_exact procesa de 4 en 4 bytes y evita picos de CPU
            for (col, pixel) in row_rgba.chunks_exact(4).enumerate() {
                let r = pixel[0] as i32;
                let g = pixel[1] as i32;
                let b = pixel[2] as i32;

                // Luma (Y)
                y_plane[y_idx] = (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16) as u8;
                y_idx += 1;

                // Croma (UV) - Submuestreo 4:2:0
                if row % 2 == 0 && col % 2 == 0 {
                    uv_plane[uv_idx] = (((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128) as u8;
                    uv_plane[uv_idx + 1] = (((112 * r - 94 * g - 18 * b + 128) >> 8) + 128) as u8;
                    uv_idx += 2;
                }
            }
        }
    }

    pub fn encode_rgba_frame(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        pts_90khz: u64,
    ) -> Result<Option<MediaPacket>, NvencError> {
        self.frame_count += 1;
        let is_keyframe = (self.frame_count % self.config.gop_size as u64) == 1;

        self.rgba_to_nv12(rgba, width, height);

        let pts_100ns = (pts_90khz as u64 * 100_000) / 90_000;
        let duration_100ns = 10_000_000 / self.config.fps as u64;

        unsafe {
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

            let event_gen: IMFMediaEventGenerator = self.encoder.cast()?;
            let mut input_pushed = false;

            if self.needs_input {
                self.encoder.ProcessInput(0, &sample, 0)?;
                self.needs_input = false;
                input_pushed = true;
            }

            loop {
                let event = event_gen.GetEvent(MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS(0))?;
                let event_type = event.GetType()?;

                if event_type == METransformNeedInput.0 as u32 {
                    if !input_pushed {
                        self.encoder.ProcessInput(0, &sample, 0)?;
                        input_pushed = true;
                    } else {
                        self.needs_input = true;
                        return Ok(None);
                    }
                } else if event_type == METransformHaveOutput.0 as u32 {
                    self.output_bytes.clear();
                    let mut status = 0u32;
                    
                    let stream_info = self.encoder.GetOutputStreamInfo(0)?;
                    let out_sample = MFCreateSample()?;
                    let out_buffer = MFCreateMemoryBuffer(stream_info.cbSize)?;
                    out_sample.AddBuffer(&out_buffer)?;
                    
                    let mut output_data_arr = [MFT_OUTPUT_DATA_BUFFER {
                        dwStreamID: 0,
                        pSample: ManuallyDrop::new(Some(out_sample.clone())),
                        dwStatus: 0,
                        pEvents: ManuallyDrop::new(None),
                    }];

                    match self.encoder.ProcessOutput(0, &mut output_data_arr, &mut status) {
                        Ok(_) => {
                            if let Some(valid_sample) = &*output_data_arr[0].pSample {
                                let final_buffer = valid_sample.ConvertToContiguousBuffer()?;
                                let mut p_out_data = std::ptr::null_mut();
                                let mut out_len = 0;
                                final_buffer.Lock(&mut p_out_data, None, Some(&mut out_len))?;

                                let slice = std::slice::from_raw_parts(p_out_data as *const u8, out_len as usize);
                                self.output_bytes.extend_from_slice(slice);

                                final_buffer.Unlock()?;
                                
                                return Ok(Some(MediaPacket::new_video(
                                    pts_90khz,
                                    pts_90khz,
                                    self.output_bytes.clone(),
                                    is_keyframe,
                                )));
                            }
                        }
                        Err(e) => {
                            if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT {
                                continue;
                            }
                            return Err(NvencError::EncodeError(format!("ProcessOutput failed: {:?}", e)));
                        }
                    }
                }
            }
        }
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