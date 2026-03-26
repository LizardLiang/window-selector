use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT};

/// Maximum number of entries in the MRU list.
const MRU_MAX_SIZE: usize = 100;

/// Maintains a most-recently-used ordering of window handles.
/// Updated in real-time by a WinEvent hook (EVENT_SYSTEM_FOREGROUND).
pub struct MruTracker {
    /// Ordered from most recent (index 0) to least recent.
    order: Vec<HWND>,
    hook_handle: HWINEVENTHOOK,
}

// MruTracker contains HWND values (raw pointers under the hood). The type is not
// Send/Sync by default because HWND is *mut c_void. We do NOT impl Send/Sync here;
// instead we rely on the single-thread invariant enforced at the call sites — all
// access to MruTracker is on the Win32 message pump thread, and the MRU_TRACKER_PTR
// cell is accessed only through the module-level API which is called exclusively on
// that thread.

impl MruTracker {
    pub fn new() -> Self {
        Self {
            order: Vec::new(),
            hook_handle: HWINEVENTHOOK::default(),
        }
    }

    /// Install the WinEvent hook. Must be called from the message pump thread.
    pub fn install_hook(&mut self) {
        unsafe {
            self.hook_handle = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(mru_winevent_callback),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
        }
        if self.hook_handle.is_invalid() {
            tracing::error!("SetWinEventHook(EVENT_SYSTEM_FOREGROUND) failed");
        } else {
            tracing::info!("MRU WinEvent hook installed");
        }
    }

    /// Uninstall the WinEvent hook.
    pub fn uninstall_hook(&mut self) {
        if !self.hook_handle.is_invalid() {
            unsafe {
                let _ = UnhookWinEvent(self.hook_handle);
            }
            self.hook_handle = HWINEVENTHOOK::default();
            tracing::info!("MRU WinEvent hook uninstalled");
        }
    }

    /// Move the given HWND to the front of the MRU list.
    pub fn on_foreground_change(&mut self, hwnd: HWND) {
        // Remove existing entry if present
        self.order.retain(|&h| h != hwnd);
        // Insert at front
        self.order.insert(0, hwnd);
        // Cap at max size
        if self.order.len() > MRU_MAX_SIZE {
            self.order.truncate(MRU_MAX_SIZE);
        }
    }

    /// Get the current MRU order.
    #[allow(dead_code)]
    pub fn get_order(&self) -> &[HWND] {
        &self.order
    }

    /// Sort a window list by MRU order. Windows not in the MRU list are appended at the end.
    pub fn sort_by_mru(&self, windows: &mut Vec<crate::window_info::WindowInfo>) {
        // Build a position map from HWND to MRU index
        let mru_pos: std::collections::HashMap<isize, usize> = self
            .order
            .iter()
            .enumerate()
            .map(|(i, &h)| (h.0 as isize, i))
            .collect();

        windows.sort_by_key(|w| {
            mru_pos
                .get(&(w.hwnd.0 as isize))
                .copied()
                .unwrap_or(usize::MAX)
        });
    }
}

impl Default for MruTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MruTracker {
    fn drop(&mut self) {
        self.uninstall_hook();
    }
}

/// A `*mut MruTracker` wrapper that is `Send + Sync` only because we document and
/// enforce the single-thread invariant: `MRU_TRACKER_CELL` is written once from the
/// message pump thread at startup, and read only from the same thread (via the
/// WinEvent callback, which is delivered in-process on the same thread due to
/// `WINEVENT_OUTOFCONTEXT` with pid=0/tid=0).
///
/// No other thread ever touches this cell.
struct SendPtr(*mut MruTracker);
// SAFETY: The pointer is only accessed on the Win32 message pump thread.
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

/// Holds the MRU tracker pointer. Initialized once at startup; never mutated again.
/// Accessed exclusively on the message pump thread.
static MRU_TRACKER_CELL: std::sync::OnceLock<SendPtr> = std::sync::OnceLock::new();

/// Set the global MRU tracker pointer. Must be called from the message pump thread
/// before installing the hook. Must be called at most once per process.
pub fn set_global_mru_tracker(tracker: *mut MruTracker) {
    // If the cell is already set (e.g., in tests), ignore the second call.
    let _ = MRU_TRACKER_CELL.set(SendPtr(tracker));
}

unsafe extern "system" fn mru_winevent_callback(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    if hwnd.is_invalid() {
        return;
    }
    if let Some(cell) = MRU_TRACKER_CELL.get() {
        if !cell.0.is_null() {
            (*cell.0).on_foreground_change(hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hwnd(n: isize) -> HWND {
        HWND(n as *mut _)
    }

    #[test]
    fn test_mru_move_to_front() {
        let mut tracker = MruTracker::new();
        let hw_a = hwnd(1);
        let hw_b = hwnd(2);

        tracker.on_foreground_change(hw_a);
        tracker.on_foreground_change(hw_b);
        tracker.on_foreground_change(hw_a);

        let order = tracker.get_order();
        assert_eq!(order[0], hw_a);
        assert_eq!(order[1], hw_b);
    }

    #[test]
    fn test_mru_list_capped_at_100() {
        let mut tracker = MruTracker::new();
        for i in 0..110 {
            tracker.on_foreground_change(hwnd(i));
        }
        assert!(tracker.get_order().len() <= 100);
    }

    #[test]
    fn test_mru_no_duplicates() {
        let mut tracker = MruTracker::new();
        let hw = hwnd(42);
        tracker.on_foreground_change(hw);
        tracker.on_foreground_change(hwnd(1));
        tracker.on_foreground_change(hw);
        // hw should appear only once at position 0
        let count = tracker.get_order().iter().filter(|&&h| h == hw).count();
        assert_eq!(count, 1);
    }
}
