/// Settings panel manager.
///
/// Creates and manages the Direct2D-rendered settings window. All mutations to
/// AppState go through the global APP_STATE_PTR (same pattern as overlay_wndproc).
///
/// The settings window is a top-level WS_OVERLAPPEDWINDOW (no resize, no maximize).
/// It appears in the taskbar via WS_EX_APPWINDOW.
use crate::config::AppConfig;
use crate::settings_renderer::{ControlRects, DrawState, SettingsRenderer};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{InvalidateRect, PtInRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, GetSystemMetrics,
    HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, PostMessageW, RegisterClassExW,
    SetWindowPos, SetWindowsHookExW, ShowWindow, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    CS_HREDRAW, CS_VREDRAW, HMENU, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
    SWP_NOACTIVATE, SWP_NOZORDER, WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_SIZE, WNDCLASSEXW, WS_EX_APPWINDOW,
    WS_CAPTION, WS_MINIMIZEBOX, WS_SYSMENU,
};
use std::sync::atomic::{AtomicUsize, Ordering};

const SETTINGS_CLASS_NAME: &str = "WindowSelectorSettings\0";
const SETTINGS_WINDOW_TITLE: &str = "Window Selector Settings\0";

/// Fixed logical size of the settings window in physical pixels.
/// Scaled by DPI at creation time.
const SETTINGS_WIDTH_BASE: i32 = 480;
const SETTINGS_HEIGHT_BASE: i32 = 600;

/// Global pointer to the active SettingsPanelManager.
/// Only valid while the settings panel is open (non-null).
/// Access is safe under the single-threaded message pump invariant.
static SETTINGS_PANEL_PTR: AtomicUsize = AtomicUsize::new(0);

fn get_settings_panel() -> *mut SettingsPanelManager {
    SETTINGS_PANEL_PTR.load(Ordering::Relaxed) as *mut SettingsPanelManager
}

/// State of the hotkey recorder.
#[derive(Debug, Clone, PartialEq)]
pub enum HotkeyRecorderState {
    /// Not recording.
    Idle,
    /// Recording for the main overlay hotkey (target=1) or label hotkey (target=2).
    Recording {
        target: u8,
        previous_modifiers: u32,
        previous_vk: u32,
    },
}

/// Manages the settings panel HWND lifecycle, renderer, and control state.
pub struct SettingsPanelManager {
    /// The settings window HWND (None when closed).
    pub hwnd: Option<HWND>,
    /// Direct2D renderer (Some when window is open).
    renderer: Option<SettingsRenderer>,
    /// Current hit-test rects for all controls.
    control_rects: ControlRects,
    /// Hotkey recorder state machine.
    recorder: HotkeyRecorderState,
    /// Low-level keyboard hook handle (installed only during recording).
    ll_hook: Option<HHOOK>,
    /// Index of slider currently being dragged (0-5), or None.
    active_slider: Option<usize>,
    /// Pending error text for main hotkey field.
    main_hotkey_error: String,
    /// Pending error text for label hotkey field.
    label_hotkey_error: String,
    /// Cached direct_switch state for the toggle.
    direct_switch: bool,
    /// Cached launch_at_startup state for the toggle.
    launch_at_startup: bool,
    /// Cached slider values (populated from config on open).
    slider_values: [f32; 6],
}

impl SettingsPanelManager {
    pub fn new() -> Self {
        Self {
            hwnd: None,
            renderer: None,
            control_rects: ControlRects::default(),
            recorder: HotkeyRecorderState::Idle,
            ll_hook: None,
            active_slider: None,
            main_hotkey_error: String::new(),
            label_hotkey_error: String::new(),
            direct_switch: false,
            launch_at_startup: false,
            slider_values: [220.0, 0.86, 150.0, 16.0, 18.0, 13.0],
        }
    }

    /// Open the settings panel. If already open, bring to front.
    pub fn open(&mut self, msg_hwnd: HWND) {
        if let Some(hwnd) = self.hwnd {
            // Already open: bring to front
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetWindowPos(hwnd, HWND::default(), 0, 0, 0, 0,
                    SWP_NOZORDER | SWP_NOACTIVATE);
            }
            return;
        }

