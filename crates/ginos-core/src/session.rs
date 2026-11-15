//! An open document being edited. The interface sends intents (pointer events
//! in page millimetres, keystrokes, input-method composition, inspector
//! values, menu commands); the session turns them into commands, re-composes,
//! and answers with patches: the SVG of every frame whose drawing changed,
//! plus selection, caret and inspector data. The session is the only holder
//! of document state.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::compose::{compose_with, Composition, ImageProblem, Overflow, Preedit};
use crate::edit::{Command, History, ImageSettings};
use crate::error::Error;
use crate::export::{imposed_pdf, reader_pdf, write_pdf};
use crate::fonts::Fonts;
use crate::guides::page_guides_svg;
use crate::hit::{frame_at, handle_at, Handle};
use crate::images::{crop_for_fit, ImageCache};
use crate::impose::{sheet_mode, SheetMode};
use crate::model::{
    CharacterStyle, Document, Frame, FrameKind, ImageFit, Page, Paragraph, ParagraphStyle, Rect,
    Settings, Size, Story,
};
use crate::presets::{new_document, NewDocument};
use crate::render_svg::{frame_to_svg, page_to_svg};
use crate::scene::{FrameScene, Item};
use crate::snap::{snap_move, snap_resize, targets};
use crate::spreads::{reader_spreads, spread_offsets};
use crate::text::{
    self, caret, character_style_at, clamp, line_edge, next_position, next_word, position_at,
    previous_position, previous_word, selection_boxes, story_end, story_start, text_between,
    vertical, word_at, Caret, Replacement, SelectionBox, TextPosition,
};
use crate::units::pt_to_mm;
use crate::zine::Zine;

/// Smallest frame side the canvas will produce, in millimetres.
const MINIMUM_FRAME_SIDE: f64 = 2.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FramePatch {
    pub page: usize,
    pub frame_id: String,
    pub kind: FrameKind,
    pub rect: Rect,
    pub svg: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SelectionInfo {
    pub page: usize,
    pub frame_id: String,
    pub kind: FrameKind,
    pub rect: Rect,
    pub image: Option<String>,
    pub fit: ImageFit,
    pub story: Option<String>,
    /// Position of the frame in its story's thread, from 1, and the thread's length.
    pub thread_position: usize,
    pub thread_length: usize,
    /// Position in the page's stacking order, from 0 at the bottom, and the frame count.
    pub stack_index: usize,
    pub stack_count: usize,
    pub stroke_width: f64,
    pub grey: f64,
    pub filled: bool,
}

/// Snap lines that were used by the last pointer move, for the canvas to show.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct SnapFeedback {
    pub page: usize,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

/// The text being edited: caret, selection and the styles at the caret.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TextView {
    pub story: String,
    pub caret: Option<Caret>,
    pub selection: Vec<SelectionBox>,
    pub selected_text: String,
    pub composing: bool,
    pub paragraph_style: String,
    pub character_style: Option<String>,
}

/// A page whose frames were added, removed or restacked.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PageFrames {
    pub page: usize,
    pub frame_ids: Vec<String>,
}

/// What changed after an intent.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Update {
    pub patches: Vec<FramePatch>,
    pub page_frames: Vec<PageFrames>,
    /// Pages or guides changed: the interface asks for the whole view again.
    pub view_changed: bool,
    pub selection: Option<SelectionInfo>,
    pub text: Option<TextView>,
    pub snap: Option<SnapFeedback>,
    pub overflows: Vec<Overflow>,
    pub image_problems: Vec<ImageProblem>,
    pub paragraph_styles: Vec<ParagraphStyle>,
    pub character_styles: Vec<CharacterStyle>,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageView {
    pub index: usize,
    pub width: f64,
    pub height: f64,
    pub guides_svg: String,
    /// Frame ids in stacking order.
    pub frame_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadView {
    pub pages: Vec<(usize, f64)>,
    pub width: f64,
    pub height: f64,
}

/// Everything the interface needs after opening a document.
#[derive(Debug, Clone, Serialize)]
pub struct DocumentView {
    pub title: String,
    pub folder: String,
    pub settings: Settings,
    pub sheet_mode: Option<SheetMode>,
    pub pages: Vec<PageView>,
    pub spreads: Vec<SpreadView>,
    pub update: Update,
}

/// What a press on the canvas does.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    #[default]
    Select,
    Text,
    Image,
    Rectangle,
    Rule,
}

/// Caret movements from the keyboard.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    StoryStart,
    StoryEnd,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Arrange {
    Forward,
    Backward,
    Front,
    Back,
}

#[derive(Debug, Clone)]
enum DragMode {
    Move { grab_dx: f64, grab_dy: f64 },
    Resize(Handle),
    Create { kind: FrameKind, origin_x: f64, origin_y: f64 },
    TextSelect { frame_id: String },
}

#[derive(Debug, Clone)]
struct Drag {
    page: usize,
    frame_id: String,
    start: Rect,
    mode: DragMode,
    tolerance: f64,
}

#[derive(Debug, Clone)]
struct TextEdit {
    story: String,
    anchor: TextPosition,
    focus: TextPosition,
    /// Text the input method is composing, shown underlined at the focus.
    preedit: String,
    /// The x the caret keeps while moving up and down.
    goal_x: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeKind {
    Typing,
    Deleting,
}

pub struct Session {
    pub zine: Zine,
    pub fonts: Fonts,
    images: ImageCache,
    history: History,
    selection: Option<(usize, String)>,
    drag: Option<Drag>,
    editing: Option<TextEdit>,
    last_merge: Option<MergeKind>,
    fingerprints: HashMap<(usize, String), u64>,
    page_frames: Vec<Vec<String>>,
    composition: Composition,
    view_changed: bool,
    dirty: bool,
}

impl Session {
    /// Opens a `.zine` folder and loads the document's fonts.
    pub fn open(path: &Path, fonts: Fonts) -> Result<Self, Error> {
        Self::from_zine(Zine::open(path)?, fonts)
    }

    fn from_zine(zine: Zine, mut fonts: Fonts) -> Result<Self, Error> {
        for folder in zine.font_folders() {
            fonts.add_folder(&folder);
        }
        let images = ImageCache::new(&zine.folder);
        let mut session = Self {
            zine,
            fonts,
            images,
            history: History::default(),
            selection: None,
            drag: None,
            editing: None,
            last_merge: None,
            fingerprints: HashMap::new(),
            page_frames: Vec::new(),
            composition: Composition::default(),
            view_changed: false,
            dirty: false,
        };
        session.recompose()?;
        Ok(session)
    }

    /// Creates a new `.zine` folder at `path` (the suffix is added when
    /// missing), writes the document and opens it.
    pub fn create(path: &Path, request: &NewDocument, mut fonts: Fonts) -> Result<Self, Error> {
        let folder = if path.extension().is_some_and(|extension| extension == "zine") {
            path.to_owned()
        } else {
            let mut name = path.as_os_str().to_owned();
            name.push(".zine");
            PathBuf::from(name)
        };
        if folder.exists() && std::fs::read_dir(&folder).map(|mut entries| entries.next().is_some()).unwrap_or(true) {
            return Err(Error::Invalid(format!(
                "{} already exists and is not empty; choose another name",
                folder.display()
            )));
        }
        fonts.require_family(&request.latin_font)?;
        fonts.require_family(&request.korean_font)?;
        let document = new_document(request)?;
        let zine = Zine { folder, document };
        zine.save()?;
        let _ = std::fs::create_dir_all(zine.folder.join("assets"));
        Self::from_zine(zine, fonts)
    }

