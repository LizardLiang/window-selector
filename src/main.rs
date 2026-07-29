#![windows_subsystem = "windows"]

mod about_dialog;
mod accent_color;
mod animation;
mod config;
mod dwm_thumbnails;
mod grid_layout;
mod hotkey;
mod icon;
mod interaction;
mod keyboard_hook;
mod keycodes;
mod letter_assignment;
mod logging;
mod monitor;
mod mru_tracker;
mod overlay;
mod overlay_renderer;
mod settings_panel;
mod settings_renderer;
mod startup;
mod state;
mod tray;
mod window_enumerator;
mod window_icon;
mod window_info;
mod window_mover;
mod window_switcher;

use config::AppConfig;
use interaction::{handle_key_down, KeyAction};
use keycodes::VK_SHIFT;
use monitor::get_all_monitors;
use mru_tracker::MruTracker;
use overlay::OverlayManager;
use state::{OverlayState, SessionTags};
use tray::{
    add_tray_icon, remove_tray_icon, show_balloon, MENU_ABOUT, MENU_DIRECT_SWITCH, MENU_EXIT,
    MENU_SETTINGS, WM_TRAY_CALLBACK,
};
use window_enumerator::{
    filter_occluded_for_label_mode, refresh_quick_tags, register_overlay_hwnds, snapshot_windows,
};
use window_switcher::{restore_focus, switch_to_window};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DrawTextW, EndPaint, FillRect,
    InvalidateRect, RoundRect, SelectObject, SetBkMode, SetTextColor, DT_CENTER, DT_SINGLELINE,
    DT_VCENTER, HDC, PAINTSTRUCT, PS_NULL, TRANSPARENT,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::DrawIconEx;
use windows::Win32::UI::WindowsAndMessaging::DI_NORMAL;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, PostQuitMessage, RegisterClassExW, SetWindowLongPtrW, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HMENU, HWND_MESSAGE, MSG, WA_INACTIVE, WM_ACTIVATE,
    WM_COMMAND, WM_CREATE, WM_DESTROY, WM_DISPLAYCHANGE, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_PAINT, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSEXW, WS_EX_TOOLWINDOW,
    WS_OVERLAPPEDWINDOW, WS_POPUP,
};

const MSG_WINDOW_CLASS: &str = "WindowSelectorMsgWnd\0";
const MSG_WINDOW_NAME: &str = "Window Selector\0";
/// Window class for the broadcast-receiver window.
///
/// `WM_DISPLAYCHANGE` (and other system broadcasts) are sent to top-level windows
/// only — they are NOT delivered to `HWND_MESSAGE` windows.  This separate class is
/// registered for a 0×0 `WS_POPUP` window whose sole purpose is to receive those
/// broadcasts and forward them to the application logic.
const BROADCAST_WINDOW_CLASS: &str = "WindowSelectorBroadcastWnd\0";

/// Application state owned by the single message pump thread.
#[allow(dead_code)]
pub(crate) struct AppState {
    pub(crate) config: AppConfig,
    pub(crate) config_dir: std::path::PathBuf,
    pub(crate) overlay_state: OverlayState,
    pub(crate) session_tags: SessionTags,
    pub(crate) mru_tracker: MruTracker,
    pub(crate) overlay_manager: OverlayManager,
    pub(crate) previous_foreground: Option<HWND>,
    pub(crate) window_snapshot: Vec<window_info::WindowInfo>,
    pub(crate) msg_hwnd: HWND,
    /// Hidden top-level 0×0 window whose only job is to receive system
    /// broadcast messages (e.g. `WM_DISPLAYCHANGE`).  `HWND_MESSAGE` windows
    /// are excluded from the broadcast list, so `msg_hwnd` never sees those
    /// messages.  This window uses `main_wndproc` and is kept alive for the
    /// lifetime of the message loop.
    pub(crate) broadcast_hwnd: HWND,
    pub(crate) settings_panel: settings_panel::SettingsPanelManager,
    /// Whether monitor-number badge chips should be drawn right now. Lives on
    /// `AppState` (rather than `OverlayManager`) because it is toggled by the
    /// keyboard hook's modifier callback, which already reaches `AppState` via
    /// `get_app_state()` — no existing code path threads a raw Shift signal
    /// through `OverlayManager` today. Only meaningful while `overlay_state`
    /// is `Active`; reset to `false` on every activate/dismiss so a stale
    /// `true` can never leak into the next session.
    pub(crate) shift_badges_visible: bool,
}

/// Global pointer to `AppState`, stored as an atomic integer so the static is safe
/// (`AtomicUsize` is `Send + Sync`).
///
/// SAFETY invariant: only the Win32 message pump thread reads or writes this value.
/// All Win32 callbacks (`WndProc`, WinEvent hooks) are dispatched on the thread that
/// called `GetMessageW`, so there is never concurrent access. The atomic is used
/// purely to avoid `static mut`, not for cross-thread synchronization.
static APP_STATE_PTR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Returns the current `AppState` pointer (may be null before init or after cleanup).
#[inline]
fn get_app_state() -> *mut AppState {
    APP_STATE_PTR.load(std::sync::atomic::Ordering::Relaxed) as *mut AppState
}

/// Public version of `get_app_state` for use by settings_panel.rs.
/// SAFETY: same invariant as `get_app_state` — only called from the message pump thread.
pub(crate) fn get_app_state_pub() -> *mut AppState {
    get_app_state()
}

/// Set (or clear) the `AppState` pointer. Must only be called from the message pump
/// thread.
#[inline]
fn set_app_state(ptr: *mut AppState) {
    APP_STATE_PTR.store(ptr as usize, std::sync::atomic::Ordering::Relaxed);
}

