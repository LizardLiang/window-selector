use crate::animation::{FadeAnimator, FADE_TIMER_ID, FADE_TIMER_INTERVAL_MS};
use crate::dwm_thumbnails::{self, ThumbnailHandle};
use crate::grid_layout::{compute_grid, GridLayout};
use crate::monitor::MonitorInfo;
use crate::state::OverlayState;
use crate::window_info::WindowInfo;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, KillTimer, RegisterClassExW, SetForegroundWindow,
    SetLayeredWindowAttributes, SetTimer, ShowWindow, HMENU,
    LWA_ALPHA, SW_HIDE, SW_SHOWNOACTIVATE, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP, CS_HREDRAW, CS_VREDRAW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::core::PCWSTR;

const OVERLAY_CLASS_NAME: &str = "WindowSelectorOverlay\0";
const OVERLAY_WINDOW_NAME: &str = "Window Selector Overlay\0";

// We declare the wndproc as extern — it will be defined in main.rs
extern "system" {
    // This resolves to overlay_wndproc defined as a pub(crate) fn in main.rs
}

// We use a function pointer so overlay.rs can set the wndproc.
// The actual overlay wndproc is set at window creation time by passing it directly.
type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

/// Global wndproc for overlay windows stored as an `AtomicUsize` (function pointer
/// cast to usize). Set once from the message pump thread before the first overlay
/// window is created. Read only from the same thread inside `overlay_class_wndproc`.
///
/// Using `AtomicUsize` avoids `static mut` while staying lock-free. The acquire/release
/// ordering is a conservative choice; in practice only one thread ever touches this.
static OVERLAY_WNDPROC_PTR: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

pub fn set_overlay_wndproc(proc: WndProc) {
    OVERLAY_WNDPROC_PTR.store(proc as usize, std::sync::atomic::Ordering::Relaxed);
}

