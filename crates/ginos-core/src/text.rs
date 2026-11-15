//! Text positions and editing within a story, and the caret's geometry.
//!
//! A position is a paragraph index and a byte offset into that paragraph's
//! text, always on a character boundary. Editing functions here are pure:
//! they take a story and return the paragraphs to replace, which
//! [`crate::edit::Command::SetParagraphs`] applies and can undo.

use std::collections::BTreeMap;
use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::layout::PlacedLine;
use crate::model::{Paragraph, Run, Story};
use crate::units::pt_to_mm;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TextPosition {
    pub paragraph: usize,
    /// Byte offset within the paragraph's text.
    pub offset: usize,
}

impl TextPosition {
    pub fn new(paragraph: usize, offset: usize) -> Self {
        Self { paragraph, offset }
    }
}

/// The paragraphs to put in place of `story.paragraphs[first..first + replaced]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Replacement {
    pub first: usize,
    pub before: Vec<Paragraph>,
    pub after: Vec<Paragraph>,
    pub caret: TextPosition,
}

/// The nearest valid position: an existing paragraph, an offset inside it on
/// a character boundary.
pub fn clamp(story: &Story, position: TextPosition) -> TextPosition {
    if story.paragraphs.is_empty() {
        return TextPosition::default();
    }
    let paragraph = position.paragraph.min(story.paragraphs.len() - 1);
    let text = story.paragraphs[paragraph].text();
    let mut offset = position.offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    TextPosition { paragraph, offset }
}

pub fn story_start() -> TextPosition {
    TextPosition::default()
}

pub fn story_end(story: &Story) -> TextPosition {
    match story.paragraphs.last() {
        Some(last) => TextPosition::new(story.paragraphs.len() - 1, last.text().len()),
        None => TextPosition::default(),
    }
}

/// One character back, or the end of the previous paragraph.
pub fn previous_position(story: &Story, position: TextPosition) -> TextPosition {
    let position = clamp(story, position);
    if position.offset > 0 {
        let text = story.paragraphs[position.paragraph].text();
        let offset = text[..position.offset]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
        TextPosition::new(position.paragraph, offset)
    } else if position.paragraph > 0 {
        let paragraph = position.paragraph - 1;
        TextPosition::new(paragraph, story.paragraphs[paragraph].text().len())
    } else {
        position
    }
}

