use crate::accent_color::AccentColor;
use crate::grid_layout::{CellRect, QUICK_LIST_BAR_HEIGHT};
use crate::window_info::WindowInfo;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT,
    D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_REGULAR, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetSystemMetrics, GetWindowRect, SM_CXPADDEDBORDER, SM_CXSIZEFRAME,
};

// Cell rendering constants — logical sizes scaled by DPI in draw_cell().
const LABEL_FONT_SIZE: f32 = 18.0;
const TITLE_FONT_SIZE: f32 = 13.0;
const BADGE_FONT_SIZE: f32 = 11.0;
const CELL_CORNER_RADIUS: f32 = 12.0;
const SELECTION_BORDER_WIDTH: f32 = 2.0;
/// Height reserved at the bottom of each cell for the letter label.
/// Must match `LABEL_STRIP_HEIGHT` in `dwm_thumbnails.rs`.
const LABEL_STRIP_HEIGHT: f32 = 40.0;
/// Subtle glass border width for cell edges.
const CELL_BORDER_WIDTH: f32 = 1.0;

// Quick list bar constants — physical pixel values used as-is.
// The render target is forced to 96 DPI so coordinates map 1:1 to physical
// pixels. These constants must NOT be multiplied by dpi_scale; doing so would
// cause the bar to render taller than the space reserved by overlay.rs.
/// Font size for quick list bar entries (scaled at text-format creation time).
const QUICK_LIST_FONT_SIZE: f32 = 11.5;
/// Horizontal padding inside each quick list entry.
const QUICK_LIST_ENTRY_PADDING_X: f32 = 8.0;
/// Maximum width of the title portion within a quick list entry.
const QUICK_LIST_TITLE_MAX_WIDTH: f32 = 140.0;
/// Width of the letter badge portion in the quick list entry.
const QUICK_LIST_LETTER_WIDTH: f32 = 18.0;
/// Corner radius for the selected entry highlight pill.
const QUICK_LIST_PILL_RADIUS: f32 = 4.0;
/// Size of the number tag badge (square) in the quick list.
const QUICK_LIST_TAG_SIZE: f32 = 16.0;
/// Separator bar between grid area and quick list.
const QUICK_LIST_SEPARATOR_HEIGHT: f32 = 1.0;

// Label mode constants — for small labels above window title bars
/// Width of each label in label mode (physical pixels).
const LABEL_MODE_WIDTH: f32 = 60.0;
/// Height of each label in label mode (physical pixels).
const LABEL_MODE_HEIGHT: f32 = 48.0;
/// Corner radius for label mode badges.
const LABEL_MODE_CORNER_RADIUS: f32 = 8.0;

/// The Direct2D + DirectWrite rendering context for an overlay window.
#[allow(dead_code)]
pub struct OverlayRenderer {
    pub d2d_factory: ID2D1Factory,
    pub dwrite_factory: IDWriteFactory,
    pub render_target: ID2D1HwndRenderTarget,

    // Brushes
    pub backdrop_brush: ID2D1SolidColorBrush,
    pub cell_bg_brush: ID2D1SolidColorBrush,
    pub cell_border_brush: ID2D1SolidColorBrush,
    pub text_brush: ID2D1SolidColorBrush,
    pub label_semi_brush: ID2D1SolidColorBrush,
    pub label_dark_text_brush: ID2D1SolidColorBrush,
    pub label_accent_brush: ID2D1SolidColorBrush,
    pub selection_border_brush: ID2D1SolidColorBrush,
    pub selection_fill_brush: ID2D1SolidColorBrush,
    // Aura glow layers (inner → outer, decreasing opacity)
    pub aura_brush_1: ID2D1SolidColorBrush,
    pub aura_brush_2: ID2D1SolidColorBrush,
    pub aura_brush_3: ID2D1SolidColorBrush,
    pub ambient_glow_brush: ID2D1SolidColorBrush,
    pub badge_brush: ID2D1SolidColorBrush,
    pub badge_text_brush: ID2D1SolidColorBrush,
    // Quick list bar
    pub quick_list_bg_brush: ID2D1SolidColorBrush,
    pub quick_list_separator_brush: ID2D1SolidColorBrush,
    pub quick_list_text_brush: ID2D1SolidColorBrush,
    pub quick_list_dim_brush: ID2D1SolidColorBrush,
    pub quick_list_selected_brush: ID2D1SolidColorBrush,
    pub quick_list_selected_text_brush: ID2D1SolidColorBrush,

