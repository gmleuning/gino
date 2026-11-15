//! Text layout with Parley: script ranges for Latin and Korean font pairing,
//! character styles, line breaking (behind the single function
//! [`break_lines`], so hyphenation can be added later in one place),
//! threading across frames and overflow.
//!
//! Broken paragraphs are cached by their text, style and width, so an edit
//! re-shapes only the paragraph it touches; every other paragraph of the
//! thread is reused and only moved.
//!
//! This module works in points, relative to the top left corner of a frame.
//! [`crate::compose`] converts to millimetres on the page.

use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use parley::{
    Alignment as ParleyAlignment, AlignmentOptions, FontFamily, FontFamilyName, FontStyle,
    FontWeight, IndentOptions, Layout, Line, LineHeight, PositionedLayoutItem, StyleProperty,
    WordBreak,
};

use crate::error::Error;
use crate::fonts::Fonts;
use crate::model::{Alignment, CharacterStyle, Document, FontSlant, KoreanBreak, Paragraph, ParagraphStyle, Story};
use crate::scene::{FontFace, Rgb};
use crate::units::mm_to_pt;

/// The content box of a text frame, in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameBox {
    pub width: f64,
    pub height: f64,
}

/// A glyph positioned in points relative to the frame's top left corner.
#[derive(Clone, Debug)]
pub struct PlacedGlyph {
    pub id: u32,
    pub x: f64,
    /// Baseline.
    pub y: f64,
    pub advance: f64,
    /// Byte range within [`PlacedRun::text`].
    pub text_range: Range<usize>,
}

#[derive(Clone, Debug)]
pub struct PlacedRun {
    pub font: Arc<FontFace>,
    /// Points.
    pub size: f64,
    pub color: Rgb,
    pub text: String,
    /// Byte offset of [`PlacedRun::text`] within its paragraph.
    pub text_offset: usize,
    pub glyphs: Vec<PlacedGlyph>,
}

/// An underline or similar decoration, in points relative to the frame.
#[derive(Clone, Debug)]
pub struct PlacedDecoration {
    pub x1: f64,
    pub x2: f64,
    pub y: f64,
    pub thickness: f64,
    pub color: Rgb,
}

#[derive(Clone, Debug)]
pub struct PlacedLine {
    pub top: f64,
    pub baseline: f64,
    pub bottom: f64,
    /// Index of the paragraph within its story.
    pub paragraph: usize,
    /// Byte range of the line within its paragraph, trailing spaces included.
    pub text_range: Range<usize>,
    /// Where the line starts after alignment and indent, for an empty line's caret.
    pub x_start: f64,
    pub runs: Vec<PlacedRun>,
    pub decorations: Vec<PlacedDecoration>,
}

