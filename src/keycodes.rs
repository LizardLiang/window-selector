// This is a constants library — not all entries are used by every consumer,
// so we suppress the dead_code lint for the whole module.
#![allow(dead_code)]

//! Centralized Windows virtual-key codes and hotkey modifier flags.
//!
//! All values are `u32` to match the `RegisterHotKey` / `GetAsyncKeyState` APIs
//! and the `AppConfig` serialization format.
//!
//! Source: <https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes>

// ---------------------------------------------------------------------------
// Hotkey modifier flags  (used with RegisterHotKey)
// ---------------------------------------------------------------------------

/// MOD_ALT — either Alt key.
pub const MOD_ALT: u32 = 0x0001;
/// MOD_CONTROL — either Ctrl key.
pub const MOD_CONTROL: u32 = 0x0002;
/// MOD_SHIFT — either Shift key.
pub const MOD_SHIFT: u32 = 0x0004;
/// MOD_WIN — either Windows logo key.
pub const MOD_WIN: u32 = 0x0008;
/// MOD_NOREPEAT — suppress repeated WM_HOTKEY messages while the key is held.
pub const MOD_NOREPEAT: u32 = 0x4000;

// ---------------------------------------------------------------------------
// Control / editing keys
// ---------------------------------------------------------------------------

/// VK_BACK — Backspace key.
pub const VK_BACK: u32 = 0x08;
/// VK_TAB — Tab key.
pub const VK_TAB: u32 = 0x09;
/// VK_RETURN — Enter key.
pub const VK_RETURN: u32 = 0x0D;
/// VK_ESCAPE — Escape key.
pub const VK_ESCAPE: u32 = 0x1B;
/// VK_SPACE — Space bar.
pub const VK_SPACE: u32 = 0x20;

// ---------------------------------------------------------------------------
// Modifier virtual keys  (used with GetAsyncKeyState)
// ---------------------------------------------------------------------------

/// VK_SHIFT — generic Shift (either left or right).
pub const VK_SHIFT: u32 = 0x10;
/// VK_CONTROL — generic Ctrl (either left or right).
pub const VK_CONTROL: u32 = 0x11;
/// VK_MENU — generic Alt (either left or right).  "Menu" is the Win32 name for Alt.
pub const VK_MENU: u32 = 0x12;
/// VK_LSHIFT — left Shift key.
pub const VK_LSHIFT: u32 = 0xA0;
/// VK_RSHIFT — right Shift key.
pub const VK_RSHIFT: u32 = 0xA1;
/// VK_LCONTROL — left Ctrl key.
pub const VK_LCONTROL: u32 = 0xA2;
/// VK_RCONTROL — right Ctrl key.
pub const VK_RCONTROL: u32 = 0xA3;
/// VK_LMENU — left Alt key.
pub const VK_LMENU: u32 = 0xA4;
/// VK_RMENU — right Alt key.
pub const VK_RMENU: u32 = 0xA5;
/// VK_LWIN — left Windows logo key.
pub const VK_LWIN: u32 = 0x5B;
/// VK_RWIN — right Windows logo key.
pub const VK_RWIN: u32 = 0x5C;

// ---------------------------------------------------------------------------
// Digit row  (0x30–0x39 = '0'–'9')
// ---------------------------------------------------------------------------

/// VK_0 through VK_9 — top-row digit keys.
pub const VK_0: u32 = 0x30;
pub const VK_1: u32 = 0x31;
pub const VK_2: u32 = 0x32;
pub const VK_3: u32 = 0x33;
pub const VK_4: u32 = 0x34;
pub const VK_5: u32 = 0x35;
pub const VK_6: u32 = 0x36;
pub const VK_7: u32 = 0x37;
pub const VK_8: u32 = 0x38;
pub const VK_9: u32 = 0x39;

// ---------------------------------------------------------------------------
// Letter keys  (0x41–0x5A = 'A'–'Z')
// ---------------------------------------------------------------------------

pub const VK_A: u32 = 0x41;
pub const VK_B: u32 = 0x42;
pub const VK_C: u32 = 0x43;
pub const VK_D: u32 = 0x44;
pub const VK_E: u32 = 0x45;
pub const VK_F: u32 = 0x46;
pub const VK_G: u32 = 0x47;
pub const VK_H: u32 = 0x48;
pub const VK_I: u32 = 0x49;
pub const VK_J: u32 = 0x4A;
pub const VK_K: u32 = 0x4B;
pub const VK_L: u32 = 0x4C;
pub const VK_M: u32 = 0x4D;
pub const VK_N: u32 = 0x4E;
pub const VK_O: u32 = 0x4F;
pub const VK_P: u32 = 0x50;
pub const VK_Q: u32 = 0x51;
pub const VK_R: u32 = 0x52;
pub const VK_S: u32 = 0x53;
pub const VK_T: u32 = 0x54;
pub const VK_U: u32 = 0x55;
pub const VK_V: u32 = 0x56;
pub const VK_W: u32 = 0x57;
pub const VK_X: u32 = 0x58;
pub const VK_Y: u32 = 0x59;
pub const VK_Z: u32 = 0x5A;

// ---------------------------------------------------------------------------
// Navigation / editing keys — reachable by the Confirm/Dismiss recorder now
// that it accepts bare non-alphanumeric keys (see `hotkey::vk_to_name`).
// ---------------------------------------------------------------------------

