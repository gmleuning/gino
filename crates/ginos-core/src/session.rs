//! An open document being edited. The interface sends intents (pointer events
//! in page millimetres, inspector values, menu commands); the session turns
//! them into commands, re-composes, and answers with patches: the SVG of every
//! frame whose drawing changed, plus selection geometry and inspector data.
//! The session is the only holder of document state.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::compose::{compose, Composition, ImageProblem, Overflow};
use crate::edit::{Command, History, ImageSettings};
use crate::error::Error;
use crate::export::{reader_pdf, write_pdf};
use crate::fonts::Fonts;
use crate::guides::page_guides_svg;
use crate::hit::{frame_at, handle_at, Handle};
use crate::images::{crop_for_fit, ImageCache};
use crate::layout::{break_lines_with, place_lines};
use crate::model::{Frame, FrameKind, ImageFit, ParagraphStyle, Rect};
use crate::render_svg::{frame_to_svg, page_to_svg};
use crate::scene::FrameScene;
use crate::snap::{snap_move, snap_resize, targets};
use crate::spreads::{reader_spreads, spread_offsets};
use crate::units::{mm_to_pt, pt_to_mm};
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
    pub thread_index: Option<u32>,
}

/// Snap lines that were used by the last pointer move, for the canvas to show.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct SnapFeedback {
    pub page: usize,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

/// What changed after an intent.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Update {
    pub patches: Vec<FramePatch>,
    pub selection: Option<SelectionInfo>,
    pub snap: Option<SnapFeedback>,
    pub overflows: Vec<Overflow>,
    pub image_problems: Vec<ImageProblem>,
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
    pub pages: Vec<PageView>,
    pub spreads: Vec<SpreadView>,
    pub update: Update,
}

#[derive(Debug, Clone, Copy)]
enum DragMode {
    Move { grab_dx: f64, grab_dy: f64 },
    Resize(Handle),
}

#[derive(Debug, Clone)]
struct Drag {
    page: usize,
    frame_id: String,
    start: Rect,
    mode: DragMode,
    tolerance: f64,
}

/// A provisional line of text set by an input method, for the milestone 2 test frame.
#[derive(Debug, Clone, Serialize)]
pub struct ImeTestView {
    pub svg: String,
    pub caret_x: f64,
    pub caret_top: f64,
    pub caret_bottom: f64,
}

pub struct Session {
    pub zine: Zine,
    pub fonts: Fonts,
    images: ImageCache,
    history: History,
    selection: Option<(usize, String)>,
    drag: Option<Drag>,
    frame_patches: HashMap<(usize, String), FramePatch>,
    composition: Composition,
    dirty: bool,
}