impl PlacedLine {
    fn moved(&self, dy: f64, paragraph: usize, offset: usize) -> PlacedLine {
        PlacedLine {
            top: self.top + dy,
            baseline: self.baseline + dy,
            bottom: self.bottom + dy,
            paragraph,
            text_range: self.text_range.start + offset..self.text_range.end + offset,
            x_start: self.x_start,
            runs: self
                .runs
                .iter()
                .map(|run| PlacedRun {
                    text_offset: run.text_offset + offset,
                    glyphs: run
                        .glyphs
                        .iter()
                        .map(|glyph| PlacedGlyph {
                            y: glyph.y + dy,
                            ..glyph.clone()
                        })
                        .collect(),
                    ..run.clone()
                })
                .collect(),
            decorations: self
                .decorations
                .iter()
                .map(|decoration| PlacedDecoration {
                    y: decoration.y + dy,
                    ..decoration.clone()
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FrameLayout {
    pub lines: Vec<PlacedLine>,
}

#[derive(Clone, Debug, Default)]
pub struct StoryLayout {
    /// One entry per frame passed in, in thread order.
    pub frames: Vec<FrameLayout>,
    /// Characters that did not fit in the last frame.
    pub overflow_characters: usize,
}

/// Which font of a paragraph style a range of text takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Latin,
    Korean,
}

/// Hangul (jamo, compatibility jamo, syllables and the extended blocks), Han
/// characters and East Asian punctuation and symbols take the Korean font.
pub fn is_korean_char(c: char) -> bool {
    matches!(
        c as u32,
        0x1100..=0x11FF       // Hangul Jamo
        | 0x3000..=0x303F     // CJK symbols and punctuation
        | 0x3130..=0x318F     // Hangul compatibility Jamo
        | 0x3200..=0x32FF     // enclosed CJK letters
        | 0x3400..=0x4DBF     // CJK unified ideographs extension A
        | 0x4E00..=0x9FFF     // CJK unified ideographs
        | 0xA960..=0xA97F     // Hangul Jamo extended A
        | 0xAC00..=0xD7AF     // Hangul syllables
        | 0xD7B0..=0xD7FF     // Hangul Jamo extended B
        | 0xF900..=0xFAFF     // CJK compatibility ideographs
        | 0xFF00..=0xFFEF     // halfwidth and fullwidth forms
        | 0x20000..=0x3134F   // CJK extensions B to G
    )
}

/// Splits text into maximal ranges that take the Latin or the Korean font.
/// Neutral characters (spaces, digits, ASCII punctuation) join the range of
/// the character before them, so a space after Korean is set in the Korean
/// font and the words sit evenly. Text that starts neutral joins what follows.
pub fn script_ranges(text: &str) -> Vec<(Range<usize>, Script)> {
    fn strong(c: char) -> Option<Script> {
        if is_korean_char(c) {
            Some(Script::Korean)
        } else if c.is_alphabetic() {
            Some(Script::Latin)
        } else {
            None
        }
    }

    let mut ranges: Vec<(Range<usize>, Script)> = Vec::new();
    let first_strong = text.chars().find_map(strong).unwrap_or(Script::Latin);
    let mut current = first_strong;
    let mut start = 0;
    for (index, c) in text.char_indices() {
        let script = strong(c).unwrap_or(current);
        if script != current {
            if index > start {
                ranges.push((start..index, current));
            }
            start = index;
            current = script;
        }
    }
    if text.len() > start {
        ranges.push((start..text.len(), current));
    }
    ranges
}

/// A range of text set in a character style.
pub type Span<'a> = (Range<usize>, &'a CharacterStyle);

/// Everything that decides how one paragraph (or the remainder of one) breaks.
pub struct BreakInput<'a> {
    pub text: &'a str,
    pub style: &'a ParagraphStyle,
    pub spans: &'a [Span<'a>],
    /// Points.
    pub width: f64,
    /// Points.
    pub first_line_indent: f64,
    /// Text being composed by an input method, drawn underlined.
    pub underline: Option<Range<usize>>,
}

/// Shapes and breaks one paragraph (or the remainder of one) into lines.
/// This is the only place that decides where lines break.
pub fn break_lines(fonts: &mut Fonts, input: &BreakInput<'_>) -> Layout<Rgb> {
    let text = input.text;
    let style = input.style;
    let korean_ranges: Vec<Range<usize>> = script_ranges(text)
        .into_iter()
        .filter(|(_, script)| *script == Script::Korean)
        .map(|(range, _)| range)
        .collect();

    let mut builder = fonts
        .layout
        .ranged_builder(&mut fonts.context, text, 1.0, false);
    builder.push_default(StyleProperty::FontFamily(FontFamily::Single(
        FontFamilyName::Named(Cow::Borrowed(style.latin_font.as_str())),
    )));
    builder.push_default(StyleProperty::FontSize(style.size as f32));
    builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(
        style.leading as f32,
    )));
    builder.push_default(StyleProperty::LetterSpacing(style.tracking as f32));
    builder.push_default(StyleProperty::Brush(Rgb::BLACK));
    if style.korean_break == KoreanBreak::Word {
        // Parley's `keep-all` forbids breaks inside runs of Hangul and Han
        // characters; breaks stay allowed at spaces and punctuation.
        builder.push_default(StyleProperty::WordBreak(WordBreak::KeepAll));
    }
    for range in korean_ranges {
        builder.push(
            StyleProperty::FontFamily(FontFamily::Single(FontFamilyName::Named(
                Cow::Borrowed(style.korean_font.as_str()),
            ))),
            range.clone(),
        );
        builder.push(
            StyleProperty::FontSize((style.size * style.korean_scale) as f32),
            range,
        );
    }
    for (range, character) in input.spans {
        let range = range.start.min(text.len())..range.end.min(text.len());
        if range.is_empty() {
            continue;
        }
        if let Some(slant) = character.font_style {
            let value = match slant {
                FontSlant::Normal => FontStyle::Normal,
                FontSlant::Italic => FontStyle::Italic,
            };
            builder.push(StyleProperty::FontStyle(value), range.clone());
        }
        if let Some(weight) = character.font_weight {
            builder.push(StyleProperty::FontWeight(FontWeight::new(f32::from(weight))), range.clone());
        }
        if character.underline {
            builder.push(StyleProperty::Underline(true), range.clone());
        }
    }
    if let Some(range) = input.underline.clone().filter(|range| !range.is_empty() && range.end <= text.len()) {
        builder.push(StyleProperty::Underline(true), range);
    }

    let mut layout = builder.build(text);
    layout.set_text_indent(input.first_line_indent as f32, IndentOptions::default());
    let alignment = match style.alignment {
        Alignment::Left => ParleyAlignment::Left,
        Alignment::Center => ParleyAlignment::Center,
        Alignment::Right => ParleyAlignment::Right,
        Alignment::Justify => ParleyAlignment::Justify,
    };
    layout.set_alignment(alignment, AlignmentOptions::default());
    layout.break_all_lines(Some(input.width as f32));
    layout
}

