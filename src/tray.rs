use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, SetForegroundWindow, TrackPopupMenu,
    MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    TPM_RETURNCMD,
};

/// Custom Windows message used for tray icon callbacks.
pub const WM_TRAY_CALLBACK: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 1;

/// Menu item command IDs.
pub const MENU_SETTINGS: u32 = 2001;
pub const MENU_ABOUT: u32 = 2002;
pub const MENU_EXIT: u32 = 2003;
pub const MENU_DIRECT_SWITCH: u32 = 2004;

/// Tray icon ID.
const TRAY_ICON_ID: u32 = 1;

/// Register the system tray icon.
pub fn add_tray_icon(hwnd: HWND) -> windows::core::Result<()> {
    unsafe {
        let icon = crate::icon::load_app_icon()?;

        let mut tooltip = [0u16; 128];
        let tip_str = "Window Selector";
        for (i, c) in tip_str.encode_utf16().enumerate() {
            if i >= tooltip.len() - 1 {
                break;
            }
            tooltip[i] = c;
        }

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY_CALLBACK,
            hIcon: icon,
            szTip: tooltip,
            ..Default::default()
        };

        Shell_NotifyIconW(NIM_ADD, &mut nid).ok()?;
        tracing::info!("Tray icon added");
        Ok(())
    }
}

/// Remove the system tray icon.
pub fn remove_tray_icon(hwnd: HWND) {
    unsafe {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
        tracing::info!("Tray icon removed");
    }
}

/// Show a balloon notification from the tray icon.
pub fn show_balloon(hwnd: HWND, title: &str, text: &str) {
    use windows::Win32::UI::Shell::{NIF_INFO, NIIF_WARNING};

    unsafe {
        let mut balloon_title = [0u16; 64];
        for (i, c) in title.encode_utf16().enumerate() {
            if i >= balloon_title.len() - 1 {
                break;
            }
            balloon_title[i] = c;
        }

        let mut balloon_text = [0u16; 256];
        for (i, c) in text.encode_utf16().enumerate() {
            if i >= balloon_text.len() - 1 {
                break;
            }
            balloon_text[i] = c;
        }

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_INFO,
            szInfoTitle: balloon_title,
            szInfo: balloon_text,
            dwInfoFlags: NIIF_WARNING,
            ..Default::default()
        };

        let _ = Shell_NotifyIconW(NIM_MODIFY, &mut nid);
        tracing::info!("Balloon notification shown: {}", title);
    }
}

/// Show the right-click context menu at the cursor position.
/// Returns the selected command ID, or 0 if none.
pub fn show_context_menu(hwnd: HWND, direct_switch: bool) -> u32 {
    unsafe {
        let menu = match CreatePopupMenu() {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("CreatePopupMenu failed: {:?}", e);
                return 0;
            }
        };

        let direct_switch_w: Vec<u16> = "Switch on key press\0".encode_utf16().collect();
        let check_flag = if direct_switch {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        let _ = AppendMenuW(
            menu,
            MF_STRING | check_flag,
            MENU_DIRECT_SWITCH as usize,
            PCWSTR(direct_switch_w.as_ptr()),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());

        let settings_w: Vec<u16> = "Settings...\0".encode_utf16().collect();
        let about_w: Vec<u16> = "About\0".encode_utf16().collect();
        let exit_w: Vec<u16> = "Exit\0".encode_utf16().collect();

        let _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_SETTINGS as usize,
            PCWSTR(settings_w.as_ptr()),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_ABOUT as usize,
            PCWSTR(about_w.as_ptr()),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT as usize, PCWSTR(exit_w.as_ptr()));

        // Required before TrackPopupMenu: bring our window to foreground
        // so the menu dismisses when the user clicks elsewhere.
        let _ = SetForegroundWindow(hwnd);

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        );

        let _ = DestroyMenu(menu);

        cmd.0 as u32
    }
}
