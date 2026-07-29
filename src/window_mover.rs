use crate::monitor::MonitorInfo;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsIconic, IsZoomed, SetWindowPos,
    ShowWindow, HWND_TOP, SW_RESTORE, SWP_NOACTIVATE, SWP_NOZORDER,
};

/// Pixel tolerance used when verifying that a window landed where we placed it.
/// Accounts for DWM/DPI rounding, not a meaningful placement error.
const VERIFY_TOLERANCE: i32 = 4;

/// Compute a centered `RECT` for `window` within `work`, clamping the window's
/// size to the work area when it doesn't fit.
///
/// Pure geometry, no Win32 calls — mirrors `grid_layout::compute_grid` so it
/// can be unit tested directly.
pub fn centered_rect(window: RECT, work: RECT) -> RECT {
    let window_width = (window.right - window.left).max(0);
    let window_height = (window.bottom - window.top).max(0);
    let work_width = (work.right - work.left).max(0);
    let work_height = (work.bottom - work.top).max(0);

    let width = window_width.min(work_width);
    let height = window_height.min(work_height);

    let left = work.left + (work_width - width) / 2;
    let top = work.top + (work_height - height) / 2;

    RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

/// Restore (if minimized/maximized), clamp, and center `hwnd` on `monitor`'s
/// work area. Verifies the result and retries once to handle cross-DPI
/// rescale or apps that reposition themselves after being moved.
///
/// Returns true if the window ended up centered (within tolerance).
pub fn move_to_monitor_center(hwnd: HWND, monitor: &MonitorInfo) -> bool {
    unsafe {
        // SetWindowPos silently ignores position changes on a maximized window,
        // so minimized/maximized windows must be restored first.
        if IsIconic(hwnd).as_bool() || IsZoomed(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            tracing::warn!(
                "move_to_monitor_center: GetWindowRect failed for {:?}",
                hwnd
            );
            return false;
        }

        let target = centered_rect(rect, monitor.work_rect);
        apply_rect(hwnd, &target);

        if verify_and_maybe_retry(hwnd, monitor, target) {
            tracing::info!("Centered window {:?} on monitor", hwnd);
            true
        } else {
            tracing::warn!(
                "Window {:?} ({}) did not land centered after retry — possibly elevated/UIPI-blocked",
                hwnd,
                window_title(hwnd)
            );
            false
        }
    }
}

unsafe fn apply_rect(hwnd: HWND, target: &RECT) {
    let _ = SetWindowPos(
        hwnd,
        HWND_TOP,
        target.left,
        target.top,
        target.right - target.left,
        target.bottom - target.top,
        SWP_NOACTIVATE | SWP_NOZORDER,
    );
}

/// Verify the window landed on `target`. If not, recompute from the window's
/// current (possibly rescaled) size and retry exactly once.
unsafe fn verify_and_maybe_retry(hwnd: HWND, monitor: &MonitorInfo, target: RECT) -> bool {
    let mut actual = RECT::default();
    if GetWindowRect(hwnd, &mut actual).is_err() {
        return false;
    }

    if rects_close(actual, target) {
        return true;
    }

    let retry_target = centered_rect(actual, monitor.work_rect);
    apply_rect(hwnd, &retry_target);

    let mut final_rect = RECT::default();
    if GetWindowRect(hwnd, &mut final_rect).is_err() {
        return false;
    }
    rects_close(final_rect, retry_target)
}

fn rects_close(a: RECT, b: RECT) -> bool {
    (a.left - b.left).abs() <= VERIFY_TOLERANCE
        && (a.top - b.top).abs() <= VERIFY_TOLERANCE
        && (a.right - b.right).abs() <= VERIFY_TOLERANCE
        && (a.bottom - b.bottom).abs() <= VERIFY_TOLERANCE
}

unsafe fn window_title(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len as usize) + 1];
    let copied = GetWindowTextW(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..copied as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn test_centered_rect_plain_centering() {
        let window = rect(0, 0, 400, 300);
        let work = rect(0, 0, 1920, 1080);
        let result = centered_rect(window, work);
        assert_eq!(result.right - result.left, 400);
        assert_eq!(result.bottom - result.top, 300);
        assert_eq!(result.left, (1920 - 400) / 2);
        assert_eq!(result.top, (1080 - 300) / 2);
    }

    #[test]
    fn test_centered_rect_clamps_width_when_wider_than_work_area() {
        let window = rect(0, 0, 3000, 300);
        let work = rect(0, 0, 1920, 1080);
        let result = centered_rect(window, work);
        assert_eq!(result.right - result.left, 1920);
        assert_eq!(result.left, 0);
        assert_eq!(result.right, 1920);
    }

    #[test]
    fn test_centered_rect_clamps_height_when_taller_than_work_area() {
        let window = rect(0, 0, 400, 2000);
        let work = rect(0, 0, 1920, 1080);
        let result = centered_rect(window, work);
        assert_eq!(result.bottom - result.top, 1080);
        assert_eq!(result.top, 0);
        assert_eq!(result.bottom, 1080);
    }

    #[test]
    fn test_centered_rect_work_area_offset_by_taskbar() {
        let window = rect(0, 0, 400, 300);
        // Taskbar reserves 40px at the bottom of a 1920x1080 monitor.
        let work = rect(0, 0, 1920, 1040);
        let result = centered_rect(window, work);
        assert_eq!(result.top, (1040 - 300) / 2);
        assert!(result.bottom <= 1040, "window must stay above the taskbar");
    }

    #[test]
    fn test_centered_rect_off_screen_window_origin() {
        // A 400x300 window positioned far off-screen (negative coordinates).
        let window = rect(-4000, -4000, -3600, -3700);
        let work = rect(0, 0, 1920, 1080);
        let result = centered_rect(window, work);
        assert_eq!(result.right - result.left, 400);
        assert_eq!(result.bottom - result.top, 300);
        assert!(result.left >= 0);
        assert!(result.top >= 0);
    }

    #[test]
    fn test_centered_rect_monitor_negative_origin() {
        let window = rect(0, 0, 400, 300);
        // A monitor positioned to the left of the primary monitor.
        let work = rect(-1920, 0, 0, 1080);
        let result = centered_rect(window, work);
        assert_eq!(result.right - result.left, 400);
        assert_eq!(result.left, -1920 + (1920 - 400) / 2);
    }
}