/// One line of a cached paragraph, placed with its layout top at zero and
/// its text range relative to the text that was broken.
#[derive(Clone, Debug)]
struct BrokenLine {
    line: PlacedLine,
    /// Bottom of the line box, in points from the layout top.
    block_bottom: f64,
}

#[derive(Debug)]
pub(crate) struct Broken {
    lines: Vec<BrokenLine>,
    height: f64,
}

/// Broken paragraphs keyed by everything that decides their breaks.
#[derive(Default)]
pub struct BreakCache {
    entries: HashMap<u64, Arc<Broken>>,
    faces: HashMap<u64, Arc<FontFace>>,
    pub(crate) hits: usize,
    pub(crate) misses: usize,
}

impl BreakCache {
    /// Above this many paragraphs the cache starts again, to bound memory.
    const LIMIT: usize = 20_000;

    pub fn clear(&mut self) {
        self.entries.clear();
        self.faces.clear();
    }

    /// How many paragraph breaks were reused and how many were computed.
    pub fn statistics(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }
}

fn hash_style(style: &ParagraphStyle, state: &mut impl Hasher) {
    style.latin_font.hash(state);
    style.korean_font.hash(state);
    for value in [
        style.korean_scale,
        style.size,
        style.leading,
        style.tracking,
        style.space_before,
        style.space_after,
        style.first_line_indent,
    ] {
        value.to_bits().hash(state);
    }
    (style.alignment as u8).hash(state);
    (style.korean_break as u8).hash(state);
}

fn hash_input(input: &BreakInput<'_>) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    input.text.hash(&mut state);
    hash_style(input.style, &mut state);
    for (range, character) in input.spans {
        range.hash(&mut state);
        character.font_style.map(|slant| slant as u8).hash(&mut state);
        character.font_weight.hash(&mut state);
        character.underline.hash(&mut state);
    }
    input.width.to_bits().hash(&mut state);
    input.first_line_indent.to_bits().hash(&mut state);
    input.underline.hash(&mut state);
    state.finish()
}

/// [`break_lines`], through the cache.
fn break_cached(fonts: &mut Fonts, input: &BreakInput<'_>) -> Arc<Broken> {
    let key = hash_input(input);
    if let Some(found) = fonts.cache.entries.get(&key) {
        fonts.cache.hits += 1;
        return found.clone();
    }
    fonts.cache.misses += 1;
    let layout = break_lines(fonts, input);
    let cache = &mut fonts.cache;
    let lines = layout
        .lines()
        .map(|line| BrokenLine {
            block_bottom: line.metrics().block_max_coord as f64,
            line: place_line(&line, 0.0, input.text, &mut cache.faces),
        })
        .collect();
    let broken = Arc::new(Broken {
        lines,
        height: layout.height() as f64,
    });
    if cache.entries.len() >= BreakCache::LIMIT {
        cache.entries.clear();
    }
    cache.entries.insert(key, broken.clone());
    broken
}

/// The character-style spans of a paragraph, with byte ranges in its text.
pub fn paragraph_spans<'d>(document: &'d Document, paragraph: &Paragraph) -> Vec<Span<'d>> {
    let mut spans = Vec::new();
    let mut offset = 0;
    for run in &paragraph.runs {
        let end = offset + run.text.len();
        if let Some(character) = run
            .character_style
            .as_deref()
            .and_then(|id| document.character_style(id))
        {
            if end > offset {
                spans.push((offset..end, character));
            }
        }
        offset = end;
    }
    spans
}

