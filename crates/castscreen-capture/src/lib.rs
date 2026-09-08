//! Hardware-accelerated screen capture and Windows Core Audio session capture.

pub mod dxgi;
pub mod sessions;
pub mod wasapi;

pub use dxgi::{DxgiCaptureError, DxgiScreenCapture, MonitorInfo};
pub use sessions::{AudioAppSession, AudioSessionController, AudioSessionError};
pub use wasapi::{WasapiCaptureError, WasapiLoopbackCapture};
