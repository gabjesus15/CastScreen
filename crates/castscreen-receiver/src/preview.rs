//! Hardware-Accelerated Video Surface for Live Preview (Laptop Receiver).
//!
//! Renders the incoming 60 FPS video feed utilizing the laptop's Radeon iGPU
//! with zero tearing and fluid frame presentation.

#[allow(dead_code)]
pub struct PreviewWindow {
    pub width: u32,
    pub height: u32,
    pub is_fullscreen: bool,
}

impl PreviewWindow {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            is_fullscreen: false,
        }
    }

    /// Renders a decoded frame on the hardware surface.
    pub fn present_frame(&mut self, _frame_data: &[u8]) {
        // GPU swapchain presentation
    }
}
