use crate::keycodes::{
    MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_A, VK_ADD, VK_BACK, VK_DECIMAL, VK_DELETE,
    VK_DIVIDE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_F12, VK_HOME, VK_INSERT, VK_LEFT,
    VK_MULTIPLY, VK_NEXT, VK_NUMPAD0, VK_NUMPAD9, VK_OEM_1, VK_OEM_2, VK_OEM_3, VK_OEM_4,
    VK_OEM_5, VK_OEM_6, VK_OEM_7, VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PERIOD, VK_OEM_PLUS,
    VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SPACE, VK_SUBTRACT, VK_TAB, VK_UP, VK_Z,
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
        VK_PRIOR => "Page Up".to_string(),
        VK_NEXT => "Page Down".to_string(),
        VK_END => "End".to_string(),
        VK_HOME => "Home".to_string(),
        VK_LEFT => "Left".to_string(),
        VK_UP => "Up".to_string(),
        VK_RIGHT => "Right".to_string(),
        VK_DOWN => "Down".to_string(),
        VK_INSERT => "Insert".to_string(),
        VK_DELETE => "Delete".to_string(),
        // OEM punctuation — standard US-layout names.
        VK_OEM_1 => ";".to_string(),
        VK_OEM_PLUS => "=".to_string(),
        VK_OEM_COMMA => ",".to_string(),
        VK_OEM_MINUS => "-".to_string(),
        VK_OEM_PERIOD => ".".to_string(),
        VK_OEM_2 => "/".to_string(),
        VK_OEM_3 => "`".to_string(),
        VK_OEM_4 => "[".to_string(),
        VK_OEM_5 => "\\".to_string(),
        VK_OEM_6 => "]".to_string(),
        VK_OEM_7 => "'".to_string(),
        // Numpad
        VK_MULTIPLY => "Num*".to_string(),
        VK_ADD => "Num+".to_string(),
        VK_SUBTRACT => "Num-".to_string(),
        VK_DECIMAL => "Num.".to_string(),
        VK_DIVIDE => "Num/".to_string(),
        VK_NUMPAD0..=VK_NUMPAD9 => format!("Num{}", vk - VK_NUMPAD0),
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

    // Plan step 25: the Confirm/Dismiss recorder now accepts bare
    // non-alphanumeric keys, so nav/edit keys must have a display name
    // instead of falling through to the raw "0x%02X" branch.
    #[test]
    fn test_format_hotkey_bare_nav_keys_have_names() {
        use crate::keycodes::{VK_DELETE, VK_F4, VK_HOME, VK_TAB};
        assert_eq!(format_hotkey(0, VK_TAB), "Tab");
        assert_eq!(format_hotkey(0, VK_F4), "F4");
        assert_eq!(format_hotkey(0, VK_HOME), "Home");
        assert_eq!(format_hotkey(0, VK_DELETE), "Delete");
    }

    // Hermes WARNING 2: OEM punctuation and Numpad keys are also reachable by
    // the Confirm/Dismiss recorder as bare non-alphanumeric bindings — none
    // of them may fall through to the raw "0x%02X" branch.
    #[test]
    fn test_format_hotkey_oem_punctuation_has_names() {
        use crate::keycodes::{
            VK_OEM_1, VK_OEM_2, VK_OEM_3, VK_OEM_4, VK_OEM_5, VK_OEM_6, VK_OEM_7, VK_OEM_COMMA,
            VK_OEM_MINUS, VK_OEM_PERIOD, VK_OEM_PLUS,
        };
        assert_eq!(format_hotkey(0, VK_OEM_1), ";");
        assert_eq!(format_hotkey(0, VK_OEM_PLUS), "=");
        assert_eq!(format_hotkey(0, VK_OEM_COMMA), ",");
        assert_eq!(format_hotkey(0, VK_OEM_MINUS), "-");
        assert_eq!(format_hotkey(0, VK_OEM_PERIOD), ".");
        assert_eq!(format_hotkey(0, VK_OEM_2), "/");
        assert_eq!(format_hotkey(0, VK_OEM_3), "`");
        assert_eq!(format_hotkey(0, VK_OEM_4), "[");
        assert_eq!(format_hotkey(0, VK_OEM_5), "\\");
        assert_eq!(format_hotkey(0, VK_OEM_6), "]");
        assert_eq!(format_hotkey(0, VK_OEM_7), "'");
    }

    #[test]
    fn test_format_hotkey_numpad_has_names() {
        use crate::keycodes::{VK_ADD, VK_DECIMAL, VK_DIVIDE, VK_MULTIPLY, VK_NUMPAD5, VK_SUBTRACT};
        assert_eq!(format_hotkey(0, VK_NUMPAD5), "Num5");
        assert_eq!(format_hotkey(0, VK_MULTIPLY), "Num*");
        assert_eq!(format_hotkey(0, VK_ADD), "Num+");
        assert_eq!(format_hotkey(0, VK_SUBTRACT), "Num-");
        assert_eq!(format_hotkey(0, VK_DECIMAL), "Num.");
        assert_eq!(format_hotkey(0, VK_DIVIDE), "Num/");
    }
}
