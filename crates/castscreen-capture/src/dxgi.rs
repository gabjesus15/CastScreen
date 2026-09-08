//! DirectX 11 / DXGI Desktop Duplication API (DDA) Screen Capture.
//!
//! Captures display frames directly within GPU video memory (VRAM)
//! via `IDXGIOutputDuplication::AcquireNextFrame`, ensuring zero CPU impact on running games.

use thiserror::Error;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
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
                    let mut desc = DXGI_OUTPUT_DESC::default();
                    if output.GetDesc(&mut desc).is_ok() {
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

            let mut desc = DXGI_OUTPUT_DESC::default();
            output.GetDesc(&mut desc)?;

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

    /// Releases the current frame so the desktop compositor can continue.
    pub fn release_frame(&mut self) -> Result<(), DxgiCaptureError> {
        unsafe {
            self.duplication.ReleaseFrame()?;
            Ok(())
        }
    }
}
