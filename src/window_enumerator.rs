use crate::mru_tracker::MruTracker;
use crate::window_info::WindowInfo;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetShellWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowRect, IsIconic, IsWindowVisible, GWL_EXSTYLE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

/// Set of overlay HWNDs to exclude from the window snapshot.
static mut OVERLAY_HWNDS: Vec<HWND> = Vec::new();

/// Register overlay HWNDs to be excluded from enumeration.
pub fn register_overlay_hwnds(hwnds: Vec<HWND>) {
    unsafe {
        OVERLAY_HWNDS = hwnds;
    }
}

struct EnumContext {
    windows: Vec<WindowInfo>,
    monitors: Vec<crate::monitor::MonitorInfo>,
}

/// Enumerate all Alt+Tab-visible windows and return a snapshot.
/// Applies the heuristic filter matching Windows Alt+Tab behavior.
// SAFETY: OVERLAY_HWNDS is only written once at startup (register_overlay_hwnds), and only
// read afterwards. This is a single-threaded Win32 app — no concurrent access is possible.
#[allow(static_mut_refs)]
pub fn enumerate_windows(
    own_hwnds: &[HWND],
    monitors: &[crate::monitor::MonitorInfo],
) -> Vec<WindowInfo> {
    let mut ctx = EnumContext {
        windows: Vec::new(),
        monitors: monitors.to_vec(),
    };

    let ctx_ptr = &mut ctx as *mut EnumContext;

    unsafe {
        let _ = EnumWindows(Some(enum_windows_callback), LPARAM(ctx_ptr as isize));
    }

    // Filter out our own overlay HWNDs
    ctx.windows.retain(|w| {
        !own_hwnds.contains(&w.hwnd) && {
            unsafe {
                // Also exclude our globally registered overlays
                !OVERLAY_HWNDS.contains(&w.hwnd)
            }
        }
    });

    ctx.windows
}

/// Returns `true` if `hwnd` should be excluded from any window enumeration.
///
/// Checks shared across both `enum_windows_callback` and `z_order_callback`:
/// - Shell window
/// - Invisible windows
/// - Cloaked windows (e.g. UWP apps on inactive virtual desktops)
///
/// SAFETY: Calls Win32 APIs. Caller must ensure `hwnd` is a valid handle.
unsafe fn should_skip_window(hwnd: HWND) -> bool {
    // Skip shell window
    if hwnd == GetShellWindow() {
        return true;
    }

    // Skip invisible windows
    if !IsWindowVisible(hwnd).as_bool() {
        return true;
    }

    // Skip cloaked windows (e.g. UWP apps on inactive virtual desktops)
    let mut cloaked: u32 = 0;
    let _ = DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        &mut cloaked as *mut u32 as *mut std::ffi::c_void,
        std::mem::size_of::<u32>() as u32,
    );
    if cloaked != 0 {
        return true;
    }

    false
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut EnumContext);

    if should_skip_window(hwnd) {
        return BOOL(1);
    }

    // Skip windows with no title
    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 {
        return BOOL(1);
    }

    // Skip tool windows (unless they also have WS_EX_APPWINDOW)
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    let is_tool = (ex_style & WS_EX_TOOLWINDOW.0) != 0;
    let is_app = (ex_style & WS_EX_APPWINDOW.0) != 0;
    if is_tool && !is_app {
        return BOOL(1);
    }

    // Get window title
    let mut buf = vec![0u16; (title_len as usize) + 1];
    GetWindowTextW(hwnd, &mut buf);
    let title = String::from_utf16_lossy(&buf[..title_len as usize]);

    let is_minimized = IsIconic(hwnd).as_bool();

    // Determine monitor index
    let monitor_handle = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let monitor_index = ctx
        .monitors
        .iter()
        .position(|m| m.handle == monitor_handle)
        .unwrap_or(0);

    ctx.windows
        .push(WindowInfo::new(hwnd, title, is_minimized, monitor_index));

    BOOL(1) // Continue enumeration
}

