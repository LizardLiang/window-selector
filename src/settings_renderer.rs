/// Direct2D renderer for the settings panel window.
///
/// All drawing is done via Direct2D + DirectWrite, following the same pattern
/// as `overlay_renderer.rs`. No Win32 GDI controls are used.
use crate::config::{ActionModifier, AppConfig, LabelOverlapStrategy};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_COLOR_F, D2D_POINT_2F, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_REGULAR, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING,
};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

/// Fixed logical panel dimensions (in the same units as `SETTINGS_WIDTH_BASE`/
/// `SETTINGS_HEIGHT_BASE` in `settings_panel.rs` — kept in sync with those).
const PANEL_WIDTH: f32 = 620.0;
const PANEL_HEIGHT: f32 = 560.0;
/// Width of the left navigation rail. All page content starts at
/// `SIDEBAR_WIDTH + margin`.
const SIDEBAR_WIDTH: f32 = 140.0;

/// Which page of the settings window is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsPage {
    /// Global hotkeys and in-overlay keys — one page, two sections. They were
    /// split at first; keeping them together is what users expect, since both
    /// answer "what key does what".
    #[default]
    Keybindings,
    Behavior,
    Appearance,
    Guide,
}

impl SettingsPage {
    /// All pages in sidebar display order. Single source of truth for both
    /// the sidebar's rendering order and `ControlRects::sidebar_items`' index.
    pub const ALL: [SettingsPage; 4] = [
        SettingsPage::Keybindings,
        SettingsPage::Behavior,
        SettingsPage::Appearance,
        SettingsPage::Guide,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingsPage::Keybindings => "Keybindings",
            SettingsPage::Behavior => "Behavior",
            SettingsPage::Appearance => "Appearance",
            SettingsPage::Guide => "Guide",
        }
    }
}

/// State passed from the panel manager to the renderer each frame.
#[derive(Debug, Clone)]
pub struct DrawState {
    /// Which page is currently active.
    pub current_page: SettingsPage,
    /// Which hotkey field is in recording mode
    /// (0=none, 1=main, 2=label, 3=confirm, 4=dismiss)
    pub recording_target: u8,
    /// Error message for hotkey field 1 (main), or empty
    pub main_hotkey_error: String,
    /// Error message for hotkey field 2 (label), or empty
    pub label_hotkey_error: String,
    /// Error message for the Confirm key field, or empty
    pub confirm_error: String,
    /// Error message for the Dismiss key field, or empty
    pub dismiss_error: String,
    /// Index of slider being dragged (0-based), or None
    pub active_slider: Option<usize>,
    /// Slider values [overlay_opacity(0-255), background_opacity(0.0-1.0),
    ///                fade_duration_ms, grid_padding, label_font_size, title_font_size]
    pub slider_values: [f32; 6],
    pub direct_switch: bool,
    pub launch_at_startup: bool,
    pub label_overlap_strategy: LabelOverlapStrategy,
}

/// Hit-test rectangles for all controls — populated during draw, used for mouse events.
///
/// Only the rects belonging to the currently active page are populated by
/// `draw_panel`; every other field is left at `RECT::default()` (all zeros),
/// which makes `PtInRect` return false for it. That is the entire mechanism
/// that prevents a click at a position where, say, an Appearance slider used
/// to be from doing anything while a different page is showing.
#[derive(Debug, Clone, Default)]
pub struct ControlRects {
    /// The 5 sidebar navigation entries, in `SettingsPage::ALL` order. Always
    /// populated regardless of the active page.
    pub sidebar_items: [RECT; 4],
    pub main_hotkey: RECT,
    pub label_hotkey: RECT,
    pub confirm_hotkey: RECT,
    pub dismiss_hotkey: RECT,
    /// Tag-assign modifier option buttons, in `ActionModifier::ALL` order.
    pub tag_modifier_buttons: [RECT; 4],
    /// Move-to-monitor modifier option buttons, in `ActionModifier::ALL` order.
    pub move_modifier_buttons: [RECT; 4],
    pub direct_switch_toggle: RECT,
    pub launch_at_startup_toggle: RECT,
    /// Track rects for the 6 sliders (overlay_opacity, background_opacity,
    /// fade_duration_ms, grid_padding, label_font_size, title_font_size)
    pub slider_tracks: [RECT; 6],
    /// Reset to Defaults — drawn in the footer strip, visible on every page.
    pub reset_button: RECT,
    /// Label overlap strategy: AutoNudge button rect
    pub label_overlap_nudge: RECT,
    /// Label overlap strategy: VisibleRegion button rect
    pub label_overlap_visible: RECT,
}

fn d2d_color(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

fn d2d_rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}