    /// The full view, with every frame's SVG as a patch.
    pub fn view(&mut self) -> Result<DocumentView, Error> {
        self.fingerprints.clear();
        self.page_frames.clear();
        let mut update = self.recompose()?;
        update.page_frames.clear();
        update.view_changed = false;
        let document = &self.zine.document;
        let size = document.page_size();
        let pages = document
            .pages
            .iter()
            .enumerate()
            .map(|(index, page)| PageView {
                index,
                width: size.width,
                height: size.height,
                guides_svg: page_guides_svg(document, index),
                frame_ids: page.frames.iter().map(|frame| frame.id.clone()).collect(),
            })
            .collect();
        let spreads = reader_spreads(document.pages.len())
            .iter()
            .map(|spread| {
                let (pages, width, height) = spread_offsets(document, spread);
                SpreadView { pages, width, height }
            })
            .collect();
        Ok(DocumentView {
            title: self.zine.title(),
            folder: self.zine.folder.to_string_lossy().into_owned(),
            settings: document.document.clone(),
            sheet_mode: sheet_mode(document, document.document.sheet).ok(),
            pages,
            spreads,
            update,
        })
    }

    fn preedit(&self) -> Option<Preedit> {
        let edit = self.editing.as_ref()?;
        (!edit.preedit.is_empty()).then(|| Preedit {
            story: edit.story.clone(),
            position: edit.focus,
            text: edit.preedit.clone(),
        })
    }

    /// Re-lays out the document and returns the frames whose drawing changed.
    /// Unchanged paragraphs come from the layout cache and unchanged frames
    /// are recognised by a fingerprint, so only edited frames become SVG.
    fn recompose(&mut self) -> Result<Update, Error> {
        let preedit = self.preedit();
        self.composition = compose_with(&mut self.fonts, &self.zine.document, &mut self.images, preedit.as_ref())?;
        let mut patches = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (page_index, page) in self.composition.scene.pages.iter().enumerate() {
            for frame in &page.frames {
                let key = (page_index, frame.id.clone());
                let model = self.zine.document.frame(page_index, &frame.id)?;
                let rect = model.rect().rounded();
                let print = fingerprint(frame, &rect, model.kind);
                if self.fingerprints.get(&key) != Some(&print) {
                    patches.push(FramePatch {
                        page: page_index,
                        frame_id: frame.id.clone(),
                        kind: model.kind,
                        rect,
                        svg: frame_to_svg(frame),
                    });
                    self.fingerprints.insert(key.clone(), print);
                }
                seen.insert(key);
            }
        }
        self.fingerprints.retain(|key, _| seen.contains(key));

        let mut page_frames = Vec::new();
        let current: Vec<Vec<String>> = self
            .zine
            .document
            .pages
            .iter()
            .map(|page| page.frames.iter().map(|frame| frame.id.clone()).collect())
            .collect();
        for (page, ids) in current.iter().enumerate() {
            if self.page_frames.get(page) != Some(ids) {
                page_frames.push(PageFrames {
                    page,
                    frame_ids: ids.clone(),
                });
            }
        }
        self.page_frames = current;
        let mut update = self.update_with(patches, None);
        update.page_frames = page_frames;
        update.view_changed = std::mem::take(&mut self.view_changed);
        Ok(update)
    }

    fn update_with(&self, patches: Vec<FramePatch>, snap: Option<SnapFeedback>) -> Update {
        let document = &self.zine.document;
        Update {
            patches,
            page_frames: Vec::new(),
            view_changed: false,
            selection: self.selection_info(),
            text: self.text_view(),
            snap,
            overflows: self.composition.overflows.clone(),
            image_problems: self.composition.image_problems.clone(),
            paragraph_styles: document.paragraph_styles.clone(),
            character_styles: document.character_styles.clone(),
            dirty: self.dirty,
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
        }
    }

    fn unchanged(&self) -> Update {
        self.update_with(Vec::new(), None)
    }

    fn selection_info(&self) -> Option<SelectionInfo> {
        let (page, frame_id) = self.selection.as_ref()?;
        let frame = self.zine.document.frame(*page, frame_id).ok()?;
        let frames = &self.zine.document.pages[*page].frames;
        let (thread_position, thread_length) = frame
            .story
            .as_ref()
            .and_then(|story| self.composition.threads.get(story))
            .map(|thread| {
                let position = thread.iter().position(|(_, id)| id == frame_id).map(|i| i + 1).unwrap_or(0);
                (position, thread.len())
            })
            .unwrap_or((0, 0));
        Some(SelectionInfo {
            page: *page,
            frame_id: frame.id.clone(),
            kind: frame.kind,
            rect: frame.rect().rounded(),
            image: frame.image.clone(),
            fit: frame.fit,
            story: frame.story.clone(),
            thread_position,
            thread_length,
            stack_index: frames.iter().position(|candidate| candidate.id == *frame_id).unwrap_or(0),
            stack_count: frames.len(),
            stroke_width: frame.stroke_width,
            grey: frame.grey,
            filled: frame.filled,
        })
    }

    fn text_view(&self) -> Option<TextView> {
        let edit = self.editing.as_ref()?;
        let story = self.zine.document.story(&edit.story).ok()?;
        let lines = self
            .composition
            .story_lines
            .get(&edit.story)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let shown = TextPosition::new(edit.focus.paragraph, edit.focus.offset + edit.preedit.len());
        let caret = caret(lines, shown).or_else(|| self.empty_story_caret(&edit.story));
        let paragraph = story.paragraphs.get(edit.focus.paragraph);
        Some(TextView {
            story: edit.story.clone(),
            caret,
            selection: if edit.preedit.is_empty() {
                selection_boxes(lines, edit.anchor, edit.focus)
            } else {
                Vec::new()
            },
            selected_text: text_between(story, edit.anchor, edit.focus),
            composing: !edit.preedit.is_empty(),
            paragraph_style: paragraph.map(|p| p.style.clone()).unwrap_or_default(),
            character_style: paragraph.and_then(|p| character_style_at(p, edit.focus.offset)),
        })
    }

    /// A caret at the top of a story's first frame, when nothing is laid out.
    fn empty_story_caret(&self, story_id: &str) -> Option<Caret> {
        let thread = self.composition.threads.get(story_id)?;
        let (page, frame_id) = thread.first()?;
        let frame = self.zine.document.frame(*page, frame_id).ok()?;
        let story = self.zine.document.story(story_id).ok()?;
        let leading = story
            .paragraphs
            .first()
            .and_then(|paragraph| self.zine.document.paragraph_style(&paragraph.style).ok())
            .map(|style| pt_to_mm(style.leading))
            .unwrap_or(5.0);
        let text_is_empty = story.paragraphs.iter().all(|paragraph| paragraph.text().is_empty());
        text_is_empty.then(|| Caret {
            page: *page,
            x: frame.x,
            top: frame.y,
            bottom: frame.y + leading,
        })
    }

    pub fn selection(&self) -> Option<(usize, &str)> {
        self.selection.as_ref().map(|(page, id)| (*page, id.as_str()))
    }

    pub fn select(&mut self, page: usize, frame_id: Option<&str>) -> Result<Update, Error> {
        self.editing = None;
        self.selection = match frame_id {
            Some(id) => {
                self.zine.document.frame(page, id)?;
                Some((page, id.to_owned()))
            }
            None => None,
        };
        self.recompose()
    }

    /// The topmost frame of a story under a point.
    fn story_frame_at(&self, story: &str, page: usize, x: f64, y: f64) -> Option<String> {
        self.zine
            .document
            .pages
            .get(page)?
            .frames
            .iter()
            .rev()
            .find(|frame| frame.story.as_deref() == Some(story) && frame.rect().contains(x, y))
            .map(|frame| frame.id.clone())
    }

