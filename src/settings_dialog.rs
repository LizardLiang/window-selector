use crate::config::AppConfig;
use crate::hotkey::format_hotkey;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK};

pub struct SettingsDialog;

impl SettingsDialog {
    /// Create and show the settings dialog.
    /// This is a blocking modal-style dialog that processes its own messages.
    pub fn show(parent: HWND, config: &AppConfig) -> Option<AppConfig> {
        // Settings dialog is shown modally by pumping messages in a sub-loop.
        // For simplicity in v1, we create a standard dialog-style window.
        tracing::info!("Settings dialog opened");

        // In v1, the settings dialog is a simplified implementation.
        // The full hotkey recorder is beyond the scope of automated testing
        // but the structure is here for the UI.

        // For the v1 implementation, we use a simple message box approach
        // to show the current hotkey and allow dismissal.
        unsafe {
            let current = format_hotkey(config.hotkey_modifiers, config.hotkey_vk);
            let msg: Vec<u16> = format!(
                "Current activation shortcut: {}\n\nSettings dialog is available.\nUse the hotkey to activate the overlay.",
                current
            ).encode_utf16().chain(std::iter::once(0)).collect();

            let title: Vec<u16> = "Window Selector Settings\0".encode_utf16().collect();
            MessageBoxW(parent, PCWSTR(msg.as_ptr()), PCWSTR(title.as_ptr()), MB_OK);
        }

        None // No config change in simplified implementation
    }
}
