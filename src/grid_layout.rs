/// Height reserved at the bottom of the overlay for the quick list bar (logical pixels).
pub const QUICK_LIST_BAR_HEIGHT: f32 = 36.0;

/// A grid cell's position and size within the overlay, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Index into the WindowInfo snapshot
    pub window_index: usize,
}

impl CellRect {
    /// Returns a scaled version of this cell centered on the same point.
    /// Used for the 1.05x selection scale-up.
    #[allow(dead_code)]
    pub fn scaled(&self, factor: f32) -> CellRect {
        let new_w = self.width * factor;
        let new_h = self.height * factor;
        let cx = self.x + self.width / 2.0;
        let cy = self.y + self.height / 2.0;
        CellRect {
            x: cx - new_w / 2.0,
            y: cy - new_h / 2.0,
            width: new_w,
            height: new_h,
            window_index: self.window_index,
        }
    }
}

/// Result of the grid layout computation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GridLayout {
    pub cells: Vec<CellRect>,
    pub cols: usize,
    pub rows: usize,
    pub cell_width: f32,
    pub cell_height: f32,
}

/// Default grid cell padding in logical pixels.
/// Use `compute_grid_with_padding` to supply a config-driven value.
#[cfg(test)]
pub const DEFAULT_PADDING: f32 = 16.0;

/// Aspect-ratio-driven grid layout algorithm.
///
/// Places `window_count` cells within an area of `area_width` x `area_height` logical pixels.
/// Columns are computed to best match the monitor aspect ratio.
/// Enforces a minimum cell size of 160x120 logical pixels.
///
/// Uses `DEFAULT_PADDING` (16.0). Call `compute_grid_with_padding` for a configurable value.
#[cfg(test)]
pub fn compute_grid(window_count: usize, area_width: f32, area_height: f32) -> GridLayout {
    compute_grid_with_padding(window_count, area_width, area_height, DEFAULT_PADDING)
}

/// Variant of `compute_grid` that accepts a configurable `padding` value.
/// Driven from `AppConfig.grid_padding`.
pub fn compute_grid_with_padding(
    window_count: usize,
    area_width: f32,
    area_height: f32,
    padding: f32,
) -> GridLayout {
    if window_count == 0 {
        return GridLayout {
            cells: vec![],
            cols: 0,
            rows: 0,
            cell_width: 0.0,
            cell_height: 0.0,
        };
    }

    const MIN_CELL_WIDTH: f32 = 160.0;
    const MIN_CELL_HEIGHT: f32 = 120.0;

    let n = window_count as f32;

    // Target columns: ceil(sqrt(N)) gives the natural square grid.
    // An aspect-ratio multiplier was previously used but over-estimated columns
    // for widescreen monitors (e.g. produced 6 columns instead of 4 for 16 windows).
    let target_cols = (n.sqrt().ceil() as usize).max(1);

    // Find best column count that satisfies minimum cell size
    let cols = find_best_cols(
        window_count,
        target_cols,
        area_width,
        area_height,
        padding,
        MIN_CELL_WIDTH,
        MIN_CELL_HEIGHT,
    );
    let rows = ((window_count as f32) / cols as f32).ceil() as usize;

    let cell_width = (area_width - padding * (cols + 1) as f32) / cols as f32;
    let cell_height = (area_height - padding * (rows + 1) as f32) / rows as f32;

    let cells = (0..window_count)
        .map(|i| {
            let row = i / cols;
            let col = i % cols;
            let x = padding + col as f32 * (cell_width + padding);
            let y = padding + row as f32 * (cell_height + padding);
            CellRect {
                x,
                y,
                width: cell_width,
                height: cell_height,
                window_index: i,
            }
        })
        .collect();

    GridLayout {
        cells,
        cols,
        rows,
        cell_width,
        cell_height,
    }
}