/// Render a Confirm/Dismiss binding for display, appending the permanent
/// fallback key only when it differs from the configured binding — otherwise
/// the default Escape dismiss renders as the nonsense "Esc (or Esc)".
/// `bracketed` parenthesizes the suffix for the compact settings field; the
/// Guide page passes it through unbracketed.
fn binding_with_fallback(modifiers: u32, vk: u32, fallback_vk: u32, bracketed: bool) -> String {
    let name = crate::hotkey::format_hotkey(modifiers, vk);
    if vk == fallback_vk && modifiers == 0 {
        return name;
    }
    let fallback = crate::hotkey::format_hotkey(0, fallback_vk);
    if bracketed {
        format!("{} (or {})", name, fallback)
    } else {
        format!("{} or {}", name, fallback)
    }
}

fn rect_to_d2d(r: &RECT) -> D2D_RECT_F {
    d2d_rect(r.left as f32, r.top as f32, r.right as f32, r.bottom as f32)
}

/// Direct2D renderer for the settings panel.
#[allow(dead_code)]
pub struct SettingsRenderer {
    d2d_factory: ID2D1Factory,
    render_target: ID2D1HwndRenderTarget,
    dwrite_factory: IDWriteFactory,

    // Brushes
    bg_brush: ID2D1SolidColorBrush,
    section_heading_brush: ID2D1SolidColorBrush,
    label_brush: ID2D1SolidColorBrush,
    value_brush: ID2D1SolidColorBrush,
    separator_brush: ID2D1SolidColorBrush,
    slider_track_brush: ID2D1SolidColorBrush,
    slider_fill_brush: ID2D1SolidColorBrush,
    slider_thumb_brush: ID2D1SolidColorBrush,
    toggle_off_brush: ID2D1SolidColorBrush,
    toggle_on_brush: ID2D1SolidColorBrush,
    toggle_knob_brush: ID2D1SolidColorBrush,
    hotkey_field_brush: ID2D1SolidColorBrush,
    hotkey_recording_brush: ID2D1SolidColorBrush,
    hotkey_error_brush: ID2D1SolidColorBrush,
    button_brush: ID2D1SolidColorBrush,
    button_text_brush: ID2D1SolidColorBrush,

    // Text formats
    heading_format: IDWriteTextFormat,
    label_format: IDWriteTextFormat,
    value_format: IDWriteTextFormat,
    hotkey_format: IDWriteTextFormat,
    button_format: IDWriteTextFormat,
}

