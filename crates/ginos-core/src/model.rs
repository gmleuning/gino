//! The document model: settings, styles, stories, pages and frames.
//! All lengths are `f64` millimetres; type size and leading are points.
//! Everything here is `serde` serialisable; `document.json` is this model.

use serde::{Deserialize, Serialize};

use crate::error::Error;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub format_version: u32,
    pub document: Settings,
    #[serde(default)]
    pub fonts: FontSettings,
    pub paragraph_styles: Vec<ParagraphStyle>,
    #[serde(default)]
    pub character_styles: Vec<CharacterStyle>,
    pub stories: Vec<Story>,
    #[serde(default)]
    pub master: MasterPage,
    pub pages: Vec<Page>,
}

impl Document {
    pub fn paragraph_style(&self, id: &str) -> Result<&ParagraphStyle, Error> {
        self.paragraph_styles
            .iter()
            .find(|style| style.id == id)
            .ok_or_else(|| Error::MissingParagraphStyle {
                style: id.to_owned(),
            })
    }

    pub fn character_style(&self, id: &str) -> Option<&CharacterStyle> {
        self.character_styles.iter().find(|style| style.id == id)
    }

    pub fn story(&self, id: &str) -> Result<&Story, Error> {
        self.stories
            .iter()
            .find(|story| story.id == id)
            .ok_or_else(|| Error::MissingStory {
                story: id.to_owned(),
            })
    }

