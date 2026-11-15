//! Commands. Every change to the document is a command that knows its
//! inverse, so undo and redo are an interface on top of this, not a retrofit.

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::model::{
    CharacterStyle, Document, Frame, ImageCrop, ImageFit, Page, Paragraph, ParagraphStyle, Rect,
    Settings, Story,
};
use crate::text::TextPosition;

/// Everything about an image frame's picture, as one value so that one
/// command carries the whole before and after state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageSettings {
    pub image: Option<String>,
    pub fit: ImageFit,
    pub crop: Option<ImageCrop>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Command {
    /// Move or resize a frame.
    SetFrameRect {
        page: usize,
        frame: String,
        before: Rect,
        after: Rect,
    },
    /// Place, replace or re-crop an image.
    SetImage {
        page: usize,
        frame: String,
        before: ImageSettings,
        after: ImageSettings,
    },
    /// Replace a frame's whole description (threading, kind settings).
    SetFrame {
        page: usize,
        before: Frame,
        after: Frame,
    },
    /// Put a frame into a page's stacking order.
    InsertFrame { page: usize, index: usize, frame: Frame },
    /// Take a frame out of a page.
    RemoveFrame { page: usize, index: usize, frame: Frame },
    /// Move a frame within its page's stacking order.
    MoveFrameInStack { page: usize, from: usize, to: usize },
    /// Replace `before.len()` paragraphs of a story, starting at `first`, with
    /// `after`. Typing, deleting, splitting and styling text are all this.
    SetParagraphs {
        story: String,
        first: usize,
        before: Vec<Paragraph>,
        after: Vec<Paragraph>,
        /// Where the caret was before, and goes after; not part of the document.
        caret_before: TextPosition,
        caret_after: TextPosition,
    },
    /// Add (before is none), change, or remove (after is none) a paragraph style.
    SetParagraphStyle {
        id: String,
        before: Option<ParagraphStyle>,
        after: Option<ParagraphStyle>,
    },
    SetCharacterStyle {
        id: String,
        before: Option<CharacterStyle>,
        after: Option<CharacterStyle>,
    },
    /// Add, replace or remove a whole story.
    SetStory {
        id: String,
        before: Option<Story>,
        after: Option<Story>,
    },
    /// Replace `before.len()` pages starting at `first` with `after`.
    SetPages {
        first: usize,
        before: Vec<Page>,
        after: Vec<Page>,
    },
    SetSettings { before: Settings, after: Settings },
    /// Several commands as one undo step, applied in order.
    Batch(Vec<Command>),
}

fn missing(what: &str) -> Error {
    Error::Edit(format!("{what} does not exist"))
}

