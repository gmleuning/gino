//! Turns a document into a scene: lays out every story through its thread of
//! frames and places the results, rectangles and rules on pages in millimetres.

use std::collections::HashMap;

use crate::error::Error;
use crate::fonts::Fonts;
use crate::layout::{layout_story, FrameBox, FrameLayout};
use crate::model::{Document, Frame, FrameKind};
use crate::scene::{Glyph, Item, Page, RectItem, Rgb, RuleItem, Scene, Stroke, TextItem};
use crate::units::pt_to_mm;

/// A story whose text did not fit in its frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overflow {
    pub story: String,
    pub characters: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Composition {
    pub scene: Scene,
    pub overflows: Vec<Overflow>,
}

pub fn compose(fonts: &mut Fonts, document: &Document) -> Result<Composition, Error> {
    for style in &document.paragraph_styles {
        fonts.require_family(&style.latin_font)?;
        fonts.require_family(&style.korean_font)?;
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
    let mut story_ids: Vec<&str> = threads.keys().copied().collect();
    story_ids.sort();
    for story_id in story_ids {
        let mut frames = threads.remove(story_id).unwrap_or_default();
        frames.sort_by_key(|(index, _)| *index);
        let story = document.story(story_id)?;
        let boxes: Vec<FrameBox> = frames
            .iter()
            .map(|(_, frame)| FrameBox {
                width: frame.w,
                height: frame.h,
            })
            .collect();
        let story_layout = layout_story(fonts, document, story, &boxes)?;
        if story_layout.overflow_characters > 0 {
            overflows.push(Overflow {
                story: story_id.to_owned(),
                characters: story_layout.overflow_characters,
            });
        }
        for ((_, frame), layout) in frames.iter().zip(story_layout.frames) {
            layouts.insert(frame.id.clone(), layout);
        }
    }

    let mut scene = Scene::default();
    for page in &document.pages {
        let mut items = Vec::new();
        for frame in &page.frames {
            match frame.kind {
                FrameKind::Text => {
                    if let Some(layout) = layouts.get(&frame.id) {
                        push_text(&mut items, frame, layout);
                    }
                }
                FrameKind::Rectangle => items.push(Item::Rect(RectItem {
                    x: frame.x,
                    y: frame.y,
                    w: frame.w,
                    h: frame.h,
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
                // Images and page numbers come with later milestones.
                FrameKind::Image | FrameKind::PageNumber => {}
            }
        }
        scene.pages.push(Page {
            width: document.document.trim.width,
            height: document.document.trim.height,
            items,
        });
    }

    Ok(Composition { scene, overflows })
}

fn push_text(items: &mut Vec<Item>, frame: &Frame, layout: &FrameLayout) {
    for line in &layout.lines {
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
                        x: frame.x + pt_to_mm(glyph.x),
                        y: frame.y + pt_to_mm(glyph.y),
                        advance: pt_to_mm(glyph.advance),
                        text_range: glyph.text_range.clone(),
                    })
                    .collect(),
            }));
        }
    }
}