/// Determine the best column count that keeps cells at or above minimum size.
fn find_best_cols(
    window_count: usize,
    target_cols: usize,
    area_width: f32,
    area_height: f32,
    padding: f32,
    min_cell_width: f32,
    min_cell_height: f32,
) -> usize {
    // Try target_cols first, then reduce if cells would be too small
    let mut cols = target_cols.min(window_count);

    loop {
        let rows = ((window_count as f32) / cols as f32).ceil() as usize;
        let cell_w = (area_width - padding * (cols + 1) as f32) / cols as f32;
        let cell_h = (area_height - padding * (rows + 1) as f32) / rows as f32;

        if cell_w >= min_cell_width && cell_h >= min_cell_height {
            return cols;
        }

        if cols <= 1 {
            // Can't reduce further; accept whatever size we get
            return 1;
        }
        cols -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_WIDTH: f32 = 1920.0;
    const TEST_HEIGHT: f32 = 1080.0;

    #[test]
    fn test_zero_windows_returns_empty() {
        let layout = compute_grid(0, TEST_WIDTH, TEST_HEIGHT);
        assert!(layout.cells.is_empty());
        assert_eq!(layout.cols, 0);
        assert_eq!(layout.rows, 0);
    }

    #[test]
    fn test_single_window_fills_most_of_area() {
        let layout = compute_grid(1, TEST_WIDTH, TEST_HEIGHT);
        assert_eq!(layout.cells.len(), 1);
        let cell = &layout.cells[0];
        // With 1 window, cell should be very large
        assert!(
            cell.width >= 1400.0,
            "Cell width {} should be >= 1400",
            cell.width
        );
        assert!(
            cell.height >= 700.0,
            "Cell height {} should be >= 700",
            cell.height
        );
    }

    #[test]
    fn test_16_windows_produce_4x4_grid() {
        let layout = compute_grid(16, TEST_WIDTH, TEST_HEIGHT);
        assert_eq!(layout.cells.len(), 16);
        assert_eq!(layout.cols, 4);
        assert_eq!(layout.rows, 4);
    }

    #[test]
    fn test_26_windows_cells_not_smaller_than_minimum() {
        let layout = compute_grid(26, TEST_WIDTH, TEST_HEIGHT);
        assert_eq!(layout.cells.len(), 26);
        for (i, cell) in layout.cells.iter().enumerate() {
            assert!(
                cell.width >= 160.0,
                "Cell {} width {} is below minimum 160px",
                i,
                cell.width
            );
            assert!(
                cell.height >= 120.0,
                "Cell {} height {} is below minimum 120px",
                i,
                cell.height
            );
        }
    }

    #[test]
    fn test_grid_layout_is_deterministic() {
        let layout1 = compute_grid(10, TEST_WIDTH, TEST_HEIGHT);
        let layout2 = compute_grid(10, TEST_WIDTH, TEST_HEIGHT);
        assert_eq!(layout1.cols, layout2.cols);
        assert_eq!(layout1.rows, layout2.rows);
        assert_eq!(layout1.cells.len(), layout2.cells.len());
        for (c1, c2) in layout1.cells.iter().zip(layout2.cells.iter()) {
            assert_eq!(c1.x, c2.x);
            assert_eq!(c1.y, c2.y);
            assert_eq!(c1.width, c2.width);
            assert_eq!(c1.height, c2.height);
        }
    }

    #[test]
    fn test_cells_do_not_overlap() {
        for n in [1, 4, 9, 16, 26] {
            let layout = compute_grid(n, TEST_WIDTH, TEST_HEIGHT);
            let cells = &layout.cells;
            for i in 0..cells.len() {
                for j in (i + 1)..cells.len() {
                    let a = &cells[i];
                    let b = &cells[j];
                    // Check no overlap (with a small epsilon for floating point)
                    let overlap = a.x < b.x + b.width
                        && a.x + a.width > b.x
                        && a.y < b.y + b.height
                        && a.y + a.height > b.y;
                    assert!(
                        !overlap,
                        "Cells {} and {} overlap for n={}: a=({},{},{},{}) b=({},{},{},{})",
                        i, j, n, a.x, a.y, a.width, a.height, b.x, b.y, b.width, b.height
                    );
                }
            }
        }
    }

    #[test]
    fn test_all_cells_fit_within_overlay_bounds() {
        let layout = compute_grid(26, TEST_WIDTH, TEST_HEIGHT);
        for (i, cell) in layout.cells.iter().enumerate() {
            assert!(cell.x >= 0.0, "Cell {} x={} is negative", i, cell.x);
            assert!(cell.y >= 0.0, "Cell {} y={} is negative", i, cell.y);
            assert!(
                cell.x + cell.width <= TEST_WIDTH + 0.01,
                "Cell {} right edge {} exceeds area width {}",
                i,
                cell.x + cell.width,
                TEST_WIDTH
            );
            assert!(
                cell.y + cell.height <= TEST_HEIGHT + 0.01,
                "Cell {} bottom edge {} exceeds area height {}",
                i,
                cell.y + cell.height,
                TEST_HEIGHT
            );
        }
    }

    // TC-4.5: compute_grid_with_padding with padding=0.0 produces same cell count as compute_grid
    #[test]
    fn test_compute_grid_with_padding_zero_same_cell_count() {
        // compute_grid uses DEFAULT_PADDING (16.0); padding=0.0 should still produce same cell count
        // for normal window counts since more space is available per cell.
        for n in [1, 4, 9, 16] {
            let default_layout = compute_grid(n, TEST_WIDTH, TEST_HEIGHT);
            let zero_pad_layout = compute_grid_with_padding(n, TEST_WIDTH, TEST_HEIGHT, 0.0);
            assert_eq!(
                default_layout.cells.len(),
                zero_pad_layout.cells.len(),
                "Cell count should match for n={} with padding=0.0 vs default",
                n
            );
        }
    }

    // TC-4.5 (continued): zero padding produces cells that fill the full area exactly
    #[test]
    fn test_compute_grid_with_padding_zero_fills_area() {
        let layout = compute_grid_with_padding(4, TEST_WIDTH, TEST_HEIGHT, 0.0);
        assert_eq!(layout.cells.len(), 4);
        // With 0 padding: cell_width = area_width / cols
        let expected_cell_w = TEST_WIDTH / layout.cols as f32;
        let expected_cell_h = TEST_HEIGHT / layout.rows as f32;
        assert!(
            (layout.cell_width - expected_cell_w).abs() < 0.01,
            "With padding=0, cell_width {} should equal area/cols={}",
            layout.cell_width,
            expected_cell_w
        );
        assert!(
            (layout.cell_height - expected_cell_h).abs() < 0.01,
            "With padding=0, cell_height {} should equal area/rows={}",
            layout.cell_height,
            expected_cell_h
        );
    }

    // TC-4.6: compute_grid_with_padding with custom padding produces smaller cells than default
    #[test]
    fn test_compute_grid_with_custom_padding_smaller_cells() {
        let default_layout = compute_grid_with_padding(9, TEST_WIDTH, TEST_HEIGHT, DEFAULT_PADDING);
        let large_pad_layout = compute_grid_with_padding(9, TEST_WIDTH, TEST_HEIGHT, 48.0);

        // More padding means less space per cell
        assert!(
            large_pad_layout.cell_width <= default_layout.cell_width,
            "Larger padding {} should produce cell_width {} <= default cell_width {}",
            48.0,
            large_pad_layout.cell_width,
            default_layout.cell_width
        );
        assert!(
            large_pad_layout.cell_height <= default_layout.cell_height,
            "Larger padding {} should produce cell_height {} <= default cell_height {}",
            48.0,
            large_pad_layout.cell_height,
            default_layout.cell_height
        );
    }

    // TC-4.6 (continued): minimum padding 4.0 produces larger cells than default 16.0
    #[test]
    fn test_compute_grid_with_min_padding_larger_cells() {
        let default_layout = compute_grid_with_padding(9, TEST_WIDTH, TEST_HEIGHT, DEFAULT_PADDING);
        let min_pad_layout = compute_grid_with_padding(9, TEST_WIDTH, TEST_HEIGHT, 4.0);

        // Less padding (4.0 < 16.0) means more space per cell
        assert!(
            min_pad_layout.cell_width >= default_layout.cell_width,
            "Minimum padding 4.0 should produce cell_width {} >= default cell_width {}",
            min_pad_layout.cell_width,
            default_layout.cell_width
        );
    }

    // TC-4.7: compute_grid_with_padding with large padding doesn't produce negative cell sizes
    #[test]
    fn test_compute_grid_with_large_padding_no_negative_cells() {
        // Even with maximum allowed padding (48.0) on a standard screen, cells must be positive.
        let layout = compute_grid_with_padding(16, TEST_WIDTH, TEST_HEIGHT, 48.0);
        assert_eq!(layout.cells.len(), 16);
        for (i, cell) in layout.cells.iter().enumerate() {
            assert!(
                cell.width > 0.0,
                "Cell {} width {} must be positive with padding=48.0",
                i,
                cell.width
            );
            assert!(
                cell.height > 0.0,
                "Cell {} height {} must be positive with padding=48.0",
                i,
                cell.height
            );
        }
    }

    // TC-4.7 (continued): zero padding also produces strictly positive cell sizes
    #[test]
    fn test_compute_grid_with_zero_padding_positive_cells() {
        let layout = compute_grid_with_padding(16, TEST_WIDTH, TEST_HEIGHT, 0.0);
        for (i, cell) in layout.cells.iter().enumerate() {
            assert!(
                cell.width > 0.0,
                "Cell {} width {} must be positive with padding=0.0",
                i,
                cell.width
            );
            assert!(
                cell.height > 0.0,
                "Cell {} height {} must be positive with padding=0.0",
                i,
                cell.height
            );
        }
    }

    #[test]
    fn test_cell_scaled() {
        let cell = CellRect {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 150.0,
            window_index: 0,
        };
        let scaled = cell.scaled(1.05);
        // Center should remain the same
        let orig_cx = cell.x + cell.width / 2.0;
        let orig_cy = cell.y + cell.height / 2.0;
        let new_cx = scaled.x + scaled.width / 2.0;
        let new_cy = scaled.y + scaled.height / 2.0;
        assert!((orig_cx - new_cx).abs() < 0.01);
        assert!((orig_cy - new_cy).abs() < 0.01);
        assert!((scaled.width - 210.0).abs() < 0.01);
        assert!((scaled.height - 157.5).abs() < 0.01);
    }
}
