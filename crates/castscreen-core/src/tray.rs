//! Windows Notification Area (System Tray) and Native Toast Notifications.
//!
//! Provides minimal, reliable system tray icon management and Windows balloon notifications
//! via `Shell_NotifyIconW` without external heavy dependencies.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NOTIFY_ICON_DATA_FLAGS, NOTIFY_ICON_INFOTIP_FLAGS,
    NOTIFY_ICON_MESSAGE,
};

const NIM_ADD: NOTIFY_ICON_MESSAGE = NOTIFY_ICON_MESSAGE(0);
const NIM_MODIFY: NOTIFY_ICON_MESSAGE = NOTIFY_ICON_MESSAGE(1);
const NIM_DELETE: NOTIFY_ICON_MESSAGE = NOTIFY_ICON_MESSAGE(2);

const NIF_TIP_AND_INFO: NOTIFY_ICON_DATA_FLAGS = NOTIFY_ICON_DATA_FLAGS(0x00000004 | 0x00000010);

const NIIF_INFO: NOTIFY_ICON_INFOTIP_FLAGS = NOTIFY_ICON_INFOTIP_FLAGS(1);
const NIIF_WARNING: NOTIFY_ICON_INFOTIP_FLAGS = NOTIFY_ICON_INFOTIP_FLAGS(2);

pub struct TrayNotifier;

impl TrayNotifier {
    /// Dispatches a native Windows notification balloon/toast to the taskbar tray.
    pub fn send_notification(title: &str, message: &str, is_warning: bool) {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: HWND::default(),
                uID: 1001,
                uFlags: NIF_TIP_AND_INFO,
                dwInfoFlags: if is_warning { NIIF_WARNING } else { NIIF_INFO },
                ..Default::default()
            };

            // Set tooltip text
            for (dest, src) in nid.szTip.iter_mut().zip("CastScreen Streaming".encode_utf16()) {
                *dest = src;
            }

            // Set notification title
            for (dest, src) in nid.szInfoTitle.iter_mut().zip(title.encode_utf16()) {
                *dest = src;
            }

            // Set notification message
            for (dest, src) in nid.szInfo.iter_mut().zip(message.encode_utf16()) {
                *dest = src;
            }

            // Add or modify the notification
            if !Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() {
                let _ = Shell_NotifyIconW(NIM_ADD, &nid);
            }
        }
    }

    /// Removes the notification icon from the system tray.
    pub fn remove() {
        unsafe {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: HWND::default(),
                uID: 1001,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }
}
