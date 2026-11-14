//! The scene: per page, a list of drawable items with every glyph already
//! positioned. Both renderers consume only this. Lengths are millimetres on the
//! page; type sizes are points.

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

/// An opaque colour. Also used as Parley's brush type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
    pub const WHITE: Rgb = Rgb { r: 255, g: 255, b: 255 };

    /// A grey level from 0 (black) to 1 (white).
    pub fn grey(level: f64) -> Self {
        let value = (level.clamp(0.0, 1.0) * 255.0).round() as u8;
        Rgb {
            r: value,
            g: value,
            b: value,
        }
    }
}

/// A loaded font face shared between the layout and both renderers.
pub struct FontFace {
    pub data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    pub index: u32,
    /// Identifier of the font data, unique within the process.
    pub id: u64,
}

impl FontFace {
    pub fn bytes(&self) -> &[u8] {
        self.data.as_ref().as_ref()
    }
}

impl fmt::Debug for FontFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontFace")
            .field("index", &self.index)
            .field("id", &self.id)
            .field("bytes", &self.bytes().len())
            .finish()
    }
}

/// One positioned glyph. `x` and `y` are the glyph origin on the page in
/// millimetres, `y` being the baseline. `advance` is in millimetres too.
#[derive(Clone, Debug)]
pub struct Glyph {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub advance: f64,
    /// Byte range of the glyph's cluster within [`TextItem::text`].
    pub text_range: Range<usize>,
}

/// A run of glyphs sharing one font, size and colour.
#[derive(Clone, Debug)]
pub struct TextItem {
    pub font: Arc<FontFace>,
    /// Points.
    pub size: f64,
    pub color: Rgb,
    /// The source text of the run, for the PDF text layer.
    pub text: String,
    pub glyphs: Vec<Glyph>,
}

#[derive(Clone, Copy, Debug)]
pub struct Stroke {
    /// Millimetres.
    pub width: f64,
    pub color: Rgb,
}

#[derive(Clone, Debug)]
pub struct RectItem {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub stroke: Option<Stroke>,
    pub fill: Option<Rgb>,
}

#[derive(Clone, Debug)]
pub struct RuleItem {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub stroke: Stroke,
}

#[derive(Clone, Debug)]
pub enum Item {
    Text(TextItem),
    Rect(RectItem),
    Rule(RuleItem),
}

#[derive(Clone, Debug)]
pub struct Page {
    /// Millimetres.
    pub width: f64,
    pub height: f64,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub pages: Vec<Page>,
}

impl Scene {
    pub fn glyph_count(&self) -> usize {
        self.pages
            .iter()
            .flat_map(|page| page.items.iter())
            .map(|item| match item {
                Item::Text(text) => text.glyphs.len(),
                _ => 0,
            })
            .sum()
    }
}
