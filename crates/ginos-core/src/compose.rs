//! Turns a document into a scene: lays out every story through its thread of
//! frames and places the results, images, rectangles and rules on pages in
//! millimetres, one [`FrameScene`] per frame.

use std::borrow::Cow;
use std::collections::HashMap;

use crate::error::Error;
use crate::fonts::Fonts;
use crate::images::{placement, ImageCache};
use crate::layout::{layout_story, FrameBox, FrameLayout, Underline};
use crate::model::{Document, Frame, FrameKind, Run, Story};
use crate::scene::{
    FrameScene, Glyph, ImageItem, Item, Page, RectItem, Rgb, RuleItem, Scene, Stroke, TextItem,
};
use crate::text::{clamp, CaretLine, TextPosition};
use crate::units::pt_to_mm;

/// A story whose text did not fit in its frames, and the frame it overflows from.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Overflow {
    pub story: String,
    pub characters: usize,
    pub page: usize,
    pub frame: String,
}

/// Text an input method is composing: laid out in place, underlined, but not
/// yet part of the document.
#[derive(Clone, Debug, PartialEq)]
pub struct Preedit {
    pub story: String,
    pub position: TextPosition,
    pub text: String,
}

/// The story with the preedit text inserted, and its underline.
fn with_preedit(story: &Story, preedit: &Preedit) -> (Story, Underline) {
    let mut story = story.clone();
    let position = clamp(&story, preedit.position);
    if story.paragraphs.is_empty() {
        // The session gives every edited story a paragraph; nothing to show otherwise.
        return (story, Underline { paragraph: 0, range: 0..0 });
    }
    let paragraph = &mut story.paragraphs[position.paragraph];
    let mut text = paragraph.text();
    text.insert_str(position.offset, &preedit.text);
    // Styles do not matter for an underlined provisional syllable; keep the
    // run boundaries by inserting into the run that holds the position.
    let mut offset = 0;
    let mut inserted = false;
    for run in &mut paragraph.runs {
        let end = offset + run.text.len();
        if !inserted && position.offset >= offset && position.offset <= end {
            run.text.insert_str(position.offset - offset, &preedit.text);
            inserted = true;
        }
        offset = end;
    }
    if !inserted {
        paragraph.runs.push(Run {
            text: preedit.text.clone(),
            character_style: None,
        });
    }
    debug_assert_eq!(paragraph.text(), text);
    (
        story,
        Underline {
            paragraph: position.paragraph,
            range: position.offset..position.offset + preedit.text.len(),
        },
    )
}

/// An image frame whose picture could not be loaded.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ImageProblem {
    pub page: usize,
    pub frame: String,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct Composition {
    pub scene: Scene,
    pub overflows: Vec<Overflow>,
    pub image_problems: Vec<ImageProblem>,
    /// Every laid-out line of every story, in thread order, for the caret.
    pub story_lines: HashMap<String, Vec<CaretLine>>,
    /// The frames of every story, in thread order, as (page, frame id).
    pub threads: HashMap<String, Vec<(usize, String)>>,
}

pub fn compose(fonts: &mut Fonts, document: &Document, images: &mut ImageCache) -> Result<Composition, Error> {
    compose_with(fonts, document, images, None)
}

