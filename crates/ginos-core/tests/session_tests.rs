//! The fixture zine through the session: open, select, drag with snapping,
//! undo, place an image, export reader spreads.

mod common;

use std::path::Path;

use ginos_core::fonts::{fixture_font_folder, Fonts};
use ginos_core::images::ImageCache;
use ginos_core::model::{ImageFit, Rect};
use ginos_core::session::{Session, Tool};
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};

fn sample_zine() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("documents")
        .join("sample.zine")
}

fn open_sample() -> Session {
    let fonts = Fonts::without_system_fonts();
    Session::open(&sample_zine(), fonts).expect("the sample zine opens")
}

#[test]
fn the_sample_zine_opens_with_all_frames_and_no_problems() {
    let mut session = open_sample();
    let view = session.view().unwrap();
    assert_eq!(view.title, "sample");
    assert_eq!(view.pages.len(), 8);
    assert_eq!(view.spreads.len(), 5);
    assert_eq!(view.spreads[1].pages, vec![(1, 0.0), (2, 148.0)]);
    assert!(view.update.overflows.is_empty(), "overflows: {:?}", view.update.overflows);
    assert!(view.update.image_problems.is_empty(), "image problems: {:?}", view.update.image_problems);
    let total_frames: usize = view.pages.iter().map(|page| page.frame_ids.len()).sum();
    assert_eq!(view.update.patches.len(), total_frames);
    let picture = view
        .update
        .patches
        .iter()
        .find(|patch| patch.frame_id == "picture")
        .expect("the picture frame has a patch");
    assert!(picture.svg.contains("<image "), "the picture renders as an image");
    assert!(picture.svg.contains("data:image/png;base64,"));
    assert!(picture.svg.contains("<clipPath id=\"picture-clip1\">"));
    // The fixture font folder was loaded from the document's relative path.
    assert!(session.fonts.has_family("Crimson Text"));
    assert!(session.fonts.has_family("Gowun Dodum"));
    assert!(fixture_font_folder().is_dir());
}

#[test]
fn dragging_a_frame_snaps_commits_one_command_and_undoes() {
    let mut session = open_sample();
    session.view().unwrap();

    // Press inside the picture frame on page 3 (index 2): it is selected.
    let update = session.pointer_down(2, 50.0, 150.0, 2.0, Tool::Select, false, 1).unwrap();
    let selection = update.selection.expect("a frame is selected");
    assert_eq!(selection.frame_id, "picture");
    assert_eq!(selection.rect, Rect::new(14.0, 120.0, 123.0, 60.0));

    // Drag 8 mm right and 0.8 mm up. Nothing is within the 2 mm tolerance
    // (the frame's own old edges are excluded; the caption top at 182 is
    // 2.8 mm from the new bottom), so the move is applied as is.
    let update = session.pointer_move(58.0, 149.2).unwrap();
    assert!(update.patches.iter().any(|patch| patch.frame_id == "picture"), "the picture re-renders while dragging");
    let moving = update.selection.unwrap().rect;
    assert_eq!(moving, Rect::new(22.0, 119.2, 123.0, 60.0));

    // Move the bottom edge within 2 mm of the caption frame's top (182): snaps to it.
    let update = session.pointer_move(58.0, 151.5).unwrap();
    assert_eq!(update.selection.unwrap().rect, Rect::new(22.0, 122.0, 123.0, 60.0));
    assert_eq!(update.snap.unwrap().y, Some(182.0));

    let update = session.pointer_up().unwrap();
    assert!(update.can_undo);
    assert!(update.dirty);
    assert_eq!(session.zine.document.frame(2, "picture").unwrap().rect(), Rect::new(22.0, 122.0, 123.0, 60.0));

    // One drag is one undo step, back to the exact original rectangle.
    let update = session.undo().unwrap();
    assert!(!update.can_undo);
    assert!(update.can_redo);
    assert_eq!(session.zine.document.frame(2, "picture").unwrap().rect(), Rect::new(14.0, 120.0, 123.0, 60.0));
}

#[test]
fn resizing_a_text_frame_reflows_the_thread() {
    let mut session = open_sample();
    session.view().unwrap();
    // Shrink the first essay frame on page 2 (index 1) from the inspector.
    let before = session.zine.document.frame(1, "essay_1").unwrap().rect();
    let update = session
        .set_frame_rect(1, "essay_1", Rect::new(before.x, before.y, before.w, 60.0))
        .unwrap();
    let changed: Vec<&str> = update.patches.iter().map(|patch| patch.frame_id.as_str()).collect();
    assert!(changed.contains(&"essay_1"));
    assert!(changed.contains(&"essay_2"), "text pushed into the next frame re-renders it: {changed:?}");
    assert!(!changed.contains(&"cover_text"), "frames of other stories do not change");
}

