use crate::keycodes::{
    MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_A, VK_BACK, VK_ESCAPE, VK_F1, VK_F12, VK_RETURN,
    VK_SPACE, VK_TAB, VK_Z,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};

/// The hotkey ID used with RegisterHotKey for the main overlay.
pub const HOTKEY_ID: i32 = 1;

/// The hotkey ID used with RegisterHotKey for label mode.
pub const HOTKEY_ID_LABEL: i32 = 2;

/// Register the global activation hotkey.
/// Returns Ok(()) on success, Err on failure (conflict with another app).
pub fn register_hotkey(msg_hwnd: HWND, modifiers: u32, vk_code: u32) -> windows::core::Result<()> {
    unsafe {
        RegisterHotKey(msg_hwnd, HOTKEY_ID, HOT_KEY_MODIFIERS(modifiers), vk_code)?;
        tracing::info!(
            "Hotkey registered: modifiers=0x{:X} vk=0x{:X}",
            modifiers,
            vk_code
        );
        Ok(())
    }
}

/// Register the label mode hotkey.
pub fn register_label_hotkey(
    msg_hwnd: HWND,
    modifiers: u32,
    vk_code: u32,
) -> windows::core::Result<()> {
    unsafe {
        RegisterHotKey(
            msg_hwnd,
            HOTKEY_ID_LABEL,
            HOT_KEY_MODIFIERS(modifiers),
            vk_code,
        )?;
        tracing::info!(
            "Label hotkey registered: modifiers=0x{:X} vk=0x{:X}",
            modifiers,
            vk_code
        );
        Ok(())
    }
}

/// Unregister the global activation hotkey.
pub fn unregister_hotkey(msg_hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(msg_hwnd, HOTKEY_ID);
        tracing::info!("Hotkey unregistered");
    }
}

/// Unregister the label mode hotkey.
pub fn unregister_label_hotkey(msg_hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(msg_hwnd, HOTKEY_ID_LABEL);
        tracing::info!("Label hotkey unregistered");
    }
}

/// Format a hotkey combo as a human-readable string (e.g., "Ctrl+Alt+Space").
pub fn format_hotkey(modifiers: u32, vk_code: u32) -> String {
    let mut parts = Vec::new();

    if (modifiers & MOD_CONTROL) != 0 {
        parts.push("Ctrl");
    }
    if (modifiers & MOD_ALT) != 0 {
        parts.push("Alt");
    }
    if (modifiers & MOD_SHIFT) != 0 {
        parts.push("Shift");
    }
    if (modifiers & MOD_WIN) != 0 {
        parts.push("Win");
    }

    let key_name = vk_to_name(vk_code);
    parts.push(&key_name);
    parts.join("+")
}

/// Map common virtual key codes to display names.
fn vk_to_name(vk: u32) -> String {
    match vk {
        VK_SPACE => "Space".to_string(),
        VK_RETURN => "Enter".to_string(),
        VK_BACK => "Backspace".to_string(),
        VK_TAB => "Tab".to_string(),
        VK_ESCAPE => "Esc".to_string(),
        VK_F1..=VK_F12 => format!("F{}", vk - VK_F1 + 1),
        VK_A..=VK_Z => {
            let c = (b'A' + (vk - VK_A) as u8) as char;
            c.to_string()
        }
        _ if (0x30..=0x39).contains(&vk) => {
            let c = (b'0' + (vk - 0x30) as u8) as char;
            c.to_string()
        }
        _ => format!("0x{:02X}", vk),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hotkey_ctrl_alt_space() {
        let s = format_hotkey(0x0002 | 0x0001 | 0x4000, 0x20);
        assert!(s.contains("Ctrl"), "Should contain Ctrl: {}", s);
        assert!(s.contains("Alt"), "Should contain Alt: {}", s);
        assert!(s.contains("Space"), "Should contain Space: {}", s);
    }

    #[test]
    fn test_format_hotkey_shift_f1() {
        let s = format_hotkey(0x0004, 0x70);
        assert!(s.contains("Shift"), "Should contain Shift: {}", s);
        assert!(s.contains("F1"), "Should contain F1: {}", s);
    }

    #[test]
    fn test_format_hotkey_letter() {
        let s = format_hotkey(0x0002, 0x41); // Ctrl+A
        assert_eq!(s, "Ctrl+A");
    }
}