fn main() {
    // --- Single-instance guard ---
    // A named mutex ensures only one copy of the app runs at a time.
    // CreateMutexW creates-or-opens the mutex.  If another process already
    // owns it, GetLastError returns ERROR_ALREADY_EXISTS and we exit cleanly.
    // The mutex handle is kept alive in `_mutex` for the entire duration of
    // main(); Windows releases it automatically when the process exits.
    let _mutex = unsafe {
        match CreateMutexW(
            None, // default security
            true, // this process claims ownership immediately
            w!("Global\\window-selector-single-instance"),
        ) {
            Ok(handle) => {
                // CreateMutexW succeeded.  Check whether we opened an existing
                // mutex (another instance is already running).
                if windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS {
                    // Another instance is already running — exit silently.
                    // `handle` is dropped here, which closes it.
                    return;
                }
                handle
            }
            Err(_) => {
                // Mutex creation failed entirely (e.g., access denied).
                // Proceed anyway — single-instance guard is best-effort.
                // Logging is not yet initialized, so we cannot log here.
                windows::Win32::Foundation::HANDLE::default()
            }
        }
    };

    // Check for --debug flag: allocate a console window so logs are visible in real-time.
    let debug_mode = std::env::args().any(|a| a == "--debug");
    if debug_mode {
        unsafe {
            use windows::Win32::System::Console::AllocConsole;
            let _ = AllocConsole();
        }
    }

    // Set per-monitor DPI awareness.
    unsafe {
        use windows::Win32::UI::HiDpi::{
            SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        };
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    // Initialize COM on the message pump thread.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    // Determine config directory.
    let config_dir =
        AppConfig::default_config_dir().unwrap_or_else(|| std::path::PathBuf::from("./config"));

    // Initialize logging.
    let logs_dir = config_dir.join("logs");
    if let Err(e) = logging::init_logging(&logs_dir, debug_mode) {
        eprintln!("Logging init failed: {} - main.rs:162", e);
    }

    tracing::info!("Window Selector starting up (debug_mode={})", debug_mode);

    let config = match AppConfig::load(&config_dir) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Config load failed: {}", e);
            AppConfig::default()
        }
    };

    unsafe {
        run_message_loop(config, config_dir);
    }

    tracing::info!("Window Selector exiting");
}

