use crate::grid_layout::CellRect;
use crate::window_info::WindowInfo;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmQueryThumbnailSourceSize, DwmRegisterThumbnail, DwmUnregisterThumbnail,
    DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES, DWM_TNP_RECTDESTINATION,
    DWM_TNP_OPACITY, DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE,
};

/// A registered DWM thumbnail with its source window and destination cell.
#[allow(dead_code)]
pub struct ThumbnailHandle {
    pub thumbnail_id: isize,
    pub source_hwnd: HWND,
    pub cell_index: usize,
    pub is_blank: bool,
}

// SAFETY: DWM thumbnail handles are opaque values managed by DWM.
unsafe impl Send for ThumbnailHandle {}
unsafe impl Sync for ThumbnailHandle {}

impl Drop for ThumbnailHandle {
    fn drop(&mut self) {
        if self.thumbnail_id != 0 {
            unsafe {
                let _ = DwmUnregisterThumbnail(self.thumbnail_id);
            }
        }
    }
}

/// No reservation — thumbnails fill the full cell. Labels are drawn
/// on a separate overlay window that sits above this one.
const LABEL_STRIP_HEIGHT: f32 = 0.0;
/// Inset margin between cell edge and thumbnail (reveals cell background).
const THUMB_MARGIN: f32 = 6.0;

/// Result of thumbnail registration: handles for DWM management, and the actual
/// letterboxed bounds of each thumbnail (for positioning badges on top).
pub struct ThumbnailRegistration {
    pub handles: Vec<ThumbnailHandle>,
    /// The actual destination rect of each thumbnail after aspect-ratio fitting.
    /// For blank/minimized windows, falls back to the full cell rect.
    pub thumb_bounds: Vec<CellRect>,
}

/// Register DWM thumbnails for all windows in the snapshot.
/// Returns handles and the actual letterboxed thumbnail bounds per window.
pub fn register_thumbnails(
    destination_hwnd: HWND,
    windows: &[WindowInfo],
    cells: &[CellRect],
) -> ThumbnailRegistration {
    let mut handles = Vec::new();
    let mut thumb_bounds = Vec::new();

    for (i, window) in windows.iter().enumerate() {
        if i >= cells.len() {
            break;
        }

        let cell = &cells[i];

        unsafe {
            let thumbnail_id = match DwmRegisterThumbnail(destination_hwnd, window.hwnd) {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!(
                        "DwmRegisterThumbnail failed for HWND {:?}: {:?}",
                        window.hwnd,
                        e
                    );
                    handles.push(ThumbnailHandle {
                        thumbnail_id: 0,
                        source_hwnd: window.hwnd,
                        cell_index: i,
                        is_blank: true,
                    });
                    thumb_bounds.push(*cell);
                    continue;
                }
            };

            // Check for blank thumbnail (minimized windows)
            let source_size = DwmQueryThumbnailSourceSize(thumbnail_id).unwrap_or_default();
            let is_blank = source_size.cx == 0 && source_size.cy == 0;

            if !is_blank {
                let thumb_cell = thumbnail_dest_rect(cell, LABEL_STRIP_HEIGHT);
                let inset_cell = CellRect {
                    x: thumb_cell.x + THUMB_MARGIN,
                    y: thumb_cell.y + THUMB_MARGIN,
                    width: (thumb_cell.width - THUMB_MARGIN * 2.0).max(0.0),
                    height: (thumb_cell.height - THUMB_MARGIN * 2.0).max(0.0),
                    window_index: thumb_cell.window_index,
                };
                let dest_rect = letterbox_rect(&inset_cell, source_size.cx, source_size.cy);

                // Store the actual thumbnail bounds as CellRect
                thumb_bounds.push(CellRect {
                    x: dest_rect.left as f32,
                    y: dest_rect.top as f32,
                    width: (dest_rect.right - dest_rect.left) as f32,
                    height: (dest_rect.bottom - dest_rect.top) as f32,
                    window_index: i,
                });

                let props = DWM_THUMBNAIL_PROPERTIES {
                    dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
                    rcDestination: dest_rect,
                    rcSource: RECT::default(),
                    opacity: 255,
                    fVisible: true.into(),
                    fSourceClientAreaOnly: true.into(),
                    ..Default::default()
                };

                if let Err(e) = DwmUpdateThumbnailProperties(thumbnail_id, &props) {
                    tracing::warn!(
                        "DwmUpdateThumbnailProperties failed for HWND {:?}: {:?}",
                        window.hwnd,
                        e
                    );
                }
            } else {
                thumb_bounds.push(*cell);
            }

            handles.push(ThumbnailHandle {
                thumbnail_id,
                source_hwnd: window.hwnd,
                cell_index: i,
                is_blank,
            });
        }
    }

    ThumbnailRegistration { handles, thumb_bounds }
}

