use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::HICON;

/// Snapshot of a single window at overlay activation time.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
    pub is_minimized: bool,
    pub monitor_index: usize,
    pub letter: Option<char>,
    pub number_tag: Option<u8>,
    /// Cached icon handle fetched once at snapshot time.
    /// Avoids sending WM_GETICON on every WM_PAINT repaint.
    /// Do NOT call DestroyIcon on this — the handle is owned by the target process.
    pub icon: Option<HICON>,
}

impl WindowInfo {
    pub fn new(hwnd: HWND, title: String, is_minimized: bool, monitor_index: usize) -> Self {
        Self {
            hwnd,
            title,
            is_minimized,
            monitor_index,
            letter: None,
            number_tag: None,
            icon: None,
        }
    }
}