unsafe fn run_message_loop(config: AppConfig, config_dir: std::path::PathBuf) {
    let instance = match GetModuleHandleW(PCWSTR::null()) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("GetModuleHandleW failed: {:?}", e);
            return;
        }
    };

    let class_name: Vec<u16> = MSG_WINDOW_CLASS.encode_utf16().collect();
    let wnd_name: Vec<u16> = MSG_WINDOW_NAME.encode_utf16().collect();

    // Load application icon from embedded resource.
    let app_icon = icon::load_app_icon().unwrap_or_default();

    // Register message-only window class.
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(main_wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance.into(),
        hIcon: app_icon,
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR::default(),
        hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        hIconSm: app_icon,
    };

    if RegisterClassExW(&wc) == 0 {
        tracing::error!("RegisterClassExW failed");
        return;
    }

    // Create the hidden message-only window.
    let msg_hwnd = match CreateWindowExW(
        WS_EX_TOOLWINDOW,
        PCWSTR(class_name.as_ptr()),
        PCWSTR(wnd_name.as_ptr()),
        WS_OVERLAPPEDWINDOW,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        HMENU::default(),
        instance,
        None,
    ) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("CreateWindowExW (msg window) failed: {:?}", e);
            return;
        }
    };

    tracing::info!("Message window HWND={:?}", msg_hwnd);

    // Create a dedicated top-level broadcast-receiver window.
    //
    // WHY: `HWND_MESSAGE` windows are excluded from the Windows broadcast
    // list, so `msg_hwnd` (created above with `HWND_MESSAGE` as parent) never
    // receives `WM_DISPLAYCHANGE` or other system-wide broadcast messages.
    // A plain `WS_POPUP` top-level window with no parent receives all
    // broadcasts.  This 0×0 invisible window handles those messages and
    // dispatches them through `main_wndproc` just like `msg_hwnd` does.
    let broadcast_class_name: Vec<u16> = BROADCAST_WINDOW_CLASS.encode_utf16().collect();
    let broadcast_wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: windows::Win32::UI::WindowsAndMessaging::WNDCLASS_STYLES(0),
        lpfnWndProc: Some(main_wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance.into(),
        hIcon: windows::Win32::UI::WindowsAndMessaging::HICON::default(),
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR::default(),
        hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: PCWSTR(broadcast_class_name.as_ptr()),
        hIconSm: windows::Win32::UI::WindowsAndMessaging::HICON::default(),
    };
    if RegisterClassExW(&broadcast_wc) == 0 {
        tracing::warn!("RegisterClassExW (broadcast window) failed — WM_DISPLAYCHANGE will not work");
    }
    let broadcast_hwnd = match CreateWindowExW(
        WS_EX_TOOLWINDOW,
        PCWSTR(broadcast_class_name.as_ptr()),
        PCWSTR::null(),
        WS_POPUP,
        0,
        0,
        0,
        0,
        None, // no parent — must be a real top-level window to receive broadcasts
        HMENU::default(),
        instance,
        None,
    ) {
        Ok(h) => {
            tracing::info!("Broadcast-receiver window HWND={:?}", h);
            h
        }
        Err(e) => {
            tracing::warn!("CreateWindowExW (broadcast window) failed: {:?} — WM_DISPLAYCHANGE will not work", e);
            HWND::default()
        }
    };

    // Initialize AppState on the heap so we can take a stable pointer.
    let mut app_state = Box::new(AppState {
        config: config.clone(),
        config_dir,
        overlay_state: OverlayState::Hidden,
        session_tags: SessionTags::new(),
        mru_tracker: MruTracker::new(),
        overlay_manager: OverlayManager::new(),
        previous_foreground: None,
        window_snapshot: Vec::new(),
        msg_hwnd,
        broadcast_hwnd,
        settings_panel: settings_panel::SettingsPanelManager::new(),
        shift_badges_visible: false,
    });

    // Set global pointer — valid for the lifetime of the message loop.
    let app_state_ptr = app_state.as_mut() as *mut AppState;
    set_app_state(app_state_ptr);
    SetWindowLongPtrW(msg_hwnd, GWLP_USERDATA, app_state_ptr as isize);

    // Sync launch_at_startup from registry on startup so the toggle shows the correct state.
    (*app_state_ptr).config.launch_at_startup = startup::get_launch_at_startup();

    // Install MRU tracker.
    mru_tracker::set_global_mru_tracker(&mut (*app_state_ptr).mru_tracker as *mut MruTracker);
    (*app_state_ptr).mru_tracker.install_hook();

    // Create overlay windows.
    let monitors = get_all_monitors();
    if monitors.is_empty() {
        tracing::warn!("No monitors detected");
    }
    if let Err(e) = (*app_state_ptr)
        .overlay_manager
        .create_windows(monitors, overlay_wndproc)
    {
        tracing::error!("Overlay window creation failed: {:?}", e);
    }

    // Register overlay HWNDs (including label overlay) to be excluded from window enumeration.
    let overlay_hwnds = (*app_state_ptr)
        .overlay_manager
        .all_hwnds_including_labels();
    register_overlay_hwnds(overlay_hwnds);

    // Add tray icon.
    if let Err(e) = add_tray_icon(msg_hwnd) {
        tracing::error!("Tray icon failed: {:?}", e);
    }

    // Register global hotkey (main overlay).
    let mod_flags = config.hotkey_modifiers;
    let vk = config.hotkey_vk;
    match hotkey::register_hotkey(msg_hwnd, mod_flags, vk) {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("RegisterHotKey failed: {:?}", e);
            let ks = hotkey::format_hotkey(mod_flags, vk);
            show_balloon(
                msg_hwnd,
                "Hotkey Conflict",
                &format!(
                    "The shortcut {} is already in use. Right-click the tray icon → Settings to change it.",
                    ks
                ),
            );
        }
    }

    // Register label mode hotkey.
    let label_mod_flags = config.label_hotkey_modifiers;
    let label_vk = config.label_hotkey_vk;
    match hotkey::register_label_hotkey(msg_hwnd, label_mod_flags, label_vk) {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("RegisterLabelHotKey failed: {:?}", e);
            let ks = hotkey::format_hotkey(label_mod_flags, label_vk);
            show_balloon(
                msg_hwnd,
                "Label Hotkey Conflict",
                &format!("The label mode shortcut {} is already in use.", ks),
            );
        }
    }

    // Install the low-level keyboard hook.  The hook starts inactive; it is
    // enabled in activate_overlay() and disabled when the overlay hides.
    keyboard_hook::install(keyboard_hook_handler);
    keyboard_hook::install_modifier_handler(shift_modifier_handler);

    tracing::info!("Entering message loop");

    // Standard Win32 message loop.
    let mut msg = MSG::default();
    loop {
        let r = GetMessageW(&mut msg, None, 0, 0);
        if r.0 <= 0 {
            break;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    // Cleanup.
    hotkey::unregister_hotkey(msg_hwnd);
    hotkey::unregister_label_hotkey(msg_hwnd);
    remove_tray_icon(msg_hwnd);
    (*app_state_ptr).mru_tracker.uninstall_hook();
    keyboard_hook::uninstall();

    set_app_state(std::ptr::null_mut());
    tracing::info!("Message loop exited, cleanup complete");

    // Drop AppState
    drop(app_state);
}

/// Main window procedure for the message-only window.
unsafe extern "system" fn main_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let app_ptr = get_app_state();
    if app_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let app = &mut *app_ptr;

    match msg {
        WM_CREATE => LRESULT(0),

        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }

        WM_HOTKEY => {
            // Guard: if settings panel is open, ignore overlay hotkeys (UI-6).
            if app.settings_panel.is_open() {
                return LRESULT(0);
            }
            let hotkey_id = wparam.0 as i32;
            if hotkey_id == hotkey::HOTKEY_ID {
                handle_hotkey(app);
            } else if hotkey_id == hotkey::HOTKEY_ID_LABEL {
                handle_label_hotkey(app);
            }
            LRESULT(0)
        }

        WM_TRAY_CALLBACK => {
            handle_tray_event(app, hwnd, lparam);
            LRESULT(0)
        }

        WM_COMMAND => {
            let cmd = (wparam.0 & 0xFFFF) as u32;
            handle_menu_command(app, hwnd, cmd);
            LRESULT(0)
        }

        WM_TIMER => {
            if wparam.0 == animation::FADE_TIMER_ID {
                handle_fade_timer(app);
            }
            LRESULT(0)
        }

        WM_DISPLAYCHANGE => {
            handle_display_change(app);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Overlay window procedure — handles keyboard input and animation on overlay HWNDs.
unsafe extern "system" fn overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            handle_overlay_key(wparam.0 as u32);
            LRESULT(0)
        }

        WM_PAINT => {
            let app_ptr = get_app_state();
            if app_ptr.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let app = &mut *app_ptr;

            // Only the label overlay HWND (primary monitor) and secondary monitor
            // thumbnail overlays get custom GDI content. The primary thumbnail
            // overlay (overlay_hwnds[0]) is painted by the Direct2D renderer instead.
            if app.overlay_manager.label_hwnd != Some(hwnd) {
                if matches!(app.overlay_state, OverlayState::Active { .. }) {
                    if let Some(idx) = app
                        .overlay_manager
                        .overlay_hwnds
                        .iter()
                        .position(|&h| h == hwnd)
                    {
                        if idx > 0 {
                            paint_secondary_monitor_overlay(
                                hwnd,
                                idx + 1,
                                app.shift_badges_visible,
                            );
                            return LRESULT(0);
                        }
                    }
                }
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }

            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            if !matches!(app.overlay_state, OverlayState::Active { .. }) {
                let _ = EndPaint(hwnd, &ps);
                return LRESULT(0);
            }

            // Fill entire window with color-key color (RGB 1,1,1) → transparent.
            let key_brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00010101));
            FillRect(hdc, &ps.rcPaint, key_brush);
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(key_brush);

            // Primary monitor's number badge ("1"), styled to match secondary screens.
            // Only drawn while Shift is physically held.
            if app.shift_badges_visible {
                draw_monitor_badge_chip(hdc, 1);
            }

            // Draw letter badges and number tags, positioned on the actual thumbnail bounds.
            if let Some(layout) = &app.overlay_manager.grid_layout {
                let thumb_bounds = &app.overlay_manager.thumbnail_bounds;
                let badge_w: i32 = 36;
                let badge_h: i32 = 30;
                let badge_margin: i32 = 8;

                // Letter badge font — Segoe UI Bold, large
                let font = CreateFontW(
                    22,
                    0,
                    0,
                    0,
                    700, // height=22, bold
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    windows::core::w!("Segoe UI"),
                );
                let old_font = SelectObject(hdc, font);
                SetBkMode(hdc, TRANSPARENT);

                // Null pen so RoundRect doesn't draw an outline stroke
                let null_pen = CreatePen(PS_NULL, 0, windows::Win32::Foundation::COLORREF(0));
                let old_pen = SelectObject(hdc, null_pen);

                let corner_sz: i32 = 10; // rounded corner radius

                for (i, cell) in layout.cells.iter().enumerate() {
                    if i >= app.window_snapshot.len() {
                        break;
                    }
                    let win = &app.window_snapshot[i];
                    let is_selected = app.overlay_manager.render_selected == Some(i);

                    // Use actual thumbnail bounds if available, fall back to cell
                    let tb = if i < thumb_bounds.len() {
                        &thumb_bounds[i]
                    } else {
                        cell
                    };

                    // Letter badge — bottom-right of the actual thumbnail
                    let badge_rect = RECT {
                        left: (tb.x + tb.width) as i32 - badge_w - badge_margin,
                        top: (tb.y + tb.height) as i32 - badge_h - badge_margin,
                        right: (tb.x + tb.width) as i32 - badge_margin,
                        bottom: (tb.y + tb.height) as i32 - badge_margin,
                    };

                    // Aura glow behind badge — soft rounded halo
                    let glow_expand: i32 = if is_selected { 4 } else { 2 };
                    let glow_brush =
                        CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00221100));
                    let old_glow = SelectObject(hdc, glow_brush);
                    let _ = RoundRect(
                        hdc,
                        badge_rect.left - glow_expand,
                        badge_rect.top - glow_expand,
                        badge_rect.right + glow_expand,
                        badge_rect.bottom + glow_expand,
                        corner_sz + glow_expand,
                        corner_sz + glow_expand,
                    );
                    SelectObject(hdc, old_glow);
                    let _ = windows::Win32::Graphics::Gdi::DeleteObject(glow_brush);

                    // Badge fill — rounded rect
                    let badge_brush = if is_selected {
                        CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00FF8800))
                    } else {
                        CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00CC6600))
                    };
                    let old_badge = SelectObject(hdc, badge_brush);
                    let _ = RoundRect(
                        hdc,
                        badge_rect.left,
                        badge_rect.top,
                        badge_rect.right,
                        badge_rect.bottom,
                        corner_sz,
                        corner_sz,
                    );
                    SelectObject(hdc, old_badge);
                    let _ = windows::Win32::Graphics::Gdi::DeleteObject(badge_brush);

                    if let Some(letter) = win.letter {
                        SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));
                        let letter_upper = letter.to_uppercase().to_string();
                        let mut wtext: Vec<u16> = letter_upper.encode_utf16().collect();
                        let mut letter_rect = badge_rect;
                        DrawTextW(
                            hdc,
                            &mut wtext,
                            &mut letter_rect,
                            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
                        );
                    }

                    // Application icon — top-left of the thumbnail, 32×32 px.
                    // The icon was fetched once at snapshot time and cached in win.icon,
                    // so no IPC call is needed here on every repaint.
                    let icon_size: i32 = 32;
                    let icon_margin: i32 = 8;
                    let icon_x = tb.x as i32 + icon_margin;
                    let icon_y = tb.y as i32 + icon_margin;
                    if let Some(hicon) = win.icon {
                        // DrawIconEx renders an HICON into a GDI HDC at any size.
                        // DI_NORMAL = draw the icon with its mask (transparency respected).
                        let _ = DrawIconEx(
                            hdc, icon_x, icon_y, hicon, icon_size, icon_size, 0, None, DI_NORMAL,
                        );
                    }

                    // Number tag badge — top-right, fully rounded (circle)
                    if let Some(tag) = win.number_tag {
                        let tag_sz: i32 = 20;
                        let tag_margin: i32 = 6;
                        let tag_brush = CreateSolidBrush(
                            windows::Win32::Foundation::COLORREF(0x0018BFF0), // amber
                        );
                        let old_tag = SelectObject(hdc, tag_brush);
                        let _ = RoundRect(
                            hdc,
                            (tb.x + tb.width) as i32 - tag_sz - tag_margin,
                            tb.y as i32 + tag_margin,
                            (tb.x + tb.width) as i32 - tag_margin,
                            tb.y as i32 + tag_margin + tag_sz,
                            tag_sz,
                            tag_sz,
                        );
                        SelectObject(hdc, old_tag);
                        let _ = windows::Win32::Graphics::Gdi::DeleteObject(tag_brush);

                        SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00101010));
                        let mut tag_text: Vec<u16> = tag.to_string().encode_utf16().collect();
                        let mut tag_text_rect = RECT {
                            left: (tb.x + tb.width) as i32 - tag_sz - tag_margin,
                            top: tb.y as i32 + tag_margin,
                            right: (tb.x + tb.width) as i32 - tag_margin,
                            bottom: tb.y as i32 + tag_margin + tag_sz,
                        };
                        DrawTextW(
                            hdc,
                            &mut tag_text,
                            &mut tag_text_rect,
                            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
                        );
                    }
                }

                SelectObject(hdc, old_pen);
                let _ = windows::Win32::Graphics::Gdi::DeleteObject(null_pen);
                SelectObject(hdc, old_font);
                let _ = windows::Win32::Graphics::Gdi::DeleteObject(font);
            }

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        WM_ACTIVATE => {
            let activation_state = (wparam.0 & 0xFFFF) as u16;
            if activation_state == WA_INACTIVE as u16 {
                let app_ptr = get_app_state();
                if !app_ptr.is_null() {
                    let app = &mut *app_ptr;
                    if matches!(app.overlay_state, OverlayState::Active { .. }) {
                        tracing::info!("Overlay lost focus → auto-dismiss");
                        dismiss_overlay(app);
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
            let app_ptr = get_app_state();
            if !app_ptr.is_null() {
                let app = &mut *app_ptr;
                if matches!(app.overlay_state, OverlayState::Active { .. }) {
                    dismiss_overlay(app);
                }
            }
            LRESULT(0)
        }

        WM_TIMER => {
            let app_ptr = get_app_state();
            if !app_ptr.is_null() && wparam.0 == animation::FADE_TIMER_ID {
                let app = &mut *app_ptr;
                handle_fade_timer(app);
            }
            LRESULT(0)
        }

        WM_DESTROY => LRESULT(0),

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Paint a secondary monitor's thumbnail overlay: a dark backdrop (matching the
/// primary monitor's Direct2D backdrop color) plus its monitor-number badge.
/// Secondary overlay HWNDs have no Direct2D renderer bound (only overlay_hwnds[0]
/// does), so this GDI path is their only visible content while the overlay is Active.
unsafe fn paint_secondary_monitor_overlay(hwnd: HWND, monitor_number: usize, show_badge: bool) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    // Dark backdrop approximating the primary monitor's D2D backdrop color
    // (RGB ~5,8,15 in overlay_renderer.rs) so secondary screens read consistently.
    // Filled unconditionally — the dimmed appearance stays even when the
    // monitor-number badge itself is hidden (Shift not held).
    let backdrop_brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x000F0805));
    FillRect(hdc, &ps.rcPaint, backdrop_brush);
    let _ = windows::Win32::Graphics::Gdi::DeleteObject(backdrop_brush);

    if show_badge {
        draw_monitor_badge_chip(hdc, monitor_number);
    }

    let _ = EndPaint(hwnd, &ps);
}