/// [`compose`], with text being composed by an input method laid out in place.
pub fn compose_with(
    fonts: &mut Fonts,
    document: &Document,
    images: &mut ImageCache,
    preedit: Option<&Preedit>,
) -> Result<Composition, Error> {
    for style in &document.paragraph_styles {
        fonts.require_family(&style.latin_font)?;
        fonts.require_family(&style.korean_font)?;
    }
    // Where each frame sits, for caret lines in page coordinates.
    let mut frame_pages: HashMap<&str, usize> = HashMap::new();
    for (page_index, page) in document.pages.iter().enumerate() {
        for frame in &page.frames {
            frame_pages.insert(frame.id.as_str(), page_index);
        }
    }

    // Collect every text frame per story, in thread order.
    let mut threads: HashMap<&str, Vec<(u32, &Frame)>> = HashMap::new();
    for page in &document.pages {
        for frame in &page.frames {
            if frame.kind == FrameKind::Text {
                if let Some(story) = &frame.story {
                    threads
                        .entry(story.as_str())
                        .or_default()
                        .push((frame.thread_index.unwrap_or(0), frame));
                }
            }
        }
    }

    let mut layouts: HashMap<String, FrameLayout> = HashMap::new();
    let mut overflows = Vec::new();
    let mut story_lines = HashMap::new();
    let mut thread_frames = HashMap::new();
    let mut story_ids: Vec<&str> = threads.keys().copied().collect();
    story_ids.sort();
    for story_id in story_ids {
        let mut frames = threads.remove(story_id).unwrap_or_default();
        frames.sort_by_key(|(index, _)| *index);
        let stored = document.story(story_id)?;
        let (story, underline) = match preedit.filter(|preedit| preedit.story == story_id && !preedit.text.is_empty()) {
            Some(preedit) => {
                let (story, underline) = with_preedit(stored, preedit);
                (Cow::Owned(story), Some(underline))
            }
            None => (Cow::Borrowed(stored), None),
        };
        let boxes: Vec<FrameBox> = frames
            .iter()
            .map(|(_, frame)| FrameBox {
                width: frame.w,
                height: frame.h,
            })
            .collect();
        let story_layout = layout_story(fonts, document, &story, &boxes, underline.as_ref())?;
        if story_layout.overflow_characters > 0 {
            if let Some((_, last)) = frames.last() {
                overflows.push(Overflow {
                    story: story_id.to_owned(),
                    characters: story_layout.overflow_characters,
                    page: frame_pages.get(last.id.as_str()).copied().unwrap_or(0),
                    frame: last.id.clone(),
                });
            }
        }
        let mut lines = Vec::new();
        for ((_, frame), layout) in frames.iter().zip(&story_layout.frames) {
            let page = frame_pages.get(frame.id.as_str()).copied().unwrap_or(0);
            for line in &layout.lines {
                lines.push(CaretLine::from_placed(page, &frame.id, frame.x, frame.y, line));
            }
        }
        story_lines.insert(story_id.to_owned(), lines);
        thread_frames.insert(
            story_id.to_owned(),
            frames
                .iter()
                .map(|(_, frame)| (frame_pages.get(frame.id.as_str()).copied().unwrap_or(0), frame.id.clone()))
                .collect(),
        );
        for ((_, frame), layout) in frames.iter().zip(story_layout.frames) {
            layouts.insert(frame.id.clone(), layout);
        }
    }

    let mut scene = Scene::default();
    let mut image_problems = Vec::new();
    for (page_index, page) in document.pages.iter().enumerate() {
        let mut frames = Vec::new();
        for frame in &page.frames {
            let mut items = Vec::new();
            match frame.kind {
                FrameKind::Text => {
                    if let Some(layout) = layouts.get(&frame.id) {
                        push_text(&mut items, frame, layout);
                    }
                }
                FrameKind::Image => {
                    if let Some(stored) = &frame.image {
                        match images.load(stored) {
                            Ok(image) => items.push(Item::Image(ImageItem {
                                placement: placement(frame, &image),
                                clip: frame.rect(),
                                image,
                            })),
                            Err(error) => image_problems.push(ImageProblem {
                                page: page_index,
                                frame: frame.id.clone(),
                                message: error.to_string(),
                            }),
                        }
                    }
                }
                FrameKind::Rectangle => items.push(Item::Rect(RectItem {
                    rect: frame.rect(),
                    stroke: (frame.stroke_width > 0.0).then(|| Stroke {
                        width: frame.stroke_width,
                        color: Rgb::grey(frame.grey),
                    }),
                    fill: frame.filled.then(|| Rgb::grey(frame.grey)),
                })),
                FrameKind::Rule => items.push(Item::Rule(RuleItem {
                    x1: frame.x,
                    y1: frame.y,
                    x2: frame.x + frame.w,
                    y2: frame.y + frame.h,
                    stroke: Stroke {
                        width: frame.stroke_width,
                        color: Rgb::grey(frame.grey),
                    },
                })),
                // Page numbers come with milestone 4.
                FrameKind::PageNumber => {}
            }
            frames.push(FrameScene {
                id: frame.id.clone(),
                items,
            });
        }
        scene.pages.push(Page {
            width: document.document.trim.width,
            height: document.document.trim.height,
            frames,
        });
    }

    Ok(Composition {
        scene,
        overflows,
        image_problems,
        story_lines,
        threads: thread_frames,
    })
}

/// Converts a frame layout (points, frame-relative) into scene items
/// (millimetres, page-relative).
pub fn push_text(items: &mut Vec<Item>, frame: &Frame, layout: &FrameLayout) {
    push_lines(items, frame.x, frame.y, &layout.lines);
}

pub fn push_lines(items: &mut Vec<Item>, x: f64, y: f64, lines: &[crate::layout::PlacedLine]) {
    for line in lines {
        for run in &line.runs {
            if run.glyphs.is_empty() {
                continue;
            }
            items.push(Item::Text(TextItem {
                font: run.font.clone(),
                size: run.size,
                color: run.color,
                text: run.text.clone(),
                glyphs: run
                    .glyphs
                    .iter()
                    .map(|glyph| Glyph {
                        id: glyph.id,
                        x: x + pt_to_mm(glyph.x),
                        y: y + pt_to_mm(glyph.y),
                        advance: pt_to_mm(glyph.advance),
                        text_range: glyph.text_range.clone(),
                    })
                    .collect(),
            }));
        }
        for decoration in &line.decorations {
            items.push(Item::Rule(RuleItem {
                x1: x + pt_to_mm(decoration.x1),
                y1: y + pt_to_mm(decoration.y),
                x2: x + pt_to_mm(decoration.x2),
                y2: y + pt_to_mm(decoration.y),
                stroke: Stroke {
                    width: pt_to_mm(decoration.thickness),
                    color: decoration.color,
                },
            }));
        }
    }
}