    /// Pointer pressed on a page. With a drawing tool, starts a new frame.
    /// While editing text, a press in the story's frames moves the caret
    /// (Shift extends the selection; two clicks select a word, three a
    /// paragraph). A double click on a text frame starts editing it.
    /// Otherwise: resize on a handle of the selected frame, or select the
    /// frame under the pointer and start a move.
    #[allow(clippy::too_many_arguments)]
    pub fn pointer_down(
        &mut self,
        page: usize,
        x: f64,
        y: f64,
        tolerance: f64,
        tool: Tool,
        shift: bool,
        clicks: u32,
    ) -> Result<Update, Error> {
        self.last_merge = None;
        if page >= self.zine.document.pages.len() {
            return Ok(self.unchanged());
        }
        if tool != Tool::Select {
            return self.start_creating(page, x, y, tool, tolerance);
        }
        if let Some(story) = self.editing.as_ref().map(|edit| edit.story.clone()) {
            if let Some(frame_id) = self.story_frame_at(&story, page, x, y) {
                self.place_caret(page, &frame_id, x, y, shift, clicks)?;
                self.drag = Some(Drag {
                    page,
                    frame_id: frame_id.clone(),
                    start: Rect::default(),
                    mode: DragMode::TextSelect { frame_id },
                    tolerance,
                });
                return self.recompose();
            }
            self.editing = None;
        }
        let document = &self.zine.document;
        if clicks >= 2 {
            if let Some(frame) = frame_at(&document.pages[page], x, y).filter(|frame| frame.kind == FrameKind::Text) {
                let frame_id = frame.id.clone();
                self.begin_editing(page, &frame_id)?;
                self.place_caret(page, &frame_id, x, y, false, 1)?;
                self.drag = None;
                return self.recompose();
            }
        }
        let document = &self.zine.document;
        if let Some((selected_page, selected_id)) = &self.selection {
            if *selected_page == page {
                if let Ok(frame) = document.frame(page, selected_id) {
                    if let Some(handle) = handle_at(&frame.rect(), x, y, tolerance) {
                        self.drag = Some(Drag {
                            page,
                            frame_id: selected_id.clone(),
                            start: frame.rect(),
                            mode: DragMode::Resize(handle),
                            tolerance,
                        });
                        return self.recompose();
                    }
                }
            }
        }
        match frame_at(&document.pages[page], x, y) {
            Some(frame) => {
                let rect = frame.rect();
                self.selection = Some((page, frame.id.clone()));
                self.drag = Some(Drag {
                    page,
                    frame_id: frame.id.clone(),
                    start: rect,
                    mode: DragMode::Move {
                        grab_dx: x - rect.x,
                        grab_dy: y - rect.y,
                    },
                    tolerance,
                });
            }
            None => {
                self.selection = None;
                self.drag = None;
            }
        }
        self.recompose()
    }

    fn start_creating(&mut self, page: usize, x: f64, y: f64, tool: Tool, tolerance: f64) -> Result<Update, Error> {
        self.editing = None;
        let kind = match tool {
            Tool::Text => FrameKind::Text,
            Tool::Image => FrameKind::Image,
            Tool::Rectangle => FrameKind::Rectangle,
            Tool::Rule => FrameKind::Rule,
            Tool::Select => return Ok(self.unchanged()),
        };
        let id = self.new_frame_id(kind);
        let mut frame = match kind {
            FrameKind::Image => Frame::image(&id, x, y, 0.0, 0.0, None),
            FrameKind::Rectangle => Frame::rectangle(&id, x, y, 0.0, 0.0, 0.25, 0.0),
            FrameKind::Rule => Frame::rule(&id, x, y, 0.0, 0.0, 0.25, 0.0),
            _ => Frame::text(&id, x, y, 0.0, 0.0, "", 0),
        };
        if kind == FrameKind::Text {
            // The story is attached when the frame is committed.
            frame.story = None;
            frame.thread_index = None;
        }
        // The provisional frame lives in the document only while dragging;
        // releasing turns it into one command.
        self.zine.document.pages[page].frames.push(frame);
        self.selection = Some((page, id.clone()));
        self.drag = Some(Drag {
            page,
            frame_id: id,
            start: Rect::new(x, y, 0.0, 0.0),
            mode: DragMode::Create {
                kind,
                origin_x: x,
                origin_y: y,
            },
            tolerance,
        });
        self.recompose()
    }

    fn new_frame_id(&self, kind: FrameKind) -> String {
        let prefix = match kind {
            FrameKind::Text => "text",
            FrameKind::Image => "image",
            FrameKind::Rectangle => "box",
            FrameKind::Rule => "rule",
            FrameKind::PageNumber => "number",
        };
        let used: std::collections::HashSet<&str> = self
            .zine
            .document
            .pages
            .iter()
            .flat_map(|page| page.frames.iter().map(|frame| frame.id.as_str()))
            .collect();
        (1..)
            .map(|n| format!("{prefix}-{n}"))
            .find(|id| !used.contains(id.as_str()))
            .unwrap_or_default()
    }

    fn new_story_id(&self) -> String {
        (1..)
            .map(|n| format!("story-{n}"))
            .find(|id| self.zine.document.stories.iter().all(|story| story.id != *id))
            .unwrap_or_default()
    }

    /// The paragraph style new text starts in: "body" if there is one.
    fn default_paragraph_style(&self) -> String {
        let styles = &self.zine.document.paragraph_styles;
        styles
            .iter()
            .find(|style| style.id == "body")
            .or(styles.first())
            .map(|style| style.id.clone())
            .unwrap_or_else(|| "body".to_owned())
    }

    /// Pointer moved while pressed.
    pub fn pointer_move(&mut self, x: f64, y: f64) -> Result<Update, Error> {
        let Some(drag) = self.drag.clone() else {
            return Ok(self.unchanged());
        };
        let lines = targets(&self.zine.document, drag.page, &drag.frame_id);
        let (rect, snapped_x, snapped_y) = match &drag.mode {
            DragMode::TextSelect { frame_id } => {
                let Some(story) = self.editing.as_ref().map(|edit| edit.story.clone()) else {
                    return Ok(self.unchanged());
                };
                let frame = self
                    .story_frame_at(&story, drag.page, x, y)
                    .unwrap_or_else(|| frame_id.clone());
                let lines = self.composition.story_lines.get(&story).map(Vec::as_slice).unwrap_or(&[]);
                if let Some(position) = position_at(lines, drag.page, &frame, x, y) {
                    if let Some(edit) = &mut self.editing {
                        edit.focus = position;
                        edit.goal_x = None;
                    }
                }
                return Ok(self.unchanged());
            }
            DragMode::Move { grab_dx, grab_dy } => {
                let moved = Rect::new(x - grab_dx, y - grab_dy, drag.start.w, drag.start.h);
                snap_move(&moved, &lines, drag.tolerance)
            }
            DragMode::Resize(handle) => {
                let (hx, hy) = handle.position(&drag.start);
                let resized = handle.resize(&drag.start, x - hx, y - hy, MINIMUM_FRAME_SIDE);
                snap_resize(&resized, *handle, &lines, drag.tolerance, MINIMUM_FRAME_SIDE)
            }
            DragMode::Create { kind, origin_x, origin_y } => {
                (created_rect(*kind, *origin_x, *origin_y, x, y), None, None)
            }
        };
        self.zine
            .document
            .frame_mut(drag.page, &drag.frame_id)?
            .set_rect(rect.rounded());
        let mut update = self.recompose()?;
        update.snap = Some(SnapFeedback {
            page: drag.page,
            x: snapped_x,
            y: snapped_y,
        });
        Ok(update)
    }