/// Draw a monitor-number chip ("1", "2", ...) in the top-left corner of `hdc`.
/// Shared by the primary monitor's label overlay paint path and each secondary
/// monitor's own WM_PAINT branch so the two look identical.
unsafe fn draw_monitor_badge_chip(hdc: HDC, monitor_number: usize) {
    let margin: i32 = 40;
    let chip_w: i32 = 110;
    let chip_h: i32 = 96;
    let corner_sz: i32 = 20;

    let chip_rect = RECT {
        left: margin,
        top: margin,
        right: margin + chip_w,
        bottom: margin + chip_h,
    };

    let font = CreateFontW(
        72,
        0,
        0,
        0,
        700, // bold
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        windows::core::w!("Segoe UI"),
    );
    let old_font = SelectObject(hdc, font);
    SetBkMode(hdc, TRANSPARENT);

    // Null pen so RoundRect doesn't draw an outline stroke.
    let null_pen = CreatePen(PS_NULL, 0, windows::Win32::Foundation::COLORREF(0));
    let old_pen = SelectObject(hdc, null_pen);

    // Same accent-orange fill as the letter badges, white text on top.
    let chip_brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00CC6600));
    let old_brush = SelectObject(hdc, chip_brush);
    let _ = RoundRect(
        hdc,
        chip_rect.left,
        chip_rect.top,
        chip_rect.right,
        chip_rect.bottom,
        corner_sz,
        corner_sz,
    );
    SelectObject(hdc, old_brush);
    let _ = windows::Win32::Graphics::Gdi::DeleteObject(chip_brush);

    SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));
    let mut text: Vec<u16> = monitor_number.to_string().encode_utf16().collect();
    let mut text_rect = chip_rect;
    DrawTextW(
        hdc,
        &mut text,
        &mut text_rect,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER,
    );

    SelectObject(hdc, old_pen);
    let _ = windows::Win32::Graphics::Gdi::DeleteObject(null_pen);
    SelectObject(hdc, old_font);
    let _ = windows::Win32::Graphics::Gdi::DeleteObject(font);
}

