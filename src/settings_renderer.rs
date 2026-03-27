/// Direct2D renderer for the settings panel window.
///
/// All drawing is done via Direct2D + DirectWrite, following the same pattern
/// as `overlay_renderer.rs`. No Win32 GDI controls are used.
use crate::config::AppConfig;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_POINT_2F, D2D_RECT_F, D2D_SIZE_U};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_REGULAR, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

/// State passed from the panel manager to the renderer each frame.
#[derive(Debug, Clone)]
pub struct DrawState {
    /// Which hotkey field is in recording mode (0=none, 1=main, 2=label)
    pub recording_target: u8,
    /// Error message for hotkey field 1 (main), or empty
    pub main_hotkey_error: String,
    /// Error message for hotkey field 2 (label), or empty
    pub label_hotkey_error: String,
    /// Index of slider being dragged (0-based), or None
    pub active_slider: Option<usize>,
    /// Slider values [overlay_opacity(0-255), background_opacity(0.0-1.0),
    ///                fade_duration_ms, grid_padding, label_font_size, title_font_size]
    pub slider_values: [f32; 6],
    pub direct_switch: bool,
    pub launch_at_startup: bool,
}

/// Hit-test rectangles for all controls — populated during draw, used for mouse events.
#[derive(Debug, Clone, Default)]
pub struct ControlRects {
    pub main_hotkey: RECT,
    pub label_hotkey: RECT,
    pub direct_switch_toggle: RECT,
    pub launch_at_startup_toggle: RECT,
    /// Track rects for the 6 sliders (overlay_opacity, background_opacity,
    /// fade_duration_ms, grid_padding, label_font_size, title_font_size)
    pub slider_tracks: [RECT; 6],
    pub reset_button: RECT,
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
            let dwrite_factory: IDWriteFactory =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

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
            let bg_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.08, 0.09, 0.13, 1.0), None)?;
            let section_heading_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.9, 0.9, 1.0, 1.0), None)?;
            let label_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.75, 0.77, 0.85, 1.0), None)?;
            let value_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.6, 0.62, 0.7, 1.0), None)?;
            let separator_brush = render_target
                .CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.08), None)?;
            let slider_track_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.2, 0.22, 0.3, 1.0), None)?;
            let slider_fill_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.35, 0.55, 1.0, 1.0), None)?;
            let slider_thumb_brush = render_target
                .CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.95), None)?;
            let toggle_off_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.2, 0.22, 0.3, 1.0), None)?;
            let toggle_on_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.35, 0.55, 1.0, 1.0), None)?;
            let toggle_knob_brush = render_target
                .CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.95), None)?;
            let hotkey_field_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.12, 0.14, 0.20, 1.0), None)?;
            let hotkey_recording_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.35, 0.55, 1.0, 0.25), None)?;
            let hotkey_error_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.9, 0.2, 0.2, 0.25), None)?;
            let button_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.2, 0.22, 0.3, 0.8), None)?;
            let button_text_brush = render_target
                .CreateSolidColorBrush(&d2d_color(0.9, 0.9, 1.0, 1.0), None)?;

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
            let _ = self
                .render_target
                .Resize(&D2D_SIZE_U { width, height });
        }
    }

    /// Render the complete settings panel. Returns updated control hit-test rects.
    pub fn draw_panel(
        &self,
        config: &AppConfig,
        state: &DrawState,
    ) -> ControlRects {
        let mut rects = ControlRects::default();
        unsafe {
            self.render_target.BeginDraw();
            self.render_target
                .Clear(Some(&d2d_color(0.08, 0.09, 0.13, 1.0)));

            let left_margin = 24.0_f32;
            let right_margin = 24.0_f32;
            let panel_width = 480.0_f32;
            let label_col_width = 180.0_f32;
            let control_left = left_margin + label_col_width;
            let control_right = panel_width - right_margin;

            // Helper: draw a section heading + separator line
            let draw_section = |y: f32, title: &str| {
                let t: Vec<u16> = title.encode_utf16().collect();
                self.render_target.DrawText(
                    &t,
                    &self.heading_format,
                    &d2d_rect(left_margin, y, panel_width - right_margin, y + 24.0),
                    &self.section_heading_brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                );
                self.render_target.DrawLine(
                    D2D_POINT_2F {
                        x: left_margin,
                        y: y + 26.0,
                    },
                    D2D_POINT_2F {
                        x: panel_width - right_margin,
                        y: y + 26.0,
                    },
                    &self.separator_brush,
                    1.0,
                    None,
                );
            };

            // Helper: draw a row label
            let draw_label = |y: f32, text: &str| {
                let t: Vec<u16> = text.encode_utf16().collect();
                self.render_target.DrawText(
                    &t,
                    &self.label_format,
                    &d2d_rect(left_margin, y, left_margin + label_col_width, y + 30.0),
                    &self.label_brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                );
            };

            // ---- HOTKEYS SECTION ----
            let hk_y = 20.0_f32;
            draw_section(hk_y, "HOTKEYS");

            // Main hotkey field (y=50)
            let mhk_y = 50.0_f32;
            draw_label(mhk_y + 4.0, "Main Overlay");
            let mhk_rect = RECT {
                left: control_left as i32,
                top: mhk_y as i32,
                right: control_right as i32,
                bottom: (mhk_y + 30.0) as i32,
            };
            rects.main_hotkey = mhk_rect;
            let mhk_text_owned;
            let mhk_text: &str = if state.recording_target == 1 {
                "Press a key combination..."
            } else if !state.main_hotkey_error.is_empty() {
                &state.main_hotkey_error
            } else {
                mhk_text_owned = crate::hotkey::format_hotkey(config.hotkey_modifiers, config.hotkey_vk);
                &mhk_text_owned
            };
            self.draw_hotkey_field(&mhk_rect, mhk_text, state.recording_target == 1, !state.main_hotkey_error.is_empty());

            // Label hotkey field (y=90)
            let lhk_y = 90.0_f32;
            draw_label(lhk_y + 4.0, "Label Mode");
            let lhk_rect = RECT {
                left: control_left as i32,
                top: lhk_y as i32,
                right: control_right as i32,
                bottom: (lhk_y + 30.0) as i32,
            };
            rects.label_hotkey = lhk_rect;
            let lhk_text_owned;
            let lhk_text: &str = if state.recording_target == 2 {
                "Press a key combination..."
            } else if !state.label_hotkey_error.is_empty() {
                &state.label_hotkey_error
            } else {
                lhk_text_owned = crate::hotkey::format_hotkey(config.label_hotkey_modifiers, config.label_hotkey_vk);
                &lhk_text_owned
            };
            self.draw_hotkey_field(&lhk_rect, lhk_text, state.recording_target == 2, !state.label_hotkey_error.is_empty());

            // ---- BEHAVIOR SECTION ----
            let beh_y = 140.0_f32;
            draw_section(beh_y, "BEHAVIOR");

            // Direct switch toggle (y=170)
            let ds_y = 170.0_f32;
            draw_label(ds_y + 5.0, "Direct switch");
            let ds_rect = RECT {
                left: (control_right - 60.0) as i32,
                top: ds_y as i32,
                right: control_right as i32,
                bottom: (ds_y + 24.0) as i32,
            };
            rects.direct_switch_toggle = ds_rect;
            self.draw_toggle(&ds_rect, state.direct_switch);

            // Launch at startup toggle (y=210)
            let las_y = 210.0_f32;
            draw_label(las_y + 5.0, "Launch at startup");
            let las_rect = RECT {
                left: (control_right - 60.0) as i32,
                top: las_y as i32,
                right: control_right as i32,
                bottom: (las_y + 24.0) as i32,
            };
            rects.launch_at_startup_toggle = las_rect;
            self.draw_toggle(&las_rect, state.launch_at_startup);

            // ---- APPEARANCE SECTION ----
            let app_y = 260.0_f32;
            draw_section(app_y, "APPEARANCE");

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

            let slider_base_y = 290.0_f32;
            let slider_row_h = 40.0_f32;
            let slider_left = control_left;
            let slider_right = control_right - 55.0; // leave room for value text

            for (i, (label, _min_val, _max_val, suffix)) in slider_configs.iter().enumerate() {
                let sy = slider_base_y + i as f32 * slider_row_h;
                draw_label(sy + 8.0, label);

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
                self.draw_slider(&track_rect, raw_val, t_min, t_max, state.active_slider == Some(i));

                // Value label
                let val_text = if i == 0 {
                    format!("{}{}", raw_val as u32, suffix)
                } else if i == 1 {
                    format!("{:.2}{}", raw_val, suffix)
                } else {
                    format!("{:.0}{}", raw_val, suffix)
                };
                let vt: Vec<u16> = val_text.encode_utf16().collect();
                self.render_target.DrawText(
                    &vt,
                    &self.value_format,
                    &d2d_rect(slider_right + 4.0, sy + 8.0, control_right + 8.0, sy + 28.0),
                    &self.value_brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                );
            }

            // ---- RESET BUTTON ----
            let btn_y = 540.0_f32;
            let btn_w = 200.0_f32;
            let btn_h = 36.0_f32;
            let btn_x = (panel_width - btn_w) / 2.0;
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
    fn draw_slider(&self, track_rect: &RECT, value: f32, min_val: f32, max_val: f32, _active: bool) {
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
}