    /// Pointer released: the whole drag becomes one command.
    pub fn pointer_up(&mut self) -> Result<Update, Error> {
        let Some(drag) = self.drag.take() else {
            return Ok(self.unchanged());
        };
        match drag.mode {
            DragMode::TextSelect { .. } => Ok(self.unchanged()),
            DragMode::Create { kind, origin_x, origin_y } => self.finish_creating(drag.page, &drag.frame_id, kind, origin_x, origin_y),
            DragMode::Move { .. } | DragMode::Resize(_) => {
                let after = self.zine.document.frame(drag.page, &drag.frame_id)?.rect();
                // Put the start geometry back so the command's apply is the only change.
                self.zine
                    .document
                    .frame_mut(drag.page, &drag.frame_id)?
                    .set_rect(drag.start);
                self.apply(Command::SetFrameRect {
                    page: drag.page,
                    frame: drag.frame_id,
                    before: drag.start,
                    after,
                })
            }
        }
    }

    fn finish_creating(&mut self, page: usize, frame_id: &str, kind: FrameKind, origin_x: f64, origin_y: f64) -> Result<Update, Error> {
        let frames = &mut self.zine.document.pages[page].frames;
        let Some(index) = frames.iter().position(|frame| frame.id == frame_id) else {
            return Ok(self.unchanged());
        };
        let mut frame = frames.remove(index);
        let rect = frame.rect();
        let too_small = match kind {
            FrameKind::Rule => rect.w.max(rect.h) < MINIMUM_FRAME_SIDE,
            _ => rect.w < MINIMUM_FRAME_SIDE || rect.h < MINIMUM_FRAME_SIDE,
        };
        if too_small {
            // A click without a drag makes a frame of a useful default size.
            let (w, h) = match kind {
                FrameKind::Text => (60.0, 40.0),
                FrameKind::Image => (60.0, 45.0),
                FrameKind::Rectangle => (40.0, 30.0),
                _ => (60.0, 0.0),
            };
            frame.set_rect(Rect::new(origin_x, origin_y, w, h).rounded());
        }
        let mut commands = Vec::new();
        if kind == FrameKind::Text {
            let story_id = self.new_story_id();
            frame.story = Some(story_id.clone());
            frame.thread_index = Some(0);
            commands.push(Command::SetStory {
                id: story_id.clone(),
                before: None,
                after: Some(Story {
                    id: story_id,
                    paragraphs: vec![Paragraph::plain(&self.default_paragraph_style(), "")],
                }),
            });
        }
        commands.push(Command::InsertFrame {
            page,
            index,
            frame: frame.clone(),
        });
        self.selection = Some((page, frame.id.clone()));
        self.history.apply(&mut self.zine.document, Command::Batch(commands))?;
        self.dirty = true;
        if kind == FrameKind::Text {
            self.begin_editing(page, &frame.id)?;
        }
        self.recompose()
    }

    pub fn cancel_drag(&mut self) -> Result<Update, Error> {
        if let Some(drag) = self.drag.take() {
            match drag.mode {
                DragMode::Create { .. } => {
                    self.zine.document.pages[drag.page].frames.retain(|frame| frame.id != drag.frame_id);
                    self.selection = None;
                }
                DragMode::TextSelect { .. } => {}
                _ => {
                    self.zine
                        .document
                        .frame_mut(drag.page, &drag.frame_id)?
                        .set_rect(drag.start);
                }
            }
            return self.recompose();
        }
        Ok(self.unchanged())
    }

    /// Geometry typed into the inspector.
    pub fn set_frame_rect(&mut self, page: usize, frame_id: &str, rect: Rect) -> Result<Update, Error> {
        let before = self.zine.document.frame(page, frame_id)?.rect();
        let mut after = rect.rounded();
        let is_rule = self.zine.document.frame(page, frame_id)?.kind == FrameKind::Rule;
        if !is_rule {
            after.w = after.w.max(MINIMUM_FRAME_SIDE);
            after.h = after.h.max(MINIMUM_FRAME_SIDE);
        }
        self.apply(Command::SetFrameRect {
            page,
            frame: frame_id.to_owned(),
            before,
            after,
        })
    }

    /// Moves the selected frame by a step (arrow keys).
    pub fn nudge(&mut self, dx: f64, dy: f64) -> Result<Update, Error> {
        let Some((page, frame_id)) = self.selection.clone() else {
            return Ok(self.unchanged());
        };
        let before = self.zine.document.frame(page, &frame_id)?.rect();
        self.apply(Command::SetFrameRect {
            page,
            frame: frame_id,
            before,
            after: before.translated(dx, dy).rounded(),
        })
    }

    /// Stroke and fill of a rectangle or rule.
    pub fn set_frame_appearance(&mut self, page: usize, frame_id: &str, stroke_width: f64, grey: f64, filled: bool) -> Result<Update, Error> {
        let before = self.zine.document.frame(page, frame_id)?.clone();
        let after = Frame {
            stroke_width: stroke_width.clamp(0.0, 20.0),
            grey: grey.clamp(0.0, 1.0),
            filled,
            ..before.clone()
        };
        self.apply(Command::SetFrame { page, before, after })
    }

    /// Deletes the selected frame.
    pub fn delete_frame(&mut self) -> Result<Update, Error> {
        let Some((page, frame_id)) = self.selection.clone() else {
            return Ok(self.unchanged());
        };
        let frames = &self.zine.document.pages[page].frames;
        let Some(index) = frames.iter().position(|frame| frame.id == frame_id) else {
            return Ok(self.unchanged());
        };
        let frame = frames[index].clone();
        self.selection = None;
        self.editing = None;
        self.apply(Command::RemoveFrame { page, index, frame })
    }

    /// Moves the selected frame up or down the page's stacking order.
    pub fn arrange(&mut self, arrange: Arrange) -> Result<Update, Error> {
        let Some((page, frame_id)) = self.selection.clone() else {
            return Ok(self.unchanged());
        };
        let count = self.zine.document.pages[page].frames.len();
        let Some(from) = self.zine.document.pages[page].frames.iter().position(|frame| frame.id == frame_id) else {
            return Ok(self.unchanged());
        };
        let to = match arrange {
            Arrange::Forward => (from + 1).min(count - 1),
            Arrange::Backward => from.saturating_sub(1),
            Arrange::Front => count - 1,
            Arrange::Back => 0,
        };
        self.apply(Command::MoveFrameInStack { page, from, to })
    }

    /// Places an image file in an image frame, filling the frame.
    pub fn place_image(&mut self, page: usize, frame_id: &str, path: &Path) -> Result<Update, Error> {
        let stored = self.zine.stored_path(path);
        self.images.forget(&stored);
        self.images.load(&stored)?;
        let frame = self.zine.document.frame(page, frame_id)?;
        self.apply(Command::SetImage {
            page,
            frame: frame_id.to_owned(),
            before: image_settings(frame),
            after: ImageSettings {
                image: Some(stored),
                fit: ImageFit::Fill,
                crop: None,
            },
        })
    }

    pub fn set_image_fit(&mut self, page: usize, frame_id: &str, fit: ImageFit) -> Result<Update, Error> {
        let frame = self.zine.document.frame(page, frame_id)?.clone();
        let before = image_settings(&frame);
        let crop = match (fit, &frame.image) {
            (ImageFit::Manual, Some(stored)) => {
                let image = self.images.load(stored)?;
                Some(crop_for_fit(&frame, &image, frame.fit))
            }
            _ => None,
        };
        self.apply(Command::SetImage {
            page,
            frame: frame_id.to_owned(),
            before,
            after: ImageSettings {
                image: frame.image.clone(),
                fit,
                crop,
            },
        })
    }

