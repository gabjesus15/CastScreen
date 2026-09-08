//! Windows Core Audio Session Manager (IAudioSessionManager2).
//!
//! Enumerates all running processes producing sound (e.g. games, Discord, Spotify, browsers)
//! and provides real-time per-application volume controls, mute toggles, and audio meters.

use thiserror::Error;
use windows::core::{Interface, GUID};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

#[derive(Error, Debug)]
pub enum AudioSessionError {
    #[error("COM initialization failed: {0}")]
    ComError(#[from] windows::core::Error),
    #[error("Default audio playback endpoint not found")]
    NoDefaultDevice,
    #[error("Failed to query IAudioSessionManager2")]
    SessionManagerQueryFailed,
}

/// Represents an active sound-producing application on the system.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioAppSession {
    pub process_id: u32,
    pub process_name: String,
    pub display_name: String,
    pub volume: f32,
    pub is_muted: bool,
    pub peak_meter: f32,
}

/// Controller for discovering and manipulating per-process Windows audio streams.
pub struct AudioSessionController;

impl AudioSessionController {
    /// Enumerates all currently active audio sessions on the default playback device.
    pub fn enumerate_active_sessions() -> Result<Vec<AudioAppSession>, AudioSessionError> {
        unsafe {
            // Ensure COM is initialized for this calling thread
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            // 1. Obtain MMDeviceEnumerator
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            // 2. Get default multimedia render endpoint
            let device: IMMDevice =
                enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;

            // 3. Activate IAudioSessionManager2
            let session_manager: IAudioSessionManager2 =
                device.Activate(CLSCTX_ALL, None)?;

            // 4. Get the session enumerator
            let session_enum: IAudioSessionEnumerator =
                session_manager.GetSessionEnumerator()?;

            let count = session_enum.GetCount()?;
            let mut sessions = Vec::with_capacity(count as usize);

            for i in 0..count {
                if let Ok(control) = session_enum.GetSession(i) {
                    if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
                        // Filter out inactive/expired sessions
                        if let Ok(state) = control2.GetState() {
                            if state != AudioSessionStateActive && state != AudioSessionStateInactive {
                                continue;
                            }
                        }

                        let pid = control2.GetProcessId().unwrap_or(0);
                        if pid == 0 {
                            continue; // Skip system sounds / system idle sessions
                        }

                        let process_name = Self::get_process_name(pid)
                            .unwrap_or_else(|| format!("PID: {}", pid));

                        // Read current volume and mute state
                        let mut volume = 1.0f32;
                        let mut is_muted = false;
                        if let Ok(simple_vol) = control.cast::<ISimpleAudioVolume>() {
                            let _ = simple_vol.GetMasterVolume(&mut volume);
                            let mut muted_bool = windows::Win32::Foundation::BOOL(0);
                            let _ = simple_vol.GetMute(&mut muted_bool);
                            is_muted = muted_bool.as_bool();
                        }

                        // Read peak audio meter
                        let mut peak = 0.0f32;
                        if let Ok(meter) = control.cast::<IAudioMeterInformation>() {
                            let _ = meter.GetPeakValue(&mut peak);
                        }

                        sessions.push(AudioAppSession {
                            process_id: pid,
                            process_name: process_name.clone(),
                            display_name: process_name,
                            volume,
                            is_muted,
                            peak_meter: peak,
                        });
                    }
                }
            }

            Ok(sessions)
        }
    }

    /// Sets mute state for a specific process ID.
    pub fn set_process_mute(target_pid: u32, mute: bool) -> Result<(), AudioSessionError> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device: IMMDevice =
                enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
            let session_manager: IAudioSessionManager2 =
                device.Activate(CLSCTX_ALL, None)?;
            let session_enum: IAudioSessionEnumerator =
                session_manager.GetSessionEnumerator()?;

            let count = session_enum.GetCount()?;
            for i in 0..count {
                if let Ok(control) = session_enum.GetSession(i) {
                    if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
                        if let Ok(pid) = control2.GetProcessId() {
                            if pid == target_pid {
                                if let Ok(simple_vol) = control.cast::<ISimpleAudioVolume>() {
                                    let _ = simple_vol.SetMute(
                                        windows::Win32::Foundation::BOOL(if mute { 1 } else { 0 }),
                                        std::ptr::null(),
                                    );
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Sets master volume [0.0, 1.0] for a specific process ID.
    pub fn set_process_volume(target_pid: u32, volume: f32) -> Result<(), AudioSessionError> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device: IMMDevice =
                enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
            let session_manager: IAudioSessionManager2 =
                device.Activate(CLSCTX_ALL, None)?;
            let session_enum: IAudioSessionEnumerator =
                session_manager.GetSessionEnumerator()?;

            let count = session_enum.GetCount()?;
            for i in 0..count {
                if let Ok(control) = session_enum.GetSession(i) {
                    if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
                        if let Ok(pid) = control2.GetProcessId() {
                            if pid == target_pid {
                                if let Ok(simple_vol) = control.cast::<ISimpleAudioVolume>() {
                                    let _ = simple_vol.SetMasterVolume(
                                        volume.clamp(0.0, 1.0),
                                        std::ptr::null(),
                                    );
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Resolves Process ID to human-readable executable name using Toolhelp32.
    fn get_process_name(pid: u32) -> Option<String> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32ProcessID == pid {
                        let _ = CloseHandle(snapshot);
                        let name_len = entry
                            .szExeFile
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry.szExeFile.len());
                        return Some(String::from_utf16_lossy(&entry.szExeFile[..name_len]));
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
            None
        }
    }
}