unsafe fn handle_hotkey(app: &mut AppState) {
    tracing::debug!("WM_HOTKEY received");

    match &app.overlay_state {
        OverlayState::Hidden => activate_overlay(app),
        OverlayState::Active { .. } => {
            dismiss_overlay(app);
        }
        _ => {}
    }
}

unsafe fn activate_overlay(app: &mut AppState) {
    app.previous_foreground = {
        let hw = GetForegroundWindow();
        if hw.is_invalid() {
            None
        } else {
            Some(hw)
        }
    };

    app.session_tags.release_closed();

    let mon_clone = app.overlay_manager.monitors.clone();
    app.window_snapshot = snapshot_windows(
        app.overlay_manager.all_hwnds(),
        &mon_clone,
        &app.mru_tracker,
    );
    refresh_quick_tags(&mut app.window_snapshot, &app.config.quick_tags, &mut app.session_tags);

    tracing::info!("Activating overlay: {} windows", app.window_snapshot.len());

    let snap = app.window_snapshot.clone();
    let render_config = overlay_renderer::RenderConfig {
        label_font_size: app.config.label_font_size,
        title_font_size: app.config.title_font_size,
        background_opacity: app.config.background_opacity,
    };
    app.overlay_manager.show(
        &snap,
        &mut app.overlay_state,
        app.config.overlay_opacity,
        app.config.grid_padding,
        render_config,
    );

    // Activate the keyboard hook so key presses reach the overlay regardless
    // of whether SetForegroundWindow succeeded.
    keyboard_hook::set_active(true);

    // Seed monitor-badge visibility from the Shift key's current physical
    // state, so badges are already showing if the user opened the overlay
    // with Shift already held down.
    app.shift_badges_visible = GetAsyncKeyState(VK_SHIFT as i32) < 0;
}