unsafe extern "system" fn overlay_class_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = OVERLAY_WNDPROC_PTR.load(std::sync::atomic::Ordering::Relaxed);
    if ptr != 0 {
        // SAFETY: ptr was stored as a valid WndProc function pointer.
        let proc: WndProc = std::mem::transmute(ptr);
        proc(hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

/// Manages per-monitor overlay windows.
pub struct OverlayManager {
    /// One HWND per monitor (primary first).
    pub overlay_hwnds: Vec<HWND>,
    pub monitors: Vec<MonitorInfo>,
    pub animator: FadeAnimator,
    thumbnails: Vec<ThumbnailHandle>,
    grid_layout: Option<GridLayout>,
    pub area_width: f32,
    pub area_height: f32,
}

// OverlayManager contains HWND values (raw pointers internally). The type is not
// Send/Sync by default, which is correct: it must only be used on the Win32 message
// pump thread. We do NOT add unsafe impl Send/Sync here. The single-thread invariant
// is enforced by AppState, which is only accessed via APP_STATE_PTR on the message
// pump thread (see main.rs).

impl OverlayManager {
    pub fn new() -> Self {
        Self {
            overlay_hwnds: Vec::new(),
            monitors: Vec::new(),
            animator: FadeAnimator::new(),
            thumbnails: Vec::new(),
            grid_layout: None,
            area_width: 0.0,
            area_height: 0.0,
        }
    }

    /// Create one overlay HWND per monitor. Called once at startup.
    pub fn create_windows(
        &mut self,
        monitors: Vec<MonitorInfo>,
        wndproc: WndProc,
    ) -> windows::core::Result<()> {
        self.monitors = monitors;
        set_overlay_wndproc(wndproc);

        unsafe {
            let instance = GetModuleHandleW(PCWSTR::null())?;
            let class_name: Vec<u16> = OVERLAY_CLASS_NAME.encode_utf16().collect();
            let wnd_name: Vec<u16> = OVERLAY_WINDOW_NAME.encode_utf16().collect();

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(overlay_class_wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance.into(),
                hIcon: windows::Win32::UI::WindowsAndMessaging::HICON::default(),
                hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR::default(),
                hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH::default(),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                hIconSm: windows::Win32::UI::WindowsAndMessaging::HICON::default(),
            };

            // Ignore ALREADY_EXISTS — happens if called twice.
            let _ = RegisterClassExW(&wc);

            for monitor in &self.monitors {
                let rect = monitor.rect;
                let hwnd = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                    PCWSTR(class_name.as_ptr()),
                    PCWSTR(wnd_name.as_ptr()),
                    WS_POPUP,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    None,
                    HMENU::default(),
                    instance,
                    None,
                )?;

                // Start fully transparent.
                let _ = SetLayeredWindowAttributes(
                    hwnd,
                    windows::Win32::Foundation::COLORREF(0),
                    0,
                    LWA_ALPHA,
                );

                self.overlay_hwnds.push(hwnd);
                tracing::debug!("Overlay HWND {:?} created for {:?}", hwnd, rect);
            }
        }

        Ok(())
    }

    /// Show the overlay and begin fade-in.
    pub fn show(&mut self, windows: &[WindowInfo], state: &mut OverlayState) {
        if self.overlay_hwnds.is_empty() {
            tracing::error!("No overlay HWNDs");
            return;
        }

        if self.monitors.is_empty() {
            tracing::error!("No monitors — cannot show overlay");
            return;
        }

        let primary = &self.monitors[0];
        let w = (primary.rect.right - primary.rect.left) as f32;
        let h = (primary.rect.bottom - primary.rect.top) as f32;
        self.area_width = w;
        self.area_height = h;

        let grid = compute_grid(windows.len(), w, h);

        // Register DWM thumbnails on the primary overlay HWND.
        self.thumbnails = dwm_thumbnails::register_thumbnails(
            self.overlay_hwnds[0],
            windows,
            &grid.cells,
        );
        self.grid_layout = Some(grid);

        unsafe {
            // Show all overlays.
            for &hwnd in &self.overlay_hwnds {
                let _ = SetLayeredWindowAttributes(
                    hwnd,
                    windows::Win32::Foundation::COLORREF(0),
                    0,
                    LWA_ALPHA,
                );
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }

            // Start fade-in timer on primary HWND.
            self.animator.start_fade_in();
            SetTimer(
                self.overlay_hwnds[0],
                FADE_TIMER_ID,
                FADE_TIMER_INTERVAL_MS,
                None,
            );

            // Take keyboard focus.
            let _ = SetForegroundWindow(self.overlay_hwnds[0]);
        }

        *state = OverlayState::FadingIn;
        tracing::info!("Overlay show: fade-in started");
    }

    /// Begin fade-out.
    pub fn begin_hide(&mut self, state: &mut OverlayState, switch_target: Option<HWND>) {
        if self.overlay_hwnds.is_empty() {
            return;
        }

        unsafe {
            self.animator.start_fade_out();
            SetTimer(
                self.overlay_hwnds[0],
                FADE_TIMER_ID,
                FADE_TIMER_INTERVAL_MS,
                None,
            );
        }

        *state = OverlayState::FadingOut { switch_target };
        tracing::info!("Overlay begin_hide: target={:?}", switch_target);
    }

    /// Advance the fade animation by one tick.
    /// Returns true if the animation is complete.
    pub fn on_fade_timer(&mut self) -> bool {
        let still_running = self.animator.tick();
        let alpha = self.animator.current_alpha;

        unsafe {
            for &hwnd in &self.overlay_hwnds {
                let _ = SetLayeredWindowAttributes(
                    hwnd,
                    windows::Win32::Foundation::COLORREF(0),
                    alpha,
                    LWA_ALPHA,
                );
            }
        }

        if !still_running {
            if let Some(&hwnd) = self.overlay_hwnds.first() {
                unsafe { KillTimer(hwnd, FADE_TIMER_ID); }
            }
            true
        } else {
            false
        }
    }

    /// Fully hide and clean up overlay resources.
    pub fn hide_windows(&mut self) {
        // Unregister DWM thumbnails.
        self.thumbnails.clear();
        self.grid_layout = None;

        unsafe {
            for &hwnd in &self.overlay_hwnds {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
        tracing::info!("Overlay hidden");
    }

    /// Invalidate the primary overlay HWND for a repaint.
    pub fn redraw(&self, _windows: &[WindowInfo], _selected: Option<usize>) {
        if let Some(&hwnd) = self.overlay_hwnds.first() {
            unsafe {
                windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
            }
        }
    }

    pub fn get_primary_hwnd(&self) -> Option<HWND> {
        self.overlay_hwnds.first().copied()
    }

    pub fn all_hwnds(&self) -> &[HWND] {
        &self.overlay_hwnds
    }
}

impl Default for OverlayManager {
    fn default() -> Self {
        Self::new()
    }
}
