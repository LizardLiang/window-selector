use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};

/// The hotkey ID used with RegisterHotKey.
pub const HOTKEY_ID: i32 = 1;

/// Register the global activation hotkey.
/// Returns Ok(()) on success, Err on failure (conflict with another app).
pub fn register_hotkey(
    msg_hwnd: HWND,
    modifiers: u32,
    vk_code: u32,
) -> windows::core::Result<()> {
    unsafe {
        RegisterHotKey(
            msg_hwnd,
            HOTKEY_ID,
            HOT_KEY_MODIFIERS(modifiers),
            vk_code,
        )?;
        tracing::info!(
            "Hotkey registered: modifiers=0x{:X} vk=0x{:X}",
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

/// Format a hotkey combo as a human-readable string (e.g., "Ctrl+Alt+Space").
pub fn format_hotkey(modifiers: u32, vk_code: u32) -> String {
    let mut parts = Vec::new();

    // MOD_CONTROL = 0x0002, MOD_ALT = 0x0001, MOD_SHIFT = 0x0004, MOD_WIN = 0x0008
    if (modifiers & 0x0002) != 0 {
        parts.push("Ctrl");
    }
    if (modifiers & 0x0001) != 0 {
        parts.push("Alt");
    }
    if (modifiers & 0x0004) != 0 {
        parts.push("Shift");
    }
    if (modifiers & 0x0008) != 0 {
        parts.push("Win");
    }

    let key_name = vk_to_name(vk_code);
    parts.push(&key_name);
    parts.join("+")
}

/// Map common virtual key codes to display names.
fn vk_to_name(vk: u32) -> String {
    match vk {
        0x20 => "Space".to_string(),
        0x0D => "Enter".to_string(),
        0x08 => "Backspace".to_string(),
        0x09 => "Tab".to_string(),
        0x1B => "Esc".to_string(),
        0x70..=0x7B => format!("F{}", vk - 0x70 + 1),
        0x41..=0x5A => {
            let c = (b'A' + (vk - 0x41) as u8) as char;
            c.to_string()
        }
        0x30..=0x39 => {
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