#[test]
fn placing_an_image_and_changing_its_fit_are_commands() {
    let mut session = open_sample();
    session.view().unwrap();
    let picture = sample_zine().join("assets").join("sample-picture.png");
    // Remove, then place again, through an empty image frame: use the picture frame itself.
    let update = session.set_image_fit(2, "picture", ImageFit::Fit).unwrap();
    assert_eq!(update.selection, None);
    assert_eq!(session.zine.document.frame(2, "picture").unwrap().fit, ImageFit::Fit);
    let update = session.place_image(2, "picture", &picture).unwrap();
    let frame = session.zine.document.frame(2, "picture").unwrap();
    assert_eq!(frame.image.as_deref(), Some("assets\\sample-picture.png"));
    assert_eq!(frame.fit, ImageFit::Fill);
    assert!(update.can_undo);
    session.adjust_crop(2, "picture", 3.0, 0.0, 1.0).unwrap();
    let frame = session.zine.document.frame(2, "picture").unwrap();
    assert_eq!(frame.fit, ImageFit::Manual);
    let crop = frame.crop.unwrap();
    // Fill of a 1200x800 image in a 123x60 frame: scale 123/1200 = 0.1025, image 82 mm tall, offset y -11.
    assert!((crop.scale - 0.1025).abs() < 1e-9);
    assert!((crop.offset_x - 3.0).abs() < 1e-9);
    assert!((crop.offset_y + 11.0).abs() < 1e-9);
    session.undo().unwrap();
    session.undo().unwrap();
    session.undo().unwrap();
    let frame = session.zine.document.frame(2, "picture").unwrap();
    assert_eq!(frame.fit, ImageFit::Fill);
    assert_eq!(frame.crop, None);
}

#[test]
fn reader_spreads_export_and_render() {
    let mut session = open_sample();
    let folder = common::output_folder("reader-export");
    let path = folder.join("sample-reader.pdf");
    let overflows = session.export_reader_pdf(&path, None).unwrap();
    assert!(overflows.is_empty());
    let bytes = std::fs::read(&path).unwrap();
    let pdf = Pdf::new(bytes).expect("the PDF parses");
    assert_eq!(pdf.pages().len(), 5, "five spreads for eight pages");
    let cache = RenderCache::new();
    let scale = 3.0 * 25.4 / 72.0;
    for (index, name) in [(1usize, "spread-2-3.png"), (0, "spread-cover.png")] {
        let page = &pdf.pages()[index];
        let pixmap = hayro::render(
            page,
            &cache,
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &PixmapSettings {
                x_scale: scale as f32,
                y_scale: scale as f32,
                bg_color: WHITE,
            },
        );
        let (width, height) = (u32::from(pixmap.width()), u32::from(pixmap.height()));
        let rgba: Vec<u8> = pixmap.data().iter().flat_map(|p| [p.r, p.g, p.b, 255]).collect();
        image::RgbaImage::from_raw(width, height, rgba).unwrap().save(folder.join(name)).unwrap();
        if index == 1 {
            assert!((f64::from(width) - 296.0 * 3.0).abs() <= 1.0, "a two-page spread is 296 mm wide");
        }
    }
    let _ = ImageCache::new(&folder);
}

#[test]
fn paged_export_has_one_trim_sized_page_per_page() {
    let mut session = open_sample();
    let folder = common::output_folder("paged-export");
    let path = folder.join("sample-paged.pdf");
    let overflows = session.export_paged_pdf(&path, None).unwrap();
    assert!(overflows.is_empty());
    let pdf = Pdf::new(std::fs::read(&path).unwrap()).expect("the PDF parses");
    assert_eq!(pdf.pages().len(), 8);
    let (width, height) = pdf.pages()[3].render_dimensions();
    let to_mm = 25.4 / 72.0;
    assert!((f64::from(width) * to_mm - 148.0).abs() < 0.1, "width {width} pt");
    assert!((f64::from(height) * to_mm - 210.0).abs() < 0.1, "height {height} pt");
}

#[test]
fn a_single_page_exports_alone() {
    let mut session = open_sample();
    let folder = common::output_folder("page-export");
    let path = folder.join("sample-page-3.pdf");
    session.export_page_pdf(&path, 2, None).unwrap();
    let pdf = Pdf::new(std::fs::read(&path).unwrap()).expect("the PDF parses");
    assert_eq!(pdf.pages().len(), 1);
    assert!(session.export_page_pdf(&folder.join("none.pdf"), 99, None).is_err());
}

#[test]
fn a_print_resolution_reduces_fine_pictures_only() {
    let mut session = open_sample();
    let folder = common::output_folder("print-dpi");
    let full = folder.join("every-pixel.pdf");
    let reduced = folder.join("at-50-dpi.pdf");
    let generous = folder.join("at-5000-dpi.pdf");
    session.export_paged_pdf(&full, None).unwrap();
    session.export_paged_pdf(&reduced, Some(50.0)).unwrap();
    session.export_paged_pdf(&generous, Some(5000.0)).unwrap();
    let size = |path: &std::path::Path| std::fs::metadata(path).unwrap().len();
    assert!(size(&reduced) < size(&full), "50 dpi must shrink the picture: {} vs {}", size(&reduced), size(&full));
    assert_eq!(size(&generous), size(&full), "a resolution above the picture's leaves it alone");
    let info = session.image_info(2, "picture").unwrap();
    assert!(info.effective_dpi > 0.0);
    assert!(info.pixel_width > 0 && info.pixel_height > 0);
    assert!(info.thumbnail.starts_with("data:image/png;base64,"));
}
