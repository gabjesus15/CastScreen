//! Single Instance Guard and Duplicate Process Prevention.
//!
//! Ensures only one instance of CastScreen (Sender or Receiver) runs at a time.
//! If a second instance is launched, it brings the existing window to the foreground,
//! notifies the user, and terminates cleanly to prevent memory bloat and port collisions.

use windows::core::HSTRING;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, MessageBoxW, SetForegroundWindow, ShowWindow, MB_ICONINFORMATION, MB_OK,
    SHOW_WINDOW_CMD,
};

pub struct SingleInstanceGuard {
    handle: Option<HANDLE>,
    is_primary: bool,
}

impl SingleInstanceGuard {
    /// Attempts to acquire the single-instance named mutex for the specified application name.
    ///
    /// If an instance is already running:
    /// - Attempts to find and focus the existing window.
    /// - Shows a native Windows message dialog if `show_alert` is true.
    /// - Returns a guard with `is_primary() == false`.
    pub fn new(app_identifier: &str, window_title: &str, show_alert: bool) -> Self {
        unsafe {
            let mutex_name = HSTRING::from(format!("Global\\{}", app_identifier));
            let handle = CreateMutexW(None, true, &mutex_name);

            match handle {
                Ok(h) => {
                    if GetLastError() == ERROR_ALREADY_EXISTS {
                        // Another instance is already holding this mutex
                        let _ = CloseHandle(h);

                        // Try to bring the existing window to the front
                        let title_hstring = HSTRING::from(window_title);
                        if let Ok(existing_hwnd) = FindWindowW(None, &title_hstring) {
                            if !existing_hwnd.is_invalid() {
                                let _ = ShowWindow(existing_hwnd, SHOW_WINDOW_CMD(9));
                                let _ = SetForegroundWindow(existing_hwnd);
                            }
                        }

                        if show_alert {
                            let msg = HSTRING::from(format!(
                                "{} ya se está ejecutando en esta computadora.\n\nSe ha traído la ventana activa al primer plano.",
                                window_title
                            ));
                            let caption = HSTRING::from(window_title);
                            let _ = MessageBoxW(
                                None,
                                &msg,
                                &caption,
                                MB_OK | MB_ICONINFORMATION,
                            );
                        }

                        Self {
                            handle: None,
                            is_primary: false,
                        }
                    } else {
                        // We are the primary, exclusive instance
                        Self {
                            handle: Some(h),
                            is_primary: true,
                        }
                    }
                }
                Err(_) => {
                    // Could not create mutex (fallback to primary)
                    Self {
                        handle: None,
                        is_primary: true,
                    }
                }
            }
        }
    }

    /// Returns true if this process is the first and only running instance.
    pub fn is_primary(&self) -> bool {
        self.is_primary
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            unsafe {
                let _ = CloseHandle(h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_instance_guard_creation() {
        let guard = SingleInstanceGuard::new(
            "CastScreen_Unit_Test_Mutex_Unique_123",
            "CastScreen Test Window",
            false,
        );
        assert!(guard.is_primary());
    }
}