    pub fn to_json(&self) -> Result<String, Error> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(text: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(text)?)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Fold {
    SaddleA5OnA4,
    SaddleA6OnA5,
    EightPageFold,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub bottom: f64,
    pub inside: f64,
    pub outside: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Columns {
    pub count: u32,
    pub gutter: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub fold: Fold,
    pub trim: Size,
    pub sheet: Size,
    /// Used only when the sheet is larger than the imposed pages.
    pub bleed: f64,
    /// Unprintable margin of the home printer; drawn as a guide on same-size sheets.
    pub printer_margin: f64,
    pub margins: Margins,
    pub columns: Columns,
    /// Shift toward the spine per sheet, in millimetres. Zero on same-size sheets.
    pub creep_per_sheet: f64,
    pub page_count: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FontSettings {
    #[serde(default)]
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KoreanBreak {
    /// Default Unicode behaviour: a break may fall between any two Hangul syllables.
    #[default]
    Syllable,
    /// Break only at spaces and punctuation, like the web property `word-break: keep-all`.
    Word,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParagraphStyle {
    pub id: String,
    pub latin_font: String,
    pub korean_font: String,
    /// Size multiplier for Korean runs; 1.0 is 100 percent.
    #[serde(default = "one")]
    pub korean_scale: f64,
    /// Type size in points.
    pub size: f64,
    /// Baseline to baseline distance in points.
    pub leading: f64,
    /// Extra letter spacing in points.
    #[serde(default)]
    pub tracking: f64,
    #[serde(default)]
    pub alignment: Alignment,
    /// Millimetres.
    #[serde(default)]
    pub space_before: f64,
    /// Millimetres.
    #[serde(default)]
    pub space_after: f64,
    /// Millimetres.
    #[serde(default)]
    pub first_line_indent: f64,
    #[serde(default)]
    pub korean_break: KoreanBreak,
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FontSlant {
    Normal,
    Italic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterStyle {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_style: Option<FontSlant>,
    /// Weight from 100 to 900, as on the web.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Story {
    pub id: String,
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Paragraph {
    pub style: String,
    pub runs: Vec<Run>,
}

impl Paragraph {
    pub fn plain(style: &str, text: &str) -> Self {
        Self {
            style: style.to_owned(),
            runs: vec![Run {
                text: text.to_owned(),
                character_style: None,
            }],
        }
    }

    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Run {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_style: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MasterPage {
    #[serde(default)]
    pub frames: Vec<MasterFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MasterFrame {
    pub kind: FrameKind,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub first_numbered_page: u32,
    pub style: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Page {
    #[serde(default)]
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrameKind {
    Text,
    Image,
    Rectangle,
    Rule,
    PageNumber,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Frame {
    pub id: String,
    pub kind: FrameKind,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub rotation: f64,
    /// Text frames: the story that flows through this frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story: Option<String>,
    /// Text frames: position of this frame in the story's thread, from 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_index: Option<u32>,
    /// Image frames: path relative to the `.zine` folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Rectangles and rules: stroke width in millimetres. Zero means no stroke.
    #[serde(default)]
    pub stroke_width: f64,
    /// Rectangles: whether the interior is filled with the stroke colour.
    #[serde(default)]
    pub filled: bool,
    /// Grey level of strokes and fills: 0 is black and 1 is white.
    #[serde(default)]
    pub grey: f64,
}

impl Frame {
    pub fn text(id: &str, x: f64, y: f64, w: f64, h: f64, story: &str, thread_index: u32) -> Self {
        Self {
            id: id.to_owned(),
            kind: FrameKind::Text,
            x,
            y,
            w,
            h,
            rotation: 0.0,
            story: Some(story.to_owned()),
            thread_index: Some(thread_index),
            image: None,
            stroke_width: 0.0,
            filled: false,
            grey: 0.0,
        }
    }

    pub fn rectangle(id: &str, x: f64, y: f64, w: f64, h: f64, stroke_width: f64, grey: f64) -> Self {
        Self {
            id: id.to_owned(),
            kind: FrameKind::Rectangle,
            x,
            y,
            w,
            h,
            rotation: 0.0,
            story: None,
            thread_index: None,
            image: None,
            stroke_width,
            filled: false,
            grey,
        }
    }

    /// A rule from (x, y) to (x + w, y + h).
    pub fn rule(id: &str, x: f64, y: f64, w: f64, h: f64, stroke_width: f64, grey: f64) -> Self {
        Self {
            kind: FrameKind::Rule,
            ..Self::rectangle(id, x, y, w, h, stroke_width, grey)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Document {
        Document {
            format_version: FORMAT_VERSION,
            document: Settings {
                fold: Fold::SaddleA5OnA4,
                trim: Size { width: 148.0, height: 210.0 },
                sheet: Size { width: 297.0, height: 210.0 },
                bleed: 3.0,
                printer_margin: 5.0,
                margins: Margins { top: 12.0, bottom: 15.0, inside: 12.0, outside: 10.0 },
                columns: Columns { count: 1, gutter: 5.0 },
                creep_per_sheet: 0.0,
                page_count: 16,
            },
            fonts: FontSettings { folders: vec!["D:\\Fonts\\Active".to_owned()] },
            paragraph_styles: vec![ParagraphStyle {
                id: "body".to_owned(),
                latin_font: "EB Garamond".to_owned(),
                korean_font: "Gowun Batang".to_owned(),
                korean_scale: 1.0,
                size: 9.5,
                leading: 14.0,
                tracking: 0.0,
                alignment: Alignment::Left,
                space_before: 0.0,
                space_after: 0.0,
                first_line_indent: 0.0,
                korean_break: KoreanBreak::Word,
            }],
            character_styles: vec![CharacterStyle {
                id: "emphasis".to_owned(),
                font_style: Some(FontSlant::Italic),
                font_weight: None,
            }],
            stories: vec![Story {
                id: "s1".to_owned(),
                paragraphs: vec![Paragraph::plain("body", "안녕하세요, world.")],
            }],
            master: MasterPage {
                frames: vec![MasterFrame {
                    kind: FrameKind::PageNumber,
                    x: 12.0,
                    y: 197.0,
                    w: 20.0,
                    h: 6.0,
                    first_numbered_page: 3,
                    style: "body".to_owned(),
                }],
            },
            pages: vec![Page {
                frames: vec![Frame::text("f1", 12.0, 12.0, 126.0, 183.0, "s1", 0)],
            }],
        }
    }

    #[test]
    fn json_round_trip_is_identical() {
        let document = sample();
        let json = document.to_json().unwrap();
        let parsed = Document::from_json(&json).unwrap();
        assert_eq!(document, parsed);
        assert!(json.contains("\"fold\": \"saddle_a5_on_a4\""));
        assert!(json.contains("\"korean_break\": \"word\""));
        assert!(json.contains("안녕하세요"));
    }

    #[test]
    fn missing_fields_take_defaults() {
        let json = r#"{
            "format_version": 1,
            "document": {"fold": "custom", "trim": {"width": 100, "height": 100},
                "sheet": {"width": 100, "height": 100}, "bleed": 0, "printer_margin": 5,
                "margins": {"top": 1, "bottom": 1, "inside": 1, "outside": 1},
                "columns": {"count": 1, "gutter": 0}, "creep_per_sheet": 0, "page_count": 1},
            "paragraph_styles": [{"id": "body", "latin_font": "A", "korean_font": "B", "size": 10, "leading": 12}],
            "stories": [],
            "pages": [{}]
        }"#;
        let document = Document::from_json(json).unwrap();
        let style = document.paragraph_style("body").unwrap();
        assert_eq!(style.korean_scale, 1.0);
        assert_eq!(style.korean_break, KoreanBreak::Syllable);
        assert_eq!(style.alignment, Alignment::Left);
        assert!(document.pages[0].frames.is_empty());
    }
}