/// Dismiss the overlay without switching to any window; restore previous foreground.
unsafe fn dismiss_overlay(app: &mut AppState) {
    // Reset so a Shift key still physically held (e.g. Shift+1 moved a window
    // and dismissed the overlay) can't leak a stale `true` into the next session.
    app.shift_badges_visible = false;
    let prev = app.previous_foreground.take();
    app.overlay_manager.begin_hide(&mut app.overlay_state, None);
    if let Some(prev) = prev {
        let _ = restore_focus(prev);
    }
}

/// Handle label mode hotkey (Win+O).
unsafe fn handle_label_hotkey(app: &mut AppState) {
    tracing::debug!("Label mode hotkey received");

    match &app.overlay_state {
        OverlayState::Hidden => activate_label_mode(app),
        OverlayState::LabelMode { .. } => {
            dismiss_label_mode(app);
        }
        _ => {}
    }
}

/// Activate label mode — show labels on overlay.
unsafe fn activate_label_mode(app: &mut AppState) {
    app.previous_foreground = {
        let hw = GetForegroundWindow();
        if hw.is_invalid() {
            None
        } else {
            Some(hw)
        }
    };

    app.session_tags.release_closed();

    let mon_clone = app.overlay_manager.monitors.clone();
    let mut raw_snapshot = snapshot_windows(
        app.overlay_manager.all_hwnds(),
        &mon_clone,
        &app.mru_tracker,
    );
    refresh_quick_tags(&mut raw_snapshot, &app.config.quick_tags, &mut app.session_tags);

    // Filter out windows that are fully occluded by higher-Z-order windows.
    // These would only add invisible labels that confuse the user.
    app.window_snapshot = filter_occluded_for_label_mode(raw_snapshot);

    tracing::info!(
        "Activating label mode: {} windows",
        app.window_snapshot.len()
    );

    // Show overlay in label mode (transparent background with labels only)
    let snap = app.window_snapshot.clone();
    let render_config = overlay_renderer::RenderConfig {
        label_font_size: app.config.label_font_size,
        title_font_size: app.config.title_font_size,
        background_opacity: app.config.background_opacity,
    };
    app.overlay_manager.label_overlap_strategy = app.config.label_overlap_strategy;
    app.overlay_manager
        .show_label_mode(&snap, &mut app.overlay_state, render_config);

    // Activate the keyboard hook
    keyboard_hook::set_active(true);

    // Label mode never shows monitor badges; keep the flag clean regardless.
    app.shift_badges_visible = false;
}

/// Dismiss label mode without switching to any window.
unsafe fn dismiss_label_mode(app: &mut AppState) {
    app.shift_badges_visible = false;
    let prev = app.previous_foreground.take();
    app.overlay_manager.begin_hide(&mut app.overlay_state, None);
    if let Some(prev) = prev {
        let _ = restore_focus(prev);
    }
}

unsafe fn handle_tray_event(app: &mut AppState, hwnd: HWND, lparam: LPARAM) {
    let event = (lparam.0 & 0xFFFF) as u32;
    // WM_RBUTTONUP
    if event == 0x0205 {
        let cmd = tray::show_context_menu(hwnd, app.config.direct_switch);
        handle_menu_command(app, hwnd, cmd);
    }
}

