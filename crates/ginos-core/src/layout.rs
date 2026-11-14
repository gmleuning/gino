//! Text layout with Parley: script ranges for Latin and Korean font pairing,
//! line breaking (behind the single function [`break_lines`], so hyphenation
//! can be added later in one place), threading across frames and overflow.
//!
//! This module works in points, relative to the top left corner of a frame.
//! [`crate::compose`] converts to millimetres on the page.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use parley::{
    Alignment as ParleyAlignment, AlignmentOptions, FontFamily, FontFamilyName, IndentOptions,
    Layout, Line, LineHeight, PositionedLayoutItem, StyleProperty, WordBreak,
};

use crate::error::Error;
use crate::fonts::Fonts;
use crate::model::{Alignment, Document, KoreanBreak, ParagraphStyle, Story};
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
    pub glyphs: Vec<PlacedGlyph>,
}

#[derive(Clone, Debug)]
pub struct PlacedLine {
    pub top: f64,
    pub baseline: f64,
    pub bottom: f64,
    pub runs: Vec<PlacedRun>,
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

/// Shapes and breaks one paragraph (or the remainder of one) into lines at the
/// given width. This is the only place that decides where lines break.
pub fn break_lines(
    fonts: &mut Fonts,
    text: &str,
    style: &ParagraphStyle,
    width: f64,
    first_line_indent: f64,
) -> Layout<Rgb> {
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

    let mut layout = builder.build(text);
    layout.set_text_indent(first_line_indent as f32, IndentOptions::default());
    let alignment = match style.alignment {
        Alignment::Left => ParleyAlignment::Left,
        Alignment::Center => ParleyAlignment::Center,
        Alignment::Right => ParleyAlignment::Right,
        Alignment::Justify => ParleyAlignment::Justify,
    };
    layout.set_alignment(alignment, AlignmentOptions::default());
    layout.break_all_lines(Some(width as f32));
    layout
}

/// Lays out a story through its frames in thread order. When a paragraph does
/// not finish in a frame, its remaining text is broken again at the next
/// frame's width, so frames of different widths thread correctly.
pub fn layout_story(
    fonts: &mut Fonts,
    document: &Document,
    story: &Story,
    frames: &[FrameBox],
) -> Result<StoryLayout, Error> {
    let mut result = StoryLayout {
        frames: frames.iter().map(|_| FrameLayout::default()).collect(),
        overflow_characters: 0,
    };
    let mut faces: HashMap<u64, Arc<FontFace>> = HashMap::new();
    let mut frame_index = 0usize;
    // Points from the top of the current frame.
    let mut cursor = 0.0f64;
    const EPSILON: f64 = 0.01;

    for paragraph in &story.paragraphs {
        let style = document.paragraph_style(&paragraph.style)?;
        let text = paragraph.text();
        if frame_index >= frames.len() {
            result.overflow_characters += text.chars().count();
            continue;
        }
        if cursor > 0.0 {
            cursor += mm_to_pt(style.space_before);
        }
        let mut start = 0usize;
        let mut indent = mm_to_pt(style.first_line_indent);

        loop {
            let Some(frame) = frames.get(frame_index) else {
                result.overflow_characters += text[start..].chars().count();
                break;
            };
            let frame_height = mm_to_pt(frame.height);
            let layout_top = cursor;
            let layout = break_lines(fonts, &text[start..], style, mm_to_pt(frame.width), indent);
            let mut stopped_at: Option<usize> = None;
            for line in layout.lines() {
                let metrics = line.metrics();
                let bottom = layout_top + metrics.block_max_coord as f64;
                if bottom > frame_height + EPSILON {
                    stopped_at = Some(start + line.text_range().start);
                    break;
                }
                let placed = place_line(&line, layout_top, &text[start..], &mut faces);
                result.frames[frame_index].lines.push(placed);
            }
            match stopped_at {
                None => {
                    cursor = layout_top + layout.height() as f64 + mm_to_pt(style.space_after);
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

fn place_line(
    line: &Line<'_, Rgb>,
    layout_top: f64,
    layout_text: &str,
    faces: &mut HashMap<u64, Arc<FontFace>>,
) -> PlacedLine {
    let metrics = line.metrics();
    let mut runs = Vec::new();
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
            glyphs,
        });
    }
    PlacedLine {
        top: layout_top + metrics.block_min_coord as f64,
        baseline: layout_top + metrics.baseline as f64,
        bottom: layout_top + metrics.block_max_coord as f64,
        runs,
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
}
