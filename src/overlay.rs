use crate::accent_color::get_accent_color;
use crate::animation::{FadeAnimator, FADE_TIMER_ID};
use crate::dwm_thumbnails::{self, ThumbnailHandle};
use crate::grid_layout::CellRect;
use crate::grid_layout::{compute_grid, GridLayout};
use crate::monitor::MonitorInfo;
use crate::overlay_renderer::OverlayRenderer;
use crate::state::OverlayState;
use crate::window_info::WindowInfo;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, KillTimer, RegisterClassExW,
    SetForegroundWindow, SetLayeredWindowAttributes, SetWindowPos,
    ShowWindow, HMENU, LWA_ALPHA, LWA_COLORKEY, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE,
    WNDCLASSEXW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP, CS_HREDRAW, CS_VREDRAW,
};
use windows::Win32::Graphics::Gdi::InvalidateRect;
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
    /// A second overlay HWND that sits on top of the thumbnail overlay.
    /// Used exclusively for GDI-rendered letter badges (DWM thumbnails
    /// are composited above the first window's surface, so labels must
    /// live on a separate, higher-z window).
    pub label_hwnd: Option<HWND>,
    pub monitors: Vec<MonitorInfo>,
    pub animator: FadeAnimator,
    thumbnails: Vec<ThumbnailHandle>,
    /// Actual letterboxed thumbnail bounds per window (for badge positioning).
    pub thumbnail_bounds: Vec<CellRect>,
    pub grid_layout: Option<GridLayout>,
    pub area_width: f32,
    pub area_height: f32,
    /// Direct2D + DirectWrite renderer for the primary overlay HWND.
    /// Created lazily in `show()` the first time the overlay is displayed.
    renderer: Option<OverlayRenderer>,
    /// Snapshot of windows for the current overlay session (used in WM_PAINT).
    pub render_snapshot: Vec<WindowInfo>,
    /// Index of the currently selected window (used in WM_PAINT).
    pub render_selected: Option<usize>,
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
            label_hwnd: None,
            monitors: Vec::new(),
            animator: FadeAnimator::new(),
            thumbnails: Vec::new(),
            thumbnail_bounds: Vec::new(),
            grid_layout: None,
            area_width: 0.0,
            area_height: 0.0,
            renderer: None,
            render_snapshot: Vec::new(),
            render_selected: None,
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
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
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

            // Create a label overlay window on the primary monitor.
            // This sits ON TOP of the thumbnail overlay and uses a color-key
            // so the background is transparent — only the letter badges are visible.
            let label_class_name_str = "WindowSelectorLabelOverlay\0";
            let label_class: Vec<u16> = label_class_name_str.encode_utf16().collect();
            let label_wnd_name: Vec<u16> = "Label Overlay\0".encode_utf16().collect();

            let label_wc = WNDCLASSEXW {
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
                lpszClassName: PCWSTR(label_class.as_ptr()),
                hIconSm: windows::Win32::UI::WindowsAndMessaging::HICON::default(),
            };
            let _ = RegisterClassExW(&label_wc);

            let primary_rect = self.monitors[0].rect;
            let label_hwnd = CreateWindowExW(
                // Layered (for color-key transparency) + topmost + transparent (click-through)
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
                PCWSTR(label_class.as_ptr()),
                PCWSTR(label_wnd_name.as_ptr()),
                WS_POPUP,
                primary_rect.left,
                primary_rect.top,
                primary_rect.right - primary_rect.left,
                primary_rect.bottom - primary_rect.top,
                None,
                HMENU::default(),
                instance,
                None,
            )?;

            // Color key: RGB(1,1,1) is the transparent color.
            // Everything painted this color becomes invisible.
            let _ = SetLayeredWindowAttributes(
                label_hwnd,
                windows::Win32::Foundation::COLORREF(0x00010101),
                255,
                LWA_COLORKEY,
            );

            self.label_hwnd = Some(label_hwnd);
            tracing::debug!("Label overlay HWND {:?} created", label_hwnd);
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
        let reg = dwm_thumbnails::register_thumbnails(
            self.overlay_hwnds[0],
            windows,
            &grid.cells,
        );
        self.thumbnails = reg.handles;
        self.thumbnail_bounds = reg.thumb_bounds;
        self.grid_layout = Some(grid);

        // Store snapshot for rendering.
        self.render_snapshot = windows.to_vec();
        self.render_selected = None;

        // Initialize (or re-initialize) the Direct2D renderer on the primary HWND.
        // DPI scale: query from system; default to 1.0 on failure.
        let dpi_scale = unsafe {
            let dpi = GetDpiForWindow(self.overlay_hwnds[0]);
            if dpi == 0 { 1.0f32 } else { dpi as f32 / 96.0 }
        };
        let accent = get_accent_color();
        match OverlayRenderer::new(self.overlay_hwnds[0], dpi_scale, accent) {
            Ok(r) => {
                self.renderer = Some(r);
                tracing::debug!("OverlayRenderer initialized (dpi_scale={})", dpi_scale);
            }
            Err(e) => {
                tracing::error!("OverlayRenderer::new failed: {:?}", e);
                self.renderer = None;
            }
        }

        // Set state BEFORE ShowWindow so WM_PAINT sees Active state.
        *state = OverlayState::Active { selected: None };

        unsafe {
            // Show thumbnail overlays first.
            for &hwnd in &self.overlay_hwnds {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }

            // Take keyboard focus on the thumbnail overlay.
            let _ = SetForegroundWindow(self.overlay_hwnds[0]);

            // Show the label overlay LAST so it's on top of everything.
            if let Some(lhwnd) = self.label_hwnd {
                let _ = ShowWindow(lhwnd, SW_SHOWNOACTIVATE);
                // Explicitly place at the very top of the Z-order.
                let _ = SetWindowPos(
                    lhwnd,
                    windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST,
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
                let _ = InvalidateRect(lhwnd, None, true);
            }

            // Force repaint on thumbnail overlays.
            for &hwnd in &self.overlay_hwnds {
                let _ = InvalidateRect(hwnd, None, true);
            }
        }

        tracing::info!("Overlay show: immediate (no fade)");
    }

    /// Hide the overlay immediately (no fade animation).
    pub fn begin_hide(&mut self, state: &mut OverlayState, switch_target: Option<HWND>) {
        if self.overlay_hwnds.is_empty() {
            return;
        }

        // Skip fade-out — hide immediately.
        self.hide_windows();
        *state = OverlayState::Hidden;

        // Deactivate keyboard hook.
        crate::keyboard_hook::set_active(false);

        // Switch to target or restore previous foreground.
        if let Some(target) = switch_target {
            let _ = crate::window_switcher::switch_to_window(target);
        }

        tracing::info!("Overlay hidden immediately: target={:?}", switch_target);
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
                unsafe { let _ = KillTimer(hwnd, FADE_TIMER_ID); }
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
        self.thumbnail_bounds.clear();
        self.grid_layout = None;

        unsafe {
            for &hwnd in &self.overlay_hwnds {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            if let Some(lhwnd) = self.label_hwnd {
                let _ = ShowWindow(lhwnd, SW_HIDE);
            }
        }
        tracing::info!("Overlay hidden");
    }

    /// Update render state and trigger a repaint of the label overlay HWND.
    pub fn redraw(&mut self, windows: &[WindowInfo], selected: Option<usize>) {
        self.render_snapshot = windows.to_vec();
        self.render_selected = selected;
        // Invalidate the label overlay to trigger WM_PAINT with GDI rendering.
        if let Some(lhwnd) = self.label_hwnd {
            unsafe { let _ = InvalidateRect(lhwnd, None, true); }
        }
    }

    /// Render the current frame immediately using Direct2D.
    pub fn render_frame(&self) {
        if let Some(renderer) = &self.renderer {
            if let Some(layout) = &self.grid_layout {
                renderer.render(
                    &self.render_snapshot,
                    &layout.cells,
                    self.render_selected,
                    self.area_width,
                    self.area_height,
                );
            }
        }
        // ValidateRect acknowledges WM_PAINT (prevents a continuous stream of
        // WM_PAINT messages from DefWindowProcW).
        if let Some(&hwnd) = self.overlay_hwnds.first() {
            unsafe {
                let _ = windows::Win32::Graphics::Gdi::ValidateRect(hwnd, None);
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_primary_hwnd(&self) -> Option<HWND> {
        self.overlay_hwnds.first().copied()
    }

    pub fn all_hwnds(&self) -> &[HWND] {
        &self.overlay_hwnds
    }

    /// All HWNDs including the label overlay — used to exclude from enumeration.
    pub fn all_hwnds_including_labels(&self) -> Vec<HWND> {
        let mut v = self.overlay_hwnds.clone();
        if let Some(lhwnd) = self.label_hwnd {
            v.push(lhwnd);
        }
        v
    }
}

impl Default for OverlayManager {
    fn default() -> Self {
        Self::new()
    }
}