    /// Manual crop: shifts the image inside its frame and scales it about the
    /// frame centre by `factor` (1.0 keeps the size).
    pub fn adjust_crop(&mut self, page: usize, frame_id: &str, dx: f64, dy: f64, factor: f64) -> Result<Update, Error> {
        let frame = self.zine.document.frame(page, frame_id)?.clone();
        let Some(stored) = &frame.image else {
            return Ok(self.unchanged());
        };
        let image = self.images.load(stored)?;
        let current = crop_for_fit(&frame, &image, frame.fit);
        let factor = if factor > 0.0 { factor } else { 1.0 };
        // Scale about the frame centre.
        let (cx, cy) = (frame.w / 2.0, frame.h / 2.0);
        let crop = crate::model::ImageCrop {
            offset_x: cx + (current.offset_x - cx) * factor + dx,
            offset_y: cy + (current.offset_y - cy) * factor + dy,
            scale: current.scale * factor,
        };
        self.apply(Command::SetImage {
            page,
            frame: frame_id.to_owned(),
            before: image_settings(&frame),
            after: ImageSettings {
                image: frame.image.clone(),
                fit: ImageFit::Manual,
                crop: Some(crop),
            },
        })
    }

    // Threading --------------------------------------------------------------

    /// Continues the selected text frame's story into the text frame under a
    /// point. The target must not already be part of a longer thread; any
    /// text it held is added to the end of the story.
    pub fn thread_to(&mut self, page: usize, x: f64, y: f64) -> Result<Update, Error> {
        let Some((source_page, source_id)) = self.selection.clone() else {
            return Err(Error::Invalid("select a text frame first".to_owned()));
        };
        let document = &self.zine.document;
        let source = document.frame(source_page, &source_id)?.clone();
        let Some(story_id) = source.story.clone() else {
            return Err(Error::Invalid("the selected frame is not a text frame".to_owned()));
        };
        let target = document
            .pages
            .get(page)
            .and_then(|page_model| frame_at(page_model, x, y))
            .filter(|frame| frame.kind == FrameKind::Text)
            .cloned()
            .ok_or_else(|| Error::Invalid("click a text frame to continue the story into".to_owned()))?;
        if target.story.as_deref() == Some(story_id.as_str()) {
            return Err(Error::Invalid("that frame is already part of this story".to_owned()));
        }
        let target_story = target.story.clone();
        if let Some(other) = &target_story {
            let uses = self.composition.threads.get(other).map(Vec::len).unwrap_or(0);
            if uses > 1 {
                return Err(Error::Invalid(
                    "that frame is part of another thread; break that thread first".to_owned(),
                ));
            }
        }

        let source_index = source.thread_index.unwrap_or(0);
        let mut commands = Vec::new();
        // Make room after the source frame.
        for (frame_page, frame_id) in self.composition.threads.get(&story_id).cloned().unwrap_or_default() {
            let frame = document.frame(frame_page, &frame_id)?.clone();
            let index = frame.thread_index.unwrap_or(0);
            if index > source_index {
                commands.push(Command::SetFrame {
                    page: frame_page,
                    before: frame.clone(),
                    after: Frame {
                        thread_index: Some(index + 1),
                        ..frame
                    },
                });
            }
        }
        commands.push(Command::SetFrame {
            page,
            before: target.clone(),
            after: Frame {
                story: Some(story_id.clone()),
                thread_index: Some(source_index + 1),
                ..target.clone()
            },
        });
        if let Some(other) = target_story.and_then(|id| document.story(&id).ok().cloned()) {
            let has_text = other.paragraphs.iter().any(|paragraph| !paragraph.text().is_empty());
            if has_text {
                let story = document.story(&story_id)?;
                let end = story_end(story);
                commands.push(Command::SetParagraphs {
                    story: story_id.clone(),
                    first: story.paragraphs.len(),
                    before: Vec::new(),
                    after: other.paragraphs.clone(),
                    caret_before: end,
                    caret_after: end,
                });
            }
            commands.push(Command::SetStory {
                id: other.id.clone(),
                before: Some(other),
                after: None,
            });
        }
        self.editing = None;
        self.apply(Command::Batch(commands))
    }

    /// Breaks the selected frame's thread after it: the frames that followed
    /// become an empty story of their own, and the text that flowed into them
    /// overflows the selected frame.
    pub fn break_thread_after(&mut self) -> Result<Update, Error> {
        let Some((page, frame_id)) = self.selection.clone() else {
            return Ok(self.unchanged());
        };
        let document = &self.zine.document;
        let frame = document.frame(page, &frame_id)?;
        let Some(story_id) = frame.story.clone() else {
            return Ok(self.unchanged());
        };
        let thread = self.composition.threads.get(&story_id).cloned().unwrap_or_default();
        let Some(position) = thread.iter().position(|(_, id)| *id == frame_id) else {
            return Ok(self.unchanged());
        };
        let rest = &thread[position + 1..];
        if rest.is_empty() {
            return Ok(self.unchanged());
        }
        let new_story = self.new_story_id();
        let mut commands = vec![Command::SetStory {
            id: new_story.clone(),
            before: None,
            after: Some(Story {
                id: new_story.clone(),
                paragraphs: vec![Paragraph::plain(&self.default_paragraph_style(), "")],
            }),
        }];
        for (index, (frame_page, id)) in rest.iter().enumerate() {
            let before = document.frame(*frame_page, id)?.clone();
            commands.push(Command::SetFrame {
                page: *frame_page,
                after: Frame {
                    story: Some(new_story.clone()),
                    thread_index: Some(index as u32),
                    ..before.clone()
                },
                before,
            });
        }
        self.editing = None;
        self.apply(Command::Batch(commands))
    }

    // Text editing -----------------------------------------------------------

    /// Starts editing the story of a text frame, giving it a paragraph if it
    /// has none, with the caret at the start.
    fn begin_editing(&mut self, page: usize, frame_id: &str) -> Result<(), Error> {
        let frame = self.zine.document.frame(page, frame_id)?.clone();
        if frame.kind != FrameKind::Text {
            return Ok(());
        }
        self.selection = Some((page, frame_id.to_owned()));
        let story_id = match &frame.story {
            Some(story) if self.zine.document.story(story).is_ok() => story.clone(),
            _ => {
                let story_id = self.new_story_id();
                let story = Story {
                    id: story_id.clone(),
                    paragraphs: vec![Paragraph::plain(&self.default_paragraph_style(), "")],
                };
                let commands = vec![
                    Command::SetStory {
                        id: story_id.clone(),
                        before: None,
                        after: Some(story),
                    },
                    Command::SetFrame {
                        page,
                        before: frame.clone(),
                        after: Frame {
                            story: Some(story_id.clone()),
                            thread_index: Some(0),
                            ..frame.clone()
                        },
                    },
                ];
                self.history.apply(&mut self.zine.document, Command::Batch(commands))?;
                self.dirty = true;
                story_id
            }
        };
        if self.zine.document.story(&story_id)?.paragraphs.is_empty() {
            let paragraph = Paragraph::plain(&self.default_paragraph_style(), "");
            self.history.apply(
                &mut self.zine.document,
                Command::SetParagraphs {
                    story: story_id.clone(),
                    first: 0,
                    before: Vec::new(),
                    after: vec![paragraph],
                    caret_before: TextPosition::default(),
                    caret_after: TextPosition::default(),
                },
            )?;
            self.dirty = true;
            // Lay out the paragraph so the caret has a line.
            self.composition = compose_with(&mut self.fonts, &self.zine.document, &mut self.images, None)?;
        }
        self.editing = Some(TextEdit {
            story: story_id,
            anchor: story_start(),
            focus: story_start(),
            preedit: String::new(),
            goal_x: None,
        });
        Ok(())
    }

