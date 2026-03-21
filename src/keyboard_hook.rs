/// Low-level keyboard hook for capturing key presses while the overlay is visible.
///
/// Uses WH_KEYBOARD_LL to intercept keystrokes system-wide.  This bypasses the
/// normal Win32 focus model, so the overlay receives keyboard input even if
/// SetForegroundWindow fails (which Windows routinely blocks to prevent focus
/// stealing from aggressive foreground-window requests).
///
/// # Safety invariant
/// The hook callback runs on the message pump thread (same thread that called
/// SetWindowsHookExW) because WH_KEYBOARD_LL callbacks are always dispatched on
/// the installing thread's message loop. We therefore access APP_STATE_PTR with
/// the same Relaxed ordering used everywhere else in the codebase.
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, HC_ACTION,
    WH_KEYBOARD_LL, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN,
};

/// Whether the overlay is currently active and should consume key presses.
static HOOK_ACTIVE: AtomicBool = AtomicBool::new(false);
/// The low-level keyboard hook handle, stored as a usize to avoid static mut.
static HOOK_HANDLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A callback type that handles a virtual key code while the overlay is active.
/// Called on every WM_KEYDOWN/WM_SYSKEYDOWN event intercepted by the hook.
/// Return true to swallow the key (prevent it from reaching other applications),
/// false to pass it through.
pub type KeyHandler = fn(vk_code: u32) -> bool;

/// Global key handler fn pointer (set once before hook is installed).
static KEY_HANDLER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Install the low-level keyboard hook.
///
/// `handler` is called with the virtual-key code on each key-down event while
/// the hook is marked active.  Must be called from the message pump thread.
pub fn install(handler: KeyHandler) {
    KEY_HANDLER.store(handler as usize, Ordering::Relaxed);

    // Do not install a second hook if one is already registered.
    if HOOK_HANDLE.load(Ordering::Relaxed) != 0 {
        return;
    }

    unsafe {
        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), None, 0) {
            Ok(hhook) => {
                HOOK_HANDLE.store(hhook.0 as usize, Ordering::Relaxed);
                tracing::debug!("Low-level keyboard hook installed: {:?}", hhook);
            }
            Err(e) => {
                tracing::error!("SetWindowsHookExW(WH_KEYBOARD_LL) failed: {:?}", e);
            }
        }
    }
}

/// Uninstall the low-level keyboard hook.
/// Safe to call if the hook was never installed (no-op).
pub fn uninstall() {
    let handle = HOOK_HANDLE.swap(0, Ordering::Relaxed);
    if handle != 0 {
        unsafe {
            let hhook = HHOOK(handle as *mut _);
            if let Err(e) = UnhookWindowsHookEx(hhook) {
                tracing::warn!("UnhookWindowsHookEx failed: {:?}", e);
            } else {
                tracing::debug!("Low-level keyboard hook uninstalled");
            }
        }
    }
    set_active(false);
}

/// Signal that the overlay is now visible and should consume keystrokes.
pub fn set_active(active: bool) {
    HOOK_ACTIVE.store(active, Ordering::Relaxed);
}

/// Returns whether the hook is currently consuming keystrokes.
#[allow(dead_code)]
pub fn is_active() -> bool {
    HOOK_ACTIVE.load(Ordering::Relaxed)
}

/// The WH_KEYBOARD_LL callback procedure.
///
/// # Safety
/// Called by Windows on the message pump thread.  Accessing global atomics is
/// safe because only this thread ever reads/writes them (same invariant as
/// APP_STATE_PTR in main.rs).
unsafe extern "system" fn ll_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    let hhook_raw = HOOK_HANDLE.load(Ordering::Relaxed);
    let hhook = HHOOK(hhook_raw as *mut _);

    if n_code < HC_ACTION as i32 {
        return CallNextHookEx(hhook, n_code, w_param, l_param);
    }

    let msg_id = w_param.0 as u32;
    let is_keydown = msg_id == WM_KEYDOWN || msg_id == WM_SYSKEYDOWN;

    if is_keydown && HOOK_ACTIVE.load(Ordering::Relaxed) {
        if l_param.0 != 0 {
            let kbd = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
            let vk = kbd.vkCode;

            let handler_ptr = KEY_HANDLER.load(Ordering::Relaxed);
            if handler_ptr != 0 {
                let handler: KeyHandler = std::mem::transmute(handler_ptr);
                let swallow = handler(vk);
                if swallow {
                    // Return non-zero to swallow the keystroke.
                    return LRESULT(1);
                }
            }
        }
    }

    CallNextHookEx(hhook, n_code, w_param, l_param)
}
