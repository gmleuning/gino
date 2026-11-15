//! Canvas guides: margins, column grid, printer margin, bleed and the spine.
//! These are drawn on screen only and never reach the PDF. Strokes use
//! `vector-effect="non-scaling-stroke"` so they stay one pixel at any zoom.

use std::fmt::Write as _;

use crate::model::Document;
use crate::snap::{column_edges, page_margins};

/// Every guide is the same light grey dotted line: guides are not part of
/// the design and should stay out of its way.
const GUIDE_GREY: &str = "#c9c9c9";
const DOTTED: &str = "stroke-dasharray=\"0.5 3\" stroke-linecap=\"round\"";
const MARGIN_COLOUR: &str = GUIDE_GREY;
const COLUMN_COLOUR: &str = GUIDE_GREY;
const PRINTER_COLOUR: &str = GUIDE_GREY;
const BLEED_COLOUR: &str = GUIDE_GREY;
const SPINE_COLOUR: &str = GUIDE_GREY;

/// An SVG fragment in page millimetres for one page's guides.
pub fn page_guides_svg(document: &Document, page_index: usize) -> String {
    let size = document.page_size();
    let settings = &document.document;
    let (left, top, right, bottom) = page_margins(document, page_index);
    let mut svg = String::from("<g class=\"guides\" fill=\"none\" vector-effect=\"non-scaling-stroke\">");

    // Type area.
    let _ = write!(
        svg,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" class=\"margin\" stroke=\"{MARGIN_COLOUR}\" stroke-width=\"1\" {DOTTED} vector-effect=\"non-scaling-stroke\"/>",
        n(left),
        n(top),
        n(size.width - left - right),
        n(size.height - top - bottom)
    );

    // Column grid, inner edges only.
    let edges = column_edges(document, page_index);
    for x in edges.iter().skip(1).take(edges.len().saturating_sub(2)) {
        let _ = write!(
            svg,
            "<line x1=\"{x}\" y1=\"{}\" x2=\"{x}\" y2=\"{}\" class=\"column\" stroke=\"{COLUMN_COLOUR}\" stroke-width=\"1\" {DOTTED} vector-effect=\"non-scaling-stroke\"/>",
            n(top),
            n(size.height - bottom),
            x = n(*x)
        );
    }

    let oversize_sheet = matches!(
        crate::impose::sheet_mode(document, settings.sheet),
        Ok(crate::impose::SheetMode::Oversize)
    );
    if oversize_sheet && settings.bleed > 0.0 {
        let b = settings.bleed;
        let _ = write!(
            svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" class=\"bleed\" stroke=\"{BLEED_COLOUR}\" stroke-width=\"1\" {DOTTED} vector-effect=\"non-scaling-stroke\"/>",
            n(-b),
            n(-b),
            n(size.width + 2.0 * b),
            n(size.height + 2.0 * b)
        );
    } else if settings.printer_margin > 0.0 {
        // Same-size sheet: what a home printer will clip, on the outer edges.
        // The spine edge is in the middle of the sheet and is not clipped.
        let m = settings.printer_margin;
        let (x, width) = if Document::is_left_page(page_index) {
            (m, size.width - m)
        } else {
            (0.0, size.width - m)
        };
        let _ = write!(
            svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" class=\"printer\" stroke=\"{PRINTER_COLOUR}\" stroke-width=\"1\" {DOTTED} vector-effect=\"non-scaling-stroke\"/>",
            n(x),
            n(m),
            n(width),
            n(size.height - 2.0 * m)
        );
    }

    // Spine.
    let spine_x = if Document::is_left_page(page_index) { size.width } else { 0.0 };
    let _ = write!(
        svg,
        "<line x1=\"{x}\" y1=\"0\" x2=\"{x}\" y2=\"{}\" class=\"spine\" stroke=\"{SPINE_COLOUR}\" stroke-width=\"1\" {DOTTED} vector-effect=\"non-scaling-stroke\"/>",
        n(size.height),
        x = n(spine_x)
    );

    svg.push_str("</g>");
    svg
}

fn n(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spike::{spike_document, SpikeFamilies};

    #[test]
    fn guides_contain_type_area_printer_margin_and_spine() {
        let document = spike_document(&SpikeFamilies {
            latin: "A".to_owned(),
            korean: "B".to_owned(),
        });
        let svg = page_guides_svg(&document, 0);
        assert!(svg.contains("x=\"20\" y=\"15\" width=\"170\" height=\"267\""));
        assert!(svg.contains(PRINTER_COLOUR));
        assert!(svg.contains("x1=\"0\" y1=\"0\" x2=\"0\""), "right page spine on the left");
        let svg = page_guides_svg(&document, 1);
        assert!(svg.contains("x1=\"210\" y1=\"0\" x2=\"210\""), "left page spine on the right");
    }
}
