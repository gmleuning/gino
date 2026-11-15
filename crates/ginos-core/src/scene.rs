//! The scene: per page, per frame, a list of drawable items with every glyph
//! already positioned. Both renderers consume only this. Lengths are
//! millimetres on the page; type sizes are points.

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use crate::images::LoadedImage;
use crate::model::Rect;

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
    pub rect: Rect,
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

/// A raster image: the whole picture at `placement`, clipped to `clip`.
#[derive(Clone, Debug)]
pub struct ImageItem {
    pub image: Arc<LoadedImage>,
    pub placement: Rect,
    pub clip: Rect,
}

/// An affine transform: x' = a·x + c·y + e, y' = b·x + d·y + f (millimetres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Transform {
    pub fn translate(x: f64, y: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: x,
            f: y,
        }
    }

    /// A half turn of a `width` by `height` box whose top left corner lands at (x, y).
    pub fn half_turn(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            a: -1.0,
            b: 0.0,
            c: 0.0,
            d: -1.0,
            e: x + width,
            f: y + height,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }
}

/// Items drawn through a transform and clipped to a rectangle given in the
/// items' own coordinates. Imposition places whole pages this way.
#[derive(Clone, Debug)]
pub struct GroupItem {
    pub transform: Transform,
    pub clip: Option<Rect>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub enum Item {
    Text(TextItem),
    Rect(RectItem),
    Rule(RuleItem),
    Image(ImageItem),
    Group(GroupItem),
}

impl Item {
    pub fn translated(&self, dx: f64, dy: f64) -> Item {
        match self {
            Item::Text(text) => Item::Text(TextItem {
                glyphs: text
                    .glyphs
                    .iter()
                    .map(|glyph| Glyph {
                        x: glyph.x + dx,
                        y: glyph.y + dy,
                        ..glyph.clone()
                    })
                    .collect(),
                ..text.clone()
            }),
            Item::Rect(rect) => Item::Rect(RectItem {
                rect: rect.rect.translated(dx, dy),
                ..rect.clone()
            }),
            Item::Rule(rule) => Item::Rule(RuleItem {
                x1: rule.x1 + dx,
                y1: rule.y1 + dy,
                x2: rule.x2 + dx,
                y2: rule.y2 + dy,
                stroke: rule.stroke,
            }),
            Item::Image(image) => Item::Image(ImageItem {
                image: image.image.clone(),
                placement: image.placement.translated(dx, dy),
                clip: image.clip.translated(dx, dy),
            }),
            Item::Group(group) => Item::Group(GroupItem {
                transform: Transform {
                    e: group.transform.e + dx,
                    f: group.transform.f + dy,
                    ..group.transform
                },
                ..group.clone()
            }),
        }
    }
}

/// The drawable items of one frame, in drawing order.
#[derive(Clone, Debug)]
pub struct FrameScene {
    pub id: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub struct Page {
    /// Millimetres.
    pub width: f64,
    pub height: f64,
    /// Frames in stacking order, lowest first.
    pub frames: Vec<FrameScene>,
}

impl Page {
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.frames.iter().flat_map(|frame| frame.items.iter())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub pages: Vec<Page>,
}

impl Scene {
    pub fn glyph_count(&self) -> usize {
        fn count(item: &Item) -> usize {
            match item {
                Item::Text(text) => text.glyphs.len(),
                Item::Group(group) => group.items.iter().map(count).sum(),
                _ => 0,
            }
        }
        self.pages.iter().flat_map(Page::items).map(count).sum()
    }
}
