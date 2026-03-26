use crate::mru_tracker::MruTracker;
use crate::window_info::WindowInfo;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED,
};
use windows::Win32::Graphics::Gdi::{
    MonitorFromWindow, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetShellWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
    IsIconic, IsWindowVisible, GWL_EXSTYLE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

/// Set of overlay HWNDs to exclude from the window snapshot.
static mut OVERLAY_HWNDS: Vec<HWND> = Vec::new();

/// Register overlay HWNDs to be excluded from enumeration.
pub fn register_overlay_hwnds(hwnds: Vec<HWND>) {
    unsafe {
        OVERLAY_HWNDS = hwnds;
    }
}

struct EnumContext {
    windows: Vec<WindowInfo>,
    monitors: Vec<crate::monitor::MonitorInfo>,
}

/// Enumerate all Alt+Tab-visible windows and return a snapshot.
/// Applies the heuristic filter matching Windows Alt+Tab behavior.
// SAFETY: OVERLAY_HWNDS is only written once at startup (register_overlay_hwnds), and only
// read afterwards. This is a single-threaded Win32 app — no concurrent access is possible.
#[allow(static_mut_refs)]
pub fn enumerate_windows(
    own_hwnds: &[HWND],
    monitors: &[crate::monitor::MonitorInfo],
) -> Vec<WindowInfo> {
    let mut ctx = EnumContext {
        windows: Vec::new(),
        monitors: monitors.to_vec(),
    };

    let ctx_ptr = &mut ctx as *mut EnumContext;

    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_callback),
            LPARAM(ctx_ptr as isize),
        );
    }

    // Filter out our own overlay HWNDs
    ctx.windows.retain(|w| {
        !own_hwnds.contains(&w.hwnd) && {
            unsafe {
                // Also exclude our globally registered overlays
                !OVERLAY_HWNDS.contains(&w.hwnd)
            }
        }
    });

    ctx.windows
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut EnumContext);

    // Skip shell window
    if hwnd == GetShellWindow() {
        return BOOL(1);
    }

    // Skip invisible windows
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    // Skip windows with no title
    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 {
        return BOOL(1);
    }

    // Skip tool windows (unless they also have WS_EX_APPWINDOW)
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    let is_tool = (ex_style & WS_EX_TOOLWINDOW.0) != 0;
    let is_app = (ex_style & WS_EX_APPWINDOW.0) != 0;
    if is_tool && !is_app {
        return BOOL(1);
    }

    // Skip cloaked windows
    let mut cloaked: u32 = 0;
    let _ = DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        &mut cloaked as *mut u32 as *mut std::ffi::c_void,
        std::mem::size_of::<u32>() as u32,
    );
    if cloaked != 0 {
        return BOOL(1);
    }

    // Get window title
    let mut buf = vec![0u16; (title_len as usize) + 1];
    GetWindowTextW(hwnd, &mut buf);
    let title = String::from_utf16_lossy(&buf[..title_len as usize]);

    let is_minimized = IsIconic(hwnd).as_bool();

    // Determine monitor index
    let monitor_handle = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let monitor_index = ctx
        .monitors
        .iter()
        .position(|m| m.handle == monitor_handle)
        .unwrap_or(0);

    ctx.windows.push(WindowInfo::new(hwnd, title, is_minimized, monitor_index));

    BOOL(1) // Continue enumeration
}

/// Produce a filtered, MRU-ordered, letter-assigned snapshot of all visible windows.
pub fn snapshot_windows(
    own_hwnds: &[HWND],
    monitors: &[crate::monitor::MonitorInfo],
    mru_tracker: &MruTracker,
    session_tags: &crate::state::SessionTags,
) -> Vec<WindowInfo> {
    let mut windows = enumerate_windows(own_hwnds, monitors);

    // Sort by MRU order
    mru_tracker.sort_by_mru(&mut windows);

    // Assign letters
    crate::letter_assignment::assign_letters(&mut windows);

    // Re-apply session tags and fetch each window's icon once.
    // Caching here avoids sending WM_GETICON on every WM_PAINT repaint.
    for window in &mut windows {
        window.number_tag = session_tags.get_tag_for_hwnd(window.hwnd);
        window.icon = crate::window_icon::get_window_icon(window.hwnd);
    }

    tracing::debug!(
        "Window snapshot: {} windows",
        windows.len()
    );
    for w in &windows {
        tracing::debug!(
            "  {:?} letter={:?} tag={:?} minimized={} title={:?}",
            w.hwnd,
            w.letter,
            w.number_tag,
            w.is_minimized,
            w.title
        );
    }

    windows
}

/// Check whether the given window would pass the Alt+Tab filter.
/// Used for unit testing the filter logic with mock data.
#[allow(dead_code)]
pub fn passes_alt_tab_filter_mock(
    visible: bool,
    title_len: usize,
    ex_style: u32,
    cloaked: bool,
) -> bool {
    if !visible {
        return false;
    }
    if title_len == 0 {
        return false;
    }
    let is_tool = (ex_style & WS_EX_TOOLWINDOW.0) != 0;
    let is_app = (ex_style & WS_EX_APPWINDOW.0) != 0;
    if is_tool && !is_app {
        return false;
    }
    if cloaked {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invisible_window_excluded() {
        assert!(!passes_alt_tab_filter_mock(false, 10, 0, false));
    }

    #[test]
    fn test_empty_title_excluded() {
        assert!(!passes_alt_tab_filter_mock(true, 0, 0, false));
    }

    #[test]
    fn test_tool_window_without_appwindow_excluded() {
        assert!(!passes_alt_tab_filter_mock(
            true,
            10,
            WS_EX_TOOLWINDOW.0,
            false
        ));
    }

    #[test]
    fn test_tool_window_with_appwindow_included() {
        assert!(passes_alt_tab_filter_mock(
            true,
            10,
            WS_EX_TOOLWINDOW.0 | WS_EX_APPWINDOW.0,
            false
        ));
    }

    #[test]
    fn test_cloaked_window_excluded() {
        assert!(!passes_alt_tab_filter_mock(true, 10, 0, true));
    }

    #[test]
    fn test_normal_window_included() {
        assert!(passes_alt_tab_filter_mock(true, 10, 0, false));
    }
}