        // Populate control state from AppState config
        self.populate_from_config();

        // Register window class (idempotent — ignores ALREADY_EXISTS)
        unsafe {
            let instance = match GetModuleHandleW(PCWSTR::null()) {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("GetModuleHandleW failed in settings open: {:?}", e);
                    return;
                }
            };

            let class_name: Vec<u16> = SETTINGS_CLASS_NAME.encode_utf16().collect();

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(settings_wndproc),
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
            let _ = RegisterClassExW(&wc); // ignore error if class already registered

            // Compute DPI-aware window size
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let screen_h = GetSystemMetrics(SM_CYSCREEN);

            // Use a temporary HWND for DPI query — fall back to msg_hwnd DPI
            let dpi = GetDpiForWindow(msg_hwnd);
            let dpi_scale = if dpi == 0 { 1.0_f32 } else { dpi as f32 / 96.0 };

            let win_w = (SETTINGS_WIDTH_BASE as f32 * dpi_scale) as i32;
            let win_h = (SETTINGS_HEIGHT_BASE as f32 * dpi_scale) as i32;
            let win_x = (screen_w - win_w) / 2;
            let win_y = (screen_h - win_h) / 2;

            let window_title: Vec<u16> = SETTINGS_WINDOW_TITLE.encode_utf16().collect();

            // WS_OVERLAPPEDWINDOW without WS_MAXIMIZEBOX and WS_THICKFRAME
            // = title bar, close button, minimize button, no resize/maximize
            let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;