    // Text formats
    pub label_format: IDWriteTextFormat,
    pub title_format: IDWriteTextFormat,
    pub badge_format: IDWriteTextFormat,
    pub quick_list_format: IDWriteTextFormat,

    pub dpi_scale: f32,
    pub accent: AccentColor,
}

impl OverlayRenderer {
    /// Initialize Direct2D and DirectWrite for the given HWND.
    pub fn new(hwnd: HWND, dpi_scale: f32, accent: AccentColor) -> windows::core::Result<Self> {
        unsafe {
            // D2D factory
            let d2d_factory: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;

            // DWrite factory
            let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

            // Get client rect
            let mut client_rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut client_rect);
            let width = (client_rect.right - client_rect.left) as u32;
            let height = (client_rect.bottom - client_rect.top) as u32;

            // Create HwndRenderTarget — force 96 DPI so coordinates map 1:1
            // to physical pixels (matching grid cells and DWM thumbnail rects).
            // Enable alpha mode for transparent background support.
            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                dpiX: 96.0,
                dpiY: 96.0,
                pixelFormat: windows::Win32::Graphics::Direct2D::Common::D2D1_PIXEL_FORMAT {
                    format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode:
                        windows::Win32::Graphics::Direct2D::Common::D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                ..Default::default()
            };
            let hwnd_rt_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: D2D_SIZE_U { width, height },
                presentOptions: windows::Win32::Graphics::Direct2D::D2D1_PRESENT_OPTIONS_NONE,
            };

            let render_target = d2d_factory.CreateHwndRenderTarget(&rt_props, &hwnd_rt_props)?;