impl Command {
    pub fn apply(&self, document: &mut Document) -> Result<(), Error> {
        match self {
            Command::SetFrameRect { page, frame, after, .. } => {
                document.frame_mut(*page, frame)?.set_rect(*after);
            }
            Command::SetImage { page, frame, after, .. } => {
                let target = document.frame_mut(*page, frame)?;
                target.image = after.image.clone();
                target.fit = after.fit;
                target.crop = after.crop;
            }
            Command::SetFrame { page, before, after } => {
                *document.frame_mut(*page, &before.id)? = after.clone();
            }
            Command::InsertFrame { page, index, frame } => {
                let frames = &mut document.pages.get_mut(*page).ok_or_else(|| missing("the page"))?.frames;
                frames.insert((*index).min(frames.len()), frame.clone());
            }
            Command::RemoveFrame { page, frame, .. } => {
                let frames = &mut document.pages.get_mut(*page).ok_or_else(|| missing("the page"))?.frames;
                let position = frames
                    .iter()
                    .position(|candidate| candidate.id == frame.id)
                    .ok_or(Error::MissingFrame {
                        page: *page,
                        frame: frame.id.clone(),
                    })?;
                frames.remove(position);
            }
            Command::MoveFrameInStack { page, from, to } => {
                let frames = &mut document.pages.get_mut(*page).ok_or_else(|| missing("the page"))?.frames;
                if *from >= frames.len() || *to >= frames.len() {
                    return Err(missing("the frame position"));
                }
                let frame = frames.remove(*from);
                frames.insert(*to, frame);
            }
            Command::SetParagraphs {
                story, first, before, after, ..
            } => {
                let target = document
                    .stories
                    .iter_mut()
                    .find(|candidate| candidate.id == *story)
                    .ok_or_else(|| Error::MissingStory { story: story.clone() })?;
                let end = first + before.len();
                if end > target.paragraphs.len() {
                    return Err(missing("the paragraph"));
                }
                target.paragraphs.splice(*first..end, after.iter().cloned());
            }
            Command::SetParagraphStyle { id, after, .. } => {
                let styles = &mut document.paragraph_styles;
                let position = styles.iter().position(|style| style.id == *id);
                match (position, after) {
                    (Some(index), Some(style)) => styles[index] = style.clone(),
                    (Some(index), None) => {
                        styles.remove(index);
                    }
                    (None, Some(style)) => styles.push(style.clone()),
                    (None, None) => {}
                }
            }
            Command::SetCharacterStyle { id, after, .. } => {
                let styles = &mut document.character_styles;
                let position = styles.iter().position(|style| style.id == *id);
                match (position, after) {
                    (Some(index), Some(style)) => styles[index] = style.clone(),
                    (Some(index), None) => {
                        styles.remove(index);
                    }
                    (None, Some(style)) => styles.push(style.clone()),
                    (None, None) => {}
                }
            }
            Command::SetStory { id, after, .. } => {
                let stories = &mut document.stories;
                let position = stories.iter().position(|story| story.id == *id);
                match (position, after) {
                    (Some(index), Some(story)) => stories[index] = story.clone(),
                    (Some(index), None) => {
                        stories.remove(index);
                    }
                    (None, Some(story)) => stories.push(story.clone()),
                    (None, None) => {}
                }
            }
            Command::SetPages { first, before, after } => {
                let end = first + before.len();
                if end > document.pages.len() {
                    return Err(missing("the page"));
                }
                document.pages.splice(*first..end, after.iter().cloned());
                document.document.page_count = document.pages.len() as u32;
            }
            Command::SetSettings { after, .. } => {
                let pages = document.pages.len() as u32;
                document.document = after.clone();
                document.document.page_count = pages;
            }
            Command::Batch(commands) => {
                for command in commands {
                    command.apply(document)?;
                }
            }
        }
        Ok(())
    }

    pub fn inverse(&self) -> Command {
        match self {
            Command::SetFrameRect {
                page,
                frame,
                before,
                after,
            } => Command::SetFrameRect {
                page: *page,
                frame: frame.clone(),
                before: *after,
                after: *before,
            },
            Command::SetImage {
                page,
                frame,
                before,
                after,
            } => Command::SetImage {
                page: *page,
                frame: frame.clone(),
                before: after.clone(),
                after: before.clone(),
            },
            Command::SetFrame { page, before, after } => Command::SetFrame {
                page: *page,
                before: after.clone(),
                after: before.clone(),
            },
            Command::InsertFrame { page, index, frame } => Command::RemoveFrame {
                page: *page,
                index: *index,
                frame: frame.clone(),
            },
            Command::RemoveFrame { page, index, frame } => Command::InsertFrame {
                page: *page,
                index: *index,
                frame: frame.clone(),
            },
            Command::MoveFrameInStack { page, from, to } => Command::MoveFrameInStack {
                page: *page,
                from: *to,
                to: *from,
            },
            Command::SetParagraphs {
                story,
                first,
                before,
                after,
                caret_before,
                caret_after,
            } => Command::SetParagraphs {
                story: story.clone(),
                first: *first,
                before: after.clone(),
                after: before.clone(),
                caret_before: *caret_after,
                caret_after: *caret_before,
            },
            Command::SetParagraphStyle { id, before, after } => Command::SetParagraphStyle {
                id: id.clone(),
                before: after.clone(),
                after: before.clone(),
            },
            Command::SetCharacterStyle { id, before, after } => Command::SetCharacterStyle {
                id: id.clone(),
                before: after.clone(),
                after: before.clone(),
            },
            Command::SetStory { id, before, after } => Command::SetStory {
                id: id.clone(),
                before: after.clone(),
                after: before.clone(),
            },
            Command::SetPages { first, before, after } => Command::SetPages {
                first: *first,
                before: after.clone(),
                after: before.clone(),
            },
            Command::SetSettings { before, after } => Command::SetSettings {
                before: after.clone(),
                after: before.clone(),
            },
            Command::Batch(commands) => Command::Batch(commands.iter().rev().map(Command::inverse).collect()),
        }
    }