/// The spans that fall in `text[start..]`, shifted to start at zero.
fn spans_from<'d>(spans: &[Span<'d>], start: usize) -> Vec<Span<'d>> {
    spans
        .iter()
        .filter(|(range, _)| range.end > start)
        .map(|(range, character)| (range.start.max(start) - start..range.end - start, *character))
        .collect()
}

/// Text being composed by an input method: which paragraph, and the byte
/// range of the provisional text within it. The story passed to
/// [`layout_story`] already contains the text.
#[derive(Clone, Debug, PartialEq)]
pub struct Underline {
    pub paragraph: usize,
    pub range: Range<usize>,
}

/// Lays out a story through its frames in thread order. When a paragraph does
/// not finish in a frame, its remaining text is broken again at the next
/// frame's width, so frames of different widths thread correctly.
pub fn layout_story(
    fonts: &mut Fonts,
    document: &Document,
    story: &Story,
    frames: &[FrameBox],
    underline: Option<&Underline>,
) -> Result<StoryLayout, Error> {
    let mut result = StoryLayout {
        frames: frames.iter().map(|_| FrameLayout::default()).collect(),
        overflow_characters: 0,
    };
    let mut frame_index = 0usize;
    // Points from the top of the current frame.
    let mut cursor = 0.0f64;
    const EPSILON: f64 = 0.01;

    for (paragraph_index, paragraph) in story.paragraphs.iter().enumerate() {
        let style = document.paragraph_style(&paragraph.style)?;
        let text = paragraph.text();
        if frame_index >= frames.len() {
            result.overflow_characters += text.chars().count();
            continue;
        }
        if cursor > 0.0 {
            cursor += mm_to_pt(style.space_before);
        }
        let spans = paragraph_spans(document, paragraph);
        let paragraph_underline = underline
            .filter(|underline| underline.paragraph == paragraph_index)
            .map(|underline| underline.range.clone());
        let mut start = 0usize;
        let mut indent = mm_to_pt(style.first_line_indent);

        loop {
            let Some(frame) = frames.get(frame_index) else {
                result.overflow_characters += text[start..].chars().count();
                break;
            };
            let frame_height = mm_to_pt(frame.height);
            let layout_top = cursor;
            let shifted_spans = spans_from(&spans, start);
            let broken = break_cached(
                fonts,
                &BreakInput {
                    text: &text[start..],
                    style,
                    spans: &shifted_spans,
                    width: mm_to_pt(frame.width),
                    first_line_indent: indent,
                    underline: paragraph_underline
                        .clone()
                        .filter(|range| range.end > start)
                        .map(|range| range.start.max(start) - start..range.end - start),
                },
            );
            let mut stopped_at: Option<usize> = None;
            for broken_line in &broken.lines {
                let bottom = layout_top + broken_line.block_bottom;
                // A line that does not fit moves to the next frame; each move
                // advances the frame, so a too-short frame cannot loop.
                if bottom > frame_height + EPSILON {
                    stopped_at = Some(start + broken_line.line.text_range.start);
                    break;
                }
                result.frames[frame_index]
                    .lines
                    .push(broken_line.line.moved(layout_top, paragraph_index, start));
            }
            match stopped_at {
                None => {
                    cursor = layout_top + broken.height + mm_to_pt(style.space_after);
                    break;
                }
                Some(resume_at) => {
                    // The rest of the paragraph continues in the next frame,
                    // without first-line indent and without space before.
                    start = resume_at;
                    indent = 0.0;
                    frame_index += 1;
                    cursor = 0.0;
                }
            }
        }
    }
    Ok(result)
}

/// Places every line of a layout at `layout_top`, in frame points.
pub fn place_lines(layout: &Layout<Rgb>, layout_top: f64, text: &str) -> Vec<PlacedLine> {
    let mut faces = HashMap::new();
    layout
        .lines()
        .map(|line| place_line(&line, layout_top, text, &mut faces))
        .collect()
}