impl SettingsRenderer {
    pub fn new(hwnd: HWND) -> windows::core::Result<Self> {
        unsafe {
            let d2d_factory: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

            // Use GetClientRect for sizing (handles DPI correctly per SA review)
            let mut client_rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut client_rect);
            let width = (client_rect.right - client_rect.left) as u32;
            let height = (client_rect.bottom - client_rect.top) as u32;

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                dpiX: 96.0,
                dpiY: 96.0,
                pixelFormat: windows::Win32::Graphics::Direct2D::Common::D2D1_PIXEL_FORMAT {
                    format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: windows::Win32::Graphics::Direct2D::Common::D2D1_ALPHA_MODE_IGNORE,
                },
                ..Default::default()
            };
            let hwnd_rt_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: D2D_SIZE_U { width, height },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };

            let render_target = d2d_factory.CreateHwndRenderTarget(&rt_props, &hwnd_rt_props)?;
            render_target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);

            // Color palette — dark theme matching overlay
            let bg_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.08, 0.09, 0.13, 1.0), None)?;
            let section_heading_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.9, 0.9, 1.0, 1.0), None)?;
            let label_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.75, 0.77, 0.85, 1.0), None)?;
            let value_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.6, 0.62, 0.7, 1.0), None)?;
            let separator_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.08), None)?;
            let slider_track_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.2, 0.22, 0.3, 1.0), None)?;
            let slider_fill_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.35, 0.55, 1.0, 1.0), None)?;
            let slider_thumb_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.95), None)?;
            let toggle_off_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.2, 0.22, 0.3, 1.0), None)?;
            let toggle_on_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.35, 0.55, 1.0, 1.0), None)?;
            let toggle_knob_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.95), None)?;
            let hotkey_field_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.12, 0.14, 0.20, 1.0), None)?;
            let hotkey_recording_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.35, 0.55, 1.0, 0.25), None)?;
            let hotkey_error_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.9, 0.2, 0.2, 0.25), None)?;
            let button_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.2, 0.22, 0.3, 0.8), None)?;
            let button_text_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.9, 0.9, 1.0, 1.0), None)?;

            let font_name: Vec<u16> = "Segoe UI Variable\0".encode_utf16().collect();
            let locale: Vec<u16> = "en-us\0".encode_utf16().collect();

            let heading_format = dwrite_factory.CreateTextFormat(
                PCWSTR(font_name.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                16.0,
                PCWSTR(locale.as_ptr()),
            )?;
            heading_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            heading_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            let label_format = dwrite_factory.CreateTextFormat(
                PCWSTR(font_name.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                13.0,
                PCWSTR(locale.as_ptr()),
            )?;
            label_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            label_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            let value_format = dwrite_factory.CreateTextFormat(
                PCWSTR(font_name.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                12.0,
                PCWSTR(locale.as_ptr()),
            )?;
            value_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            value_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            let hotkey_format = dwrite_factory.CreateTextFormat(
                PCWSTR(font_name.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                14.0,
                PCWSTR(locale.as_ptr()),
            )?;
            hotkey_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            hotkey_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            let button_format = dwrite_factory.CreateTextFormat(
                PCWSTR(font_name.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                13.0,
                PCWSTR(locale.as_ptr()),
            )?;
            button_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            button_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            Ok(Self {
                d2d_factory,
                render_target,
                dwrite_factory,
                bg_brush,
                section_heading_brush,
                label_brush,
                value_brush,
                separator_brush,
                slider_track_brush,
                slider_fill_brush,
                slider_thumb_brush,
                toggle_off_brush,
                toggle_on_brush,
                toggle_knob_brush,
                hotkey_field_brush,
                hotkey_recording_brush,
                hotkey_error_brush,
                button_brush,
                button_text_brush,
                heading_format,
                label_format,
                value_format,
                hotkey_format,
                button_format,
            })
        }
    }

    /// Resize the render target when the window is resized.
    pub fn resize(&self, width: u32, height: u32) {
        unsafe {
            let _ = self.render_target.Resize(&D2D_SIZE_U { width, height });
        }
    }

    /// Render the complete settings panel. Returns updated control hit-test rects.
    ///
    /// Only the active page's `draw_*_page` method is called, and it is the
    /// only one that populates its slice of `rects` — every field belonging
    /// to another page is left at `RECT::default()`, so `PtInRect` fails for
    /// it and a click there is a no-op. The sidebar and the footer Reset
    /// button are drawn unconditionally on every page.
    pub fn draw_panel(&self, config: &AppConfig, state: &DrawState) -> ControlRects {
        let mut rects = ControlRects::default();
        unsafe {
            self.render_target.BeginDraw();
            self.render_target
                .Clear(Some(&d2d_color(0.08, 0.09, 0.13, 1.0)));

            rects.sidebar_items = self.draw_sidebar(state.current_page);

            let content_left = SIDEBAR_WIDTH + 24.0_f32;
            let right_margin = 24.0_f32;
            let content_right = PANEL_WIDTH - right_margin;

            match state.current_page {
                SettingsPage::Keybindings => self.draw_keybindings_page(
                    config,
                    state,
                    content_left,
                    content_right,
                    &mut rects,
                ),
                SettingsPage::Behavior => {
                    self.draw_behavior_page(state, content_left, content_right, &mut rects)
                }
                SettingsPage::Appearance => {
                    self.draw_appearance_page(state, content_left, content_right, &mut rects)
                }
                SettingsPage::Guide => self.draw_guide_page(config, content_left, content_right),
            }

            // ---- FOOTER: Reset to Defaults (every page) ----
            let btn_y = PANEL_HEIGHT - 60.0_f32;
            let btn_w = 200.0_f32;
            let btn_h = 36.0_f32;
            let btn_x = SIDEBAR_WIDTH + (PANEL_WIDTH - SIDEBAR_WIDTH - btn_w) / 2.0;
            let btn_rect = RECT {
                left: btn_x as i32,
                top: btn_y as i32,
                right: (btn_x + btn_w) as i32,
                bottom: (btn_y + btn_h) as i32,
            };
            rects.reset_button = btn_rect;
            self.draw_button(&btn_rect, "Reset to Defaults");

            if let Err(e) = self.render_target.EndDraw(None, None) {
                tracing::error!("SettingsRenderer EndDraw failed: {:?}", e);
            }
        }
        rects
    }

    /// Draw a section heading + separator line spanning `[x_left, x_right]`.
    fn draw_section(&self, x_left: f32, x_right: f32, y: f32, title: &str) {
        unsafe {
            let t: Vec<u16> = title.encode_utf16().collect();
            self.render_target.DrawText(
                &t,
                &self.heading_format,
                &d2d_rect(x_left, y, x_right, y + 24.0),
                &self.section_heading_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
            self.render_target.DrawLine(
                D2D_POINT_2F {
                    x: x_left,
                    y: y + 26.0,
                },
                D2D_POINT_2F {
                    x: x_right,
                    y: y + 26.0,
                },
                &self.separator_brush,
                1.0,
                None,
            );
        }
    }

    /// Draw a row label of `width` starting at `x_left`.
    fn draw_label(&self, x_left: f32, width: f32, y: f32, text: &str) {
        unsafe {
            let t: Vec<u16> = text.encode_utf16().collect();
            self.render_target.DrawText(
                &t,
                &self.label_format,
                &d2d_rect(x_left, y, x_left + width, y + 30.0),
                &self.label_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    /// Draw the left navigation rail (always visible) and return its 5 hit-test
    /// rects in `SettingsPage::ALL` order.
    fn draw_sidebar(&self, active: SettingsPage) -> [RECT; 4] {
        let mut rects = [RECT::default(); 4];
        let item_h = 44.0_f32;
        let gap = 6.0_f32;
        let start_y = 20.0_f32;

        unsafe {
            for (i, page) in SettingsPage::ALL.iter().enumerate() {
                let top = start_y + i as f32 * (item_h + gap);
                let rect = RECT {
                    left: 12,
                    top: top as i32,
                    right: (SIDEBAR_WIDTH - 12.0) as i32,
                    bottom: (top + item_h) as i32,
                };
                rects[i] = rect;

                let selected = *page == active;
                let r = rect_to_d2d(&rect);
                if selected {
                    let rounded = D2D1_ROUNDED_RECT {
                        rect: r,
                        radiusX: 8.0,
                        radiusY: 8.0,
                    };
                    self.render_target
                        .FillRoundedRectangle(&rounded, &self.toggle_on_brush);
                }

                let text_brush = if selected {
                    &self.button_text_brush
                } else {
                    &self.label_brush
                };
                let t: Vec<u16> = page.label().encode_utf16().collect();
                self.render_target.DrawText(
                    &t,
                    &self.label_format,
                    &d2d_rect(r.left + 12.0, r.top, r.right, r.bottom),
                    text_brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                );
            }

            // Separator between the rail and the content area.
            self.render_target.DrawLine(
                D2D_POINT_2F {
                    x: SIDEBAR_WIDTH,
                    y: 0.0,
                },
                D2D_POINT_2F {
                    x: SIDEBAR_WIDTH,
                    y: PANEL_HEIGHT,
                },
                &self.separator_brush,
                1.0,
                None,
            );
        }
        rects
    }

    /// Keybindings page — every key the app responds to, in two sections:
    /// GLOBAL HOTKEYS (Main Overlay, Label Mode — work anywhere in Windows) and
    /// IN-OVERLAY KEYS (Confirm, Dismiss, and the two action modifiers — only
    /// while the overlay is showing).
    fn draw_keybindings_page(
        &self,
        config: &AppConfig,
        state: &DrawState,
        content_left: f32,
        content_right: f32,
        rects: &mut ControlRects,
    ) {
        let label_col_width = 180.0_f32;
        let control_left = content_left + label_col_width;

        // ---- GLOBAL HOTKEYS: work anywhere in Windows ----
        let hk_y = 20.0_f32;
        self.draw_section(content_left, content_right, hk_y, "GLOBAL HOTKEYS");

        // Main hotkey field (y=50)
        let mhk_y = 50.0_f32;
        self.draw_label(content_left, label_col_width, mhk_y + 4.0, "Main Overlay");
        let mhk_rect = RECT {
            left: control_left as i32,
            top: mhk_y as i32,
            right: content_right as i32,
            bottom: (mhk_y + 30.0) as i32,
        };
        rects.main_hotkey = mhk_rect;
        let mhk_text_owned;
        let mhk_text: &str = if state.recording_target == 1 {
            "Press a key combination..."
        } else if !state.main_hotkey_error.is_empty() {
            &state.main_hotkey_error
        } else {
            mhk_text_owned =
                crate::hotkey::format_hotkey(config.hotkey_modifiers, config.hotkey_vk);
            &mhk_text_owned
        };
        self.draw_hotkey_field(
            &mhk_rect,
            mhk_text,
            state.recording_target == 1,
            !state.main_hotkey_error.is_empty(),
        );

        // Label hotkey field (y=90)
        let lhk_y = 90.0_f32;
        self.draw_label(content_left, label_col_width, lhk_y + 4.0, "Label Mode");
        let lhk_rect = RECT {
            left: control_left as i32,
            top: lhk_y as i32,
            right: content_right as i32,
            bottom: (lhk_y + 30.0) as i32,
        };
        rects.label_hotkey = lhk_rect;
        let lhk_text_owned;
        let lhk_text: &str = if state.recording_target == 2 {
            "Press a key combination..."
        } else if !state.label_hotkey_error.is_empty() {
            &state.label_hotkey_error
        } else {
            lhk_text_owned = crate::hotkey::format_hotkey(
                config.label_hotkey_modifiers,
                config.label_hotkey_vk,
            );
            &lhk_text_owned
        };
        self.draw_hotkey_field(
            &lhk_rect,
            lhk_text,
            state.recording_target == 2,
            !state.label_hotkey_error.is_empty(),
        );

        // ---- IN-OVERLAY KEYS: only while the overlay is showing ----
        let ov_y = 140.0_f32;
        self.draw_section(content_left, content_right, ov_y, "IN-OVERLAY KEYS");

        // Confirm field — Space always confirms too; shown in the field text.
        let confirm_y = 170.0_f32;
        self.draw_label(content_left, label_col_width, confirm_y + 4.0, "Confirm");
        let confirm_rect = RECT {
            left: control_left as i32,
            top: confirm_y as i32,
            right: content_right as i32,
            bottom: (confirm_y + 30.0) as i32,
        };
        rects.confirm_hotkey = confirm_rect;
        let confirm_text_owned;
        let confirm_text: &str = if state.recording_target == 3 {
            "Press a key..."
        } else if !state.confirm_error.is_empty() {
            &state.confirm_error
        } else {
            confirm_text_owned = binding_with_fallback(
                config.confirm_modifiers,
                config.confirm_vk,
                crate::keycodes::VK_SPACE,
                true,
            );
            &confirm_text_owned
        };
        self.draw_hotkey_field(
            &confirm_rect,
            confirm_text,
            state.recording_target == 3,
            !state.confirm_error.is_empty(),
        );

        // Dismiss field — Escape always dismisses too; shown in the field text.
        let dismiss_y = 210.0_f32;
        self.draw_label(content_left, label_col_width, dismiss_y + 4.0, "Dismiss");
        let dismiss_rect = RECT {
            left: control_left as i32,
            top: dismiss_y as i32,
            right: content_right as i32,
            bottom: (dismiss_y + 30.0) as i32,
        };
        rects.dismiss_hotkey = dismiss_rect;
        let dismiss_text_owned;
        let dismiss_text: &str = if state.recording_target == 4 {
            "Press a key..."
        } else if !state.dismiss_error.is_empty() {
            &state.dismiss_error
        } else {
            dismiss_text_owned = binding_with_fallback(
                config.dismiss_modifiers,
                config.dismiss_vk,
                crate::keycodes::VK_ESCAPE,
                true,
            );
            &dismiss_text_owned
        };
        self.draw_hotkey_field(
            &dismiss_rect,
            dismiss_text,
            state.recording_target == 4,
            !state.dismiss_error.is_empty(),
        );

        // Tag-assign modifier
        let tag_y = 270.0_f32;
        self.draw_label(
            content_left,
            content_right - content_left,
            tag_y,
            "Tag-assign modifier (hold with 1-9)",
        );
        rects.tag_modifier_buttons =
            self.draw_modifier_buttons(content_left, content_right, tag_y + 26.0, config.tag_modifier);

        // Move-to-monitor modifier
        let move_y = 340.0_f32;
        self.draw_label(
            content_left,
            content_right - content_left,
            move_y,
            "Move-to-monitor modifier (hold with 1-9)",
        );
        rects.move_modifier_buttons = self.draw_modifier_buttons(
            content_left,
            content_right,
            move_y + 26.0,
            config.move_modifier,
        );
    }

    /// Draw the 4 Ctrl/Alt/Shift/Win option buttons spanning `[x_left, x_right]`
    /// at `y`, in `ActionModifier::ALL` order. Returns their hit-test rects.
    fn draw_modifier_buttons(
        &self,
        x_left: f32,
        x_right: f32,
        y: f32,
        selected: ActionModifier,
    ) -> [RECT; 4] {
        let mut rects = [RECT::default(); 4];
        let gap = 8.0_f32;
        let btn_w = (x_right - x_left - 3.0 * gap) / 4.0;
        let btn_h = 28.0_f32;

        for (i, modifier) in ActionModifier::ALL.iter().enumerate() {
            let left = x_left + i as f32 * (btn_w + gap);
            let rect = RECT {
                left: left as i32,
                top: y as i32,
                right: (left + btn_w) as i32,
                bottom: (y + btn_h) as i32,
            };
            rects[i] = rect;
            self.draw_option_button(&rect, modifier.display_name(), *modifier == selected);
        }
        rects
    }

    /// Behavior page: direct-switch and launch-at-startup toggles, plus the
    /// label-mode overlap strategy picker.
    fn draw_behavior_page(
        &self,
        state: &DrawState,
        content_left: f32,
        content_right: f32,
        rects: &mut ControlRects,
    ) {
        let label_col_width = 180.0_f32;

        let beh_y = 20.0_f32;
        self.draw_section(content_left, content_right, beh_y, "BEHAVIOR");

        // Direct switch toggle (y=50)
        let ds_y = 50.0_f32;
        self.draw_label(content_left, label_col_width, ds_y + 5.0, "Direct switch");
        let ds_rect = RECT {
            left: (content_right - 60.0) as i32,
            top: ds_y as i32,
            right: content_right as i32,
            bottom: (ds_y + 24.0) as i32,
        };
        rects.direct_switch_toggle = ds_rect;
        self.draw_toggle(&ds_rect, state.direct_switch);

        // Launch at startup toggle (y=90)
        let las_y = 90.0_f32;
        self.draw_label(content_left, label_col_width, las_y + 5.0, "Launch at startup");
        let las_rect = RECT {
            left: (content_right - 60.0) as i32,
            top: las_y as i32,
            right: content_right as i32,
            bottom: (las_y + 24.0) as i32,
        };
        rects.launch_at_startup_toggle = las_rect;
        self.draw_toggle(&las_rect, state.launch_at_startup);

        // ---- LABEL MODE ----
        let lm_y = 140.0_f32;
        self.draw_section(content_left, content_right, lm_y, "LABEL MODE");

        // Label overlap strategy: AutoNudge / VisibleRegion (y=170)
        let lon_y = 170.0_f32;
        self.draw_label(content_left, label_col_width, lon_y + 5.0, "Label overlap");
        let opt_w = 130.0_f32;
        let opt_h = 28.0_f32;
        let opt_gap = 8.0_f32;
        let opt_start_x = content_right - opt_w * 2.0 - opt_gap;
        let nudge_rect = RECT {
            left: opt_start_x as i32,
            top: lon_y as i32,
            right: (opt_start_x + opt_w) as i32,
            bottom: (lon_y + opt_h) as i32,
        };
        rects.label_overlap_nudge = nudge_rect;
        self.draw_option_button(
            &nudge_rect,
            "Auto nudge",
            state.label_overlap_strategy == LabelOverlapStrategy::AutoNudge,
        );

        let visible_rect = RECT {
            left: (opt_start_x + opt_w + opt_gap) as i32,
            top: lon_y as i32,
            right: (opt_start_x + opt_w * 2.0 + opt_gap) as i32,
            bottom: (lon_y + opt_h) as i32,
        };
        rects.label_overlap_visible = visible_rect;
        self.draw_option_button(
            &visible_rect,
            "Visible region",
            state.label_overlap_strategy == LabelOverlapStrategy::VisibleRegion,
        );
    }

    /// Appearance page: the six numeric sliders.
    fn draw_appearance_page(
        &self,
        state: &DrawState,
        content_left: f32,
        content_right: f32,
        rects: &mut ControlRects,
    ) {
        let label_col_width = 180.0_f32;
        let control_left = content_left + label_col_width;

        let app_y = 20.0_f32;
        self.draw_section(content_left, content_right, app_y, "APPEARANCE");

        // Sliders: overlay_opacity, background_opacity, fade_duration_ms,
        //          grid_padding, label_font_size, title_font_size
        let slider_configs: [(&str, f32, f32, &str); 6] = [
            ("Overlay opacity", 50.0, 255.0, ""),
            ("Background opacity", 0.0, 1.0, ""),
            ("Fade duration", 0.0, 500.0, " ms"),
            ("Grid padding", 4.0, 48.0, " px"),
            ("Label font size", 10.0, 32.0, " px"),
            ("Title font size", 8.0, 24.0, " px"),
        ];

        let slider_base_y = 50.0_f32;
        let slider_row_h = 40.0_f32;
        let slider_left = control_left;
        let slider_right = content_right - 55.0; // leave room for value text

        for (i, (label, _min_val, _max_val, suffix)) in slider_configs.iter().enumerate() {
            let sy = slider_base_y + i as f32 * slider_row_h;
            self.draw_label(content_left, label_col_width, sy + 8.0, label);

            let track_rect = RECT {
                left: slider_left as i32,
                top: (sy + 12.0) as i32,
                right: slider_right as i32,
                bottom: (sy + 18.0) as i32,
            };
            rects.slider_tracks[i] = track_rect;

            let raw_val = state.slider_values[i];
            let t_min = slider_configs[i].1;
            let t_max = slider_configs[i].2;
            self.draw_slider(
                &track_rect,
                raw_val,
                t_min,
                t_max,
                state.active_slider == Some(i),
            );

            // Value label
            let val_text = if i == 0 {
                format!("{}{}", raw_val as u32, suffix)
            } else if i == 1 {
                format!("{:.2}{}", raw_val, suffix)
            } else {
                format!("{:.0}{}", raw_val, suffix)
            };
            unsafe {
                let vt: Vec<u16> = val_text.encode_utf16().collect();
                self.render_target.DrawText(
                    &vt,
                    &self.value_format,
                    &d2d_rect(slider_right + 4.0, sy + 8.0, content_right + 8.0, sy + 28.0),
                    &self.value_brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
    }

    /// Guide page: a read-only two-column reference (key, description) built
    /// entirely from the live `AppConfig` via `hotkey::format_hotkey`, so it
    /// can never drift from the actual configured bindings. Contributes no
    /// rects — this page has nothing to click.
    fn draw_guide_page(&self, config: &AppConfig, content_left: f32, content_right: f32) {
        let g_y = 20.0_f32;
        self.draw_section(content_left, content_right, g_y, "GUIDE");

        let open_key = crate::hotkey::format_hotkey(config.hotkey_modifiers, config.hotkey_vk);
        let label_key =
            crate::hotkey::format_hotkey(config.label_hotkey_modifiers, config.label_hotkey_vk);
        let confirm_key = binding_with_fallback(
            config.confirm_modifiers,
            config.confirm_vk,
            crate::keycodes::VK_SPACE,
            false,
        );
        let dismiss_key = binding_with_fallback(
            config.dismiss_modifiers,
            config.dismiss_vk,
            crate::keycodes::VK_ESCAPE,
            false,
        );
        let tag_key = format!("{}+1-9", config.tag_modifier.display_name());
        let move_key = format!("{}+1-9", config.move_modifier.display_name());

        let rows: [(&str, &str); 8] = [
            (open_key.as_str(), "Open the overlay"),
            (label_key.as_str(), "Open label mode"),
            ("A-Z", "Select a window by its letter"),
            (confirm_key.as_str(), "Confirm the selection"),
            (dismiss_key.as_str(), "Dismiss the overlay"),
            (tag_key.as_str(), "Assign a tag to the selected window"),
            ("1-9", "Jump to a tagged window"),
            (move_key.as_str(), "Move the selected window to a monitor"),
        ];

        let key_col_width = 190.0_f32;
        let desc_col_left = content_left + key_col_width + 16.0;
        let desc_col_width = content_right - desc_col_left;
        let row_base_y = 56.0_f32;
        let row_h = 34.0_f32;

        unsafe {
            for (i, (key, desc)) in rows.iter().enumerate() {
                let y = row_base_y + i as f32 * row_h;

                let kt: Vec<u16> = key.encode_utf16().collect();
                self.render_target.DrawText(
                    &kt,
                    &self.hotkey_format,
                    &d2d_rect(content_left, y, content_left + key_col_width, y + row_h - 6.0),
                    &self.value_brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                );

                let dt: Vec<u16> = desc.encode_utf16().collect();
                self.render_target.DrawText(
                    &dt,
                    &self.label_format,
                    &d2d_rect(
                        desc_col_left,
                        y,
                        desc_col_left + desc_col_width,
                        y + row_h - 6.0,
                    ),
                    &self.label_brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
    }

    /// Draw a hotkey field (rounded rect with text).
    fn draw_hotkey_field(&self, rect: &RECT, text: &str, recording: bool, error: bool) {
        unsafe {
            let r = rect_to_d2d(rect);
            let rounded = D2D1_ROUNDED_RECT {
                rect: r,
                radiusX: 6.0,
                radiusY: 6.0,
            };

            let fill_brush = if recording {
                &self.hotkey_recording_brush
            } else if error {
                &self.hotkey_error_brush
            } else {
                &self.hotkey_field_brush
            };

            self.render_target
                .FillRoundedRectangle(&rounded, fill_brush);
            self.render_target
                .DrawRoundedRectangle(&rounded, &self.separator_brush, 1.0, None);

            let t: Vec<u16> = text.encode_utf16().collect();
            self.render_target.DrawText(
                &t,
                &self.hotkey_format,
                &r,
                &self.section_heading_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    /// Draw a toggle control (pill shape, ON = filled, OFF = outline).
    fn draw_toggle(&self, rect: &RECT, on: bool) {
        unsafe {
            let r = rect_to_d2d(rect);
            let cy = (r.top + r.bottom) / 2.0;
            let knob_r = (r.bottom - r.top) / 2.0 - 2.0;
            let pill = D2D1_ROUNDED_RECT {
                rect: r,
                radiusX: (r.bottom - r.top) / 2.0,
                radiusY: (r.bottom - r.top) / 2.0,
            };

            let fill_brush = if on {
                &self.toggle_on_brush
            } else {
                &self.toggle_off_brush
            };
            self.render_target.FillRoundedRectangle(&pill, fill_brush);

            // Knob position: right if on, left if off
            let knob_x = if on {
                r.right - knob_r - 2.0
            } else {
                r.left + knob_r + 2.0
            };

            let knob_rect = D2D_RECT_F {
                left: knob_x - knob_r,
                top: cy - knob_r,
                right: knob_x + knob_r,
                bottom: cy + knob_r,
            };
            let knob_rounded = D2D1_ROUNDED_RECT {
                rect: knob_rect,
                radiusX: knob_r,
                radiusY: knob_r,
            };
            self.render_target
                .FillRoundedRectangle(&knob_rounded, &self.toggle_knob_brush);
        }
    }

    /// Draw a horizontal slider (track + filled portion + thumb).
    fn draw_slider(
        &self,
        track_rect: &RECT,
        value: f32,
        min_val: f32,
        max_val: f32,
        _active: bool,
    ) {
        unsafe {
            let r = rect_to_d2d(track_rect);
            let range = (max_val - min_val).max(0.001);
            let t = ((value - min_val) / range).clamp(0.0, 1.0);
            let track_w = r.right - r.left;
            let cy = (r.top + r.bottom) / 2.0;
            let thumb_r = 6.0_f32;

            // Full track
            let track_rounded = D2D1_ROUNDED_RECT {
                rect: r,
                radiusX: 2.0,
                radiusY: 2.0,
            };
            self.render_target
                .FillRoundedRectangle(&track_rounded, &self.slider_track_brush);

            // Filled portion
            let fill_x = r.left + t * track_w;
            if fill_x > r.left {
                let fill_rect = D2D1_ROUNDED_RECT {
                    rect: d2d_rect(r.left, r.top, fill_x, r.bottom),
                    radiusX: 2.0,
                    radiusY: 2.0,
                };
                self.render_target
                    .FillRoundedRectangle(&fill_rect, &self.slider_fill_brush);
            }

            // Thumb circle
            let thumb_rect = D2D_RECT_F {
                left: fill_x - thumb_r,
                top: cy - thumb_r,
                right: fill_x + thumb_r,
                bottom: cy + thumb_r,
            };
            let thumb_rounded = D2D1_ROUNDED_RECT {
                rect: thumb_rect,
                radiusX: thumb_r,
                radiusY: thumb_r,
            };
            self.render_target
                .FillRoundedRectangle(&thumb_rounded, &self.slider_thumb_brush);
        }
    }

    /// Draw a rounded button with centered text.
    fn draw_button(&self, rect: &RECT, text: &str) {
        unsafe {
            let r = rect_to_d2d(rect);
            let rounded = D2D1_ROUNDED_RECT {
                rect: r,
                radiusX: 8.0,
                radiusY: 8.0,
            };
            self.render_target
                .FillRoundedRectangle(&rounded, &self.button_brush);
            self.render_target
                .DrawRoundedRectangle(&rounded, &self.separator_brush, 1.0, None);

            let t: Vec<u16> = text.encode_utf16().collect();
            self.render_target.DrawText(
                &t,
                &self.button_format,
                &r,
                &self.button_text_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    /// Draw an option button (radio-button-style pill).
    /// `selected` determines whether it uses the accent fill or the muted background.
    fn draw_option_button(&self, rect: &RECT, text: &str, selected: bool) {
        unsafe {
            let r = rect_to_d2d(rect);
            let rounded = D2D1_ROUNDED_RECT {
                rect: r,
                radiusX: 6.0,
                radiusY: 6.0,
            };

            // Fill: accent when selected, muted dark when not
            let fill_brush = if selected {
                &self.toggle_on_brush
            } else {
                &self.hotkey_field_brush
            };
            self.render_target
                .FillRoundedRectangle(&rounded, fill_brush);

            // Border: subtle in both states
            self.render_target
                .DrawRoundedRectangle(&rounded, &self.separator_brush, 1.0, None);

            // Text: bright when selected, dim when not
            let text_brush = if selected {
                &self.button_text_brush
            } else {
                &self.value_brush
            };
            let t: Vec<u16> = text.encode_utf16().collect();
            self.render_target.DrawText(
                &t,
                &self.button_format,
                &r,
                text_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::binding_with_fallback;
    use crate::keycodes::{MOD_CONTROL, VK_ESCAPE, VK_J, VK_SPACE, VK_TAB};

    // The default Dismiss binding IS Escape, so naively appending the permanent
    // fallback rendered "Esc (or Esc)" in the settings field and on the Guide.
    #[test]
    fn test_binding_matching_fallback_omits_the_suffix() {
        assert_eq!(
            binding_with_fallback(0, VK_ESCAPE, VK_ESCAPE, true),
            "Esc",
            "Dismiss bound to its own fallback must not render 'Esc (or Esc)'"
        );
        assert_eq!(
            binding_with_fallback(0, VK_SPACE, VK_SPACE, false),
            "Space",
            "Confirm bound to Space must not render 'Space or Space'"
        );
    }

    #[test]
    fn test_binding_differing_from_fallback_keeps_the_suffix() {
        assert_eq!(binding_with_fallback(0, VK_TAB, VK_SPACE, true), "Tab (or Space)");
        assert_eq!(binding_with_fallback(0, VK_TAB, VK_SPACE, false), "Tab or Space");
    }

    // A modifier-qualified binding on the fallback key is a *different* combo,
    // so the fallback hint must still show — Ctrl+Esc is not plain Esc.
    #[test]
    fn test_modifier_qualified_fallback_key_keeps_the_suffix() {
        assert_eq!(
            binding_with_fallback(MOD_CONTROL, VK_ESCAPE, VK_ESCAPE, true),
            "Ctrl+Esc (or Esc)"
        );
        assert_eq!(
            binding_with_fallback(MOD_CONTROL, VK_J, VK_SPACE, true),
            "Ctrl+J (or Space)"
        );
    }
}
