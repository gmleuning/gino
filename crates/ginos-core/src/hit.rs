//! Canvas coordinates to frames and resize handles. Text positions within a
//! story come with milestone 3.

use serde::{Deserialize, Serialize};

use crate::model::{Frame, Page, Rect};

/// The topmost frame under a point (frames later in the list are on top).
pub fn frame_at(page: &Page, x: f64, y: f64) -> Option<&Frame> {
    page.frames.iter().rev().find(|frame| frame.rect().contains(x, y))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::TopLeft,
        Handle::Top,
        Handle::TopRight,
        Handle::Right,
        Handle::BottomRight,
        Handle::Bottom,
        Handle::BottomLeft,
        Handle::Left,
    ];

    /// The handle's position on a rectangle.
    pub fn position(self, rect: &Rect) -> (f64, f64) {
        let (cx, cy) = (rect.centre_x(), rect.centre_y());
        match self {
            Handle::TopLeft => (rect.x, rect.y),
            Handle::Top => (cx, rect.y),
            Handle::TopRight => (rect.right(), rect.y),
            Handle::Right => (rect.right(), cy),
            Handle::BottomRight => (rect.right(), rect.bottom()),
            Handle::Bottom => (cx, rect.bottom()),
            Handle::BottomLeft => (rect.x, rect.bottom()),
            Handle::Left => (rect.x, cy),
        }
    }

    pub fn moves_left(self) -> bool {
        matches!(self, Handle::TopLeft | Handle::Left | Handle::BottomLeft)
    }

    pub fn moves_right(self) -> bool {
        matches!(self, Handle::TopRight | Handle::Right | Handle::BottomRight)
    }

    pub fn moves_top(self) -> bool {
        matches!(self, Handle::TopLeft | Handle::Top | Handle::TopRight)
    }

    pub fn moves_bottom(self) -> bool {
        matches!(self, Handle::BottomLeft | Handle::Bottom | Handle::BottomRight)
    }

    /// The rectangle after dragging this handle by (dx, dy), never smaller
    /// than `minimum` in either direction.
    pub fn resize(self, start: &Rect, dx: f64, dy: f64, minimum: f64) -> Rect {
        let mut left = start.x;
        let mut right = start.right();
        let mut top = start.y;
        let mut bottom = start.bottom();
        if self.moves_left() {
            left = (start.x + dx).min(right - minimum);
        }
        if self.moves_right() {
            right = (start.right() + dx).max(left + minimum);
        }
        if self.moves_top() {
            top = (start.y + dy).min(bottom - minimum);
        }
        if self.moves_bottom() {
            bottom = (start.bottom() + dy).max(top + minimum);
        }
        Rect::new(left, top, right - left, bottom - top)
    }
}

/// The handle of a rectangle within `tolerance` millimetres of a point.
/// Corners win over edges when both are in reach.
pub fn handle_at(rect: &Rect, x: f64, y: f64, tolerance: f64) -> Option<Handle> {
    let mut best: Option<(f64, Handle)> = None;
    for handle in Handle::ALL {
        let (hx, hy) = handle.position(rect);
        let distance = ((hx - x).powi(2) + (hy - y).powi(2)).sqrt();
        if distance <= tolerance && best.is_none_or(|(d, _)| distance < d) {
            best = Some((distance, handle));
        }
    }
    best.map(|(_, handle)| handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Frame;

    #[test]
    fn the_topmost_frame_wins() {
        let page = Page {
            frames: vec![
                Frame::rectangle("under", 0.0, 0.0, 50.0, 50.0, 0.2, 0.0),
                Frame::rectangle("over", 25.0, 25.0, 50.0, 50.0, 0.2, 0.0),
            ],
        };
        assert_eq!(frame_at(&page, 30.0, 30.0).unwrap().id, "over");
        assert_eq!(frame_at(&page, 10.0, 10.0).unwrap().id, "under");
        assert!(frame_at(&page, 90.0, 90.0).is_none());
    }

    #[test]
    fn handles_are_found_within_tolerance() {
        let rect = Rect::new(10.0, 10.0, 40.0, 20.0);
        assert_eq!(handle_at(&rect, 50.5, 30.4, 1.0), Some(Handle::BottomRight));
        assert_eq!(handle_at(&rect, 30.0, 10.2, 1.0), Some(Handle::Top));
        assert_eq!(handle_at(&rect, 30.0, 20.0, 1.0), None);
    }

    #[test]
    fn resizing_keeps_the_opposite_edge_and_a_minimum_size() {
        let rect = Rect::new(10.0, 10.0, 40.0, 20.0);
        assert_eq!(Handle::BottomRight.resize(&rect, 5.0, -3.0, 2.0), Rect::new(10.0, 10.0, 45.0, 17.0));
        assert_eq!(Handle::TopLeft.resize(&rect, 4.0, 4.0, 2.0), Rect::new(14.0, 14.0, 36.0, 16.0));
        assert_eq!(Handle::Left.resize(&rect, 100.0, 0.0, 2.0), Rect::new(48.0, 10.0, 2.0, 20.0));
    }
}
