# Product

<!-- impeccable:product-schema 1 -->

## Platform

desktop (Windows)

## Users
Streamers and gamers who play on a primary Gaming PC and broadcast from a secondary PC or Laptop over a local network (LAN) using platforms like OBS Studio or TikTok Live Studio.

## Product Purpose
CastScreen is a dual-PC streaming solution designed to provide absolute A/V synchronization and high-quality sound over Wi-Fi/LAN, with zero performance impact on the gaming PC. 

## Positioning
Unlike OBS Teleport, NDI, or VoiceMeeter, CastScreen prioritizes absolute mathematical A/V sync and zero frame drops on the gaming PC by directly capturing the VRAM (DXGI Desktop Duplication) and using hardware encoding (NVIDIA NVENC H.264), transmitting over the SRT protocol with a robust buffer.

## Operating Context
- Running in the background on a Windows Gaming PC during intense gameplay.
- Connected via LAN/Wi-Fi to a receiver Laptop/PC running OBS or similar.
- Requires quick toggling of audio sources (per-application mixer).

## Capabilities and Constraints
- Direct DirectX 11 VRAM capture and NVENC encoding.
- App-level audio mixing via Windows WASAPI and `IAudioSessionManager2`.
- UI built with `egui` in Rust.
- Lock-free multithreading for zero-allocation main loops.
- Receiver application with 60 FPS live preview and VU meters.

## Brand Commitments
- High performance, zero latency feel, professional tool.
- "Obsidian" palette and modern dark theme for the UI.

## Evidence on Hand
- Fully functional `castscreen-sender` and `castscreen-receiver` crates.
- `egui` based GUI implemented in `castscreen-sender/src/gui.rs` and `castscreen-launcher/src/main.rs`.

## Product Principles
1. **Zero Impact:** The gaming PC must not suffer any FPS drops; the tool should be invisible to the game.
2. **Absolute Sync:** Audio is the master clock; video frames sync to it, never the reverse.
3. **Streamer Control:** Fast, reliable granular control over what audio and video is broadcasted.
4. **Professional Polish:** The interface must feel like a native, reliable production tool (Apple HIG principles, clear hierarchy, accessible).
