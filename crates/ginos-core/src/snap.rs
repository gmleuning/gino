//! Snapping to margins, the column grid, the spine, page edges and other
//! frames. The tolerance comes from the interface in millimetres, so it stays
//! a constant number of screen pixels whatever the zoom.

use crate::hit::Handle;
use crate::model::{Document, Rect};

/// Vertical (`xs`) and horizontal (`ys`) snap lines, in page millimetres.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SnapTargets {
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
}

/// Margins of a page as left, top, right, bottom distances from the page
/// edges; the inside margin is on the spine side.
pub fn page_margins(document: &Document, page_index: usize) -> (f64, f64, f64, f64) {
    let margins = document.document.margins;
    if Document::is_left_page(page_index) {
        (margins.outside, margins.top, margins.inside, margins.bottom)
    } else {
        (margins.inside, margins.top, margins.outside, margins.bottom)
    }
}

/// The x positions of column edges within the type area.
pub fn column_edges(document: &Document, page_index: usize) -> Vec<f64> {
    let size = document.page_size();
    let (left, _, right, _) = page_margins(document, page_index);
    let columns = document.document.columns;
    let count = columns.count.max(1) as f64;
    let area = size.width - left - right;
    let column_width = (area - columns.gutter * (count - 1.0)) / count;
    let mut edges = Vec::new();
    for index in 0..columns.count.max(1) {
        let x = left + index as f64 * (column_width + columns.gutter);
        edges.push(x);
        edges.push(x + column_width);
    }
    edges
}

/// Snap lines for a page, leaving out the frame being dragged.
pub fn targets(document: &Document, page_index: usize, exclude_frame: &str) -> SnapTargets {
    let size = document.page_size();
    let (left, top, right, bottom) = page_margins(document, page_index);
    let mut xs = vec![0.0, size.width, left, size.width - right];
    let mut ys = vec![0.0, size.height, top, size.height - bottom];
    xs.extend(column_edges(document, page_index));
    if let Some(page) = document.pages.get(page_index) {
        for frame in &page.frames {
            if frame.id == exclude_frame {
                continue;
            }
            let rect = frame.rect();
            xs.extend([rect.x, rect.right(), rect.centre_x()]);
            ys.extend([rect.y, rect.bottom(), rect.centre_y()]);
        }
    }
    SnapTargets { xs, ys }
}

fn nearest(value: f64, lines: &[f64], tolerance: f64) -> Option<f64> {
    lines
        .iter()
        .copied()
        .filter(|line| (line - value).abs() <= tolerance)
        .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs()))
}

/// Snaps a rectangle being moved: its left, right or centre to a vertical
/// line and its top, bottom or centre to a horizontal one, whichever is
/// closest. Returns the snapped rectangle and the lines used.
pub fn snap_move(rect: &Rect, targets: &SnapTargets, tolerance: f64) -> (Rect, Option<f64>, Option<f64>) {
    let mut result = *rect;
    let mut snapped_x = None;
    let mut snapped_y = None;

    let mut best_dx: Option<(f64, f64)> = None;
    for edge in [rect.x, rect.right(), rect.centre_x()] {
        if let Some(line) = nearest(edge, &targets.xs, tolerance) {
            let delta = line - edge;
            if best_dx.is_none_or(|(d, _)| delta.abs() < d.abs()) {
                best_dx = Some((delta, line));
            }
        }
    }
    if let Some((delta, line)) = best_dx {
        result.x += delta;
        snapped_x = Some(line);
    }

    let mut best_dy: Option<(f64, f64)> = None;
    for edge in [rect.y, rect.bottom(), rect.centre_y()] {
        if let Some(line) = nearest(edge, &targets.ys, tolerance) {
            let delta = line - edge;
            if best_dy.is_none_or(|(d, _)| delta.abs() < d.abs()) {
                best_dy = Some((delta, line));
            }
        }
    }
    if let Some((delta, line)) = best_dy {
        result.y += delta;
        snapped_y = Some(line);
    }

    (result, snapped_x, snapped_y)
}