    fn place_caret(&mut self, page: usize, frame_id: &str, x: f64, y: f64, extend: bool, clicks: u32) -> Result<(), Error> {
        let Some(edit) = self.editing.as_ref() else {
            return Ok(());
        };
        let story = self.zine.document.story(&edit.story)?;
        let lines = self.composition.story_lines.get(&edit.story).map(Vec::as_slice).unwrap_or(&[]);
        // A frame past the story's end holds no lines: the caret goes to the end.
        let position = position_at(lines, page, frame_id, x, y).unwrap_or_else(|| story_end(story));
        let (anchor, focus) = match clicks {
            0 | 1 if extend => (edit.anchor, position),
            0 | 1 => (position, position),
            2 => word_at(story, position),
            _ => {
                let length = story.paragraphs.get(position.paragraph).map(|p| p.text().len()).unwrap_or(0);
                (
                    TextPosition::new(position.paragraph, 0),
                    TextPosition::new(position.paragraph, length),
                )
            }
        };
        if let Some(edit) = &mut self.editing {
            edit.anchor = anchor;
            edit.focus = focus;
            edit.goal_x = None;
            edit.preedit.clear();
        }
        Ok(())
    }

    /// Starts editing a text frame without a pointer (after creating it, or
    /// from the keyboard), with the caret at the end of its story.
    pub fn edit_frame(&mut self, page: usize, frame_id: &str) -> Result<Update, Error> {
        self.begin_editing(page, frame_id)?;
        if let Some(edit) = &mut self.editing {
            let end = story_end(self.zine.document.story(&edit.story)?);
            edit.anchor = end;
            edit.focus = end;
        }
        self.recompose()
    }

    pub fn end_editing(&mut self) -> Result<Update, Error> {
        self.editing = None;
        self.last_merge = None;
        self.recompose()
    }

    /// Applies a text replacement as one command, merging with the previous
    /// one when it continues the same kind of edit.
    fn edit_text(
        &mut self,
        make: impl FnOnce(&Story, TextPosition, TextPosition) -> Replacement,
        merge: Option<MergeKind>,
        keep_selection: bool,
    ) -> Result<Update, Error> {
        let Some(edit) = self.editing.clone() else {
            return Ok(self.unchanged());
        };
        let story = self.zine.document.story(&edit.story)?;
        let replacement = make(story, edit.anchor, edit.focus);
        let command = Command::SetParagraphs {
            story: edit.story.clone(),
            first: replacement.first,
            before: replacement.before,
            after: replacement.after,
            caret_before: edit.focus,
            caret_after: replacement.caret,
        };
        let merging = merge.is_some() && merge == self.last_merge;
        let changed = !command.is_no_op();
        self.history.apply_merging(&mut self.zine.document, command, merging)?;
        if changed {
            self.dirty = true;
        }
        self.last_merge = merge;
        if let Some(edit) = &mut self.editing {
            if !keep_selection {
                edit.anchor = replacement.caret;
                edit.focus = replacement.caret;
            }
            edit.goal_x = None;
        }
        self.recompose()
    }

    /// Typed or pasted text replaces the selection. A line break starts a new paragraph.
    pub fn insert_text(&mut self, text: &str) -> Result<Update, Error> {
        if let Some(edit) = &mut self.editing {
            edit.preedit.clear();
        }
        let simple = !text.contains(['\n', '\r']) && self.editing.as_ref().is_some_and(|edit| edit.anchor == edit.focus);
        let owned = text.to_owned();
        self.edit_text(
            move |story, anchor, focus| text::replace(story, anchor, focus, &owned),
            simple.then_some(MergeKind::Typing),
            false,
        )
    }

    /// Deletes the selection, or one character (or word) before or after the caret.
    pub fn delete(&mut self, backward: bool, word: bool) -> Result<Update, Error> {
        let Some(edit) = self.editing.clone() else {
            return Ok(self.unchanged());
        };
        let selection = edit.anchor != edit.focus;
        self.edit_text(
            move |story, anchor, focus| {
                if anchor != focus {
                    return text::replace(story, anchor, focus, "");
                }
                let other = match (backward, word) {
                    (true, false) => previous_position(story, focus),
                    (true, true) => previous_word(story, focus),
                    (false, false) => next_position(story, focus),
                    (false, true) => next_word(story, focus),
                };
                text::replace(story, other, focus, "")
            },
            (!selection).then_some(MergeKind::Deleting),
            false,
        )
    }

    /// The input method's provisional text, laid out underlined at the caret.
    /// A selection is deleted when composition starts.
    pub fn set_preedit(&mut self, text: &str) -> Result<Update, Error> {
        let Some(edit) = self.editing.clone() else {
            return Ok(self.unchanged());
        };
        if edit.anchor != edit.focus && edit.preedit.is_empty() && !text.is_empty() {
            self.edit_text(|story, anchor, focus| text::replace(story, anchor, focus, ""), None, false)?;
        }
        if let Some(edit) = &mut self.editing {
            edit.preedit = text.to_owned();
        }
        self.recompose()
    }

    /// The input method committed its text.
    pub fn commit_text(&mut self, text: &str) -> Result<Update, Error> {
        if let Some(edit) = &mut self.editing {
            edit.preedit.clear();
        }
        if text.is_empty() {
            return self.recompose();
        }
        self.insert_text(text)
    }

    pub fn move_caret(&mut self, motion: Motion, extend: bool) -> Result<Update, Error> {
        self.last_merge = None;
        let Some(edit) = self.editing.clone() else {
            return Ok(self.unchanged());
        };
        let story = self.zine.document.story(&edit.story)?;
        let lines = self.composition.story_lines.get(&edit.story).map(Vec::as_slice).unwrap_or(&[]);
        let focus = edit.focus;
        let collapsed = edit.anchor == edit.focus;
        let (start, end) = if edit.anchor <= edit.focus { (edit.anchor, edit.focus) } else { (edit.focus, edit.anchor) };
        let mut goal_x = None;
        let target = match motion {
            Motion::Left if !extend && !collapsed => start,
            Motion::Right if !extend && !collapsed => end,
            Motion::Left => previous_position(story, focus),
            Motion::Right => next_position(story, focus),
            Motion::WordLeft => previous_word(story, focus),
            Motion::WordRight => next_word(story, focus),
            Motion::Up | Motion::Down => {
                let x = edit.goal_x.or_else(|| caret(lines, focus).map(|c| c.x)).unwrap_or(0.0);
                goal_x = Some(x);
                let down = motion == Motion::Down;
                vertical(lines, focus, x, down).unwrap_or(if down { story_end(story) } else { story_start() })
            }
            Motion::LineStart => line_edge(lines, focus, false).unwrap_or(TextPosition::new(focus.paragraph, 0)),
            Motion::LineEnd => line_edge(lines, focus, true).unwrap_or(clamp(story, TextPosition::new(focus.paragraph, usize::MAX))),
            Motion::StoryStart => story_start(),
            Motion::StoryEnd => story_end(story),
        };
        if let Some(edit) = &mut self.editing {
            edit.focus = target;
            if !extend {
                edit.anchor = target;
            }
            edit.goal_x = goal_x;
        }
        Ok(self.unchanged())
    }

