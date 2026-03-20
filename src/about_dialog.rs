use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Show the About dialog.
pub fn show_about(parent: HWND) {
    unsafe {
        let text = format!(
            "Window Selector v{}\n\nKeyboard-driven window switching for Windows 11.\n\nPress Ctrl+Alt+Space to activate the overlay.\nPress a letter to select a window, then Enter to switch.",
            VERSION
        );
        let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let title_w: Vec<u16> = "About Window Selector\0".encode_utf16().collect();
        MessageBoxW(
            parent,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}
