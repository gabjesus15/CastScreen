//! Windows Notification Area (System Tray) and Native Toast Notifications.
//!
//! Provides minimal, reliable system tray icon management and Windows balloon notifications
//! via `Shell_NotifyIconW` without external heavy dependencies.

use windows::core::HSTRING;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_INFO, NIF_TIP, NIIF_INFO, NIIF_WARNING, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NOTIFYICONDATAW,
};

pub struct TrayNotifier;

impl TrayNotifier {
    /// Dispatches a native Windows notification balloon/toast to the taskbar tray.
    pub fn send_notification(title: &str, message: &str, is_warning: bool) {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: HWND::default(),
                uID: 1001,
                uFlags: NIF_INFO | NIF_TIP,
                dwInfoFlags: if is_warning { NIIF_WARNING } else { NIIF_INFO },
                ..Default::default()
            };

            // Set tooltip text
            let tip = HSTRING::from("CastScreen Streaming");
            let tip_slice = tip.as_wide();
            let tip_len = tip_slice.len().min(nid.szTip.len() - 1);
            nid.szTip[..tip_len].copy_from_slice(&tip_slice[..tip_len]);

            // Set notification title
            let title_hstring = HSTRING::from(title);
            let title_slice = title_hstring.as_wide();
            let title_len = title_slice.len().min(nid.szInfoTitle.len() - 1);
            nid.szInfoTitle[..title_len].copy_from_slice(&title_slice[..title_len]);

            // Set notification message
            let msg_hstring = HSTRING::from(message);
            let msg_slice = msg_hstring.as_wide();
            let msg_len = msg_slice.len().min(nid.szInfo.len() - 1);
            nid.szInfo[..msg_len].copy_from_slice(&msg_slice[..msg_len]);

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
