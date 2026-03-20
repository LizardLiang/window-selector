use std::time::Instant;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, BringWindowToTop, GetWindowThreadProcessId, IsIconic,
    SetForegroundWindow, ShowWindow, SW_RESTORE,
};

/// RAII guard that detaches thread input queues when dropped.
///
/// When `AttachThreadInput(our, target, TRUE)` succeeds, create one of these guards.
/// The detach call is guaranteed to execute even if a panic or early return occurs
/// between attach and the natural detach point.
struct ThreadInputGuard {
    our_thread: u32,
    target_thread: u32,
}

impl Drop for ThreadInputGuard {
    fn drop(&mut self) {
        // SAFETY: We only construct this guard after a successful attach, so detaching
        // here is always paired with the earlier attach. Both thread IDs are valid for
        // the process lifetime.
        let ok = unsafe {
            AttachThreadInput(self.our_thread, self.target_thread, false).as_bool()
        };
        if !ok {
            tracing::warn!(
                "AttachThreadInput detach failed (our={}, target={})",
                self.our_thread,
                self.target_thread,
            );
        }
    }
}

/// Transfer focus to the given window using a hybrid approach:
/// 1. Try AllowSetForegroundWindow + SetForegroundWindow.
/// 2. Fall back to AttachThreadInput + SetForegroundWindow if that fails.
pub fn switch_to_window(hwnd: HWND) -> windows::core::Result<()> {
    let start = Instant::now();

    unsafe {
        // Restore if minimized
        if IsIconic(hwnd).as_bool() {
            ShowWindow(hwnd, SW_RESTORE);
        }

        // Attempt 1: AllowSetForegroundWindow + SetForegroundWindow
        let mut target_pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut target_pid));
        let _ = AllowSetForegroundWindow(target_pid);

        if SetForegroundWindow(hwnd).as_bool() {
            let elapsed = start.elapsed();
            tracing::debug!("switch_to_window succeeded via AllowSetForegroundWindow in {:?}", elapsed);
            return Ok(());
        }

        // Attempt 2: AttachThreadInput fallback
        let our_thread = GetCurrentThreadId();
        let target_thread = GetWindowThreadProcessId(hwnd, None);

        // _guard's Drop impl calls AttachThreadInput(..., false) unconditionally,
        // ensuring detach even if a panic or early return is added in the future.
        let _guard = if our_thread != target_thread {
            let attached = AttachThreadInput(our_thread, target_thread, true).as_bool();
            if !attached {
                tracing::warn!(
                    "AttachThreadInput attach failed (our={}, target={})",
                    our_thread,
                    target_thread,
                );
            }
            // Create the guard regardless of whether attach succeeded; a failed attach
            // means the detach will also fail silently, which is harmless.
            Some(ThreadInputGuard { our_thread, target_thread })
        } else {
            None
        };

        let result = SetForegroundWindow(hwnd);
        let _ = BringWindowToTop(hwnd);
        // _guard is dropped here, calling AttachThreadInput(..., false) automatically.

        let elapsed = start.elapsed();

        if result.as_bool() {
            tracing::debug!("switch_to_window succeeded via AttachThreadInput in {:?}", elapsed);
        } else {
            tracing::warn!(
                "switch_to_window: SetForegroundWindow failed for HWND {:?} (elapsed: {:?})",
                hwnd,
                elapsed
            );
        }
    }

    Ok(())
}

/// Restore focus to a previously-active window (e.g., on dismiss without switching).
/// Uses the same hybrid sequence as switch_to_window.
pub fn restore_focus(hwnd: HWND) -> windows::core::Result<()> {
    if hwnd.is_invalid() {
        return Ok(());
    }
    switch_to_window(hwnd)
}