/// Snaps the edges a handle moves, keeping the opposite edges fixed.
pub fn snap_resize(
    rect: &Rect,
    handle: Handle,
    targets: &SnapTargets,
    tolerance: f64,
    minimum: f64,
) -> (Rect, Option<f64>, Option<f64>) {
    let mut left = rect.x;
    let mut right = rect.right();
    let mut top = rect.y;
    let mut bottom = rect.bottom();
    let mut snapped_x = None;
    let mut snapped_y = None;

    if handle.moves_left() {
        if let Some(line) = nearest(left, &targets.xs, tolerance) {
            left = line.min(right - minimum);
            snapped_x = Some(line);
        }
    }
    if handle.moves_right() {
        if let Some(line) = nearest(right, &targets.xs, tolerance) {
            right = line.max(left + minimum);
            snapped_x = Some(line);
        }
    }
    if handle.moves_top() {
        if let Some(line) = nearest(top, &targets.ys, tolerance) {
            top = line.min(bottom - minimum);
            snapped_y = Some(line);
        }
    }
    if handle.moves_bottom() {
        if let Some(line) = nearest(bottom, &targets.ys, tolerance) {
            bottom = line.max(top + minimum);
            snapped_y = Some(line);
        }
    }
    (Rect::new(left, top, right - left, bottom - top), snapped_x, snapped_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spike::{spike_document, SpikeFamilies};

    fn document() -> Document {
        spike_document(&SpikeFamilies {
            latin: "A".to_owned(),
            korean: "B".to_owned(),
        })
    }

    #[test]
    fn margins_mirror_on_left_pages() {
        let document = document();
        // The spike has inside 20 and outside 20, so set them apart.
        let mut document = document;
        document.document.margins.inside = 12.0;
        document.document.margins.outside = 8.0;
        assert_eq!(page_margins(&document, 0), (12.0, 15.0, 8.0, 15.0));
        assert_eq!(page_margins(&document, 1), (8.0, 15.0, 12.0, 15.0));
    }

    #[test]
    fn column_edges_divide_the_type_area() {
        let mut document = document();
        document.document.columns.count = 2;
        document.document.columns.gutter = 10.0;
        // Type area is 210 - 20 - 20 = 170; columns are 80 each.
        assert_eq!(column_edges(&document, 0), vec![20.0, 100.0, 110.0, 190.0]);
    }

    #[test]
    fn moving_snaps_the_closest_edge() {
        let document = document();
        let targets = targets(&document, 0, "thread_a");
        // Left edge 1.5 mm from the margin at 20.
        let (snapped, x, _) = snap_move(&Rect::new(21.5, 50.0, 30.0, 30.0), &targets, 2.0);
        assert_eq!(snapped.x, 20.0);
        assert_eq!(x, Some(20.0));
        // Right edge near frame B's left edge at 90.
        let (snapped, x, _) = snap_move(&Rect::new(58.8, 50.0, 30.0, 30.0), &targets, 2.0);
        assert_eq!(snapped.right(), 90.0);
        assert_eq!(x, Some(90.0));
        // Out of reach: unchanged.
        let (snapped, x, y) = snap_move(&Rect::new(42.0, 50.0, 5.0, 10.0), &targets, 1.0);
        assert_eq!(snapped, Rect::new(42.0, 50.0, 5.0, 10.0));
        assert_eq!((x, y), (None, None));
    }

    #[test]
    fn resizing_snaps_only_the_moving_edge() {
        let document = document();
        let targets = targets(&document, 0, "thread_a");
        let (snapped, x, y) =
            snap_resize(&Rect::new(40.0, 50.0, 48.5, 30.0), Handle::Right, &targets, 2.0, 2.0);
        assert_eq!(snapped, Rect::new(40.0, 50.0, 50.0, 30.0));
        assert_eq!((x, y), (Some(90.0), None));
    }
}
