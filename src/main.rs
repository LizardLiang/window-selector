#![windows_subsystem = "windows"]

mod about_dialog;
mod accent_color;
mod animation;
mod config;
mod dwm_thumbnails;
mod grid_layout;
mod hotkey;
mod interaction;
mod keyboard_hook;
mod letter_assignment;
mod logging;
mod monitor;
mod mru_tracker;
mod overlay;
mod overlay_renderer;
mod settings_dialog;
mod state;
mod tray;
mod window_enumerator;
mod window_info;
mod window_switcher;

use config::AppConfig;
use interaction::{handle_key_down, KeyAction};
use monitor::get_all_monitors;
use mru_tracker::MruTracker;
use overlay::OverlayManager;
use state::{OverlayState, SessionTags};
use tray::{
    add_tray_icon, remove_tray_icon, show_balloon, MENU_ABOUT, MENU_EXIT, MENU_SETTINGS,
    WM_TRAY_CALLBACK,
};
use window_enumerator::{register_overlay_hwnds, snapshot_windows};
use window_switcher::{restore_focus, switch_to_window};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, EndPaint, FillRect, SelectObject,
    SetBkMode, SetTextColor, HBRUSH, PAINTSTRUCT, TRANSPARENT,
    DrawTextW, DT_CENTER, DT_SINGLELINE, DT_VCENTER,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, PostQuitMessage, RegisterClassExW, SetWindowLongPtrW, TranslateMessage,
    GWLP_USERDATA, HMENU, MSG, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_HOTKEY, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_TIMER, WM_ACTIVATE, WM_PAINT,
    WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_OVERLAPPEDWINDOW, CS_HREDRAW, CS_VREDRAW, WA_INACTIVE,
    HWND_MESSAGE,
};

const MSG_WINDOW_CLASS: &str = "WindowSelectorMsgWnd\0";
const MSG_WINDOW_NAME: &str = "Window Selector\0";

/// Application state owned by the single message pump thread.
#[allow(dead_code)]
struct AppState {
    config: AppConfig,
    config_dir: std::path::PathBuf,
    overlay_state: OverlayState,
    session_tags: SessionTags,
    mru_tracker: MruTracker,
    overlay_manager: OverlayManager,
    previous_foreground: Option<HWND>,
    window_snapshot: Vec<window_info::WindowInfo>,
    msg_hwnd: HWND,
}

/// Global pointer to `AppState`, stored as an atomic integer so the static is safe
/// (`AtomicUsize` is `Send + Sync`).
///
/// SAFETY invariant: only the Win32 message pump thread reads or writes this value.
/// All Win32 callbacks (`WndProc`, WinEvent hooks) are dispatched on the thread that
/// called `GetMessageW`, so there is never concurrent access. The atomic is used
/// purely to avoid `static mut`, not for cross-thread synchronization.
static APP_STATE_PTR: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Returns the current `AppState` pointer (may be null before init or after cleanup).
#[inline]
fn get_app_state() -> *mut AppState {
    APP_STATE_PTR.load(std::sync::atomic::Ordering::Relaxed) as *mut AppState
}

/// Set (or clear) the `AppState` pointer. Must only be called from the message pump
/// thread.
#[inline]
fn set_app_state(ptr: *mut AppState) {
    APP_STATE_PTR.store(ptr as usize, std::sync::atomic::Ordering::Relaxed);
}

