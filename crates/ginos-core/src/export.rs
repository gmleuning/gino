//! PDF export: reader spreads, and sheets imposed for printing.

use std::path::Path;

use crate::compose::{compose, Composition};
use crate::error::Error;
use crate::fonts::Fonts;
use crate::images::ImageCache;
use crate::impose::imposed_scene;
use crate::model::{Document, Size};
use crate::render_pdf::scene_to_pdf;
use crate::scene::Scene;
use crate::spreads::{reader_spreads, spread_page};

/// A scene with one page per reader spread.
pub fn reader_spread_scene(document: &Document, composition: &Composition) -> Scene {
    let spreads = reader_spreads(document.pages.len());
    Scene {
        pages: spreads
            .iter()
            .map(|spread| spread_page(document, &composition.scene, spread))
            .collect(),
    }
}

/// Renders the document as reader spreads and returns the PDF bytes and any
/// overflowing stories.
pub fn reader_pdf(
    fonts: &mut Fonts,
    images: &mut ImageCache,
    document: &Document,
) -> Result<(Vec<u8>, Composition), Error> {
    let composition = compose(fonts, document, images)?;
    let scene = reader_spread_scene(document, &composition);
    let pdf = scene_to_pdf(&scene)?;
    Ok((pdf, composition))
}

/// Renders every page on its own PDF page at the trim size, in reading order.
pub fn paged_pdf(
    fonts: &mut Fonts,
    images: &mut ImageCache,
    document: &Document,
) -> Result<(Vec<u8>, Composition), Error> {
    let composition = compose(fonts, document, images)?;
    let pdf = scene_to_pdf(&composition.scene)?;
    Ok((pdf, composition))
}

/// Renders the document imposed for printing: one PDF page per sheet side,
/// on the document's sheet or on `sheet` when given.
pub fn imposed_pdf(
    fonts: &mut Fonts,
    images: &mut ImageCache,
    document: &Document,
    sheet: Option<Size>,
) -> Result<(Vec<u8>, Composition), Error> {
    let composition = compose(fonts, document, images)?;
    let scene = imposed_scene(document, &composition, sheet.unwrap_or(document.document.sheet))?;
    let pdf = scene_to_pdf(&scene)?;
    Ok((pdf, composition))
}

/// Writes a PDF through a temporary file and a rename.
pub fn write_pdf(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let temporary = path.with_extension("pdf.tmp");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::WriteFile {
            path: parent.to_owned(),
            source,
        })?;
    }
    std::fs::write(&temporary, bytes).map_err(|source| Error::WriteFile {
        path: temporary.clone(),
        source,
    })?;
    std::fs::rename(&temporary, path).map_err(|source| Error::WriteFile {
        path: path.to_owned(),
        source,
    })
}