/// One character forward, or the start of the next paragraph.
pub fn next_position(story: &Story, position: TextPosition) -> TextPosition {
    let position = clamp(story, position);
    let Some(paragraph) = story.paragraphs.get(position.paragraph) else {
        return position;
    };
    let text = paragraph.text();
    if position.offset < text.len() {
        let width = text[position.offset..].chars().next().map(char::len_utf8).unwrap_or(0);
        TextPosition::new(position.paragraph, position.offset + width)
    } else if position.paragraph + 1 < story.paragraphs.len() {
        TextPosition::new(position.paragraph + 1, 0)
    } else {
        position
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '’'
}

/// The start of the word before the position (Control and Left arrow).
pub fn previous_word(story: &Story, position: TextPosition) -> TextPosition {
    let position = clamp(story, position);
    if position.offset == 0 {
        return previous_position(story, position);
    }
    let text = story.paragraphs[position.paragraph].text();
    let before: Vec<(usize, char)> = text[..position.offset].char_indices().collect();
    let mut index = before.len();
    while index > 0 && !is_word_char(before[index - 1].1) {
        index -= 1;
    }
    while index > 0 && is_word_char(before[index - 1].1) {
        index -= 1;
    }
    let offset = before.get(index).map(|(offset, _)| *offset).unwrap_or(position.offset);
    TextPosition::new(position.paragraph, if index == 0 { 0 } else { offset })
}

/// The end of the word after the position (Control and Right arrow).
pub fn next_word(story: &Story, position: TextPosition) -> TextPosition {
    let position = clamp(story, position);
    let Some(paragraph) = story.paragraphs.get(position.paragraph) else {
        return position;
    };
    let text = paragraph.text();
    if position.offset >= text.len() {
        return next_position(story, position);
    }
    let mut offset = position.offset;
    let mut chars = text[position.offset..].chars().peekable();
    while let Some(&c) = chars.peek() {
        if is_word_char(c) {
            break;
        }
        offset += c.len_utf8();
        chars.next();
    }
    while let Some(&c) = chars.peek() {
        if !is_word_char(c) {
            break;
        }
        offset += c.len_utf8();
        chars.next();
    }
    TextPosition::new(position.paragraph, offset)
}

/// The word around a position (double-click).
pub fn word_at(story: &Story, position: TextPosition) -> (TextPosition, TextPosition) {
    let position = clamp(story, position);
    let Some(paragraph) = story.paragraphs.get(position.paragraph) else {
        return (position, position);
    };
    let text = paragraph.text();
    let mut start = position.offset;
    for (index, c) in text[..position.offset].char_indices().rev() {
        if !is_word_char(c) {
            break;
        }
        start = index;
    }
    let mut end = position.offset;
    for c in text[position.offset..].chars() {
        if !is_word_char(c) {
            break;
        }
        end += c.len_utf8();
    }
    (
        TextPosition::new(position.paragraph, start),
        TextPosition::new(position.paragraph, end),
    )
}

/// Runs covering a byte range of a paragraph, cut at the range's ends.
fn slice_runs(runs: &[Run], range: Range<usize>) -> Vec<Run> {
    let mut sliced = Vec::new();
    let mut offset = 0;
    for run in runs {
        let run_range = offset..offset + run.text.len();
        offset = run_range.end;
        let start = range.start.max(run_range.start);
        let end = range.end.min(run_range.end);
        if start < end {
            sliced.push(Run {
                text: run.text[start - run_range.start..end - run_range.start].to_owned(),
                character_style: run.character_style.clone(),
            });
        }
    }
    sliced
}

/// Merges neighbouring runs of the same character style and drops empty
/// runs, keeping one run so an empty paragraph remembers its style.
pub fn normalise(runs: Vec<Run>) -> Vec<Run> {
    let fallback = runs.first().map(|run| run.character_style.clone()).unwrap_or(None);
    let mut merged: Vec<Run> = Vec::new();
    for run in runs {
        if run.text.is_empty() {
            continue;
        }
        match merged.last_mut() {
            Some(last) if last.character_style == run.character_style => last.text.push_str(&run.text),
            _ => merged.push(run),
        }
    }
    if merged.is_empty() {
        merged.push(Run {
            text: String::new(),
            character_style: fallback,
        });
    }
    merged
}

/// The character style text typed at a position takes: that of the
/// character before it, or of the paragraph's first run at its start.
pub fn character_style_at(paragraph: &Paragraph, offset: usize) -> Option<String> {
    let mut start = 0;
    for run in &paragraph.runs {
        let end = start + run.text.len();
        if offset > start && offset <= end {
            return run.character_style.clone();
        }
        start = end;
    }
    paragraph.runs.first().and_then(|run| run.character_style.clone())
}

fn ordered(a: TextPosition, b: TextPosition) -> (TextPosition, TextPosition) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Replaces the text between two positions with `text`, in which a line
/// break (`\n`, `\r\n` or `\r`) starts a new paragraph of the same style.
/// The caret ends after the inserted text.
pub fn replace(story: &Story, from: TextPosition, to: TextPosition, text: &str) -> Replacement {
    let (start, end) = ordered(clamp(story, from), clamp(story, to));
    if story.paragraphs.is_empty() {
        let mut paragraphs: Vec<Paragraph> = Vec::new();
        for piece in split_paragraphs(text) {
            paragraphs.push(Paragraph::plain("body", &piece));
        }
        let caret = TextPosition::new(paragraphs.len() - 1, paragraphs.last().map(|p| p.text().len()).unwrap_or(0));
        return Replacement {
            first: 0,
            before: Vec::new(),
            after: paragraphs,
            caret,
        };
    }
    let first = &story.paragraphs[start.paragraph];
    let last = &story.paragraphs[end.paragraph];
    let style = first.style.clone();
    let character_style = character_style_at(first, start.offset);
    let prefix = slice_runs(&first.runs, 0..start.offset);
    let suffix = slice_runs(&last.runs, end.offset..last.text().len());

    let pieces = split_paragraphs(text);
    let mut after = Vec::new();
    let mut runs = prefix;
    let mut caret = TextPosition::new(start.paragraph, start.offset);
    for (index, piece) in pieces.iter().enumerate() {
        if index > 0 {
            after.push(Paragraph {
                style: style.clone(),
                runs: normalise(std::mem::take(&mut runs)),
            });
            caret = TextPosition::new(start.paragraph + index, 0);
        }
        if !piece.is_empty() {
            runs.push(Run {
                text: piece.clone(),
                character_style: character_style.clone(),
            });
        }
        caret.offset += piece.len();
    }
    // The empty run keeps the typing style when a paragraph is left empty.
    if runs.is_empty() {
        runs.push(Run {
            text: String::new(),
            character_style: character_style.clone(),
        });
    }
    runs.extend(suffix);
    after.push(Paragraph {
        style,
        runs: normalise(runs),
    });
    Replacement {
        first: start.paragraph,
        before: story.paragraphs[start.paragraph..=end.paragraph].to_vec(),
        after,
        caret,
    }
}

fn split_paragraphs(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(str::to_owned)
        .collect()
}

/// Sets a character style (or none) on the text between two positions.
pub fn set_character_style(story: &Story, from: TextPosition, to: TextPosition, style: Option<&str>) -> Replacement {
    let (start, end) = ordered(clamp(story, from), clamp(story, to));
    let mut after = Vec::new();
    for index in start.paragraph..=end.paragraph.min(story.paragraphs.len().saturating_sub(1)) {
        let paragraph = &story.paragraphs[index];
        let length = paragraph.text().len();
        let a = if index == start.paragraph { start.offset } else { 0 };
        let b = if index == end.paragraph { end.offset } else { length };
        let mut runs = slice_runs(&paragraph.runs, 0..a);
        runs.extend(slice_runs(&paragraph.runs, a..b).into_iter().map(|run| Run {
            text: run.text,
            character_style: style.map(str::to_owned),
        }));
        runs.extend(slice_runs(&paragraph.runs, b..length));
        if a == b && length == 0 {
            runs = vec![Run {
                text: String::new(),
                character_style: style.map(str::to_owned),
            }];
        }
        after.push(Paragraph {
            style: paragraph.style.clone(),
            runs: normalise(runs),
        });
    }
    Replacement {
        first: start.paragraph,
        before: story.paragraphs.get(start.paragraph..start.paragraph + after.len()).map(<[_]>::to_vec).unwrap_or_default(),
        after,
        caret: end,
    }
}

/// Sets the paragraph style of every paragraph touched by the range.
pub fn set_paragraph_style(story: &Story, from: TextPosition, to: TextPosition, style: &str) -> Replacement {
    let (start, end) = ordered(clamp(story, from), clamp(story, to));
    let range = start.paragraph..(end.paragraph + 1).min(story.paragraphs.len());
    let before = story.paragraphs[range.clone()].to_vec();
    let after = before
        .iter()
        .map(|paragraph| Paragraph {
            style: style.to_owned(),
            runs: paragraph.runs.clone(),
        })
        .collect();
    Replacement {
        first: start.paragraph,
        before,
        after,
        caret: end,
    }
}

/// The plain text between two positions, paragraphs separated by line breaks.
pub fn text_between(story: &Story, from: TextPosition, to: TextPosition) -> String {
    let (start, end) = ordered(clamp(story, from), clamp(story, to));
    let mut text = String::new();
    for index in start.paragraph..=end.paragraph.min(story.paragraphs.len().saturating_sub(1)) {
        let paragraph = story.paragraphs[index].text();
        let a = if index == start.paragraph { start.offset } else { 0 };
        let b = if index == end.paragraph { end.offset } else { paragraph.len() };
        if index > start.paragraph {
            text.push('\n');
        }
        text.push_str(&paragraph[a.min(b)..b]);
    }
    text
}

/// A laid-out line as the caret sees it, in page millimetres.
#[derive(Clone, Debug, PartialEq)]
pub struct CaretLine {
    pub page: usize,
    pub frame_id: String,
    pub top: f64,
    pub bottom: f64,
    pub paragraph: usize,
    pub range: Range<usize>,
    /// Caret stops: byte offset in the paragraph and x on the page, in order.
    pub stops: Vec<(usize, f64)>,
}

impl CaretLine {
    /// Builds caret stops from a placed line of a frame at (`frame_x`, `frame_y`).
    pub fn from_placed(page: usize, frame_id: &str, frame_x: f64, frame_y: f64, line: &PlacedLine) -> Self {
        let mut starts: BTreeMap<usize, f64> = BTreeMap::new();
        let mut ends: BTreeMap<usize, f64> = BTreeMap::new();
        for run in &line.runs {
            for glyph in &run.glyphs {
                let start = run.text_offset + glyph.text_range.start;
                let end = run.text_offset + glyph.text_range.end;
                let x = frame_x + pt_to_mm(glyph.x);
                let x_end = frame_x + pt_to_mm(glyph.x + glyph.advance);
                starts.entry(start).and_modify(|known| *known = known.min(x)).or_insert(x);
                ends.entry(end).and_modify(|known| *known = known.max(x_end)).or_insert(x_end);
            }
        }
        for (offset, x) in ends {
            starts.entry(offset).or_insert(x);
        }
        if starts.is_empty() {
            starts.insert(line.text_range.start, frame_x + pt_to_mm(line.x_start));
        }
        CaretLine {
            page,
            frame_id: frame_id.to_owned(),
            top: frame_y + pt_to_mm(line.top),
            bottom: frame_y + pt_to_mm(line.bottom),
            paragraph: line.paragraph,
            range: line.text_range.clone(),
            stops: starts.into_iter().collect(),
        }
    }

    /// The x of the stop nearest to a byte offset.
    pub fn x_of(&self, offset: usize) -> f64 {
        let mut best = self.stops[0];
        for &(stop, x) in &self.stops {
            if stop <= offset {
                best = (stop, x);
            }
        }
        best.1
    }

    /// The stop nearest to an x position.
    pub fn offset_at(&self, x: f64) -> usize {
        self.stops
            .iter()
            .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
            .map(|(offset, _)| *offset)
            .unwrap_or(self.range.start)
    }
}

/// The caret as drawn: a vertical line on one page.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Caret {
    pub page: usize,
    pub x: f64,
    pub top: f64,
    pub bottom: f64,
}