            render_target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);

            // Create brushes — refined dark palette with subtle cool tint
            let backdrop_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.02, 0.03, 0.06, 0.86), None)?;
            let cell_bg_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.07, 0.08, 0.12, 0.95), None)?;
            let cell_border_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.07), None)?;
            let text_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.95), None)?;
            // Frosted glass pill for unselected letter labels.
            let label_semi_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.12), None)?;
            // White text on frosted glass pill.
            let label_dark_text_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.92), None)?;
            let label_accent_brush = render_target
                .CreateSolidColorBrush(&d2d_color(accent.r, accent.g, accent.b, 0.90), None)?;
            let selection_border_brush = render_target
                .CreateSolidColorBrush(&d2d_color(accent.r, accent.g, accent.b, 0.85), None)?;
            let selection_fill_brush = render_target
                .CreateSolidColorBrush(&d2d_color(accent.r, accent.g, accent.b, 0.10), None)?;
            // Aura glow layers — concentric bloom rings around selected cells
            let aura_brush_1 = render_target
                .CreateSolidColorBrush(&d2d_color(accent.r, accent.g, accent.b, 0.25), None)?;
            let aura_brush_2 = render_target
                .CreateSolidColorBrush(&d2d_color(accent.r, accent.g, accent.b, 0.14), None)?;
            let aura_brush_3 = render_target
                .CreateSolidColorBrush(&d2d_color(accent.r, accent.g, accent.b, 0.07), None)?;
            // Ambient glow — soft luminance around every cell
            let ambient_glow_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.06), None)?;
            let badge_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 0.75, 0.15, 0.90), None)?;
            let badge_text_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.0, 0.0, 0.0, 0.95), None)?;

            // Quick list bar brushes
            // Background: slightly lighter than main backdrop
            let quick_list_bg_brush =
                render_target.CreateSolidColorBrush(&d2d_color(0.04, 0.05, 0.09, 0.92), None)?;
            // 1-pixel separator line between grid area and quick list
            let quick_list_separator_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.08), None)?;
            // Dim white for unselected entry text
            let quick_list_text_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.65), None)?;
            // Even dimmer for non-selected letter badges
            let quick_list_dim_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.38), None)?;
            // Accent-colored pill for the selected entry
            let quick_list_selected_brush = render_target
                .CreateSolidColorBrush(&d2d_color(accent.r, accent.g, accent.b, 0.22), None)?;
            // Bright white text for the selected entry
            let quick_list_selected_text_brush =
                render_target.CreateSolidColorBrush(&d2d_color(1.0, 1.0, 1.0, 0.95), None)?;

            // Text formats — Segoe UI Variable for Windows 11 polish
            let font_name: Vec<u16> = "Segoe UI Variable\0".encode_utf16().collect();
            let locale: Vec<u16> = "en-us\0".encode_utf16().collect();

            let label_format = dwrite_factory.CreateTextFormat(
                PCWSTR(font_name.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                LABEL_FONT_SIZE * dpi_scale,
                PCWSTR(locale.as_ptr()),
            )?;
            label_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            label_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            let title_format = dwrite_factory.CreateTextFormat(
                PCWSTR(font_name.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                TITLE_FONT_SIZE * dpi_scale,
                PCWSTR(locale.as_ptr()),
            )?;
            title_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            title_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            let badge_format = dwrite_factory.CreateTextFormat(
                PCWSTR(font_name.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                BADGE_FONT_SIZE * dpi_scale,
                PCWSTR(locale.as_ptr()),
            )?;
            badge_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            badge_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            // Quick list text format — compact, left-aligned, vertically centered
            let quick_list_format = dwrite_factory.CreateTextFormat(
                PCWSTR(font_name.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                QUICK_LIST_FONT_SIZE * dpi_scale,
                PCWSTR(locale.as_ptr()),
            )?;
            quick_list_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            quick_list_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            Ok(Self {
                d2d_factory,
                dwrite_factory,
                render_target,
                backdrop_brush,
                cell_bg_brush,
                cell_border_brush,
                text_brush,
                label_semi_brush,
                label_dark_text_brush,
                label_accent_brush,
                selection_border_brush,
                selection_fill_brush,
                aura_brush_1,
                aura_brush_2,
                aura_brush_3,
                ambient_glow_brush,
                badge_brush,
                badge_text_brush,
                quick_list_bg_brush,
                quick_list_separator_brush,
                quick_list_text_brush,
                quick_list_dim_brush,
                quick_list_selected_brush,
                quick_list_selected_text_brush,
                label_format,
                title_format,
                badge_format,
                quick_list_format,
                dpi_scale,
                accent,
            })
        }
    }

    /// Recreate the render target after device loss.
    #[allow(dead_code)]
    pub fn recreate_render_target(&mut self, hwnd: HWND) -> windows::core::Result<()> {
        unsafe {
            let mut client_rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut client_rect);
            let width = (client_rect.right - client_rect.left) as u32;
            let height = (client_rect.bottom - client_rect.top) as u32;

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES::default();
            let hwnd_rt_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: D2D_SIZE_U { width, height },
                presentOptions: windows::Win32::Graphics::Direct2D::D2D1_PRESENT_OPTIONS_NONE,
            };

            self.render_target = self
                .d2d_factory
                .CreateHwndRenderTarget(&rt_props, &hwnd_rt_props)?;
            self.render_target
                .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);

            tracing::info!("Direct2D render target recreated (device recovery)");
        }
        Ok(())
    }

    /// Render the full overlay frame.
    pub fn render(
        &self,
        windows: &[WindowInfo],
        cells: &[CellRect],
        selected: Option<usize>,
        area_width: f32,
        area_height: f32,
    ) {
        unsafe {
            self.render_target.BeginDraw();

            // Clear to transparent
            self.render_target
                .Clear(Some(&d2d_color(0.0, 0.0, 0.0, 0.0)));

            // Draw backdrop
            let full_rect = d2d_rect(0.0, 0.0, area_width, area_height);
            self.render_target
                .FillRectangle(&full_rect, &self.backdrop_brush);

            if windows.is_empty() {
                self.draw_empty_state(area_width, area_height);
            } else {
                for (i, window) in windows.iter().enumerate() {
                    if i >= cells.len() {
                        break;
                    }
                    let cell = &cells[i];
                    let is_selected = selected == Some(i);
                    self.draw_cell(cell, window, is_selected);
                }
            }

            // Quick list bar drawn over the reserved strip at the bottom.
            self.draw_quick_list(windows, selected, area_width, area_height);

            if let Err(e) = self.render_target.EndDraw(None, None) {
                tracing::error!("Direct2D EndDraw failed (device lost): {:?}", e);
            }
        }
    }

    fn draw_cell(&self, cell: &CellRect, _window: &WindowInfo, is_selected: bool) {
        unsafe {
            let dpi = self.dpi_scale;
            let corner_r = CELL_CORNER_RADIUS * dpi;
            let _label_h = LABEL_STRIP_HEIGHT * dpi;

            // --- Aura glow layers (drawn BEFORE cell so they appear behind) ---
            if is_selected {
                // 3-layer bloom: each ring is progressively larger and more transparent
                let aura_layers: [(&ID2D1SolidColorBrush, f32); 3] = [
                    (&self.aura_brush_3, 24.0 * dpi), // outermost — faintest
                    (&self.aura_brush_2, 14.0 * dpi),
                    (&self.aura_brush_1, 6.0 * dpi), // innermost — brightest
                ];
                for (brush, expand) in &aura_layers {
                    let aura_rect = d2d_rect(
                        cell.x - expand,
                        cell.y - expand,
                        cell.x + cell.width + expand,
                        cell.y + cell.height + expand,
                    );
                    let aura_rounded = D2D1_ROUNDED_RECT {
                        rect: aura_rect,
                        radiusX: corner_r + expand,
                        radiusY: corner_r + expand,
                    };
                    self.render_target
                        .FillRoundedRectangle(&aura_rounded, *brush);
                }
            } else {
                // Ambient glow — subtle luminance halo around every cell
                let expand = 3.0 * dpi;
                let glow_rect = d2d_rect(
                    cell.x - expand,
                    cell.y - expand,
                    cell.x + cell.width + expand,
                    cell.y + cell.height + expand,
                );
                let glow_rounded = D2D1_ROUNDED_RECT {
                    rect: glow_rect,
                    radiusX: corner_r + expand,
                    radiusY: corner_r + expand,
                };
                self.render_target
                    .FillRoundedRectangle(&glow_rounded, &self.ambient_glow_brush);
            }

            // Cell background
            let cell_rect = d2d_rect(cell.x, cell.y, cell.x + cell.width, cell.y + cell.height);
            let rounded = D2D1_ROUNDED_RECT {
                rect: cell_rect,
                radiusX: corner_r,
                radiusY: corner_r,
            };
            self.render_target
                .FillRoundedRectangle(&rounded, &self.cell_bg_brush);

            // Glass border — subtle white edge for depth
            self.render_target.DrawRoundedRectangle(
                &rounded,
                &self.cell_border_brush,
                CELL_BORDER_WIDTH * dpi,
                None,
            );

            // Selection: accent fill + border
            if is_selected {
                self.render_target
                    .FillRoundedRectangle(&rounded, &self.selection_fill_brush);
                self.render_target.DrawRoundedRectangle(
                    &rounded,
                    &self.selection_border_brush,
                    SELECTION_BORDER_WIDTH * dpi,
                    None,
                );
            }

            // Letters, titles, and number badges are rendered on the label
            // overlay (GDI) which sits above DWM thumbnails — not here.
        }
    }

    fn draw_empty_state(&self, area_width: f32, area_height: f32) {
        let text: Vec<u16> = "No windows open.".encode_utf16().collect();
        let rect = d2d_rect(
            area_width * 0.2,
            area_height * 0.45,
            area_width * 0.8,
            area_height * 0.55,
        );
        unsafe {
            self.render_target.DrawText(
                &text,
                &self.title_format,
                &rect,
                &self.text_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    /// Draw the compact quick-list bar at the very bottom of the overlay.
    ///
    /// The bar occupies the strip from `(area_height - QUICK_LIST_BAR_HEIGHT)` to
    /// `area_height`. Each window gets a compact entry showing its letter and a
    /// truncated title. The selected window entry is highlighted with the accent colour.
    fn draw_quick_list(
        &self,
        windows: &[WindowInfo],
        selected: Option<usize>,
        area_width: f32,
        area_height: f32,
    ) {
        if windows.is_empty() {
            return;
        }

        unsafe {
            // The render target is forced to 96 DPI (physical pixels), so all
            // layout constants are used as-is — no DPI scaling applied here.
            // The grid in overlay.rs reserves `QUICK_LIST_BAR_HEIGHT` physical
            // pixels, and this function must use the same value without scaling.
            let bar_h = QUICK_LIST_BAR_HEIGHT;
            let bar_top = area_height - bar_h;

            // Background strip
            let bar_rect = d2d_rect(0.0, bar_top, area_width, area_height);
            self.render_target
                .FillRectangle(&bar_rect, &self.quick_list_bg_brush);

            // Separator line at the top edge of the strip
            let sep_h = QUICK_LIST_SEPARATOR_HEIGHT;
            let sep_rect = d2d_rect(0.0, bar_top, area_width, bar_top + sep_h);
            self.render_target
                .FillRectangle(&sep_rect, &self.quick_list_separator_brush);

            // Layout: each entry is [pad] [letter] [gap] [tag?] [gap?] [title] [pad]
            // We compute a fixed entry width and pack them left-to-right, clipping
            // to the bar width if there are more windows than fit.
            let entry_pad_x = QUICK_LIST_ENTRY_PADDING_X;
            let letter_w = QUICK_LIST_LETTER_WIDTH;
            let title_max_w = QUICK_LIST_TITLE_MAX_WIDTH;
            let tag_sz = QUICK_LIST_TAG_SIZE;
            let gap = 4.0;
            // Reserve space for tag badge in every entry so alignment stays uniform
            let entry_w = entry_pad_x + letter_w + gap + tag_sz + gap + title_max_w + entry_pad_x;
            // Pill corner radius for the selected highlight
            let pill_r = QUICK_LIST_PILL_RADIUS;
            // Vertical padding inside each entry (above/below text)
            let v_pad = 4.0;
            let pill_top = bar_top + v_pad;
            let pill_bottom = area_height - v_pad;

            let mut cursor_x = entry_pad_x / 2.0;

            for (i, window) in windows.iter().enumerate() {
                // Stop drawing if we've run past the right edge of the screen
                if cursor_x + entry_w > area_width {
                    break;
                }

                let is_selected = selected == Some(i);

                // Highlight pill for selected entry
                if is_selected {
                    let pill_rect = d2d_rect(cursor_x, pill_top, cursor_x + entry_w, pill_bottom);
                    let pill_rounded = D2D1_ROUNDED_RECT {
                        rect: pill_rect,
                        radiusX: pill_r,
                        radiusY: pill_r,
                    };
                    self.render_target
                        .FillRoundedRectangle(&pill_rounded, &self.quick_list_selected_brush);
                }

                // Letter badge text
                let letter_str: Vec<u16> = match window.letter {
                    Some(c) => c.to_uppercase().to_string().encode_utf16().collect(),
                    None => "-".encode_utf16().collect(),
                };
                let letter_rect = d2d_rect(
                    cursor_x + entry_pad_x,
                    bar_top,
                    cursor_x + entry_pad_x + letter_w,
                    area_height,
                );
                let letter_brush = if is_selected {
                    &self.quick_list_selected_text_brush
                } else {
                    &self.quick_list_dim_brush
                };
                self.render_target.DrawText(
                    &letter_str,
                    &self.quick_list_format,
                    &letter_rect,
                    letter_brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                );

                // Number tag badge — small accent circle with digit
                let tag_x = cursor_x + entry_pad_x + letter_w + gap;
                if let Some(tag) = window.number_tag {
                    let tag_y = bar_top + (bar_h - tag_sz) / 2.0;
                    let tag_rect = d2d_rect(tag_x, tag_y, tag_x + tag_sz, tag_y + tag_sz);
                    let tag_rounded = D2D1_ROUNDED_RECT {
                        rect: tag_rect,
                        radiusX: tag_sz / 2.0,
                        radiusY: tag_sz / 2.0,
                    };
                    self.render_target
                        .FillRoundedRectangle(&tag_rounded, &self.label_accent_brush);
                    let tag_str: Vec<u16> = tag.to_string().encode_utf16().collect();
                    self.render_target.DrawText(
                        &tag_str,
                        &self.badge_format,
                        &tag_rect,
                        &self.badge_text_brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                    );
                }

                // Title text (truncated via clip rect)
                let title_x = tag_x + tag_sz + gap;
                let title_rect = d2d_rect(title_x, bar_top, title_x + title_max_w, area_height);
                let title_brush = if is_selected {
                    &self.quick_list_selected_text_brush
                } else {
                    &self.quick_list_text_brush
                };
                // Trim title to a reasonable length before encoding to avoid huge
                // temporary allocations for very long window titles.
                let title_trimmed: String = window.title.chars().take(60).collect();
                let title_utf16: Vec<u16> = title_trimmed.encode_utf16().collect();
                if !title_utf16.is_empty() {
                    self.render_target.DrawText(
                        &title_utf16,
                        &self.quick_list_format,
                        &title_rect,
                        title_brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                    );
                }

                cursor_x += entry_w;
            }
        }
    }

    /// Resize the render target to match a new window size.
    #[allow(dead_code)]
    pub fn resize(&self, width: u32, height: u32) {
        unsafe {
            let _ = self.render_target.Resize(&D2D_SIZE_U { width, height });
        }
    }

    /// Render label mode: draw small letter labels above each window's title bar.
    ///
    /// `overlay_hwnd` is used to get the overlay window's position for coordinate conversion.
    pub fn render_labels_only(
        &self,
        windows: &[WindowInfo],
        selected: Option<usize>,
        overlay_hwnd: HWND,
    ) {
        tracing::info!(
            "render_labels_only: {} windows, selected={:?}",
            windows.len(),
            selected
        );

        // Get overlay window position (top-left corner in screen coordinates)
        let overlay_offset = unsafe {
            let mut overlay_rect = RECT::default();
            if GetWindowRect(overlay_hwnd, &mut overlay_rect).is_ok() {
                (overlay_rect.left, overlay_rect.top)
            } else {
                tracing::warn!("Failed to get overlay window rect, using (0, 0)");
                (0, 0)
            }
        };

        tracing::debug!(
            "Overlay window offset: ({}, {})",
            overlay_offset.0,
            overlay_offset.1
        );

        unsafe {
            self.render_target.BeginDraw();

            // Clear to color-key color (RGB 1,1,1) for transparency via LWA_COLORKEY
            // This is a very dark gray that will be made transparent by the layered window attribute
            self.render_target
                .Clear(Some(&d2d_color(1.0 / 255.0, 1.0 / 255.0, 1.0 / 255.0, 1.0)));

            // Draw a label for each window
            for (i, window) in windows.iter().enumerate() {
                let is_selected = selected == Some(i);
                tracing::debug!(
                    "Drawing label for window {}: {:?}, letter={:?}, minimized={}, selected={}",
                    i,
                    window.hwnd,
                    window.letter,
                    window.is_minimized,
                    is_selected
                );
                self.draw_label_for_window(window, is_selected, overlay_offset);
            }

            if let Err(e) = self.render_target.EndDraw(None, None) {
                tracing::error!("Direct2D EndDraw failed in label mode: {:?}", e);
            }
        }
    }

    /// Draw a single label above a window's title bar.
    ///
    /// `overlay_offset` is the (left, top) position of the overlay window in screen coordinates.
    fn draw_label_for_window(
        &self,
        window: &WindowInfo,
        is_selected: bool,
        overlay_offset: (i32, i32),
    ) {
        unsafe {
            // Get window rectangle (absolute screen coordinates)
            let mut window_rect = RECT::default();
            if GetWindowRect(window.hwnd, &mut window_rect).is_err() {
                tracing::warn!("Failed to get window rect for {:?}", window.hwnd);
                return;
            }

            tracing::debug!(
                "Window rect for {:?}: left={}, top={}, right={}, bottom={}",
                window.hwnd,
                window_rect.left,
                window_rect.top,
                window_rect.right,
                window_rect.bottom
            );

            // Skip minimized windows (they don't have visible title bars)
            if window.is_minimized {
                tracing::debug!("Skipping minimized window {:?}", window.hwnd);
                return;
            }

            // Calculate label position (in screen coordinates)
            let (screen_x, screen_y) = calculate_label_position(&window_rect);

            // Convert to overlay client coordinates
            let label_x = screen_x - overlay_offset.0 as f32;
            let label_y = screen_y - overlay_offset.1 as f32;

            tracing::debug!(
                "Label position for {:?}: screen=({}, {}), client=({}, {}), letter={:?}",
                window.hwnd,
                screen_x,
                screen_y,
                label_x,
                label_y,
                window.letter
            );

            // Label dimensions
            let label_w = LABEL_MODE_WIDTH;
            let label_h = LABEL_MODE_HEIGHT;
            let corner_r = LABEL_MODE_CORNER_RADIUS;

            // Draw glow effect for selected label (3 layers)
            if is_selected {
                let glow_layers: [(&ID2D1SolidColorBrush, f32); 3] = [
                    (&self.aura_brush_3, 16.0), // outermost
                    (&self.aura_brush_2, 10.0),
                    (&self.aura_brush_1, 4.0), // innermost
                ];
                for (brush, expand) in &glow_layers {
                    let glow_rect = d2d_rect(
                        label_x - expand,
                        label_y - expand,
                        label_x + label_w + expand,
                        label_y + label_h + expand,
                    );
                    let glow_rounded = D2D1_ROUNDED_RECT {
                        rect: glow_rect,
                        radiusX: corner_r + expand,
                        radiusY: corner_r + expand,
                    };
                    self.render_target
                        .FillRoundedRectangle(&glow_rounded, *brush);
                }
            }

            // Draw label background
            let label_rect = d2d_rect(label_x, label_y, label_x + label_w, label_y + label_h);
            let rounded = D2D1_ROUNDED_RECT {
                rect: label_rect,
                radiusX: corner_r,
                radiusY: corner_r,
            };

            let bg_brush = if is_selected {
                &self.label_accent_brush
            } else {
                &self.label_semi_brush
            };
            self.render_target.FillRoundedRectangle(&rounded, bg_brush);

            // Draw letter text
            if let Some(letter) = window.letter {
                let letter_str: Vec<u16> =
                    letter.to_uppercase().to_string().encode_utf16().collect();
                let text_brush = if is_selected {
                    &self.badge_text_brush // Dark text on accent background
                } else {
                    &self.label_dark_text_brush // White text on frosted glass
                };

                tracing::debug!("Drawing letter '{}' for window {:?}", letter, window.hwnd);

                self.render_target.DrawText(
                    &letter_str,
                    &self.label_format,
                    &label_rect,
                    text_brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
                );
            } else {
                tracing::warn!("Window {:?} has no letter assigned!", window.hwnd);
            }
        }
    }
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

/// Calculate the position for a label inside a window (top-left corner area).
/// Returns (x, y) coordinates for the top-left corner of the label.
///
/// The label is positioned inside the window to avoid going off-screen,
/// especially for maximized or full-screen windows.
fn calculate_label_position(window_rect: &RECT) -> (f32, f32) {
    unsafe {
        // Get system metrics for window border width
        let border_width = GetSystemMetrics(SM_CXSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
        let title_bar_height =
            GetSystemMetrics(windows::Win32::UI::WindowsAndMessaging::SM_CYCAPTION);

        // Position label in the top-left corner, inside the window
        // Add some padding from the edge
        let padding = 12.0;
        let label_x = window_rect.left as f32 + border_width as f32 + padding;

        // Position below the title bar, inside the client area
        let label_y =
            window_rect.top as f32 + border_width as f32 + title_bar_height as f32 + padding;

        (label_x, label_y)
    }
}
