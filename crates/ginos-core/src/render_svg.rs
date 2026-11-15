//! Scene to SVG. Glyphs are drawn as outlines: each distinct glyph of each
//! font becomes one `<path>` in font units inside the frame's `<defs>`, and
//! every occurrence is a `<use>` with a translate and scale. The webview never
//! needs the font, and the outline is the same one the PDF embeds.
//!
//! Each frame renders to its own `<g>` fragment, so the canvas can replace one
//! frame at a time; a page is the fragments wrapped in an `<svg>`.

use std::collections::HashMap;
use std::fmt::Write as _;

use base64::Engine as _;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::scene::{FontFace, FrameScene, Item, Page, Rgb};
use crate::units::pt_to_mm;

/// Renders one page to a complete SVG document. The user unit is one millimetre.
pub fn page_to_svg(page: &Page) -> String {
    let mut body = String::new();
    for frame in &page.frames {
        body.push_str(&frame_to_svg(frame));
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">{body}</svg>",
        w = num(page.width),
        h = num(page.height),
    )
}

/// Renders one frame to an SVG `<g>` fragment in page millimetres. Ids inside
/// are prefixed with the frame id so fragments can share one document.
pub fn frame_to_svg(frame: &FrameScene) -> String {
    let mut writer = Writer {
        prefix: id_safe(&frame.id),
        defs: String::new(),
        known: HashMap::new(),
        unit_cache: HashMap::new(),
        clip_count: 0,
    };
    let mut body = String::new();
    writer.items(&frame.items, &mut body);
    format!("<g data-frame=\"{}\"><defs>{}</defs>{body}</g>", writer.prefix, writer.defs)
}

struct Writer {
    prefix: String,
    defs: String,
    known: HashMap<(u64, u32), Option<f64>>,
    unit_cache: HashMap<u64, f64>,
    clip_count: usize,
}

impl Writer {
    fn clip_id(&mut self) -> String {
        self.clip_count += 1;
        format!("{}-clip{}", self.prefix, self.clip_count)
    }

    fn items(&mut self, items: &[Item], body: &mut String) {
        for item in items {
            self.item(item, body);
        }
    }

    fn item(&mut self, item: &Item, body: &mut String) {
        let prefix = self.prefix.clone();
        match item {
            Item::Text(text) => {
                let _ = write!(body, "<g fill=\"{}\">", color(text.color));
                for glyph in &text.glyphs {
                    let key = (text.font.id, glyph.id);
                    let (defs, unit_cache) = (&mut self.defs, &mut self.unit_cache);
                    let units_per_em = *self.known.entry(key).or_insert_with(|| {
                        define_glyph(defs, &prefix, &text.font, glyph.id, unit_cache)
                    });
                    let Some(units_per_em) = units_per_em else {
                        continue;
                    };
                    let scale = pt_to_mm(text.size) / units_per_em;
                    let _ = write!(
                        body,
                        "<use href=\"#{prefix}-g{}-{}\" transform=\"translate({} {}) scale({} {})\"/>",
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
                    num(rect.rect.x),
                    num(rect.rect.y),
                    num(rect.rect.w),
                    num(rect.rect.h),
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
            Item::Group(group) => {
                let t = group.transform;
                let _ = write!(
                    body,
                    "<g transform=\"matrix({} {} {} {} {} {})\">",
                    num6(t.a),
                    num6(t.b),
                    num6(t.c),
                    num6(t.d),
                    num(t.e),
                    num(t.f)
                );
                match group.clip {
                    Some(clip) => {
                        let clip_id = self.clip_id();
                        let _ = write!(
                            self.defs,
                            "<clipPath id=\"{clip_id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
                            num(clip.x),
                            num(clip.y),
                            num(clip.w),
                            num(clip.h)
                        );
                        let _ = write!(body, "<g clip-path=\"url(#{clip_id})\">");
                        self.items(&group.items, body);
                        body.push_str("</g>");
                    }
                    None => self.items(&group.items, body),
                }
                body.push_str("</g>");
            }
            Item::Image(image) => {
                let clip_id = self.clip_id();
                let _ = write!(
                    self.defs,
                    "<clipPath id=\"{clip_id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
                    num(image.clip.x),
                    num(image.clip.y),
                    num(image.clip.w),
                    num(image.clip.h)
                );
                let encoded = base64::engine::general_purpose::STANDARD.encode(&image.image.png);
                let _ = write!(
                    body,
                    "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" clip-path=\"url(#{clip_id})\" href=\"data:image/png;base64,{encoded}\"/>",
                    num(image.placement.x),
                    num(image.placement.y),
                    num(image.placement.w),
                    num(image.placement.h)
                );
            }
        }
    }
}

/// Adds the glyph's outline to `defs` and returns the font's units per em, or
/// `None` when the glyph has no outline (a space, for example).
fn define_glyph(
    defs: &mut String,
    prefix: &str,
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
    let _ = write!(defs, "<path id=\"{prefix}-g{}-{}\" d=\"{}\"/>", face.id, glyph_id, pen.path);
    Some(units_per_em)
}

/// Frame ids become part of SVG ids, which allow only a safe subset of characters.
pub fn id_safe(id: &str) -> String {
    let mut safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if safe.is_empty() || !safe.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        safe.insert(0, 'f');
    }
    safe
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
pub fn num(value: f64) -> String {
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
    fn ids_are_made_safe() {
        assert_eq!(id_safe("thread_a"), "thread_a");
        assert_eq!(id_safe("본문 1"), "f___1");
        assert_eq!(id_safe("1st"), "f1st");
    }

    #[test]
    fn empty_page_is_valid_svg() {
        let svg = page_to_svg(&Page {
            width: 148.0,
            height: 210.0,
            frames: Vec::new(),
        });
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"148mm\""));
        assert!(svg.ends_with("</svg>"));
    }
}