            let hwnd = match CreateWindowExW(
                WS_EX_APPWINDOW,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(window_title.as_ptr()),
                style,
                win_x,
                win_y,
                win_w,
                win_h,
                None,
                HMENU::default(),
                instance,
                None,
            ) {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("Settings window creation failed: {:?}", e);
                    return;
                }
            };

            self.hwnd = Some(hwnd);
            // Store global pointer so wndproc can access this struct
            SETTINGS_PANEL_PTR.store(self as *mut _ as usize, Ordering::Relaxed);

            // Initialize renderer
            match SettingsRenderer::new(hwnd) {
                Ok(r) => {
                    self.renderer = Some(r);
                }
                Err(e) => {
                    tracing::error!("SettingsRenderer::new failed: {:?}", e);
                }
            }

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = InvalidateRect(hwnd, None, true);

            tracing::info!("Settings panel opened (HWND={:?})", hwnd);
        }
    }

    /// Close the settings panel and clean up resources.
    pub fn close(&mut self) {
        // SA review: uninstall WH_KEYBOARD_LL if in Recording state before HWND destruction
        if self.recorder != HotkeyRecorderState::Idle {
            self.uninstall_ll_hook();
            self.recorder = HotkeyRecorderState::Idle;
        }

        self.renderer = None;

        if let Some(hwnd) = self.hwnd.take() {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            tracing::info!("Settings panel closed");
        }

        SETTINGS_PANEL_PTR.store(0, Ordering::Relaxed);
    }

    /// Returns true if the settings panel is currently open.
    pub fn is_open(&self) -> bool {
        self.hwnd.is_some()
    }

    /// Populate cached slider/toggle values from AppState.config.
    fn populate_from_config(&mut self) {
        let app_ptr = crate::get_app_state_pub();
        if app_ptr.is_null() {
            return;
        }
        unsafe {
            let app = &*app_ptr;
            self.direct_switch = app.config.direct_switch;
            self.launch_at_startup = crate::startup::get_launch_at_startup();
            self.slider_values = [
                app.config.overlay_opacity as f32,
                app.config.background_opacity,
                app.config.fade_duration_ms as f32,
                app.config.grid_padding,
                app.config.label_font_size,
                app.config.title_font_size,
            ];
        }
    }

    /// Install the WH_KEYBOARD_LL hook for hotkey recording.
    pub fn install_ll_hook(&mut self) {
        unsafe {
            let instance = GetModuleHandleW(PCWSTR::null()).unwrap_or_default();
            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), instance, 0) {
                Ok(hook) => {
                    self.ll_hook = Some(hook);
                    tracing::debug!("WH_KEYBOARD_LL hook installed for hotkey recording");
                }
                Err(e) => {
                    tracing::error!("SetWindowsHookExW failed: {:?}", e);
                }
            }
        }
    }

    /// Uninstall the WH_KEYBOARD_LL hook.
    pub fn uninstall_ll_hook(&mut self) {
        if let Some(hook) = self.ll_hook.take() {
            unsafe {
                let _ = UnhookWindowsHookEx(hook);
                tracing::debug!("WH_KEYBOARD_LL hook uninstalled");
            }
        }
    }

    /// Build DrawState for the renderer from current panel state.
    pub fn build_draw_state(&self) -> DrawState {
        let recording_target = match &self.recorder {
            HotkeyRecorderState::Recording { target, .. } => *target,
            _ => 0,
        };
        DrawState {
            recording_target,
            main_hotkey_error: self.main_hotkey_error.clone(),
            label_hotkey_error: self.label_hotkey_error.clone(),
            active_slider: self.active_slider,
            slider_values: self.slider_values,
            direct_switch: self.direct_switch,
            launch_at_startup: self.launch_at_startup,
        }
    }

    /// Handle a mouse click at position (x, y).
    pub fn handle_click(&mut self, x: i32, y: i32) {
        let pt = POINT { x, y };
        let rects = self.control_rects.clone();

        unsafe {
            // Check main hotkey field
            if PtInRect(&rects.main_hotkey, pt).as_bool() {
                self.start_recording(1);
                return;
            }

            // Check label hotkey field
            if PtInRect(&rects.label_hotkey, pt).as_bool() {
                self.start_recording(2);
                return;
            }

            // Check direct_switch toggle
            if PtInRect(&rects.direct_switch_toggle, pt).as_bool() {
                self.toggle_direct_switch();
                return;
            }

            // Check launch_at_startup toggle
            if PtInRect(&rects.launch_at_startup_toggle, pt).as_bool() {
                self.toggle_launch_at_startup();
                return;
            }

            // Check reset button
            if PtInRect(&rects.reset_button, pt).as_bool() {
                self.reset_to_defaults();
                return;
            }

            // Check sliders (begin drag)
            for (i, track) in rects.slider_tracks.iter().enumerate() {
                // Extend hit area vertically for easier drag start
                let extended = RECT {
                    left: track.left,
                    top: track.top - 10,
                    right: track.right,
                    bottom: track.bottom + 10,
                };
                if PtInRect(&extended, pt).as_bool() {
                    self.active_slider = Some(i);
                    self.update_slider_from_x(i, x);
                    return;
                }
            }
        }
    }

    /// Handle mouse move (slider drag).
    pub fn handle_mouse_move(&mut self, x: i32) {
        if let Some(idx) = self.active_slider {
            self.update_slider_from_x(idx, x);
        }
    }

    /// Handle mouse button up (end drag).
    pub fn handle_mouse_up(&mut self) {
        if self.active_slider.take().is_some() {
            self.commit_slider_values();
        }
    }

    /// Update slider value from mouse x position.
    fn update_slider_from_x(&mut self, idx: usize, x: i32) {
        let track = self.control_rects.slider_tracks[idx];
        let track_w = (track.right - track.left).max(1) as f32;
        let t = ((x - track.left) as f32 / track_w).clamp(0.0, 1.0);

        let (min_v, max_v) = match idx {
            0 => (50.0_f32, 255.0_f32),  // overlay_opacity
            1 => (0.0_f32, 1.0_f32),     // background_opacity
            2 => (0.0_f32, 500.0_f32),   // fade_duration_ms
            3 => (4.0_f32, 48.0_f32),    // grid_padding
            4 => (10.0_f32, 32.0_f32),   // label_font_size
            5 => (8.0_f32, 24.0_f32),    // title_font_size
            _ => return,
        };

        let val = min_v + t * (max_v - min_v);
        // Round integer sliders
        self.slider_values[idx] = match idx {
            0 | 2 => val.round(),
            _ => val,
        };

        self.invalidate();
    }

    /// Persist slider values to AppState.config and save.
    fn commit_slider_values(&mut self) {
        let app_ptr = crate::get_app_state_pub();
        if app_ptr.is_null() {
            return;
        }
        unsafe {
            let app = &mut *app_ptr;
            app.config.overlay_opacity = self.slider_values[0].clamp(50.0, 255.0) as u8;
            app.config.background_opacity = self.slider_values[1].clamp(0.0, 1.0);
            app.config.fade_duration_ms = self.slider_values[2].clamp(0.0, 500.0) as u32;
            app.config.grid_padding = self.slider_values[3].clamp(4.0, 48.0);
            app.config.label_font_size = self.slider_values[4].clamp(10.0, 32.0);
            app.config.title_font_size = self.slider_values[5].clamp(8.0, 24.0);
            if let Err(e) = AppConfig::save(&app.config_dir, &app.config) {
                tracing::error!("Failed to save config after slider change: {}", e);
            }
        }
    }

    /// Toggle direct_switch and save.
    fn toggle_direct_switch(&mut self) {
        let app_ptr = crate::get_app_state_pub();
        if app_ptr.is_null() {
            return;
        }
        unsafe {
            let app = &mut *app_ptr;
            app.config.direct_switch = !app.config.direct_switch;
            self.direct_switch = app.config.direct_switch;
            if let Err(e) = AppConfig::save(&app.config_dir, &app.config) {
                tracing::error!("Failed to save config after direct_switch toggle: {}", e);
            }
        }
        self.invalidate();
    }

    /// Toggle launch_at_startup and write registry.
    fn toggle_launch_at_startup(&mut self) {
        let app_ptr = crate::get_app_state_pub();
        if app_ptr.is_null() {
            return;
        }
        unsafe {
            let app = &mut *app_ptr;
            let new_val = !app.config.launch_at_startup;
            match crate::startup::set_launch_at_startup(new_val) {
                Ok(()) => {
                    app.config.launch_at_startup = new_val;
                    self.launch_at_startup = new_val;
                    if let Err(e) = AppConfig::save(&app.config_dir, &app.config) {
                        tracing::error!("Failed to save config after startup toggle: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("set_launch_at_startup({}) failed: {:?}", new_val, e);
                }
            }
        }
        self.invalidate();
    }

    /// Reset all settings to defaults and save.
    pub fn reset_to_defaults(&mut self) {
        let app_ptr = crate::get_app_state_pub();
        if app_ptr.is_null() {
            return;
        }
        unsafe {
            let app = &mut *app_ptr;
            let defaults = AppConfig::default();

            // Re-register hotkeys with default values
            crate::hotkey::unregister_hotkey(app.msg_hwnd);
            crate::hotkey::unregister_label_hotkey(app.msg_hwnd);

            if let Err(e) = crate::hotkey::register_hotkey(app.msg_hwnd, defaults.hotkey_modifiers, defaults.hotkey_vk) {
                tracing::error!("Failed to register default hotkey: {:?}", e);
            }
            if let Err(e) = crate::hotkey::register_label_hotkey(app.msg_hwnd, defaults.label_hotkey_modifiers, defaults.label_hotkey_vk) {
                tracing::error!("Failed to register default label hotkey: {:?}", e);
            }

            // Reset startup registry
            let _ = crate::startup::set_launch_at_startup(false);

            app.config = defaults.clone();
            self.populate_from_config();

            if let Err(e) = AppConfig::save(&app.config_dir, &app.config) {
                tracing::error!("Failed to save default config: {}", e);
            }
        }
        self.invalidate();
        tracing::info!("Settings reset to defaults");
    }

    /// Enter recording mode for a hotkey field.
    fn start_recording(&mut self, target: u8) {
        let app_ptr = crate::get_app_state_pub();
        if app_ptr.is_null() {
            return;
        }
        let (prev_mod, prev_vk) = unsafe {
            let app = &*app_ptr;
            if target == 1 {
                (app.config.hotkey_modifiers, app.config.hotkey_vk)
            } else {
                (app.config.label_hotkey_modifiers, app.config.label_hotkey_vk)
            }
        };

        self.recorder = HotkeyRecorderState::Recording {
            target,
            previous_modifiers: prev_mod,
            previous_vk: prev_vk,
        };
        self.install_ll_hook();
        self.invalidate();
        tracing::debug!("Hotkey recording started for target={}", target);
    }

    /// Cancel recording and revert.
    pub fn cancel_recording(&mut self) {
        self.uninstall_ll_hook();
        self.recorder = HotkeyRecorderState::Idle;
        self.invalidate();
        tracing::debug!("Hotkey recording cancelled");
    }

    /// Commit a captured hotkey combination.
    pub fn commit_hotkey(&mut self, modifiers: u32, vk: u32) {
        let (target, prev_mod, prev_vk) = match &self.recorder {
            HotkeyRecorderState::Recording {
                target,
                previous_modifiers,
                previous_vk,
            } => (*target, *previous_modifiers, *previous_vk),
            _ => return,
        };

        self.uninstall_ll_hook();
        self.recorder = HotkeyRecorderState::Idle;

        let app_ptr = crate::get_app_state_pub();
        if app_ptr.is_null() {
            return;
        }
        unsafe {
            let app = &mut *app_ptr;

            if target == 1 {
                // Try to register the new main hotkey
                crate::hotkey::unregister_hotkey(app.msg_hwnd);
                match crate::hotkey::register_hotkey(app.msg_hwnd, modifiers, vk) {
                    Ok(()) => {
                        app.config.hotkey_modifiers = modifiers;
                        app.config.hotkey_vk = vk;
                        self.main_hotkey_error.clear();
                        if let Err(e) = AppConfig::save(&app.config_dir, &app.config) {
                            tracing::error!("Failed to save config after hotkey change: {}", e);
                        }
                        tracing::info!("Main hotkey changed to modifiers=0x{:X} vk=0x{:X}", modifiers, vk);
                    }
                    Err(e) => {
                        tracing::warn!("New main hotkey conflict: {:?}", e);
                        self.main_hotkey_error = "Hotkey already in use".to_string();
                        // Revert to previous hotkey
                        if let Err(e2) = crate::hotkey::register_hotkey(app.msg_hwnd, prev_mod, prev_vk) {
                            tracing::error!("Failed to re-register previous hotkey: {:?}", e2);
                        } else {
                            app.config.hotkey_modifiers = prev_mod;
                            app.config.hotkey_vk = prev_vk;
                        }
                    }
                }
            } else {
                // Label hotkey
                crate::hotkey::unregister_label_hotkey(app.msg_hwnd);
                match crate::hotkey::register_label_hotkey(app.msg_hwnd, modifiers, vk) {
                    Ok(()) => {
                        app.config.label_hotkey_modifiers = modifiers;
                        app.config.label_hotkey_vk = vk;
                        self.label_hotkey_error.clear();
                        if let Err(e) = AppConfig::save(&app.config_dir, &app.config) {
                            tracing::error!("Failed to save config after label hotkey change: {}", e);
                        }
                        tracing::info!("Label hotkey changed to modifiers=0x{:X} vk=0x{:X}", modifiers, vk);
                    }
                    Err(e) => {
                        tracing::warn!("New label hotkey conflict: {:?}", e);
                        self.label_hotkey_error = "Hotkey already in use".to_string();
                        // Revert
                        if let Err(e2) = crate::hotkey::register_label_hotkey(app.msg_hwnd, prev_mod, prev_vk) {
                            tracing::error!("Failed to re-register previous label hotkey: {:?}", e2);
                        } else {
                            app.config.label_hotkey_modifiers = prev_mod;
                            app.config.label_hotkey_vk = prev_vk;
                        }
                    }
                }
            }
        }

        self.invalidate();
    }

    /// Invalidate the panel window to trigger a repaint.
    fn invalidate(&self) {
        if let Some(hwnd) = self.hwnd {
            unsafe {
                let _ = InvalidateRect(hwnd, None, false);
            }
        }
    }
}

impl Default for SettingsPanelManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Low-level keyboard hook callback — captures key combinations during recording mode.
/// Installed only when `HotkeyRecorderState::Recording` is active.
unsafe extern "system" fn ll_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code != HC_ACTION as i32 {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    // WM_KEYDOWN = 0x100, WM_SYSKEYDOWN = 0x104
    let is_key_down = wparam.0 == 0x100 || wparam.0 == 0x104;
    if !is_key_down {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    let vk = kb.vkCode;

    // Escape: cancel recording
    if vk == 0x1B {
        let panel_ptr = get_settings_panel();
        if !panel_ptr.is_null() {
            (*panel_ptr).cancel_recording();
        }
        return LRESULT(1); // swallow Escape
    }

    // Modifier-only keys: do not commit
    let is_modifier = matches!(vk,
        0x10 | 0x11 | 0x12 | // VK_SHIFT, VK_CONTROL, VK_MENU (generic)
        0xA0 | 0xA1 | // VK_LSHIFT, VK_RSHIFT
        0xA2 | 0xA3 | // VK_LCONTROL, VK_RCONTROL
        0xA4 | 0xA5 | // VK_LMENU, VK_RMENU
        0x5B | 0x5C   // VK_LWIN, VK_RWIN
    );
    if is_modifier {
        // Let modifier pass through so GetAsyncKeyState can read state
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    // Compute modifier flags from async key state
    let ctrl = (GetAsyncKeyState(0x11) as u16 & 0x8000) != 0;
    let alt = (GetAsyncKeyState(0x12) as u16 & 0x8000) != 0;
    let shift = (GetAsyncKeyState(0x10) as u16 & 0x8000) != 0;
    let win = (GetAsyncKeyState(0x5B) as u16 & 0x8000) != 0
        || (GetAsyncKeyState(0x5C) as u16 & 0x8000) != 0;

    let mut modifiers: u32 = 0x4000; // MOD_NOREPEAT always set
    if ctrl { modifiers |= 0x0002; }
    if alt { modifiers |= 0x0001; }
    if shift { modifiers |= 0x0004; }
    if win { modifiers |= 0x0008; }

    // Must have at least one modifier besides MOD_NOREPEAT
    let has_modifier = (modifiers & !0x4000) != 0;
    if !has_modifier {
        // Single key without modifier — not a valid hotkey combination
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    // Commit the hotkey
    let panel_ptr = get_settings_panel();
    if !panel_ptr.is_null() {
        (*panel_ptr).commit_hotkey(modifiers, vk);
    }

    LRESULT(1) // swallow the key
}

/// Settings window procedure.
pub unsafe extern "system" fn settings_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let panel_ptr = get_settings_panel();
    if panel_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let panel = &mut *panel_ptr;

    match msg {
        WM_PAINT => {
            use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
            let mut ps = PAINTSTRUCT::default();
            let _hdc = BeginPaint(hwnd, &mut ps);

            if let (Some(renderer), Some(app_ptr)) = (&panel.renderer, Some(crate::get_app_state_pub())) {
                if !app_ptr.is_null() {
                    let app = &*app_ptr;
                    let draw_state = panel.build_draw_state();
                    let new_rects = renderer.draw_panel(&app.config, &draw_state);
                    panel.control_rects = new_rects;
                }
            }

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            panel.handle_click(x, y);
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            panel.handle_mouse_move(x);
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            panel.handle_mouse_up();
            LRESULT(0)
        }

        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            if vk == 0x1B {
                // Escape: close the settings panel
                let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            LRESULT(0)
        }

        WM_SIZE => {
            if let Some(renderer) = &panel.renderer {
                let width = (lparam.0 & 0xFFFF) as u32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as u32;
                if width > 0 && height > 0 {
                    renderer.resize(width, height);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        WM_CLOSE => {
            // SA review: ensure hook is cleaned up before HWND destruction
            if panel.recorder != HotkeyRecorderState::Idle {
                panel.uninstall_ll_hook();
                panel.recorder = HotkeyRecorderState::Idle;
            }
            panel.renderer = None;
            panel.hwnd = None;
            SETTINGS_PANEL_PTR.store(0, Ordering::Relaxed);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }

        WM_DESTROY => {
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}