    pub fn select_all(&mut self) -> Result<Update, Error> {
        let Some(edit) = self.editing.clone() else {
            return Ok(self.unchanged());
        };
        let story = self.zine.document.story(&edit.story)?;
        let end = story_end(story);
        if let Some(edit) = &mut self.editing {
            edit.anchor = story_start();
            edit.focus = end;
        }
        Ok(self.unchanged())
    }

    // Styles -----------------------------------------------------------------

    /// Applies a paragraph style to the paragraphs of the text selection, or
    /// to the whole story of the selected text frame when not editing.
    pub fn apply_paragraph_style(&mut self, style: &str) -> Result<Update, Error> {
        self.zine.document.paragraph_style(style)?;
        if self.editing.is_none() {
            let story = self
                .selection
                .as_ref()
                .and_then(|(page, id)| self.zine.document.frame(*page, id).ok())
                .and_then(|frame| frame.story.clone());
            let Some(story_id) = story else {
                return Ok(self.unchanged());
            };
            let story = self.zine.document.story(&story_id)?;
            let replacement = text::set_paragraph_style(story, story_start(), story_end(story), style);
            return self.apply(Command::SetParagraphs {
                story: story_id,
                first: replacement.first,
                before: replacement.before,
                after: replacement.after,
                caret_before: replacement.caret,
                caret_after: replacement.caret,
            });
        }
        let owned = style.to_owned();
        self.edit_text(move |story, anchor, focus| text::set_paragraph_style(story, anchor, focus, &owned), None, true)
    }

    /// Sets a character style (or none) on the selected text.
    pub fn apply_character_style(&mut self, style: Option<&str>) -> Result<Update, Error> {
        if let Some(id) = style {
            if self.zine.document.character_style(id).is_none() {
                return Err(Error::Invalid(format!("the character style \"{id}\" does not exist")));
            }
        }
        let owned = style.map(str::to_owned);
        self.edit_text(
            move |story, anchor, focus| text::set_character_style(story, anchor, focus, owned.as_deref()),
            None,
            true,
        )
    }

    /// Changes a paragraph style's settings.
    pub fn update_paragraph_style(&mut self, style: ParagraphStyle) -> Result<Update, Error> {
        let before = self.zine.document.paragraph_style(&style.id)?.clone();
        self.fonts.require_family(&style.latin_font)?;
        self.fonts.require_family(&style.korean_font)?;
        if style.size <= 0.0 || style.leading <= 0.0 || style.korean_scale <= 0.0 {
            return Err(Error::Invalid("size, leading and Korean scale must be above zero".to_owned()));
        }
        self.apply(Command::SetParagraphStyle {
            id: style.id.clone(),
            before: Some(before),
            after: Some(style),
        })
    }

    /// Adds a paragraph style copied from another.
    pub fn create_paragraph_style(&mut self, id: &str, based_on: &str) -> Result<Update, Error> {
        let id = id.trim();
        if id.is_empty() {
            return Err(Error::Invalid("a style needs a name".to_owned()));
        }
        if self.zine.document.paragraph_style(id).is_ok() {
            return Err(Error::Invalid(format!("a paragraph style called \"{id}\" already exists")));
        }
        let base = self.zine.document.paragraph_style(based_on)?.clone();
        self.apply(Command::SetParagraphStyle {
            id: id.to_owned(),
            before: None,
            after: Some(ParagraphStyle { id: id.to_owned(), ..base }),
        })
    }

    pub fn update_character_style(&mut self, style: CharacterStyle) -> Result<Update, Error> {
        let before = self
            .zine
            .document
            .character_style(&style.id)
            .cloned()
            .ok_or_else(|| Error::Invalid(format!("the character style \"{}\" does not exist", style.id)))?;
        if let Some(weight) = style.font_weight {
            if !(100..=900).contains(&weight) {
                return Err(Error::Invalid("weight runs from 100 to 900".to_owned()));
            }
        }
        self.apply(Command::SetCharacterStyle {
            id: style.id.clone(),
            before: Some(before),
            after: Some(style),
        })
    }

    pub fn create_character_style(&mut self, id: &str) -> Result<Update, Error> {
        let id = id.trim();
        if id.is_empty() {
            return Err(Error::Invalid("a style needs a name".to_owned()));
        }
        if self.zine.document.character_style(id).is_some() {
            return Err(Error::Invalid(format!("a character style called \"{id}\" already exists")));
        }
        self.apply(Command::SetCharacterStyle {
            id: id.to_owned(),
            before: None,
            after: Some(CharacterStyle {
                id: id.to_owned(),
                font_style: Some(crate::model::FontSlant::Italic),
                font_weight: None,
                underline: false,
            }),
        })
    }

    // Pages and document settings -------------------------------------------

    /// Adds blank pages at the end.
    pub fn add_pages(&mut self, count: usize) -> Result<Update, Error> {
        let first = self.zine.document.pages.len();
        self.view_changed = true;
        self.apply(Command::SetPages {
            first,
            before: Vec::new(),
            after: vec![Page::default(); count.clamp(1, 64)],
        })
    }

    /// Removes pages from the end, with their frames. Undo brings them back.
    pub fn remove_pages(&mut self, count: usize) -> Result<Update, Error> {
        let total = self.zine.document.pages.len();
        let count = count.min(total.saturating_sub(1));
        if count == 0 {
            return Ok(self.unchanged());
        }
        let first = total - count;
        if self.selection.as_ref().is_some_and(|(page, _)| *page >= first) {
            self.selection = None;
        }
        self.editing = None;
        self.view_changed = true;
        self.apply(Command::SetPages {
            first,
            before: self.zine.document.pages[first..].to_vec(),
            after: Vec::new(),
        })
    }

    /// Margins, columns, sheet, bleed, printer margin and creep. The fold and
    /// trim size stay as the document was created.
    pub fn set_settings(&mut self, settings: Settings) -> Result<Update, Error> {
        let before = self.zine.document.document.clone();
        let values = [
            settings.bleed,
            settings.printer_margin,
            settings.creep_per_sheet,
            settings.margins.top,
            settings.margins.bottom,
            settings.margins.inside,
            settings.margins.outside,
            settings.columns.gutter,
        ];
        if values.iter().any(|value| !value.is_finite() || *value < 0.0) {
            return Err(Error::Invalid("margins, bleed and creep cannot be negative".to_owned()));
        }
        let after = Settings {
            fold: before.fold,
            trim: before.trim,
            page_count: before.page_count,
            columns: crate::model::Columns {
                count: settings.columns.count.clamp(1, 12),
                gutter: settings.columns.gutter,
            },
            ..settings
        };
        let mut check = self.zine.document.clone();
        check.document = after.clone();
        sheet_mode(&check, after.sheet)?;
        self.view_changed = true;
        self.apply(Command::SetSettings { before, after })
    }

    // Commands, undo, saving, export ----------------------------------------

    fn apply(&mut self, command: Command) -> Result<Update, Error> {
        if !command.is_no_op() {
            if affects_view(&command) {
                self.view_changed = true;
            }
            self.history.apply(&mut self.zine.document, command)?;
            self.dirty = true;
        }
        self.last_merge = None;
        self.repair_state();
        self.recompose()
    }

    /// After a change, keep the selection and the caret pointing at things
    /// that still exist.
    fn repair_state(&mut self) {
        let document = &self.zine.document;
        if let Some((page, id)) = &self.selection {
            if document.frame(*page, id).is_err() {
                self.selection = None;
            }
        }
        if let Some(edit) = &mut self.editing {
            match document.story(&edit.story) {
                Ok(story) if !story.paragraphs.is_empty() => {
                    edit.anchor = clamp(story, edit.anchor);
                    edit.focus = clamp(story, edit.focus);
                }
                _ => self.editing = None,
            }
        }
    }

