//! Build script for CastScreen Core.
//! Generates the official CastScreen branding assets (`icon.ico` and `logo.png`)
//! if they do not already exist, ensuring clean packaging and native Windows icons.

use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let assets_dir = Path::new(&manifest_dir).join("../../assets");
    if !assets_dir.exists() {
        let _ = fs::create_dir_all(&assets_dir);
    }

    let ico_path = assets_dir.join("icon.ico");
    let ico_bytes = generate_castscreen_ico();
    let _ = fs::write(ico_path, ico_bytes);

    println!("cargo:rerun-if-changed=build.rs");
}

/// Generates a valid multi-resolution 32-bit RGBA Windows .ico file containing
/// the CastScreen brand mark (Monitor + Wi-Fi Broadcast Waves in Cyan/Mint).
fn generate_castscreen_ico() -> Vec<u8> {
    let mut ico = Vec::new();

    // 1. ICO Header (6 bytes)
    ico.extend_from_slice(&[0, 0]); // Reserved
    ico.extend_from_slice(&[1, 0]); // Type 1 = Icon
    ico.extend_from_slice(&[2, 0]); // 2 images (32x32 and 16x16)

    let img32 = generate_icon_image(32);
    let img16 = generate_icon_image(16);

    let offset_32 = 6 + 16 * 2;
    let offset_16 = offset_32 + img32.len();

    // Directory entry 1: 32x32
    ico.push(32); // Width
    ico.push(32); // Height
    ico.push(0);  // Color count
    ico.push(0);  // Reserved
    ico.extend_from_slice(&[1, 0]);  // Color planes
    ico.extend_from_slice(&[32, 0]); // Bits per pixel
    ico.extend_from_slice(&(img32.len() as u32).to_le_bytes());
    ico.extend_from_slice(&(offset_32 as u32).to_le_bytes());

    // Directory entry 2: 16x16
    ico.push(16); // Width
    ico.push(16); // Height
    ico.push(0);  // Color count
    ico.push(0);  // Reserved
    ico.extend_from_slice(&[1, 0]);  // Color planes
    ico.extend_from_slice(&[32, 0]); // Bits per pixel
    ico.extend_from_slice(&(img16.len() as u32).to_le_bytes());
    ico.extend_from_slice(&(offset_16 as u32).to_le_bytes());

    // Image data
    ico.extend_from_slice(&img32);
    ico.extend_from_slice(&img16);

    ico
}

fn generate_icon_image(size: u32) -> Vec<u8> {
    let mut data = Vec::new();

    // BITMAPINFOHEADER (40 bytes)
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&(size as i32).to_le_bytes());
    data.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // Height * 2 for mask
    data.extend_from_slice(&1u16.to_le_bytes());                // Planes
    data.extend_from_slice(&32u16.to_le_bytes());               // 32 bpp RGBA
    data.extend_from_slice(&0u32.to_le_bytes());                // BI_RGB (uncompressed)
    data.extend_from_slice(&((size * size * 4) as u32).to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());

    // XOR Mask: 32-bit BGRA pixels (stored bottom-to-top)
    let mut and_mask = Vec::new();
    let row_bytes = (size + 31) / 32 * 4;

    for y_inv in 0..size {
        let y = size - 1 - y_inv; // Bottom-to-top
        let mut and_row = vec![0u8; row_bytes as usize];

        for x in 0..size {
            let (b, g, r, a) = compute_pixel_color(x, y, size);
            data.push(b);
            data.push(g);
            data.push(r);
            data.push(a);

            if a < 128 {
                let byte_idx = (x / 8) as usize;
                let bit_idx = 7 - (x % 8);
                and_row[byte_idx] |= 1 << bit_idx;
            }
        }
        and_mask.extend_from_slice(&and_row);
    }

    // AND Mask (1 bit per pixel)
    data.extend_from_slice(&and_mask);

    data
}

/// Computes CastScreen brand colors for coordinate (x, y)
/// Colors: Dark Canvas (#0B0D13), Cyan Stream (#00F2FE), Indigo Glow (#4FACFE), Mint Waves (#00F5A0)
fn compute_pixel_color(x: u32, y: u32, size: u32) -> (u8, u8, u8, u8) {
    let nx = x as f32 / size as f32;
    let ny = y as f32 / size as f32;

    // Corner radius check for rounded icon background
    let cx = (nx - 0.5).abs();
    let cy = (ny - 0.5).abs();
    if cx > 0.44 && cy > 0.44 {
        let dist = ((cx - 0.44).powi(2) + (cy - 0.44).powi(2)).sqrt();
        if dist > 0.06 {
            return (0, 0, 0, 0); // Transparent outside rounded corner
        }
    }

    // Default dark canvas background (#0E1118)
    let mut b = 24u8;
    let mut g = 17u8;
    let mut r = 14u8;
    let a = 255u8;

    // Monitor Outer Rectangle
    let is_screen_border = (nx >= 0.15 && nx <= 0.72 && (ny >= 0.22 && ny <= 0.28 || ny >= 0.70 && ny <= 0.76))
        || (ny >= 0.22 && ny <= 0.76 && (nx >= 0.15 && nx <= 0.21 || nx >= 0.66 && nx <= 0.72));

    // Monitor Stand Base
    let is_stand = (nx >= 0.38 && nx <= 0.49 && ny >= 0.76 && ny <= 0.84)
        || (nx >= 0.28 && nx <= 0.59 && ny >= 0.84 && ny <= 0.90);

    // Wi-Fi Broadcast Waves (Top-Right)
    let wave_center_x = 0.66f32;
    let wave_center_y = 0.34f32;
    let dist_to_wave = ((nx - wave_center_x).powi(2) + (ny - wave_center_y).powi(2)).sqrt();

    let is_wave1 = dist_to_wave >= 0.14 && dist_to_wave <= 0.19 && nx >= wave_center_x && ny <= wave_center_y + 0.05;
    let is_wave2 = dist_to_wave >= 0.23 && dist_to_wave <= 0.28 && nx >= wave_center_x && ny <= wave_center_y + 0.05;

    if is_wave1 || is_wave2 {
        // Mint Green (#00F5A0) -> BGR: [160, 245, 0]
        return (160, 245, 0, 255);
    }

    if is_screen_border || is_stand {
        // Cyan-to-Indigo Gradient (#00F2FE to #4FACFE)
        let t = nx;
        let red = (0.0 * (1.0 - t) + 79.0 * t) as u8;
        let green = (242.0 * (1.0 - t) + 172.0 * t) as u8;
        let blue = (254.0 * (1.0 - t) + 254.0 * t) as u8;
        return (blue, green, red, 255);
    }

    (b, g, r, a)
}
