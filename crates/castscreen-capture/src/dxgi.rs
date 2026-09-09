//! DirectX 11 / DXGI Desktop Duplication API (DDA) Screen Capture.
//!
//! Captures display frames directly within GPU video memory (VRAM)
//! via `IDXGIOutputDuplication::AcquireNextFrame`, ensuring zero CPU impact on running games.

use thiserror::Error;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

#[derive(Error, Debug)]
pub enum DxgiCaptureError {
    #[error("DirectX API error: {0}")]
    Dxgi(#[from] windows::core::Error),
    #[error("DXGI device creation failed")]
    DeviceCreationFailed,
    #[error("Display output index out of range")]
    OutputNotFound,
    #[error("Desktop duplication access was lost (display mode change / UAC prompt)")]
    AccessLost,
    #[error("AcquireNextFrame timed out")]
    Timeout,
}

/// Information about a detected physical monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: u32,
    pub device_name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate_hz: u32,
}

/// Zero-Copy DirectX 11 Screen Capture session.
pub struct DxgiScreenCapture {
    pub width: u32,
    pub height: u32,
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging_texture: Option<ID3D11Texture2D>,
}

impl DxgiScreenCapture {
    /// Enumerate all connected physical displays available for hardware capture.
    pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>, DxgiCaptureError> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
            let mut monitors = Vec::new();
            let mut adapter_idx = 0;

            while let Ok(adapter) = factory.EnumAdapters1(adapter_idx) {
                let mut output_idx = 0;
                while let Ok(output) = adapter.EnumOutputs(output_idx) {
                    if let Ok(desc) = output.GetDesc() {
                        let width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left).abs() as u32;
                        let height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top).abs() as u32;

                        let name_len = desc
                            .DeviceName
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(desc.DeviceName.len());
                        let name = String::from_utf16_lossy(&desc.DeviceName[..name_len]);

                        monitors.push(MonitorInfo {
                            index: monitors.len() as u32,
                            device_name: name,
                            width,
                            height,
                            refresh_rate_hz: 60, // Default baseline, updated upon duplication
                        });
                    }
                    output_idx += 1;
                }
                adapter_idx += 1;
            }

            Ok(monitors)
        }
    }

    /// Initializes DirectX 11 device and duplicates the chosen display output.
    pub fn new(display_index: u32) -> Result<Self, DxgiCaptureError> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
            let adapter = factory
                .EnumAdapters1(0)
                .map_err(|_| DxgiCaptureError::DeviceCreationFailed)?;

            let mut device = None;
            let mut context = None;
            let feature_levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];

            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                windows::Win32::Foundation::HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;

            let device = device.ok_or(DxgiCaptureError::DeviceCreationFailed)?;
            let context = context.ok_or(DxgiCaptureError::DeviceCreationFailed)?;

            let output = adapter
                .EnumOutputs(display_index)
                .map_err(|_| DxgiCaptureError::OutputNotFound)?;

            let output1: IDXGIOutput1 = output.cast()?;
            let duplication = output1.DuplicateOutput(&device)?;

            let desc = output.GetDesc()?;

            let width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left).abs() as u32;
            let height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top).abs() as u32;

            tracing::info!(
                "DirectX 11 Desktop Duplication initialized: {}x{} on Display {}",
                width,
                height,
                display_index
            );

            Ok(Self {
                width,
                height,
                device,
                context,
                duplication,
                staging_texture: None,
            })
        }
    }

    /// Acquires the next frame texture residing in GPU VRAM.
    ///
    /// Returns the `ID3D11Texture2D` handle directly in VRAM without host memory copies.
    pub fn acquire_frame_texture(
        &mut self,
        timeout_ms: u32,
    ) -> Result<ID3D11Texture2D, DxgiCaptureError> {
        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;

            let hr = self
                .duplication
                .AcquireNextFrame(timeout_ms, &mut frame_info, &mut resource);

            if let Err(e) = hr {
                if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                    return Err(DxgiCaptureError::Timeout);
                } else if e.code() == DXGI_ERROR_ACCESS_LOST {
                    return Err(DxgiCaptureError::AccessLost);
                } else {
                    return Err(DxgiCaptureError::Dxgi(e));
                }
            }

            let resource = resource.ok_or(DxgiCaptureError::DeviceCreationFailed)?;
            let texture: ID3D11Texture2D = resource.cast()?;

            Ok(texture)
        }
    }

    /// Acquires the next frame, copies pixels to CPU via a staging texture,
    /// and formats the output into RGBA format (ideal for compression).
    pub fn acquire_frame_rgba(
        &mut self,
        timeout_ms: u32,
        out_rgba: &mut Vec<u8>,
    ) -> Result<(u32, u32), DxgiCaptureError> {
        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;

            let hr = self
                .duplication
                .AcquireNextFrame(timeout_ms, &mut frame_info, &mut resource);

            if let Err(e) = hr {
                if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                    return Err(DxgiCaptureError::Timeout);
                } else if e.code() == DXGI_ERROR_ACCESS_LOST {
                    return Err(DxgiCaptureError::AccessLost);
                } else {
                    return Err(DxgiCaptureError::Dxgi(e));
                }
            }

            let resource = resource.ok_or(DxgiCaptureError::DeviceCreationFailed)?;
            let texture: ID3D11Texture2D = resource.cast()?;

            // Lazily create staging texture if not already allocated
            if self.staging_texture.is_none() {
                let staging_desc = D3D11_TEXTURE2D_DESC {
                    Width: self.width,
                    Height: self.height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_STAGING,
                    BindFlags: 0,
                    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                    MiscFlags: 0,
                };
                let mut staging = None;
                self.device
                    .CreateTexture2D(&staging_desc, None, Some(&mut staging))?;
                self.staging_texture = staging;
            }

            if let Some(staging) = &self.staging_texture {
                self.context.CopyResource(staging, &texture);

                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                self.context
                    .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;

                let row_pitch = mapped.RowPitch as usize;
                let row_bytes = (self.width * 4) as usize;
                let total_bytes = (self.width * self.height * 4) as usize;

                if out_rgba.len() != total_bytes {
                    out_rgba.resize(total_bytes, 0);
                }

                let src_ptr = mapped.pData as *const u8;
                for y in 0..self.height as usize {
                    let src_row = std::slice::from_raw_parts(src_ptr.add(y * row_pitch), row_bytes);
                    let dst_start = y * row_bytes;
                    out_rgba[dst_start..dst_start + row_bytes].copy_from_slice(src_row);
                }

                self.context.Unmap(staging, 0);

                // Convert BGRA to RGBA in-place: swap B and R, set A to 255
                for pixel in out_rgba.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                    pixel[3] = 255;
                }
            }

            self.duplication.ReleaseFrame()?;
            Ok((self.width, self.height))
        }
    }

    /// Releases the current frame so the desktop compositor can continue.
    pub fn release_frame(&mut self) -> Result<(), DxgiCaptureError> {
        unsafe {
            self.duplication.ReleaseFrame()?;
            Ok(())
        }
    }
}
