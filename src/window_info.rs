use windows::Win32::Foundation::HWND;

/// Snapshot of a single window at overlay activation time.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
    pub is_minimized: bool,
    pub monitor_index: usize,
    pub letter: Option<char>,
    pub number_tag: Option<u8>,
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
        }
    }
}