pub const VK_PRIOR: u32 = 0x21; // Page Up
pub const VK_NEXT: u32 = 0x22; // Page Down
pub const VK_END: u32 = 0x23;
pub const VK_HOME: u32 = 0x24;
pub const VK_LEFT: u32 = 0x25;
pub const VK_UP: u32 = 0x26;
pub const VK_RIGHT: u32 = 0x27;
pub const VK_DOWN: u32 = 0x28;
pub const VK_INSERT: u32 = 0x2D;
pub const VK_DELETE: u32 = 0x2E;

// ---------------------------------------------------------------------------
// Numpad keys  (0x60–0x6F) — reachable by the Confirm/Dismiss recorder.
// ---------------------------------------------------------------------------

pub const VK_NUMPAD0: u32 = 0x60;
pub const VK_NUMPAD1: u32 = 0x61;
pub const VK_NUMPAD2: u32 = 0x62;
pub const VK_NUMPAD3: u32 = 0x63;
pub const VK_NUMPAD4: u32 = 0x64;
pub const VK_NUMPAD5: u32 = 0x65;
pub const VK_NUMPAD6: u32 = 0x66;
pub const VK_NUMPAD7: u32 = 0x67;
pub const VK_NUMPAD8: u32 = 0x68;
pub const VK_NUMPAD9: u32 = 0x69;
pub const VK_MULTIPLY: u32 = 0x6A; // Numpad *
pub const VK_ADD: u32 = 0x6B; // Numpad +
pub const VK_SUBTRACT: u32 = 0x6D; // Numpad -
pub const VK_DECIMAL: u32 = 0x6E; // Numpad .
pub const VK_DIVIDE: u32 = 0x6F; // Numpad /

// ---------------------------------------------------------------------------
// Function keys  (0x70–0x7B = F1–F12)
// ---------------------------------------------------------------------------

pub const VK_F1: u32 = 0x70;
pub const VK_F2: u32 = 0x71;
pub const VK_F3: u32 = 0x72;
pub const VK_F4: u32 = 0x73;
pub const VK_F5: u32 = 0x74;
pub const VK_F6: u32 = 0x75;
pub const VK_F7: u32 = 0x76;
pub const VK_F8: u32 = 0x77;
pub const VK_F9: u32 = 0x78;
pub const VK_F10: u32 = 0x79;
pub const VK_F11: u32 = 0x7A;
pub const VK_F12: u32 = 0x7B;

// ---------------------------------------------------------------------------
// OEM punctuation keys — US-layout names. Reachable by the Confirm/Dismiss
// recorder now that it accepts bare non-alphanumeric keys.
// ---------------------------------------------------------------------------

pub const VK_OEM_1: u32 = 0xBA; // ;:
pub const VK_OEM_PLUS: u32 = 0xBB; // =+
pub const VK_OEM_COMMA: u32 = 0xBC; // ,<
pub const VK_OEM_MINUS: u32 = 0xBD; // -_
pub const VK_OEM_PERIOD: u32 = 0xBE; // .>
pub const VK_OEM_2: u32 = 0xBF; // /?
pub const VK_OEM_3: u32 = 0xC0; // `~
pub const VK_OEM_4: u32 = 0xDB; // [{
pub const VK_OEM_5: u32 = 0xDC; // \|
pub const VK_OEM_6: u32 = 0xDD; // ]}
pub const VK_OEM_7: u32 = 0xDE; // '"

// ---------------------------------------------------------------------------
// Windows message IDs used as raw integers in wndproc callbacks.
// (The windows crate exposes these as typed constants in WM_* names, but
// the low-level hook callback receives them as usize and needs raw comparison.)
// ---------------------------------------------------------------------------

/// WM_KEYDOWN — key pressed (non-system).
pub const WM_KEYDOWN_RAW: u32 = 0x0100;
/// WM_SYSKEYDOWN — key pressed while Alt is held.
pub const WM_SYSKEYDOWN_RAW: u32 = 0x0104;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Range check: true if `vk` is a letter key (A–Z).
#[inline]
pub fn is_letter(vk: u32) -> bool {
    (VK_A..=VK_Z).contains(&vk)
}

/// Range check: true if `vk` is a digit key (0–9).
#[inline]
pub fn is_digit(vk: u32) -> bool {
    (VK_0..=VK_9).contains(&vk)
}

/// Range check: true if `vk` is a function key (F1–F12).
#[inline]
pub fn is_function_key(vk: u32) -> bool {
    (VK_F1..=VK_F12).contains(&vk)
}

/// True if `vk` is a pure modifier key (Shift, Ctrl, Alt, Win — any side).
/// Used by the hotkey recorder to reject modifier-only combinations.
#[inline]
pub fn is_modifier_only(vk: u32) -> bool {
    matches!(
        vk,
        VK_SHIFT
            | VK_CONTROL
            | VK_MENU
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_LMENU
            | VK_RMENU
            | VK_LWIN
            | VK_RWIN
    )
}

/// Normalize a raw (possibly side-specific) modifier VK — as reported by the
/// low-level keyboard hook — to the "generic" VK used by
/// `ActionModifier::to_vk()` and `GetAsyncKeyState`. Returns `None` if `vk`
/// is not a modifier key at all.
#[inline]
pub fn normalize_modifier_vk(vk: u32) -> Option<u32> {
    match vk {
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT => Some(VK_SHIFT),
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL => Some(VK_CONTROL),
        VK_MENU | VK_LMENU | VK_RMENU => Some(VK_MENU),
        VK_LWIN | VK_RWIN => Some(VK_LWIN),
        _ => None,
    }
}