/// Produce a filtered, MRU-ordered, letter-assigned snapshot of all visible windows.
pub fn snapshot_windows(
    own_hwnds: &[HWND],
    monitors: &[crate::monitor::MonitorInfo],
    mru_tracker: &MruTracker,
    session_tags: &crate::state::SessionTags,
) -> Vec<WindowInfo> {
    let mut windows = enumerate_windows(own_hwnds, monitors);

    // Sort by MRU order
    mru_tracker.sort_by_mru(&mut windows);

    // Assign letters
    crate::letter_assignment::assign_letters(&mut windows);

    // Re-apply session tags and fetch each window's icon once.
    // Caching here avoids sending WM_GETICON on every WM_PAINT repaint.
    for window in &mut windows {
        window.number_tag = session_tags.get_tag_for_hwnd(window.hwnd);
        window.icon = crate::window_icon::get_window_icon(window.hwnd);
    }

    tracing::debug!("Window snapshot: {} windows", windows.len());
    for w in &windows {
        tracing::debug!(
            "  {:?} letter={:?} tag={:?} minimized={} title={:?}",
            w.hwnd,
            w.letter,
            w.number_tag,
            w.is_minimized,
            w.title
        );
    }

    windows
}

// ---------------------------------------------------------------------------
// Label mode occlusion filtering
// ---------------------------------------------------------------------------

/// A simple axis-aligned rectangle used for occlusion calculations.
#[derive(Clone, Copy, Debug)]
struct SimpleRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl SimpleRect {
    fn from_win32(r: &RECT) -> Option<Self> {
        if r.right <= r.left || r.bottom <= r.top {
            return None;
        }
        Some(Self {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        })
    }

    fn intersects(&self, other: &Self) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
    }

    /// Subtract `other` from `self`, returning the remaining pieces (0-4 sub-rects).
    fn subtract(&self, other: &Self) -> Vec<Self> {
        if !self.intersects(other) {
            return vec![*self];
        }

        // Clamp intersection
        let ix_left = self.left.max(other.left);
        let ix_top = self.top.max(other.top);
        let ix_right = self.right.min(other.right);
        let ix_bottom = self.bottom.min(other.bottom);

        let mut pieces = Vec::with_capacity(4);

        // Top strip (above intersection)
        if ix_top > self.top {
            pieces.push(Self {
                left: self.left,
                top: self.top,
                right: self.right,
                bottom: ix_top,
            });
        }
        // Bottom strip (below intersection)
        if ix_bottom < self.bottom {
            pieces.push(Self {
                left: self.left,
                top: ix_bottom,
                right: self.right,
                bottom: self.bottom,
            });
        }
        // Left strip (between top and bottom strips)
        if ix_left > self.left {
            pieces.push(Self {
                left: self.left,
                top: ix_top,
                right: ix_left,
                bottom: ix_bottom,
            });
        }
        // Right strip (between top and bottom strips)
        if ix_right < self.right {
            pieces.push(Self {
                left: ix_right,
                top: ix_top,
                right: self.right,
                bottom: ix_bottom,
            });
        }

        pieces
    }
}

/// Z-order snapshot used for occlusion detection.
struct ZOrderEntry {
    hwnd: HWND,
    rect: SimpleRect,
}

/// Collect all visible, non-minimized windows in Z-order (front to back).
/// Used by occlusion detection.
// SAFETY: OVERLAY_HWNDS is only written once at startup (register_overlay_hwnds), and only
// read afterwards. This is a single-threaded Win32 app — no concurrent access is possible.
#[allow(static_mut_refs)]
unsafe extern "system" fn z_order_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let entries = &mut *(lparam.0 as *mut Vec<ZOrderEntry>);

    if should_skip_window(hwnd) {
        return BOOL(1);
    }

    // Skip minimized windows — they are not visible occluders.
    if IsIconic(hwnd).as_bool() {
        return BOOL(1);
    }

    // Skip the application's own overlay HWNDs. They are shown after
    // filter_occluded_for_label_mode runs today, so they happen to be absent
    // from the Z-order snapshot at call time — but that is fragile. Making
    // the exclusion explicit means the filter stays correct even if call
    // order changes in the future.
    if OVERLAY_HWNDS.contains(&hwnd) {
        return BOOL(1);
    }

    let mut r = RECT::default();
    if GetWindowRect(hwnd, &mut r).is_err() {
        return BOOL(1);
    }
    if let Some(rect) = SimpleRect::from_win32(&r) {
        entries.push(ZOrderEntry { hwnd, rect });
    }

    BOOL(1)
}