/// A highlighted piece of a selection on one page.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SelectionBox {
    pub page: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Which line holds a position: the line whose range contains it, or for a
/// position at a line's end, that line only when it ends the paragraph.
pub fn line_index(lines: &[CaretLine], position: TextPosition) -> Option<usize> {
    let mut candidate = None;
    for (index, line) in lines.iter().enumerate() {
        if line.paragraph != position.paragraph {
            continue;
        }
        if position.offset >= line.range.start && position.offset < line.range.end {
            return Some(index);
        }
        if position.offset == line.range.end {
            candidate = Some(index);
        }
        if line.range.is_empty() && position.offset == line.range.start {
            return Some(index);
        }
    }
    candidate
}

pub fn caret(lines: &[CaretLine], position: TextPosition) -> Option<Caret> {
    let line = &lines[line_index(lines, position)?];
    Some(Caret {
        page: line.page,
        x: line.x_of(position.offset),
        top: line.top,
        bottom: line.bottom,
    })
}

/// The boxes covering the text between two positions, one per line.
pub fn selection_boxes(lines: &[CaretLine], from: TextPosition, to: TextPosition) -> Vec<SelectionBox> {
    let (start, end) = ordered(from, to);
    if start == end {
        return Vec::new();
    }
    let mut boxes = Vec::new();
    for line in lines {
        let line_start = TextPosition::new(line.paragraph, line.range.start);
        let line_end = TextPosition::new(line.paragraph, line.range.end);
        if line_end < start || line_start > end || (line_end == start && !line.range.is_empty()) {
            continue;
        }
        let a = if start > line_start { line.x_of(start.offset) } else { line.stops[0].1 };
        let b = if end < line_end {
            line.x_of(end.offset)
        } else {
            let last = line.stops.last().map(|stop| stop.1).unwrap_or(a);
            // A selection running on into the next paragraph shows a little
            // box for the paragraph break.
            if end.paragraph > line.paragraph && line_end.offset == line.range.end { last + 1.0 } else { last }
        };
        if b > a {
            boxes.push(SelectionBox {
                page: line.page,
                x: a,
                y: line.top,
                w: b - a,
                h: line.bottom - line.top,
            });
        }
    }
    boxes
}