fn place_line(
    line: &Line<'_, Rgb>,
    layout_top: f64,
    layout_text: &str,
    faces: &mut HashMap<u64, Arc<FontFace>>,
) -> PlacedLine {
    let metrics = line.metrics();
    let mut runs = Vec::new();
    let mut decorations = Vec::new();
    for item in line.items() {
        let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
            continue;
        };
        let run = glyph_run.run();
        let font_data = run.font();
        let face = faces
            .entry(font_data.data.id())
            .or_insert_with(|| {
                let (data, id) = font_data.data.clone().into_raw_parts();
                Arc::new(FontFace {
                    data,
                    index: font_data.index,
                    id,
                })
            })
            .clone();
        let run_text_range = run.text_range();
        let run_text = layout_text[run_text_range.clone()].to_owned();
        if let Some(underline) = &glyph_run.style().underline {
            let font_metrics = run.font_metrics();
            let offset = underline.offset.unwrap_or(font_metrics.underline_offset);
            let thickness = underline.size.unwrap_or(font_metrics.underline_size).max(0.3);
            decorations.push(PlacedDecoration {
                x1: glyph_run.offset() as f64,
                x2: (glyph_run.offset() + glyph_run.advance()) as f64,
                y: layout_top + (metrics.baseline - offset) as f64,
                thickness: thickness as f64,
                color: underline.brush,
            });
        }

        // Text ranges per glyph come from the clusters; positions from Parley's
        // positioned glyphs. They line up one to one unless a style boundary
        // splits a shaped run, in which case every glyph maps to the whole run.
        let mut cluster_ranges: Vec<Range<usize>> = Vec::new();
        for cluster in run.visual_clusters() {
            let range = cluster.text_range();
            let local = range.start - run_text_range.start..range.end - run_text_range.start;
            for _ in cluster.glyphs() {
                cluster_ranges.push(local.clone());
            }
        }
        let positioned: Vec<_> = glyph_run.positioned_glyphs().collect();
        let ranges_match = cluster_ranges.len() == positioned.len();

        let glyphs = positioned
            .iter()
            .enumerate()
            .map(|(i, glyph)| PlacedGlyph {
                id: glyph.id,
                x: glyph.x as f64,
                y: layout_top + glyph.y as f64,
                advance: glyph.advance as f64,
                text_range: if ranges_match {
                    cluster_ranges[i].clone()
                } else {
                    0..run_text.len()
                },
            })
            .collect();

        runs.push(PlacedRun {
            font: face,
            size: run.font_size() as f64,
            color: glyph_run.style().brush,
            text: run_text,
            text_offset: run_text_range.start,
            glyphs,
        });
    }
    PlacedLine {
        top: layout_top + metrics.block_min_coord as f64,
        baseline: layout_top + metrics.baseline as f64,
        bottom: layout_top + metrics.block_max_coord as f64,
        paragraph: 0,
        text_range: line.text_range(),
        // Parley's offset already holds the alignment shift and the indent.
        x_start: metrics.offset as f64,
        runs,
        decorations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hangul_and_han_are_korean_and_latin_is_not() {
        assert!(is_korean_char('한'));
        assert!(is_korean_char('ㄱ'));
        assert!(is_korean_char('漢'));
        assert!(is_korean_char('。'));
        assert!(!is_korean_char('a'));
        assert!(!is_korean_char('é'));
        assert!(!is_korean_char(' '));
        assert!(!is_korean_char('1'));
    }

    #[test]
    fn neutral_characters_join_the_preceding_range() {
        let text = "안녕, world 2024년.";
        let ranges = script_ranges(text);
        let pieces: Vec<(&str, Script)> = ranges
            .iter()
            .map(|(range, script)| (&text[range.clone()], *script))
            .collect();
        assert_eq!(
            pieces,
            vec![
                ("안녕, ", Script::Korean),
                ("world 2024", Script::Latin),
                ("년.", Script::Korean),
            ]
        );
    }

    #[test]
    fn leading_neutral_text_joins_what_follows() {
        let ranges = script_ranges("  123 한글");
        assert_eq!(ranges, vec![(0..12, Script::Korean)]);
        let ranges = script_ranges("...");
        assert_eq!(ranges, vec![(0..3, Script::Latin)]);
        assert!(script_ranges("").is_empty());
    }

    #[test]
    fn spans_are_shifted_to_the_remainder() {
        let italic = CharacterStyle {
            id: "i".to_owned(),
            font_style: Some(FontSlant::Italic),
            font_weight: None,
            underline: false,
        };
        let spans = vec![(2..6, &italic), (10..12, &italic)];
        let shifted = spans_from(&spans, 4);
        assert_eq!(shifted.iter().map(|(range, _)| range.clone()).collect::<Vec<_>>(), vec![0..2, 6..8]);
    }
}