/// Returns `true` if `candidate` is fully covered by the windows that are strictly
/// higher in Z-order (earlier in `z_order`).
fn is_fully_occluded(candidate_hwnd: HWND, candidate_rect: SimpleRect, z_order: &[ZOrderEntry]) -> bool {
    // Find the candidate's position in Z-order
    let pos = match z_order.iter().position(|e| e.hwnd == candidate_hwnd) {
        Some(p) => p,
        None => return false, // Not in Z-order list → assume visible
    };

    // Start with the full rect as uncovered
    let mut uncovered: Vec<SimpleRect> = vec![candidate_rect];

    // Subtract every window that is strictly higher in Z-order (index < pos)
    for occluder in &z_order[..pos] {
        if uncovered.is_empty() {
            break;
        }
        uncovered = uncovered
            .iter()
            .flat_map(|r| r.subtract(&occluder.rect))
            .collect();
    }

    uncovered.is_empty()
}

/// Remove fully occluded windows from the label mode snapshot and re-assign letters.
///
/// This prevents labels from being drawn on windows that are completely covered by
/// other windows — they would be invisible to the user but would waste letter slots.
pub fn filter_occluded_for_label_mode(windows: Vec<WindowInfo>) -> Vec<WindowInfo> {
    // Collect Z-order + rects for all visible non-minimized windows
    let mut z_order: Vec<ZOrderEntry> = Vec::new();
    unsafe {
        let ptr = &mut z_order as *mut Vec<ZOrderEntry>;
        let _ = EnumWindows(Some(z_order_callback), LPARAM(ptr as isize));
    }

    let filtered: Vec<WindowInfo> = windows
        .into_iter()
        .filter(|w| {
            // Minimized windows are already skipped in rendering; keep them so they
            // remain selectable via keyboard even in label mode.
            if w.is_minimized {
                return true;
            }

            // Get the current rect for this window
            let rect = unsafe {
                let mut r = RECT::default();
                match GetWindowRect(w.hwnd, &mut r) {
                    Ok(_) => SimpleRect::from_win32(&r),
                    Err(_) => None,
                }
            };

            match rect {
                None => false, // Can't get rect → skip
                Some(r) => {
                    let occluded = is_fully_occluded(w.hwnd, r, &z_order);
                    if occluded {
                        tracing::debug!(
                            "label mode: skipping fully occluded window {:?} {:?}",
                            w.hwnd,
                            w.title
                        );
                    }
                    !occluded
                }
            }
        })
        .collect();

    // Re-assign letters since the window set changed
    let mut filtered = filtered;
    crate::letter_assignment::assign_letters(&mut filtered);
    filtered
}

/// Check whether the given window would pass the Alt+Tab filter.
/// Used for unit testing the filter logic with mock data.
#[allow(dead_code)]
pub fn passes_alt_tab_filter_mock(
    visible: bool,
    title_len: usize,
    ex_style: u32,
    cloaked: bool,
) -> bool {
    if !visible {
        return false;
    }
    if title_len == 0 {
        return false;
    }
    let is_tool = (ex_style & WS_EX_TOOLWINDOW.0) != 0;
    let is_app = (ex_style & WS_EX_APPWINDOW.0) != 0;
    if is_tool && !is_app {
        return false;
    }
    if cloaked {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invisible_window_excluded() {
        assert!(!passes_alt_tab_filter_mock(false, 10, 0, false));
    }

    #[test]
    fn test_empty_title_excluded() {
        assert!(!passes_alt_tab_filter_mock(true, 0, 0, false));
    }

    #[test]
    fn test_tool_window_without_appwindow_excluded() {
        assert!(!passes_alt_tab_filter_mock(
            true,
            10,
            WS_EX_TOOLWINDOW.0,
            false
        ));
    }

    #[test]
    fn test_tool_window_with_appwindow_included() {
        assert!(passes_alt_tab_filter_mock(
            true,
            10,
            WS_EX_TOOLWINDOW.0 | WS_EX_APPWINDOW.0,
            false
        ));
    }

    #[test]
    fn test_cloaked_window_excluded() {
        assert!(!passes_alt_tab_filter_mock(true, 10, 0, true));
    }

    #[test]
    fn test_normal_window_included() {
        assert!(passes_alt_tab_filter_mock(true, 10, 0, false));
    }
}
