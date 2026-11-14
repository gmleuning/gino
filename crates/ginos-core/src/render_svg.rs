//! Scene to SVG. Glyphs are drawn as outlines: each distinct glyph of each
//! font becomes one `<path>` in font units inside `<defs>`, and every
//! occurrence is a `<use>` with a translate and scale. The webview never needs
//! the font, and the outline is the same one the PDF embeds.

use std::collections::HashMap;
use std::fmt::Write as _;

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::scene::{FontFace, Item, Page, Rgb};
use crate::units::pt_to_mm;

/// Renders one page to a complete SVG document. The user unit is one millimetre.
pub fn page_to_svg(page: &Page) -> String {
    let mut defs = String::new();
    let mut body = String::new();
    let mut known: HashMap<(u64, u32), Option<f64>> = HashMap::new();
    let mut unit_cache: HashMap<u64, f64> = HashMap::new();

    for item in &page.items {
        match item {
            Item::Text(text) => {
                let _ = write!(body, "<g fill=\"{}\">", color(text.color));
                for glyph in &text.glyphs {
                    let key = (text.font.id, glyph.id);
                    let units_per_em = *known
                        .entry(key)
                        .or_insert_with(|| define_glyph(&mut defs, &text.font, glyph.id, &mut unit_cache));
                    let Some(units_per_em) = units_per_em else {
                        continue;
                    };
                    let scale = pt_to_mm(text.size) / units_per_em;
                    let _ = write!(
                        body,
                        "<use href=\"#g{}-{}\" transform=\"translate({} {}) scale({} {})\"/>",
                        text.font.id,
                        glyph.id,
                        num(glyph.x),
                        num(glyph.y),
                        num6(scale),
                        num6(-scale)
                    );
                }
                body.push_str("</g>");
            }
            Item::Rect(rect) => {
                let fill = rect.fill.map(color).unwrap_or_else(|| "none".to_owned());
                let _ = write!(
                    body,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"",
                    num(rect.x),
                    num(rect.y),
                    num(rect.w),
                    num(rect.h),
                    fill
                );
                if let Some(stroke) = rect.stroke {
                    let _ = write!(
                        body,
                        " stroke=\"{}\" stroke-width=\"{}\"",
                        color(stroke.color),
                        num(stroke.width)
                    );
                }
                body.push_str("/>");
            }
            Item::Rule(rule) => {
                let _ = write!(
                    body,
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
                    num(rule.x1),
                    num(rule.y1),
                    num(rule.x2),
                    num(rule.y2),
                    color(rule.stroke.color),
                    num(rule.stroke.width)
                );
            }
        }
    }

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\"><defs>{defs}</defs>{body}</svg>",
        w = num(page.width),
        h = num(page.height),
    )
}

/// Adds the glyph's outline to `defs` and returns the font's units per em, or
/// `None` when the glyph has no outline (a space, for example).
fn define_glyph(
    defs: &mut String,
    face: &FontFace,
    glyph_id: u32,
    unit_cache: &mut HashMap<u64, f64>,
) -> Option<f64> {
    let font = FontRef::from_index(face.bytes(), face.index).ok()?;
    let units_per_em = *unit_cache.entry(face.id).or_insert_with(|| {
        f64::from(font.metrics(Size::unscaled(), LocationRef::default()).units_per_em)
    });
    let outline = font.outline_glyphs().get(GlyphId::new(glyph_id))?;
    let mut pen = PathPen::default();
    outline
        .draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut pen)
        .ok()?;
    if pen.path.is_empty() {
        return None;
    }
    let _ = write!(defs, "<path id=\"g{}-{}\" d=\"{}\"/>", face.id, glyph_id, pen.path);
    Some(units_per_em)
}

#[derive(Default)]
struct PathPen {
    path: String,
}

impl OutlinePen for PathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.path, "M{} {}", fnum(x), fnum(y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.path, "L{} {}", fnum(x), fnum(y));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let _ = write!(self.path, "Q{} {} {} {}", fnum(cx0), fnum(cy0), fnum(x), fnum(y));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let _ = write!(
            self.path,
            "C{} {} {} {} {} {}",
            fnum(cx0),
            fnum(cy0),
            fnum(cx1),
            fnum(cy1),
            fnum(x),
            fnum(y)
        );
    }

    fn close(&mut self) {
        self.path.push('Z');
    }
}

fn color(rgb: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b)
}

/// Millimetre values to four decimals, trailing zeros removed.
fn num(value: f64) -> String {
    trim(format!("{value:.4}"))
}

fn num6(value: f64) -> String {
    trim(format!("{value:.6}"))
}

/// Font-unit values to two decimals.
fn fnum(value: f32) -> String {
    trim(format!("{value:.2}"))
}

fn trim(mut text: String) -> String {
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".to_owned();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_short() {
        assert_eq!(num(12.0), "12");
        assert_eq!(num(12.5), "12.5");
        assert_eq!(num(0.00001), "0");
        assert_eq!(num(-0.00001), "0");
        assert_eq!(fnum(-1.0), "-1");
    }

    #[test]
    fn empty_page_is_valid_svg() {
        let svg = page_to_svg(&Page {
            width: 148.0,
            height: 210.0,
            items: Vec::new(),
        });
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"148mm\""));
        assert!(svg.ends_with("</svg>"));
    }
}