    /// A command whose before and after are the same changes nothing and is
    /// not worth an undo step.
    pub fn is_no_op(&self) -> bool {
        match self {
            Command::SetFrameRect { before, after, .. } => before == after,
            Command::SetImage { before, after, .. } => before == after,
            Command::SetFrame { before, after, .. } => before == after,
            Command::InsertFrame { .. } | Command::RemoveFrame { .. } => false,
            Command::MoveFrameInStack { from, to, .. } => from == to,
            Command::SetParagraphs { before, after, .. } => before == after,
            Command::SetParagraphStyle { before, after, .. } => before == after,
            Command::SetCharacterStyle { before, after, .. } => before == after,
            Command::SetStory { before, after, .. } => before == after,
            Command::SetPages { before, after, .. } => before == after,
            Command::SetSettings { before, after } => before == after,
            Command::Batch(commands) => commands.iter().all(Command::is_no_op),
        }
    }

    /// Where the caret goes after this command, when it edits text.
    pub fn caret_after(&self) -> Option<(&str, TextPosition)> {
        match self {
            Command::SetParagraphs { story, caret_after, .. } => Some((story, *caret_after)),
            Command::Batch(commands) => commands.iter().rev().find_map(Command::caret_after),
            _ => None,
        }
    }
}

/// Undo and redo stacks.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    /// Applies a command and records it. No-op commands are dropped.
    pub fn apply(&mut self, document: &mut Document, command: Command) -> Result<(), Error> {
        self.apply_merging(document, command, false)
    }

    /// As [`History::apply`]; with `merge`, a text edit that continues the
    /// previous one (same paragraphs, picking up where it left off) joins it,
    /// so a typed word is one undo step rather than one per keystroke.
    pub fn apply_merging(&mut self, document: &mut Document, command: Command, merge: bool) -> Result<(), Error> {
        if command.is_no_op() {
            return Ok(());
        }
        command.apply(document)?;
        self.redo.clear();
        if merge {
            if let (
                Some(Command::SetParagraphs {
                    story: last_story,
                    first: last_first,
                    after: last_after,
                    caret_after: last_caret,
                    ..
                }),
                Command::SetParagraphs {
                    story,
                    first,
                    before,
                    after,
                    caret_after,
                    ..
                },
            ) = (self.undo.last_mut(), &command)
            {
                if last_story == story && last_first == first && last_after == before {
                    *last_after = after.clone();
                    *last_caret = *caret_after;
                    return Ok(());
                }
            }
        }
        self.undo.push(command);
        Ok(())
    }

    /// Undoes the last command and returns the command that was applied to
    /// do so (the inverse), so the caller can put the caret back.
    pub fn undo(&mut self, document: &mut Document) -> Result<Option<Command>, Error> {
        let Some(command) = self.undo.pop() else {
            return Ok(None);
        };
        let inverse = command.inverse();
        inverse.apply(document)?;
        self.redo.push(command);
        Ok(Some(inverse))
    }

    pub fn redo(&mut self, document: &mut Document) -> Result<Option<Command>, Error> {
        let Some(command) = self.redo.pop() else {
            return Ok(None);
        };
        command.apply(document)?;
        self.undo.push(command.clone());
        Ok(Some(command))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Frame;
    use crate::spike::{spike_document, SpikeFamilies};
    use crate::text::replace;

    fn document() -> Document {
        spike_document(&SpikeFamilies {
            latin: "A".to_owned(),
            korean: "B".to_owned(),
        })
    }

    fn typing(document: &Document, at: TextPosition, text: &str) -> Command {
        let story = document.story("break_syllable").unwrap();
        let replacement = replace(story, at, at, text);
        Command::SetParagraphs {
            story: story.id.clone(),
            first: replacement.first,
            before: replacement.before,
            after: replacement.after,
            caret_before: at,
            caret_after: replacement.caret,
        }
    }

    #[test]
    fn apply_then_undo_returns_the_identical_document() {
        let original = document();
        let mut edited = original.clone();
        let mut history = History::default();
        let before = edited.frame(0, "thread_a").unwrap().rect();
        let commands = vec![
            Command::SetFrameRect {
                page: 0,
                frame: "thread_a".to_owned(),
                before,
                after: Rect::new(30.0, 200.0, 50.0, 60.0),
            },
            Command::SetImage {
                page: 0,
                frame: "edge_a".to_owned(),
                before: ImageSettings {
                    image: None,
                    fit: ImageFit::Fill,
                    crop: None,
                },
                after: ImageSettings {
                    image: Some("pictures\\cat.png".to_owned()),
                    fit: ImageFit::Manual,
                    crop: Some(ImageCrop {
                        offset_x: -3.0,
                        offset_y: 1.0,
                        scale: 0.1,
                    }),
                },
            },
            Command::InsertFrame {
                page: 0,
                index: 1,
                frame: Frame::rectangle("box", 1.0, 2.0, 3.0, 4.0, 0.2, 0.0),
            },
            Command::MoveFrameInStack { page: 0, from: 0, to: 2 },
            typing(&original, TextPosition::new(0, 0), "새 글\n"),
            Command::SetParagraphStyle {
                id: "new".to_owned(),
                before: None,
                after: Some(ParagraphStyle {
                    id: "new".to_owned(),
                    ..original.paragraph_styles[0].clone()
                }),
            },
            Command::SetStory {
                id: "extra".to_owned(),
                before: None,
                after: Some(Story {
                    id: "extra".to_owned(),
                    paragraphs: vec![Paragraph::plain("body", "x")],
                }),
            },
            Command::SetPages {
                first: 1,
                before: Vec::new(),
                after: vec![Page::default(), Page::default()],
            },
            Command::Batch(vec![
                Command::SetSettings {
                    before: original.document.clone(),
                    after: Settings {
                        bleed: 9.0,
                        ..original.document.clone()
                    },
                },
                // After the insert at 1 and the move from 0 to 2, the
                // original second frame sits at index 1.
                Command::RemoveFrame {
                    page: 0,
                    index: 1,
                    frame: original.pages[0].frames[1].clone(),
                },
            ]),
        ];
        let count = commands.len();
        for command in commands {
            history.apply(&mut edited, command).unwrap();
        }
        assert_ne!(original, edited);
        assert_eq!(edited.pages.len(), 3);
        assert_eq!(edited.document.page_count, 3);

        for _ in 0..count {
            assert!(history.undo(&mut edited).unwrap().is_some());
        }
        assert!(history.undo(&mut edited).unwrap().is_none());
        assert_eq!(original.document, edited.document);
        assert_eq!(original.paragraph_styles, edited.paragraph_styles);
        assert_eq!(original.stories, edited.stories);
        assert_eq!(original.pages.len(), edited.pages.len());
        for (a, b) in original.pages[0].frames.iter().zip(&edited.pages[0].frames) {
            assert_eq!(a, b);
        }
        assert_eq!(original, edited);

        assert!(history.redo(&mut edited).unwrap().is_some());
        assert_eq!(edited.frame(0, "thread_a").unwrap().rect(), Rect::new(30.0, 200.0, 50.0, 60.0));
    }

    #[test]
    fn typing_merges_into_one_undo_step() {
        let original = document();
        let mut edited = original.clone();
        let mut history = History::default();
        let mut caret = TextPosition::new(0, 0);
        for letter in ["가", "나", "다"] {
            let command = typing(&edited, caret, letter);
            caret = command.caret_after().unwrap().1;
            history.apply_merging(&mut edited, command, true).unwrap();
        }
        assert!(edited.story("break_syllable").unwrap().paragraphs[0].text().starts_with("가나다"));
        let undone = history.undo(&mut edited).unwrap().unwrap();
        assert_eq!(undone.caret_after().unwrap().1, TextPosition::new(0, 0));
        assert!(!history.can_undo());
        assert_eq!(original, edited);
    }

    #[test]
    fn no_op_commands_are_not_recorded() {
        let mut edited = document();
        let mut history = History::default();
        let rect = edited.frame(0, "thread_a").unwrap().rect();
        history
            .apply(
                &mut edited,
                Command::SetFrameRect {
                    page: 0,
                    frame: "thread_a".to_owned(),
                    before: rect,
                    after: rect,
                },
            )
            .unwrap();
        assert!(!history.can_undo());
    }

    #[test]
    fn a_missing_frame_is_a_plain_error() {
        let mut edited = document();
        let error = Command::SetFrameRect {
            page: 3,
            frame: "nowhere".to_owned(),
            before: Rect::default(),
            after: Rect::default(),
        }
        .apply(&mut edited)
        .unwrap_err();
        assert_eq!(error.to_string(), "the frame \"nowhere\" does not exist on page 3");
    }
}