fn main() {
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
    let config_dir = AppConfig::default_config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("./config"));

    // Initialize logging.
    let logs_dir = config_dir.join("logs");
    if let Err(e) = logging::init_logging(&logs_dir, debug_mode) {
        eprintln!("Logging init failed: {}", e);
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

    // Register message-only window class.
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(main_wndproc),
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
        0, 0, 0, 0,
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
    });

    // Set global pointer — valid for the lifetime of the message loop.
    let app_state_ptr = app_state.as_mut() as *mut AppState;
    set_app_state(app_state_ptr);
    SetWindowLongPtrW(msg_hwnd, GWLP_USERDATA, app_state_ptr as isize);

    // Install MRU tracker.
    mru_tracker::set_global_mru_tracker(&mut (*app_state_ptr).mru_tracker as *mut MruTracker);
    (*app_state_ptr).mru_tracker.install_hook();

    // Create overlay windows.
    let monitors = get_all_monitors();
    if monitors.is_empty() {
        tracing::warn!("No monitors detected");
    }
    if let Err(e) = (*app_state_ptr).overlay_manager.create_windows(monitors, overlay_wndproc) {
        tracing::error!("Overlay window creation failed: {:?}", e);
    }

    // Register overlay HWNDs (including label overlay) to be excluded from window enumeration.
    let overlay_hwnds = (*app_state_ptr).overlay_manager.all_hwnds_including_labels();
    register_overlay_hwnds(overlay_hwnds);

    // Add tray icon.
    if let Err(e) = add_tray_icon(msg_hwnd) {
        tracing::error!("Tray icon failed: {:?}", e);
    }

    // Register global hotkey.
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

    // Install the low-level keyboard hook.  The hook starts inactive; it is
    // enabled in activate_overlay() and disabled when the overlay hides.
    keyboard_hook::install(keyboard_hook_handler);

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
            if wparam.0 as i32 == hotkey::HOTKEY_ID {
                handle_hotkey(app);
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

            // Only paint the label overlay HWND (not the thumbnail overlay).
            let is_label_hwnd = app.overlay_manager.label_hwnd == Some(hwnd);
            tracing::debug!(
                "WM_PAINT on {:?}, is_label_hwnd={}, state={:?}",
                hwnd, is_label_hwnd, app.overlay_state
            );
            if !is_label_hwnd {
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

            tracing::info!(
                "Label WM_PAINT: painting badges, rcPaint=({},{},{},{})",
                ps.rcPaint.left, ps.rcPaint.top, ps.rcPaint.right, ps.rcPaint.bottom
            );

            // Draw letter badge at bottom-right of each cell
            if let Some(layout) = &app.overlay_manager.grid_layout {
                let badge_w: i32 = 32;
                let badge_h: i32 = 28;
                let badge_margin: i32 = 12;

                // Font sized to fit the badge height well
                let font = CreateFontW(
                    20, 0, 0, 0, 700, // height=20, bold
                    0, 0, 0, 0, 0, 0, 0, 0,
                    windows::core::w!("Consolas"),
                );
                let old_font = SelectObject(hdc, font);
                SetBkMode(hdc, TRANSPARENT);

                for (i, cell) in layout.cells.iter().enumerate() {
                    if i >= app.window_snapshot.len() {
                        break;
                    }
                    let win = &app.window_snapshot[i];
                    let is_selected = app.overlay_manager.render_selected == Some(i);

                    let badge_rect = RECT {
                        left: (cell.x + cell.width) as i32 - badge_w - badge_margin,
                        top: (cell.y + cell.height) as i32 - badge_h - badge_margin,
                        right: (cell.x + cell.width) as i32 - badge_margin,
                        bottom: (cell.y + cell.height) as i32 - badge_margin,
                    };

                    // COLORREF is 0x00BBGGRR
                    let badge_brush = if is_selected {
                        CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x000088FF)) // orange
                    } else {
                        CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00553322)) // dark blue-gray
                    };
                    FillRect(hdc, &badge_rect, badge_brush);
                    let _ = windows::Win32::Graphics::Gdi::DeleteObject(badge_brush);

                    if let Some(letter) = win.letter {
                        let text_color = if is_selected {
                            windows::Win32::Foundation::COLORREF(0x00FFFFFF) // white
                        } else {
                            windows::Win32::Foundation::COLORREF(0x00EEEEFF) // light
                        };
                        SetTextColor(hdc, text_color);
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
                }

                SelectObject(hdc, old_font);
                let _ = windows::Win32::Graphics::Gdi::DeleteObject(font);
            }

            EndPaint(hwnd, &ps);
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
        if hw.is_invalid() { None } else { Some(hw) }
    };

    app.session_tags.release_closed();

    let mon_clone = app.overlay_manager.monitors.clone();
    app.window_snapshot = snapshot_windows(
        app.overlay_manager.all_hwnds(),
        &mon_clone,
        &app.mru_tracker,
        &app.session_tags,
    );

    tracing::info!("Activating overlay: {} windows", app.window_snapshot.len());

    let snap = app.window_snapshot.clone();
    app.overlay_manager.show(&snap, &mut app.overlay_state);

    // Activate the keyboard hook so key presses reach the overlay regardless
    // of whether SetForegroundWindow succeeded.
    keyboard_hook::set_active(true);
}

/// Dismiss the overlay without switching to any window; restore previous foreground.
unsafe fn dismiss_overlay(app: &mut AppState) {
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
        let cmd = tray::show_context_menu(hwnd);
        handle_menu_command(app, hwnd, cmd);
    }
}

unsafe fn handle_menu_command(app: &mut AppState, hwnd: HWND, cmd: u32) {
    match cmd {
        MENU_SETTINGS => {
            settings_dialog::SettingsDialog::show(hwnd, &app.config);
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
    tracing::debug!("Hook: vk=0x{:02X} ({})", vk_code, vk_code);
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

unsafe fn handle_overlay_key(vk_code: u32) {
    let app_ptr = get_app_state();
    if app_ptr.is_null() {
        return;
    }
    let app = &mut *app_ptr;

    let action = handle_key_down(
        vk_code,
        &app.overlay_state,
        &app.window_snapshot,
        &mut app.session_tags,
    );

    match action {
        KeyAction::None => {}
        KeyAction::Select(idx) => {
            app.overlay_state = OverlayState::Active { selected: Some(idx) };
            let snap = app.window_snapshot.clone();
            app.overlay_manager.redraw(&snap, Some(idx));
        }
        KeyAction::SwitchTo(target) => {
            app.overlay_manager.begin_hide(&mut app.overlay_state, Some(target));
        }
        KeyAction::Dismiss => {
            dismiss_overlay(app);
        }
        KeyAction::TagAssigned => {
            let sel = app.overlay_state.selected_index();
            let snap = app.window_snapshot.clone();
            app.overlay_manager.redraw(&snap, sel);
        }
    }
}

unsafe fn handle_fade_timer(app: &mut AppState) {
    let animation_complete = app.overlay_manager.on_fade_timer();

    if animation_complete {
        match app.overlay_state.clone() {
            OverlayState::FadingIn => {
                app.overlay_state = OverlayState::Active { selected: None };
                // Remove WS_EX_LAYERED so Direct2D HwndRenderTarget can paint.
                app.overlay_manager.remove_layered_style();
                // Render initial frame now that we are fully visible.
                app.overlay_manager.render_frame();
                tracing::info!("Fade-in complete → Active");
            }
            OverlayState::FadingOut { switch_target } => {
                app.overlay_manager.hide_windows();
                app.overlay_state = OverlayState::Hidden;

                // Deactivate keyboard hook before switching focus.
                keyboard_hook::set_active(false);

                if let Some(target) = switch_target {
                    let _ = switch_to_window(target);
                } else if let Some(prev) = app.previous_foreground {
                    let _ = restore_focus(prev);
                }
                app.previous_foreground = None;

                tracing::debug!("Fade-out complete");
            }
            _ => {}
        }
    }
}
