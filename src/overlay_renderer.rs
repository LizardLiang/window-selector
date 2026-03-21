use crate::accent_color::AccentColor;
use crate::grid_layout::CellRect;
use crate::window_info::WindowInfo;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget,
    ID2D1SolidColorBrush,
    D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_REGULAR, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_CENTER,
};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

// Logical size constants (scaled by DPI at render time)
const LABEL_FONT_SIZE: f32 = 18.0;
const TITLE_FONT_SIZE: f32 = 13.0;
const BADGE_FONT_SIZE: f32 = 11.0;
const CELL_CORNER_RADIUS: f32 = 12.0;
const LABEL_PILL_CORNER_RADIUS: f32 = 6.0;
const SELECTION_BORDER_WIDTH: f32 = 2.0;
/// Height reserved at the bottom of each cell for the letter label.
/// Must match `LABEL_STRIP_HEIGHT` in `dwm_thumbnails.rs`.
const LABEL_STRIP_HEIGHT: f32 = 40.0;
const BADGE_SIZE: f32 = 20.0;
const LABEL_PILL_W: f32 = 34.0;
const LABEL_PILL_H: f32 = 28.0;
/// Subtle glass border width for cell edges.
const CELL_BORDER_WIDTH: f32 = 1.0;

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

    // Text formats
    pub label_format: IDWriteTextFormat,
    pub title_format: IDWriteTextFormat,
    pub badge_format: IDWriteTextFormat,

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
            let dwrite_factory: IDWriteFactory =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

            // Get client rect
            let mut client_rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut client_rect);
            let width = (client_rect.right - client_rect.left) as u32;
            let height = (client_rect.bottom - client_rect.top) as u32;

            // Create HwndRenderTarget — force 96 DPI so coordinates map 1:1
            // to physical pixels (matching grid cells and DWM thumbnail rects).
            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            let hwnd_rt_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: D2D_SIZE_U { width, height },
                presentOptions: windows::Win32::Graphics::Direct2D::D2D1_PRESENT_OPTIONS_NONE,
            };

            let render_target = d2d_factory.CreateHwndRenderTarget(&rt_props, &hwnd_rt_props)?;

            render_target
                .SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);

            // Create brushes — refined dark palette with subtle cool tint
            let backdrop_brush = render_target.CreateSolidColorBrush(
                &d2d_color(0.02, 0.03, 0.06, 0.86),
                None,
            )?;
            let cell_bg_brush = render_target.CreateSolidColorBrush(
                &d2d_color(0.07, 0.08, 0.12, 0.95),
                None,
            )?;
            let cell_border_brush = render_target.CreateSolidColorBrush(
                &d2d_color(1.0, 1.0, 1.0, 0.07),
                None,
            )?;
            let text_brush = render_target.CreateSolidColorBrush(
                &d2d_color(1.0, 1.0, 1.0, 0.95),
                None,
            )?;
            // Frosted glass pill for unselected letter labels.
            let label_semi_brush = render_target.CreateSolidColorBrush(
                &d2d_color(1.0, 1.0, 1.0, 0.12),
                None,
            )?;
            // White text on frosted glass pill.
            let label_dark_text_brush = render_target.CreateSolidColorBrush(
                &d2d_color(1.0, 1.0, 1.0, 0.92),
                None,
            )?;
            let label_accent_brush = render_target.CreateSolidColorBrush(
                &d2d_color(accent.r, accent.g, accent.b, 0.90),
                None,
            )?;
            let selection_border_brush = render_target.CreateSolidColorBrush(
                &d2d_color(accent.r, accent.g, accent.b, 0.85),
                None,
            )?;
            let selection_fill_brush = render_target.CreateSolidColorBrush(
                &d2d_color(accent.r, accent.g, accent.b, 0.10),
                None,
            )?;
            // Aura glow layers — concentric bloom rings around selected cells
            let aura_brush_1 = render_target.CreateSolidColorBrush(
                &d2d_color(accent.r, accent.g, accent.b, 0.25),
                None,
            )?;
            let aura_brush_2 = render_target.CreateSolidColorBrush(
                &d2d_color(accent.r, accent.g, accent.b, 0.14),
                None,
            )?;
            let aura_brush_3 = render_target.CreateSolidColorBrush(
                &d2d_color(accent.r, accent.g, accent.b, 0.07),
                None,
            )?;
            // Ambient glow — soft luminance around every cell
            let ambient_glow_brush = render_target.CreateSolidColorBrush(
                &d2d_color(1.0, 1.0, 1.0, 0.06),
                None,
            )?;
            let badge_brush = render_target.CreateSolidColorBrush(
                &d2d_color(1.0, 0.75, 0.15, 0.90),
                None,
            )?;
            let badge_text_brush = render_target.CreateSolidColorBrush(
                &d2d_color(0.0, 0.0, 0.0, 0.95),
                None,
            )?;

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
                label_format,
                title_format,
                badge_format,
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

            self.render_target =
                self.d2d_factory.CreateHwndRenderTarget(&rt_props, &hwnd_rt_props)?;
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
            self.render_target.Clear(Some(&d2d_color(0.0, 0.0, 0.0, 0.0)));

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

            if let Err(e) = self.render_target.EndDraw(None, None) {
                tracing::error!("Direct2D EndDraw failed (device lost): {:?}", e);
            }
        }
    }

    fn draw_cell(&self, cell: &CellRect, window: &WindowInfo, is_selected: bool) {
        unsafe {
            let dpi = self.dpi_scale;
            let corner_r = CELL_CORNER_RADIUS * dpi;
            let label_h = LABEL_STRIP_HEIGHT * dpi;

            // --- Aura glow layers (drawn BEFORE cell so they appear behind) ---
            if is_selected {
                // 3-layer bloom: each ring is progressively larger and more transparent
                let aura_layers: [(&ID2D1SolidColorBrush, f32); 3] = [
                    (&self.aura_brush_3, 24.0 * dpi), // outermost — faintest
                    (&self.aura_brush_2, 14.0 * dpi),
                    (&self.aura_brush_1, 6.0 * dpi),  // innermost — brightest
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
                    self.render_target.FillRoundedRectangle(&aura_rounded, *brush);
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

    /// Resize the render target to match a new window size.
    #[allow(dead_code)]
    pub fn resize(&self, width: u32, height: u32) {
        unsafe {
            let _ = self.render_target.Resize(&D2D_SIZE_U { width, height });
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