    fn after_history(&mut self, applied: Option<Command>) -> Result<Update, Error> {
        if let Some(command) = &applied {
            self.dirty = true;
            if affects_view(command) {
                self.view_changed = true;
            }
            if let (Some(edit), Some((story, caret))) = (&mut self.editing, command.caret_after()) {
                if edit.story == story {
                    edit.anchor = caret;
                    edit.focus = caret;
                    edit.preedit.clear();
                }
            }
        }
        self.last_merge = None;
        self.repair_state();
        self.recompose()
    }

    pub fn undo(&mut self) -> Result<Update, Error> {
        let applied = self.history.undo(&mut self.zine.document)?;
        self.after_history(applied)
    }

    pub fn redo(&mut self) -> Result<Update, Error> {
        let applied = self.history.redo(&mut self.zine.document)?;
        self.after_history(applied)
    }

    pub fn save(&mut self) -> Result<Update, Error> {
        self.zine.save()?;
        self.dirty = false;
        Ok(self.unchanged())
    }

    /// Writes a reader-spread PDF and returns the overflows found on the way.
    pub fn export_reader_pdf(&mut self, path: &Path, dpi: Option<f64>) -> Result<Vec<Overflow>, Error> {
        let (pdf, composition) = reader_pdf(&mut self.fonts, &mut self.images, &self.zine.document, dpi)?;
        write_pdf(path, &pdf)?;
        Ok(composition.overflows)
    }

    /// Writes one PDF page per document page, at the trim size.
    pub fn export_paged_pdf(&mut self, path: &Path, dpi: Option<f64>) -> Result<Vec<Overflow>, Error> {
        let (pdf, composition) = crate::export::paged_pdf(&mut self.fonts, &mut self.images, &self.zine.document, dpi)?;
        write_pdf(path, &pdf)?;
        Ok(composition.overflows)
    }

    /// Pixel size, stated and effective resolution, and a thumbnail of the
    /// image in an image frame.
    pub fn image_info(&mut self, page: usize, frame_id: &str) -> Result<crate::images::ImageInfo, Error> {
        use base64::Engine;
        let frame = self.zine.document.frame(page, frame_id)?.clone();
        let stored = frame
            .image
            .clone()
            .ok_or_else(|| Error::Invalid("this frame has no image".to_owned()))?;
        let image = self.images.load(&stored)?;
        let path = self.images.resolve(&stored);
        let thumbnail = crate::images::thumbnail_png(&image, 480)?;
        Ok(crate::images::ImageInfo {
            path: path.to_string_lossy().into_owned(),
            pixel_width: image.width,
            pixel_height: image.height,
            stated_dpi: crate::images::stated_dpi(&path),
            effective_dpi: crate::images::effective_dpi(&frame, &image),
            thumbnail: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(thumbnail)
            ),
        })
    }

    /// Writes one page (counted from zero) as a one-page PDF.
    pub fn export_page_pdf(&mut self, path: &Path, page: usize, dpi: Option<f64>) -> Result<Vec<Overflow>, Error> {
        let (pdf, composition) = crate::export::page_pdf(&mut self.fonts, &mut self.images, &self.zine.document, page, dpi)?;
        write_pdf(path, &pdf)?;
        Ok(composition.overflows)
    }

    /// Writes the imposed PDF for printing, on the document's sheet or on `sheet`.
    pub fn export_imposed_pdf(&mut self, path: &Path, sheet: Option<Size>, dpi: Option<f64>) -> Result<Vec<Overflow>, Error> {
        let (pdf, composition) = imposed_pdf(&mut self.fonts, &mut self.images, &self.zine.document, sheet, dpi)?;
        write_pdf(path, &pdf)?;
        Ok(composition.overflows)
    }

    /// The default export path inside the `.zine` folder.
    pub fn default_export_path(&self, suffix: &str) -> PathBuf {
        self.zine
            .folder
            .join("exports")
            .join(format!("{}-{suffix}.pdf", self.zine.title()))
    }

    /// Whole-page SVG, for thumbnails or debugging.
    pub fn page_svg(&self, page: usize) -> Option<String> {
        self.composition.scene.pages.get(page).map(page_to_svg)
    }

    /// Paragraph breaks reused from the cache and computed, since opening.
    pub fn fonts_cache_statistics(&self) -> (usize, usize) {
        self.fonts.cache.statistics()
    }

    pub fn is_editing_text(&self) -> bool {
        self.editing.is_some()
    }

    pub fn document(&self) -> &Document {
        &self.zine.document
    }
}

fn affects_view(command: &Command) -> bool {
    match command {
        Command::SetPages { .. } | Command::SetSettings { .. } => true,
        Command::Batch(commands) => commands.iter().any(affects_view),
        _ => false,
    }
}

/// The rectangle of a frame being drawn from an origin to the pointer. Rules
/// are horizontal or vertical, whichever the drag is closer to.
fn created_rect(kind: FrameKind, origin_x: f64, origin_y: f64, x: f64, y: f64) -> Rect {
    if kind == FrameKind::Rule {
        if (x - origin_x).abs() >= (y - origin_y).abs() {
            Rect::new(origin_x.min(x), origin_y, (x - origin_x).abs(), 0.0)
        } else {
            Rect::new(origin_x, origin_y.min(y), 0.0, (y - origin_y).abs())
        }
    } else {
        Rect::new(origin_x.min(x), origin_y.min(y), (x - origin_x).abs(), (y - origin_y).abs())
    }
}

/// A hash of everything drawn in a frame, to skip rendering unchanged frames.
fn fingerprint(frame: &FrameScene, rect: &Rect, kind: FrameKind) -> u64 {
    let mut state = DefaultHasher::new();
    for value in [rect.x, rect.y, rect.w, rect.h] {
        value.to_bits().hash(&mut state);
    }
    (kind as u8).hash(&mut state);
    hash_items(&frame.items, &mut state);
    state.finish()
}

fn hash_items(items: &[Item], state: &mut DefaultHasher) {
    for item in items {
        match item {
            Item::Text(text) => {
                0u8.hash(state);
                text.font.id.hash(state);
                text.size.to_bits().hash(state);
                text.color.hash(state);
                for glyph in &text.glyphs {
                    glyph.id.hash(state);
                    glyph.x.to_bits().hash(state);
                    glyph.y.to_bits().hash(state);
                }
            }
            Item::Rect(rect) => {
                1u8.hash(state);
                for value in [rect.rect.x, rect.rect.y, rect.rect.w, rect.rect.h] {
                    value.to_bits().hash(state);
                }
                rect.fill.hash(state);
                rect.stroke.map(|stroke| (stroke.width.to_bits(), stroke.color)).hash(state);
            }
            Item::Rule(rule) => {
                2u8.hash(state);
                for value in [rule.x1, rule.y1, rule.x2, rule.y2, rule.stroke.width] {
                    value.to_bits().hash(state);
                }
                rule.stroke.color.hash(state);
            }
            Item::Image(image) => {
                3u8.hash(state);
                (std::sync::Arc::as_ptr(&image.image) as usize).hash(state);
                for value in [
                    image.placement.x,
                    image.placement.y,
                    image.placement.w,
                    image.placement.h,
                    image.clip.x,
                    image.clip.y,
                    image.clip.w,
                    image.clip.h,
                ] {
                    value.to_bits().hash(state);
                }
            }
            Item::Group(group) => {
                4u8.hash(state);
                hash_items(&group.items, state);
            }
        }
    }
}

fn image_settings(frame: &Frame) -> ImageSettings {
    ImageSettings {
        image: frame.image.clone(),
        fit: frame.fit,
        crop: frame.crop,
    }
}
