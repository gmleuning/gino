//! Commands. Every change to the document is a command that knows its
//! inverse, so undo and redo are an interface on top of this, not a retrofit.

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::model::{Document, ImageCrop, ImageFit, Rect};

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
        }
    }

    /// A command whose before and after are the same changes nothing and is
    /// not worth an undo step.
    pub fn is_no_op(&self) -> bool {
        match self {
            Command::SetFrameRect { before, after, .. } => before == after,
            Command::SetImage { before, after, .. } => before == after,
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
        if command.is_no_op() {
            return Ok(());
        }
        command.apply(document)?;
        self.undo.push(command);
        self.redo.clear();
        Ok(())
    }

    pub fn undo(&mut self, document: &mut Document) -> Result<bool, Error> {
        let Some(command) = self.undo.pop() else {
            return Ok(false);
        };
        command.inverse().apply(document)?;
        self.redo.push(command);
        Ok(true)
    }

    pub fn redo(&mut self, document: &mut Document) -> Result<bool, Error> {
        let Some(command) = self.redo.pop() else {
            return Ok(false);
        };
        command.apply(document)?;
        self.undo.push(command);
        Ok(true)
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
    use crate::spike::{spike_document, SpikeFamilies};

    fn document() -> Document {
        spike_document(&SpikeFamilies {
            latin: "A".to_owned(),
            korean: "B".to_owned(),
        })
    }

    #[test]
    fn apply_then_undo_returns_the_identical_document() {
        let original = document();
        let mut edited = original.clone();
        let mut history = History::default();
        let before = edited.frame(0, "thread_a").unwrap().rect();
        history
            .apply(
                &mut edited,
                Command::SetFrameRect {
                    page: 0,
                    frame: "thread_a".to_owned(),
                    before,
                    after: Rect::new(30.0, 200.0, 50.0, 60.0),
                },
            )
            .unwrap();
        history
            .apply(
                &mut edited,
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
            )
            .unwrap();
        assert_ne!(original, edited);
        assert!(history.can_undo());

        assert!(history.undo(&mut edited).unwrap());
        assert!(history.undo(&mut edited).unwrap());
        assert!(!history.undo(&mut edited).unwrap());
        assert_eq!(original, edited);

        assert!(history.redo(&mut edited).unwrap());
        assert_eq!(edited.frame(0, "thread_a").unwrap().rect(), Rect::new(30.0, 200.0, 50.0, 60.0));
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