unsafe fn handle_menu_command(app: &mut AppState, hwnd: HWND, cmd: u32) {
    match cmd {
        MENU_DIRECT_SWITCH => {
            app.config.direct_switch = !app.config.direct_switch;
            tracing::info!("Direct switch toggled to {}", app.config.direct_switch);
            if let Err(e) = AppConfig::save(&app.config_dir, &app.config) {
                tracing::error!("Failed to save config: {}", e);
            }
        }
        MENU_SETTINGS => {
            app.settings_panel.open(hwnd);
        }
        MENU_ABOUT => {
            about_dialog::show_about(hwnd);
        }
        MENU_EXIT => {
            tracing::info!("Exit selected from tray menu");
            let _ = DestroyWindow(hwnd);
        }
        _ => {}
    }
}

/// Low-level keyboard hook handler.
/// Called by `keyboard_hook::ll_keyboard_proc` on every key-down event while the
/// overlay is active. Dispatches to `handle_overlay_key` and returns true to
/// swallow the keystroke (prevent it from reaching the application below).
fn keyboard_hook_handler(vk_code: u32) -> bool {
    unsafe { handle_overlay_key(vk_code) };

    // Don't swallow modifier keys — let them pass through so
    // GetAsyncKeyState can see Ctrl/Shift/Alt state for Ctrl+Number tagging.
    match vk_code {
        0xA0..=0xA5 | // VK_LSHIFT, VK_RSHIFT, VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU
        0x10..=0x12   // VK_SHIFT, VK_CONTROL, VK_MENU (generic)
        => false, // pass through
        _ => true, // swallow
    }
}

/// Low-level keyboard hook's Shift-modifier callback (see `keyboard_hook::ModifierHandler`).
/// Fired on every Shift press/release while the hook is active — including
/// auto-repeat keydowns from a held key, so `set_shift_badges_visible` must
/// no-op when the flag hasn't actually changed to avoid repainting on every
/// repeat tick.
fn shift_modifier_handler(shift_down: bool) {
    unsafe { set_shift_badges_visible(shift_down) };
}

/// Pure decision extracted from `set_shift_badges_visible`: should the flag be
/// updated (and the overlay repainted) for this Shift transition? Only `true`
/// when the overlay is `Active` AND the incoming value actually differs from
/// the current one — this is what suppresses repeated repaints while Shift is
/// held down and auto-repeat keydowns keep reporting `shift_down: true`.
fn should_update_shift_badges(overlay_state: &OverlayState, current: bool, new_value: bool) -> bool {
    matches!(overlay_state, OverlayState::Active { .. }) && current != new_value
}

/// Update `shift_badges_visible` and repaint the overlay if the flag actually
/// changed. Only takes effect while `overlay_state` is `Active` — the flag is
/// otherwise irrelevant (label mode never shows badges; `Hidden`/fading states
/// have nothing to repaint).
unsafe fn set_shift_badges_visible(visible: bool) {
    let app_ptr = get_app_state();
    if app_ptr.is_null() {
        return;
    }
    let app = &mut *app_ptr;

    if !should_update_shift_badges(&app.overlay_state, app.shift_badges_visible, visible) {
        return;
    }
    app.shift_badges_visible = visible;

    for &hwnd in &app.overlay_manager.overlay_hwnds {
        let _ = InvalidateRect(hwnd, None, true);
    }
    if let Some(label_hwnd) = app.overlay_manager.label_hwnd {
        let _ = InvalidateRect(label_hwnd, None, true);
    }
}

unsafe fn handle_overlay_key(vk_code: u32) {
    let app_ptr = get_app_state();
    if app_ptr.is_null() {
        return;
    }
    let app = &mut *app_ptr;

    // Check if we're in label mode
    let is_label_mode = matches!(app.overlay_state, OverlayState::LabelMode { .. });

    tracing::debug!(
        "Key pressed: vk={} (0x{:X}), label_mode={}, windows={}",
        vk_code,
        vk_code,
        is_label_mode,
        app.window_snapshot.len()
    );

    let action = handle_key_down(
        vk_code,
        &app.overlay_state,
        &app.window_snapshot,
        &mut app.session_tags,
        app.config.direct_switch,
        app.overlay_manager.monitors.len(),
    );

    tracing::debug!("Key action: {:?}", action);

    match action {
        KeyAction::None => {}
        KeyAction::Select(idx) => {
            if is_label_mode {
                // Update label mode selection
                app.overlay_state = OverlayState::LabelMode {
                    selected: Some(idx),
                };
                app.overlay_manager.redraw(&app.window_snapshot, Some(idx));
            } else {
                // Update overlay selection
                app.overlay_state = OverlayState::Active {
                    selected: Some(idx),
                };
                let snap = app.window_snapshot.clone();
                app.overlay_manager.redraw(&snap, Some(idx));
            }
        }
        KeyAction::SwitchTo(target) => {
            // Both modes use the same hide mechanism
            app.overlay_manager
                .begin_hide(&mut app.overlay_state, Some(target));
            app.previous_foreground = None;
        }
        KeyAction::MoveToMonitor {
            hwnd: target,
            monitor_index,
        } => {
            // Clone the MonitorInfo out first to avoid borrowing app.overlay_manager
            // both immutably (for the move) and mutably (for begin_hide below).
            if let Some(monitor) = app.overlay_manager.monitors.get(monitor_index).cloned() {
                let moved = window_mover::move_to_monitor_center(target, &monitor);
                if !moved {
                    tracing::warn!(
                        "MoveToMonitor: window {:?} was not confirmed centered on monitor {}",
                        target,
                        monitor_index
                    );
                }
            } else {
                tracing::warn!(
                    "MoveToMonitor: monitor_index {} out of range",
                    monitor_index
                );
            }
            // Same tail as SwitchTo — the overlay is topmost, so the reposition
            // is invisible until the fade reveals the window. The switch itself
            // happens inside handle_fade_timer once state leaves Active (see 7bb2660).
            app.overlay_manager
                .begin_hide(&mut app.overlay_state, Some(target));
            app.previous_foreground = None;
        }
        KeyAction::Dismiss => {
            if is_label_mode {
                dismiss_label_mode(app);
            } else {
                dismiss_overlay(app);
            }
        }
        KeyAction::TagAssigned { number, hwnd } => {
            if let Some(exe_path) = app
                .window_snapshot
                .iter()
                .find(|window| window.hwnd == hwnd)
                .and_then(|window| window.exe_path.clone())
            {
                app.config.set_quick_tag(number, exe_path);
                if let Err(e) = AppConfig::save(&app.config_dir, &app.config) {
                    tracing::error!("Failed to save config after quick tag assignment: {}", e);
                }
            } else {
                app.config.remove_quick_tag(number);
                app.session_tags.remove(number);
                if let Err(e) = AppConfig::save(&app.config_dir, &app.config) {
                    tracing::error!("Failed to save config after quick tag removal: {}", e);
                }
                tracing::warn!(
                    "Quick tag {} assigned to window without executable path",
                    number
                );
            }

            // Refresh number_tag fields from session_tags so the quick list updates
            for w in &mut app.window_snapshot {
                w.number_tag = app.session_tags.get_tag_for_hwnd(w.hwnd);
            }
            if !is_label_mode {
                let sel = app.overlay_state.selected_index();
                let snap = app.window_snapshot.clone();
                app.overlay_manager.redraw(&snap, sel);
            }
        }
    }
}

