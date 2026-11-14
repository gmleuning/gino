//! Scene to PDF with krilla. Fonts are embedded and subset by krilla. Every
//! glyph is placed at the position the scene holds; krilla's pen advance is
//! corrected per glyph so nothing drifts.

use std::collections::HashMap;

use krilla::color::rgb;
use krilla::geom::{Path, PathBuilder, Point, Rect, Size, Transform};
use krilla::image::Image;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, Stroke};
use krilla::text::{Font, GlyphId, KrillaGlyph};
use krilla::{Data, Document};

use crate::error::Error;
use crate::scene::{Item, Rgb, Scene, Stroke as SceneStroke};
use crate::units::mm_to_pt;

pub fn scene_to_pdf(scene: &Scene) -> Result<Vec<u8>, Error> {
    let mut document = Document::new();
    let mut fonts: HashMap<u64, Font> = HashMap::new();
    let mut images: HashMap<*const u8, Image> = HashMap::new();

    for page in &scene.pages {
        let settings = PageSettings::from_wh(mm_to_pt(page.width) as f32, mm_to_pt(page.height) as f32)
            .ok_or(Error::InvalidPageSize {
                width: page.width,
                height: page.height,
            })?;
        let mut pdf_page = document.start_page_with(settings);
        let mut surface = pdf_page.surface();

        for item in page.items() {
            match item {
                Item::Text(text) => {
                    let font = match fonts.get(&text.font.id) {
                        Some(font) => font.clone(),
                        None => {
                            let font = Font::new(Data::from(text.font.data.clone()), text.font.index)
                                .ok_or_else(|| Error::Pdf("a font could not be embedded".to_owned()))?;
                            fonts.insert(text.font.id, font.clone());
                            font
                        }
                    };
                    let Some(first) = text.glyphs.first() else {
                        continue;
                    };
                    let size = text.size as f32;
                    let start_x = mm_to_pt(first.x) as f32;
                    let start_y = mm_to_pt(first.y) as f32;
                    let mut pen_x = start_x;
                    let glyphs: Vec<KrillaGlyph> = text
                        .glyphs
                        .iter()
                        .map(|glyph| {
                            let x = mm_to_pt(glyph.x) as f32;
                            let y = mm_to_pt(glyph.y) as f32;
                            let advance = mm_to_pt(glyph.advance) as f32;
                            let krilla_glyph = KrillaGlyph::new(
                                GlyphId::new(glyph.id),
                                advance / size,
                                (x - pen_x) / size,
                                (start_y - y) / size,
                                0.0,
                                glyph.text_range.clone(),
                                None,
                            );
                            pen_x += advance;
                            krilla_glyph
                        })
                        .collect();
                    surface.set_stroke(None);
                    surface.set_fill(Some(fill(text.color)));
                    surface.draw_glyphs(
                        Point::from_xy(start_x, start_y),
                        &glyphs,
                        font,
                        &text.text,
                        size,
                        false,
                    );
                }
                Item::Rect(rect) => {
                    let Some(area) = Rect::from_xywh(
                        mm_to_pt(rect.rect.x) as f32,
                        mm_to_pt(rect.rect.y) as f32,
                        mm_to_pt(rect.rect.w) as f32,
                        mm_to_pt(rect.rect.h) as f32,
                    ) else {
                        continue;
                    };
                    let mut builder = PathBuilder::new();
                    builder.push_rect(area);
                    let Some(path) = builder.finish() else {
                        continue;
                    };
                    draw_path(&mut surface, &path, rect.fill, rect.stroke);
                }
                Item::Rule(rule) => {
                    let mut builder = PathBuilder::new();
                    builder.move_to(mm_to_pt(rule.x1) as f32, mm_to_pt(rule.y1) as f32);
                    builder.line_to(mm_to_pt(rule.x2) as f32, mm_to_pt(rule.y2) as f32);
                    let Some(path) = builder.finish() else {
                        continue;
                    };
                    draw_path(&mut surface, &path, None, Some(rule.stroke));
                }
                Item::Image(image) => {
                    let key = image.image.png.as_ptr();
                    let pdf_image = match images.get(&key) {
                        Some(found) => found.clone(),
                        None => {
                            let created = Image::from_png(Data::from(image.image.png.clone()), true)
                                .map_err(|message| Error::Pdf(format!("an image could not be embedded: {message}")))?;
                            images.insert(key, created.clone());
                            created
                        }
                    };
                    let Some(clip) = Rect::from_xywh(
                        mm_to_pt(image.clip.x) as f32,
                        mm_to_pt(image.clip.y) as f32,
                        mm_to_pt(image.clip.w) as f32,
                        mm_to_pt(image.clip.h) as f32,
                    ) else {
                        continue;
                    };
                    let Some(size) = Size::from_wh(
                        mm_to_pt(image.placement.w) as f32,
                        mm_to_pt(image.placement.h) as f32,
                    ) else {
                        continue;
                    };
                    let mut builder = PathBuilder::new();
                    builder.push_rect(clip);
                    let Some(clip_path) = builder.finish() else {
                        continue;
                    };
                    surface.push_clip_path(&clip_path, &FillRule::NonZero);
                    surface.push_transform(&Transform::from_translate(
                        mm_to_pt(image.placement.x) as f32,
                        mm_to_pt(image.placement.y) as f32,
                    ));
                    surface.draw_image(pdf_image, size);
                    surface.pop();
                    surface.pop();
                }
            }
        }

        surface.finish();
        pdf_page.finish();
    }

    document
        .finish()
        .map_err(|error| Error::Pdf(format!("{error:?}")))
}

fn draw_path(
    surface: &mut krilla::surface::Surface<'_>,
    path: &Path,
    fill_color: Option<Rgb>,
    stroke: Option<SceneStroke>,
) {
    surface.set_fill(fill_color.map(fill));
    surface.set_stroke(stroke.map(|stroke| Stroke {
        paint: paint(stroke.color),
        width: mm_to_pt(stroke.width) as f32,
        ..Stroke::default()
    }));
    if fill_color.is_some() || stroke.is_some() {
        surface.draw_path(path);
    }
}

fn paint(color: Rgb) -> krilla::paint::Paint {
    rgb::Color::new(color.r, color.g, color.b).into()
}

fn fill(color: Rgb) -> Fill {
    Fill {
        paint: paint(color),
        opacity: NormalizedF32::ONE,
        rule: Default::default(),
    }
}