/// Update thumbnail destination rects (e.g., on selection change causing scale-up).
#[allow(dead_code)]
pub fn update_thumbnail_dest(
    handle: &ThumbnailHandle,
    cell: &CellRect,
    opacity: u8,
) {
    if handle.thumbnail_id == 0 || handle.is_blank {
        return;
    }

    unsafe {
        let thumbnail_id = handle.thumbnail_id;
        let dest_rect = cell_to_rect(cell);
        let props = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
            rcDestination: dest_rect,
            rcSource: RECT::default(),
            opacity,
            fVisible: true.into(),
            fSourceClientAreaOnly: true.into(),
            ..Default::default()
        };
        let _ = DwmUpdateThumbnailProperties(thumbnail_id, &props);
    }
}

/// Hide a specific thumbnail (used during fade-out or for blank thumbnails).
#[allow(dead_code)]
pub fn hide_thumbnail(handle: &ThumbnailHandle) {
    if handle.thumbnail_id == 0 {
        return;
    }

    unsafe {
        let thumbnail_id = handle.thumbnail_id;
        let props = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_VISIBLE,
            fVisible: false.into(),
            ..Default::default()
        };
        let _ = DwmUpdateThumbnailProperties(thumbnail_id, &props);
    }
}

/// Convert a CellRect to a Win32 RECT (integer pixel coordinates).
fn cell_to_rect(cell: &CellRect) -> RECT {
    RECT {
        left: cell.x as i32,
        top: cell.y as i32,
        right: (cell.x + cell.width) as i32,
        bottom: (cell.y + cell.height) as i32,
    }
}

/// Compute a destination RECT that fits the source aspect ratio within the cell,
/// centered with letterboxing/pillarboxing as needed.
fn letterbox_rect(cell: &CellRect, src_w: i32, src_h: i32) -> RECT {
    if src_w <= 0 || src_h <= 0 {
        return cell_to_rect(cell);
    }

    let src_aspect = src_w as f32 / src_h as f32;
    let cell_aspect = cell.width / cell.height;

    let (fit_w, fit_h) = if src_aspect > cell_aspect {
        // Source is wider — fit to cell width, letterbox vertically
        (cell.width, cell.width / src_aspect)
    } else {
        // Source is taller — fit to cell height, pillarbox horizontally
        (cell.height * src_aspect, cell.height)
    };

    let offset_x = (cell.width - fit_w) / 2.0;
    let offset_y = (cell.height - fit_h) / 2.0;

    RECT {
        left: (cell.x + offset_x) as i32,
        top: (cell.y + offset_y) as i32,
        right: (cell.x + offset_x + fit_w) as i32,
        bottom: (cell.y + offset_y + fit_h) as i32,
    }
}

/// Compute the thumbnail destination rect with label area reserved at bottom.
/// The thumbnail occupies the top portion; the bottom strip is for labels.
#[allow(dead_code)]
pub fn thumbnail_dest_rect(cell: &CellRect, label_strip_height: f32) -> CellRect {
    CellRect {
        x: cell.x,
        y: cell.y,
        width: cell.width,
        height: (cell.height - label_strip_height).max(0.0),
        window_index: cell.window_index,
    }
}
