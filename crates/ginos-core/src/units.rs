//! Length units. The document stores millimetres; type size and leading are in
//! points. The layout engine works in points, the scene stores millimetres.

/// PostScript points per millimetre (72 points to the inch).
pub const POINTS_PER_MILLIMETRE: f64 = 72.0 / 25.4;

/// Converts millimetres to points.
pub fn mm_to_pt(millimetres: f64) -> f64 {
    millimetres * POINTS_PER_MILLIMETRE
}

/// Converts points to millimetres.
pub fn pt_to_mm(points: f64) -> f64 {
    points / POINTS_PER_MILLIMETRE
}

/// ISO A-series paper sizes in millimetres (width, height), portrait.
pub mod paper {
    pub const A3: (f64, f64) = (297.0, 420.0);
    pub const A4: (f64, f64) = (210.0, 297.0);
    pub const A5: (f64, f64) = (148.0, 210.0);
    pub const A6: (f64, f64) = (105.0, 148.0);
    /// North American sizes: 8.5 x 11, 8.5 x 14 and 11 x 17 inches.
    pub const LETTER: (f64, f64) = (215.9, 279.4);
    pub const LEGAL: (f64, f64) = (215.9, 355.6);
    pub const TABLOID: (f64, f64) = (279.4, 431.8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inch_round_trips() {
        assert!((mm_to_pt(25.4) - 72.0).abs() < 1e-9);
        assert!((pt_to_mm(72.0) - 25.4).abs() < 1e-9);
        assert!((pt_to_mm(mm_to_pt(148.0)) - 148.0).abs() < 1e-9);
    }

    #[test]
    fn two_a5_pages_leave_one_millimetre_on_a4() {
        assert_eq!(paper::A5, (148.0, 210.0));
        assert_eq!(paper::A5.0 * 2.0 + 1.0, paper::A4.1);
    }
}