/// Called on `WM_DISPLAYCHANGE` — re-enumerate monitors and update every overlay HWND
/// so the selector mask covers the correct area at the new resolution.
///
/// If the overlay is currently visible (Active / LabelMode) it is dismissed first so
/// the user is not stuck inside a mispositioned overlay.  The next hotkey press will
/// open it at the correct size.
unsafe fn handle_display_change(app: &mut AppState) {
    tracing::info!("WM_DISPLAYCHANGE received — re-enumerating monitors");

    // Dismiss the overlay if it is currently visible so we don't leave a
    // stale, wrongly-sized overlay on screen.
    match &app.overlay_state {
        OverlayState::Active { .. } => {
            tracing::info!("Display changed while overlay active — dismissing");
            dismiss_overlay(app);
        }
        OverlayState::LabelMode { .. } => {
            tracing::info!("Display changed while label mode active — dismissing");
            dismiss_label_mode(app);
        }
        _ => {}
    }

    // Re-enumerate monitors and push new geometry into the overlay manager.
    let new_monitors = get_all_monitors();
    tracing::info!(
        "New monitor layout: {} monitor(s)",
        new_monitors.len()
    );
    app.overlay_manager.on_display_change(new_monitors);
}

unsafe fn handle_fade_timer(app: &mut AppState) {
    let animation_complete = app.overlay_manager.on_fade_timer();

    if animation_complete {
        match app.overlay_state.clone() {
            OverlayState::FadingIn => {
                app.overlay_state = OverlayState::Active { selected: None };
                app.overlay_manager.render_frame();
                tracing::info!("Fade-in complete");
            }
            OverlayState::FadingOut { switch_target } => {
                // Switch before hiding — we still hold the foreground lock at this point.
                // Calling hide_windows() first would transfer the foreground back to the
                // previous window, causing SetForegroundWindow to fail for the target.
                if let Some(target) = switch_target {
                    let _ = switch_to_window(target);
                }
                app.overlay_manager.hide_windows();
                app.overlay_state = OverlayState::Hidden;
                keyboard_hook::set_active(false);
                if switch_target.is_none() {
                    if let Some(prev) = app.previous_foreground {
                        let _ = restore_focus(prev);
                    }
                }
                app.previous_foreground = None;
                tracing::debug!("Fade-out complete");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `should_update_shift_badges` is the one pure, Win32-free seam in this
    // feature — everything else here is GDI paint calls and hook plumbing
    // that require a live desktop to exercise (per the mission's validation
    // note, manual verification is left to the user).

    #[test]
    fn shift_down_while_active_updates_when_flag_was_false() {
        let state = OverlayState::Active { selected: None };
        assert!(should_update_shift_badges(&state, false, true));
    }

    #[test]
    fn shift_up_while_active_updates_when_flag_was_true() {
        let state = OverlayState::Active { selected: Some(2) };
        assert!(should_update_shift_badges(&state, true, false));
    }

    #[test]
    fn repeated_shift_down_while_active_is_a_noop() {
        // Auto-repeat keydowns report shift_down: true on every tick while
        // Shift is held. Without this guard the overlay would repaint
        // continuously for as long as the key stays down.
        let state = OverlayState::Active { selected: None };
        assert!(!should_update_shift_badges(&state, true, true));
    }

    #[test]
    fn repeated_shift_up_while_active_is_a_noop() {
        let state = OverlayState::Active { selected: None };
        assert!(!should_update_shift_badges(&state, false, false));
    }

    #[test]
    fn shift_transition_outside_active_state_is_ignored() {
        // Label mode, Hidden, FadingIn, and FadingOut must never toggle the
        // flag — badges only ever apply to the Active grid overlay.
        assert!(!should_update_shift_badges(
            &OverlayState::LabelMode { selected: None },
            false,
            true
        ));
        assert!(!should_update_shift_badges(&OverlayState::Hidden, false, true));
        assert!(!should_update_shift_badges(
            &OverlayState::FadingIn,
            false,
            true
        ));
        assert!(!should_update_shift_badges(
            &OverlayState::FadingOut {
                switch_target: None
            },
            false,
            true
        ));
    }
}
