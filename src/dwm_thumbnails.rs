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

/// Register DWM thumbnails for all windows in the snapshot.
/// Returns a Vec of ThumbnailHandle, one per window.
/// Windows with blank thumbnails (e.g., minimized) are marked is_blank=true.
///
/// Thumbnails are placed in the upper portion of each cell, leaving the bottom
/// `LABEL_STRIP_HEIGHT` pixels free for the letter label rendered by Direct2D.
pub fn register_thumbnails(
    destination_hwnd: HWND,
    windows: &[WindowInfo],
    cells: &[CellRect],
) -> Vec<ThumbnailHandle> {
    let mut handles = Vec::new();

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
                    continue;
                }
            };

            // Check for blank thumbnail (minimized windows)
            let source_size = DwmQueryThumbnailSourceSize(thumbnail_id).unwrap_or_default();
            let is_blank = source_size.cx == 0 && source_size.cy == 0;

            if !is_blank {
                // Reserve the bottom of the cell for the letter label strip.
                let thumb_cell = thumbnail_dest_rect(cell, LABEL_STRIP_HEIGHT);
                let dest_rect = cell_to_rect(&thumb_cell);
                tracing::debug!(
                    "Thumbnail[{}] dest=({},{},{},{}) label_strip={}",
                    i, dest_rect.left, dest_rect.top, dest_rect.right, dest_rect.bottom,
                    LABEL_STRIP_HEIGHT
                );
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
            }

            handles.push(ThumbnailHandle {
                thumbnail_id,
                source_hwnd: window.hwnd,
                cell_index: i,
                is_blank,
            });
        }
    }

    handles
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