impl Session {
    /// Opens a `.zine` folder and loads the document's fonts.
    pub fn open(path: &Path, mut fonts: Fonts) -> Result<Self, Error> {
        let zine = Zine::open(path)?;
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
            frame_patches: HashMap::new(),
            composition: Composition::default(),
            dirty: false,
        };
        session.recompose()?;
        Ok(session)
    }

    /// The full view, with every frame's SVG as a patch.
    pub fn view(&mut self) -> Result<DocumentView, Error> {
        self.frame_patches.clear();
        let update = self.recompose()?;
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
            pages,
            spreads,
            update,
        })
    }

    /// Re-lays out the document and returns the frames whose SVG changed.
    fn recompose(&mut self) -> Result<Update, Error> {
        self.composition = compose(&mut self.fonts, &self.zine.document, &mut self.images)?;
        let mut patches = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (page_index, page) in self.composition.scene.pages.iter().enumerate() {
            for frame in &page.frames {
                let key = (page_index, frame.id.clone());
                seen.insert(key.clone());
                let model = self.zine.document.frame(page_index, &frame.id)?;
                let patch = FramePatch {
                    page: page_index,
                    frame_id: frame.id.clone(),
                    kind: model.kind,
                    rect: model.rect().rounded(),
                    svg: frame_to_svg(frame),
                };
                if self.frame_patches.get(&key) != Some(&patch) {
                    patches.push(patch.clone());
                    self.frame_patches.insert(key, patch);
                }
            }
        }
        self.frame_patches.retain(|key, _| seen.contains(key));
        Ok(self.update_with(patches, None))
    }

    fn update_with(&self, patches: Vec<FramePatch>, snap: Option<SnapFeedback>) -> Update {
        Update {
            patches,
            selection: self.selection_info(),
            snap,
            overflows: self.composition.overflows.clone(),
            image_problems: self.composition.image_problems.clone(),
            dirty: self.dirty,
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
        }
    }

    fn selection_info(&self) -> Option<SelectionInfo> {
        let (page, frame_id) = self.selection.as_ref()?;
        let frame = self.zine.document.frame(*page, frame_id).ok()?;
        Some(SelectionInfo {
            page: *page,
            frame_id: frame.id.clone(),
            kind: frame.kind,
            rect: frame.rect().rounded(),
            image: frame.image.clone(),
            fit: frame.fit,
            story: frame.story.clone(),
            thread_index: frame.thread_index,
        })
    }

    pub fn selection(&self) -> Option<(usize, &str)> {
        self.selection.as_ref().map(|(page, id)| (*page, id.as_str()))
    }

    pub fn select(&mut self, page: usize, frame_id: Option<&str>) -> Result<Update, Error> {
        self.selection = match frame_id {
            Some(id) => {
                self.zine.document.frame(page, id)?;
                Some((page, id.to_owned()))
            }
            None => None,
        };
        Ok(self.update_with(Vec::new(), None))
    }

    /// Pointer pressed on a page. Starts a resize on a handle of the selected
    /// frame, otherwise selects the frame under the pointer and starts a move.
    pub fn pointer_down(&mut self, page: usize, x: f64, y: f64, tolerance: f64) -> Result<Update, Error> {
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
                        return Ok(self.update_with(Vec::new(), None));
                    }
                }
            }
        }
        let Some(page_model) = document.pages.get(page) else {
            return Ok(self.update_with(Vec::new(), None));
        };
        match frame_at(page_model, x, y) {
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
        Ok(self.update_with(Vec::new(), None))
    }

    /// Pointer moved while pressed: applies the provisional geometry with
    /// snapping and returns the changed frames.
    pub fn pointer_move(&mut self, x: f64, y: f64) -> Result<Update, Error> {
        let Some(drag) = self.drag.clone() else {
            return Ok(self.update_with(Vec::new(), None));
        };
        let lines = targets(&self.zine.document, drag.page, &drag.frame_id);
        let (rect, snapped_x, snapped_y) = match drag.mode {
            DragMode::Move { grab_dx, grab_dy } => {
                let moved = Rect::new(x - grab_dx, y - grab_dy, drag.start.w, drag.start.h);
                snap_move(&moved, &lines, drag.tolerance)
            }
            DragMode::Resize(handle) => {
                let (hx, hy) = handle.position(&drag.start);
                let resized = handle.resize(&drag.start, x - hx, y - hy, MINIMUM_FRAME_SIDE);
                snap_resize(&resized, handle, &lines, drag.tolerance, MINIMUM_FRAME_SIDE)
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
            return Ok(self.update_with(Vec::new(), None));
        };
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

    pub fn cancel_drag(&mut self) -> Result<Update, Error> {
        if let Some(drag) = self.drag.take() {
            self.zine
                .document
                .frame_mut(drag.page, &drag.frame_id)?
                .set_rect(drag.start);
            return self.recompose();
        }
        Ok(self.update_with(Vec::new(), None))
    }

    /// Geometry typed into the inspector.
    pub fn set_frame_rect(&mut self, page: usize, frame_id: &str, rect: Rect) -> Result<Update, Error> {
        let before = self.zine.document.frame(page, frame_id)?.rect();
        let mut after = rect.rounded();
        after.w = after.w.max(MINIMUM_FRAME_SIDE);
        after.h = after.h.max(MINIMUM_FRAME_SIDE);
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
            return Ok(self.update_with(Vec::new(), None));
        };
        let before = self.zine.document.frame(page, &frame_id)?.rect();
        self.apply(Command::SetFrameRect {
            page,
            frame: frame_id,
            before,
            after: before.translated(dx, dy).rounded(),
        })
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
            return Ok(self.update_with(Vec::new(), None));
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

    fn apply(&mut self, command: Command) -> Result<Update, Error> {
        if !command.is_no_op() {
            self.history.apply(&mut self.zine.document, command)?;
            self.dirty = true;
        }
        self.recompose()
    }

    pub fn undo(&mut self) -> Result<Update, Error> {
        if self.history.undo(&mut self.zine.document)? {
            self.dirty = true;
        }
        self.recompose()
    }

    pub fn redo(&mut self) -> Result<Update, Error> {
        if self.history.redo(&mut self.zine.document)? {
            self.dirty = true;
        }
        self.recompose()
    }

    pub fn save(&mut self) -> Result<Update, Error> {
        self.zine.save()?;
        self.dirty = false;
        Ok(self.update_with(Vec::new(), None))
    }

    /// Writes a reader-spread PDF and returns the overflows found on the way.
    pub fn export_reader_pdf(&mut self, path: &Path) -> Result<Vec<Overflow>, Error> {
        let (pdf, composition) = reader_pdf(&mut self.fonts, &mut self.images, &self.zine.document)?;
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

    /// Lays out the milestone 2 Korean input test: committed text followed by
    /// the text being composed, underlined, in a frame of the given width.
    /// Returns the SVG fragment and the caret after the last character.
    pub fn ime_test(&mut self, committed: &str, composing: &str, width: f64) -> Result<ImeTestView, Error> {
        let style = self
            .zine
            .document
            .paragraph_styles
            .first()
            .cloned()
            .unwrap_or_else(fallback_style);
        let text = format!("{committed}{composing}");
        let underline = (!composing.is_empty()).then(|| committed.len()..text.len());
        let layout = break_lines_with(&mut self.fonts, &text, &style, mm_to_pt(width), 0.0, underline);
        let lines = place_lines(&layout, 0.0, &text);
        let mut items = Vec::new();
        crate::compose::push_lines(&mut items, 0.0, 0.0, &lines);
        let svg = frame_to_svg(&FrameScene {
            id: "ime-test".to_owned(),
            items,
        });
        let (caret_x, caret_top, caret_bottom) = match lines.last() {
            Some(line) => {
                let end = line
                    .runs
                    .iter()
                    .flat_map(|run| run.glyphs.iter())
                    .map(|glyph| glyph.x + glyph.advance)
                    .fold(0.0_f64, f64::max);
                (pt_to_mm(end), pt_to_mm(line.top), pt_to_mm(line.bottom))
            }
            None => (0.0, 0.0, pt_to_mm(style.leading)),
        };
        Ok(ImeTestView {
            svg,
            caret_x,
            caret_top,
            caret_bottom,
        })
    }
}

fn image_settings(frame: &Frame) -> ImageSettings {
    ImageSettings {
        image: frame.image.clone(),
        fit: frame.fit,
        crop: frame.crop,
    }
}

fn fallback_style() -> ParagraphStyle {
    ParagraphStyle {
        id: "fallback".to_owned(),
        latin_font: "Crimson Text".to_owned(),
        korean_font: "Gowun Dodum".to_owned(),
        korean_scale: 1.0,
        size: 12.0,
        leading: 18.0,
        tracking: 0.0,
        alignment: crate::model::Alignment::Left,
        space_before: 0.0,
        space_after: 0.0,
        first_line_indent: 0.0,
        korean_break: crate::model::KoreanBreak::Word,
    }
}
