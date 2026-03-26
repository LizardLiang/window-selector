/// Window icon fetching utilities.
///
/// Provides a single public function `get_window_icon` that returns the best
/// available HICON for a given HWND.  The caller is responsible for NOT
/// destroying the returned icon — all icons returned by this module are either
/// shared system icons or owned by the target process, so they must not be
/// passed to `DestroyIcon`.
///
/// # Icon fetch priority (small icon preferred)
///
/// 1. `WM_GETICON(ICON_SMALL2)` — small icon at system small-icon DPI size
/// 2. `WM_GETICON(ICON_SMALL)`  — 16-px small icon set by the app
/// 3. `WM_GETICON(ICON_BIG)`    — 32-px large icon (GDI scales it down)
/// 4. `GetClassLongPtrW(GCLP_HICONSM)` — class-level small icon
/// 5. `GetClassLongPtrW(GCLP_HICON)`   — class-level large icon
///
/// Returns `None` if no icon is available (caller should skip drawing).
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassLongPtrW, SendMessageTimeoutW, GCLP_HICON, GCLP_HICONSM, HICON,
    SMTO_ABORTIFHUNG, SMTO_BLOCK, WM_GETICON,
};

// WM_GETICON wParam constants from WinUser.h.
// The `windows` crate exposes these as ICON_TYPE, but using plain integer
// literals via WPARAM is the simplest cross-version approach.
const ICON_SMALL_WPARAM: usize = 0; // ICON_SMALL
const ICON_BIG_WPARAM: usize = 1;   // ICON_BIG
const ICON_SMALL2_WPARAM: usize = 2; // ICON_SMALL2 — DPI-aware small icon

/// Attempt to retrieve a suitable HICON for the given window.
///
/// Returns `None` if every fallback fails (e.g., the window has no icon at
/// all and its window class does not register one).  The returned handle
/// must NOT be destroyed by the caller.
pub fn get_window_icon(hwnd: HWND) -> Option<HICON> {
    unsafe {
        // 1. DPI-aware small icon (Windows Vista+).
        let icon = send_get_icon(hwnd, ICON_SMALL2_WPARAM);
        if icon.is_some() {
            return icon;
        }

        // 2. 16-px small icon set by the application via WM_SETICON.
        let icon = send_get_icon(hwnd, ICON_SMALL_WPARAM);
        if icon.is_some() {
            return icon;
        }

        // 3. 32-px large icon — DrawIconEx will scale it to the requested size.
        let icon = send_get_icon(hwnd, ICON_BIG_WPARAM);
        if icon.is_some() {
            return icon;
        }

        // 4. Class-level small icon (registered with WNDCLASSEX.hIconSm).
        let raw = GetClassLongPtrW(hwnd, GCLP_HICONSM);
        if raw != 0 {
            // GetClassLongPtrW returns usize; cast to *mut c_void for HICON.
            return Some(HICON(raw as *mut core::ffi::c_void));
        }

        // 5. Class-level large icon.
        let raw = GetClassLongPtrW(hwnd, GCLP_HICON);
        if raw != 0 {
            return Some(HICON(raw as *mut core::ffi::c_void));
        }

        None
    }
}

/// Send WM_GETICON to the window and return the icon handle if non-null.
///
/// Uses `SendMessageTimeoutW` with `SMTO_ABORTIFHUNG` to avoid blocking
/// indefinitely if the target window is not responding.  The 50 ms timeout
/// is generous for WM_GETICON (a trivial message) but short enough to keep
/// the overlay responsive even when a target window is hung.
unsafe fn send_get_icon(hwnd: HWND, icon_type: usize) -> Option<HICON> {
    let mut result: usize = 0;
    // SendMessageTimeoutW returns 0 on timeout / failure.
    let ok = SendMessageTimeoutW(
        hwnd,
        WM_GETICON,
        WPARAM(icon_type),
        LPARAM(0),
        SMTO_ABORTIFHUNG | SMTO_BLOCK,
        50, // milliseconds
        Some(&mut result),
    );
    if ok.0 != 0 && result != 0 {
        Some(HICON(result as *mut core::ffi::c_void))
    } else {
        None
    }
}