/// The position nearest to a point among the lines of one frame.
pub fn position_at(lines: &[CaretLine], page: usize, frame_id: &str, x: f64, y: f64) -> Option<TextPosition> {
    let in_frame: Vec<&CaretLine> = lines
        .iter()
        .filter(|line| line.page == page && line.frame_id == frame_id)
        .collect();
    let line = in_frame
        .iter()
        .find(|line| y >= line.top && y < line.bottom)
        .copied()
        .or_else(|| {
            in_frame
                .iter()
                .min_by(|a, b| distance(a, y).total_cmp(&distance(b, y)))
                .copied()
        })?;
    let mut offset = line.offset_at(x);
    // A click past a wrapped line's end lands before its trailing space, so
    // the caret stays on that line.
    if offset == line.range.end && line_index(lines, TextPosition::new(line.paragraph, offset)).is_some_and(|index| lines[index].range != line.range) {
        offset = line
            .stops
            .iter()
            .rev()
            .map(|(stop, _)| *stop)
            .find(|stop| *stop < line.range.end)
            .unwrap_or(line.range.start);
    }
    Some(TextPosition::new(line.paragraph, offset))
}

fn distance(line: &CaretLine, y: f64) -> f64 {
    if y < line.top {
        line.top - y
    } else if y > line.bottom {
        y - line.bottom
    } else {
        0.0
    }
}

/// The position on the line above or below, nearest to `x`.
pub fn vertical(lines: &[CaretLine], position: TextPosition, x: f64, down: bool) -> Option<TextPosition> {
    let index = line_index(lines, position)?;
    let target = if down { index + 1 } else { index.checked_sub(1)? };
    let line = lines.get(target)?;
    let mut offset = line.offset_at(x);
    if offset == line.range.end && target + 1 < lines.len() && lines[target + 1].paragraph == line.paragraph {
        offset = line
            .stops
            .iter()
            .rev()
            .map(|(stop, _)| *stop)
            .find(|stop| *stop < line.range.end)
            .unwrap_or(line.range.start);
    }
    Some(TextPosition::new(line.paragraph, offset))
}

/// The start or end of the line holding a position.
pub fn line_edge(lines: &[CaretLine], position: TextPosition, end: bool) -> Option<TextPosition> {
    let line = &lines[line_index(lines, position)?];
    let offset = if end {
        // Before a trailing space when the line wraps.
        let last = line.range.end;
        let wraps = lines
            .iter()
            .any(|other| other.paragraph == line.paragraph && other.range.start == last && other.range != line.range);
        if wraps {
            line.stops
                .iter()
                .rev()
                .map(|(stop, _)| *stop)
                .find(|stop| *stop < last)
                .unwrap_or(line.range.start)
        } else {
            last
        }
    } else {
        line.range.start
    };
    Some(TextPosition::new(line.paragraph, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn story(paragraphs: &[&str]) -> Story {
        Story {
            id: "s".to_owned(),
            paragraphs: paragraphs.iter().map(|text| Paragraph::plain("body", text)).collect(),
        }
    }

    fn texts(paragraphs: &[Paragraph]) -> Vec<String> {
        paragraphs.iter().map(Paragraph::text).collect()
    }

    #[test]
    fn insert_typing_and_korean() {
        let s = story(&["Hello"]);
        let r = replace(&s, TextPosition::new(0, 5), TextPosition::new(0, 5), " 한글");
        assert_eq!(texts(&r.after), vec!["Hello 한글"]);
        assert_eq!(r.caret, TextPosition::new(0, 5 + " 한글".len()));
        assert_eq!(r.first, 0);
        assert_eq!(texts(&r.before), vec!["Hello"]);
    }

    #[test]
    fn line_break_splits_a_paragraph_keeping_its_style() {
        let mut s = story(&["onetwo"]);
        s.paragraphs[0].style = "title".to_owned();
        let r = replace(&s, TextPosition::new(0, 3), TextPosition::new(0, 3), "\n");
        assert_eq!(texts(&r.after), vec!["one", "two"]);
        assert!(r.after.iter().all(|p| p.style == "title"));
        assert_eq!(r.caret, TextPosition::new(1, 0));
        let r = replace(&s, TextPosition::new(0, 6), TextPosition::new(0, 6), "\r\nA\nB");
        assert_eq!(texts(&r.after), vec!["onetwo", "A", "B"]);
        assert_eq!(r.caret, TextPosition::new(2, 1));
    }

    #[test]
    fn deleting_across_paragraphs_joins_them() {
        let s = story(&["abc", "def", "ghi"]);
        let r = replace(&s, TextPosition::new(0, 1), TextPosition::new(2, 2), "");
        assert_eq!(texts(&r.after), vec!["ai"]);
        assert_eq!(r.before.len(), 3);
        assert_eq!(r.caret, TextPosition::new(0, 1));
        // Backspace at a paragraph start joins it to the previous one.
        let back = previous_position(&s, TextPosition::new(1, 0));
        assert_eq!(back, TextPosition::new(0, 3));
        let r = replace(&s, back, TextPosition::new(1, 0), "");
        assert_eq!(texts(&r.after), vec!["abcdef"]);
    }

    #[test]
    fn character_steps_respect_hangul_syllables() {
        let s = story(&["가나"]);
        assert_eq!(next_position(&s, TextPosition::new(0, 0)), TextPosition::new(0, 3));
        assert_eq!(previous_position(&s, TextPosition::new(0, 6)), TextPosition::new(0, 3));
        assert_eq!(clamp(&s, TextPosition::new(0, 4)), TextPosition::new(0, 3));
        assert_eq!(clamp(&s, TextPosition::new(5, 99)), TextPosition::new(0, 6));
    }

    #[test]
    fn character_styles_split_and_merge_runs() {
        let s = story(&["one two three"]);
        let r = set_character_style(&s, TextPosition::new(0, 4), TextPosition::new(0, 7), Some("em"));
        let runs = &r.after[0].runs;
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[1].text, "two");
        assert_eq!(runs[1].character_style.as_deref(), Some("em"));
        // Typing after the styled word continues the style.
        let mut styled = s.clone();
        styled.paragraphs = r.after.clone();
        let typed = replace(&styled, TextPosition::new(0, 7), TextPosition::new(0, 7), "!");
        assert_eq!(typed.after[0].runs[1].text, "two!");
        // Clearing merges the runs back into one.
        let cleared = set_character_style(&styled, TextPosition::new(0, 0), TextPosition::new(0, 13), None);
        assert_eq!(cleared.after[0].runs.len(), 1);
    }

    #[test]
    fn words_and_text_between() {
        let s = story(&["안녕 world, 하세요", "next"]);
        let (a, b) = word_at(&s, TextPosition::new(0, 8));
        assert_eq!(text_between(&s, a, b), "world");
        assert_eq!(next_word(&s, TextPosition::new(0, 0)), TextPosition::new(0, 6));
        assert_eq!(previous_word(&s, TextPosition::new(0, 13)), TextPosition::new(0, 7));
        assert_eq!(text_between(&s, TextPosition::new(0, 7), TextPosition::new(1, 2)), "world, 하세요\nne");
    }

    #[test]
    fn caret_lines_map_offsets_and_points() {
        let lines = vec![
            CaretLine {
                page: 0,
                frame_id: "f".to_owned(),
                top: 10.0,
                bottom: 15.0,
                paragraph: 0,
                range: 0..6,
                stops: vec![(0, 20.0), (3, 23.0), (5, 25.0), (6, 26.0)],
            },
            CaretLine {
                page: 0,
                frame_id: "f".to_owned(),
                top: 15.0,
                bottom: 20.0,
                paragraph: 0,
                range: 6..9,
                stops: vec![(6, 20.0), (9, 23.0)],
            },
        ];
        // The end of a wrapped line belongs to the next line.
        assert_eq!(caret(&lines, TextPosition::new(0, 6)).unwrap().top, 15.0);
        assert_eq!(caret(&lines, TextPosition::new(0, 9)).unwrap().x, 23.0);
        assert_eq!(caret(&lines, TextPosition::new(0, 3)).unwrap().x, 23.0);
        // A click past the first line's end stays on that line.
        assert_eq!(position_at(&lines, 0, "f", 40.0, 12.0), Some(TextPosition::new(0, 5)));
        assert_eq!(position_at(&lines, 0, "f", 0.0, 17.0), Some(TextPosition::new(0, 6)));
        assert_eq!(vertical(&lines, TextPosition::new(0, 3), 23.0, true), Some(TextPosition::new(0, 9)));
        assert_eq!(line_edge(&lines, TextPosition::new(0, 2), true), Some(TextPosition::new(0, 5)));
        let boxes = selection_boxes(&lines, TextPosition::new(0, 3), TextPosition::new(0, 9));
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[0].x, 23.0);
    }
}
