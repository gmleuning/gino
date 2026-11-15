//! Milestone 1 gate: the SVG and the PDF of the spike are rasterised at the
//! same resolution and compared pixel by pixel.
//!
//! The SVG is rasterised by resvg, the PDF by hayro. The two rasterisers
//! antialias edges differently, so a pixel only counts as a mismatch when it
//! differs by more than `LEVEL_TOLERANCE` grey levels from every pixel in the
//! other image's 3 by 3 neighbourhood. The test passes when such mismatches
//! are below `MISMATCH_TOLERANCE` of all pixels, and when the amount of ink
//! and its centre of mass agree, which catches a global offset or a wrong
//! scale that neighbourhood forgiveness would hide.

mod common;

use ginos_core::compose::compose;
use ginos_core::render_pdf::scene_to_pdf;
use ginos_core::render_svg::page_to_svg;
use ginos_core::spike::spike_document;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};

/// Pixels per millimetre of the comparison rasters (about 203 dots per inch).
const PIXELS_PER_MM: f64 = 8.0;
/// Grey-level difference (0 to 255) above which two pixels are considered different.
const LEVEL_TOLERANCE: i32 = 96;
/// Highest acceptable share of mismatched pixels.
const MISMATCH_TOLERANCE: f64 = 0.001;
/// Highest acceptable relative difference in total ink.
const INK_TOLERANCE: f64 = 0.04;
/// Highest acceptable distance between the two ink centres, in pixels.
const CENTRE_TOLERANCE: f64 = 0.5;

struct Grey {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Grey {
    fn at(&self, x: isize, y: isize) -> Option<u8> {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            None
        } else {
            Some(self.pixels[y as usize * self.width + x as usize])
        }
    }

    /// Total ink (darkness) and its centre of mass.
    fn ink(&self) -> (f64, f64, f64) {
        let mut total = 0.0;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        for y in 0..self.height {
            for x in 0..self.width {
                let ink = f64::from(255 - self.pixels[y * self.width + x]);
                total += ink;
                sum_x += ink * x as f64;
                sum_y += ink * y as f64;
            }
        }
        (total, sum_x / total.max(1.0), sum_y / total.max(1.0))
    }

    fn save_png(&self, path: &std::path::Path) {
        let image = image::GrayImage::from_raw(self.width as u32, self.height as u32, self.pixels.clone())
            .expect("the raster has the right size");
        image.save(path).expect("the PNG can be written");
    }
}

fn rasterise_svg(svg: &str, width_mm: f64, height_mm: f64) -> Grey {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("the SVG parses");
    let width = (width_mm * PIXELS_PER_MM).round() as u32;
    let height = (height_mm * PIXELS_PER_MM).round() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("the pixmap can be created");
    pixmap.fill(tiny_skia::Color::WHITE);
    // The tree's size is in CSS pixels (96 to the inch); scale so one millimetre is PIXELS_PER_MM.
    let scale = (width_mm * PIXELS_PER_MM / f64::from(tree.size().width())) as f32;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let pixels = pixmap
        .pixels()
        .iter()
        .map(|pixel| {
            let demultiplied = pixel.demultiply();
            luma(demultiplied.red(), demultiplied.green(), demultiplied.blue())
        })
        .collect();
    Grey {
        width: width as usize,
        height: height as usize,
        pixels,
    }
}

fn rasterise_pdf(pdf_bytes: Vec<u8>, page_index: usize) -> Grey {
    let pdf = Pdf::new(pdf_bytes).expect("the PDF parses");
    let page = &pdf.pages()[page_index];
    let cache = RenderCache::new();
    let scale = (PIXELS_PER_MM * 25.4 / 72.0) as f32;
    let pixmap = hayro::render(
        page,
        &cache,
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &PixmapSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: WHITE,
        },
    );
    let width = usize::from(pixmap.width());
    let height = usize::from(pixmap.height());
    let pixels = pixmap
        .data()
        .iter()
        .map(|pixel| luma(pixel.r, pixel.g, pixel.b))
        .collect();
    Grey {
        width,
        height,
        pixels,
    }
}

fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000) as u8
}

/// A pixel mismatches when it differs from every pixel of the other image's
/// 3 by 3 neighbourhood by more than the level tolerance, in either direction.
fn count_mismatches(a: &Grey, b: &Grey, width: usize, height: usize) -> usize {
    let differs_from_neighbourhood = |value: u8, other: &Grey, x: usize, y: usize| {
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(candidate) = other.at(x as isize + dx, y as isize + dy) {
                    if (i32::from(value) - i32::from(candidate)).abs() <= LEVEL_TOLERANCE {
                        return false;
                    }
                }
            }
        }
        true
    };
    let mut mismatches = 0;
    for y in 0..height {
        for x in 0..width {
            let va = a.pixels[y * a.width + x];
            let vb = b.pixels[y * b.width + x];
            if (i32::from(va) - i32::from(vb)).abs() <= LEVEL_TOLERANCE {
                continue;
            }
            if differs_from_neighbourhood(va, b, x, y) || differs_from_neighbourhood(vb, a, x, y) {
                mismatches += 1;
            }
        }
    }
    mismatches
}

#[test]
fn svg_and_pdf_of_the_spike_match_pixel_for_pixel() {
    let (mut fonts, families) = common::fixture_fonts();
    let document = spike_document(&families);
    let mut images = ginos_core::images::ImageCache::new(std::path::Path::new("."));
    let composition = compose(&mut fonts, &document, &mut images).expect("the spike composes");
    assert!(composition.overflows.is_empty(), "the spike must not overflow: {:?}", composition.overflows);
    let page = &composition.scene.pages[0];

    let svg = page_to_svg(page);
    let pdf = scene_to_pdf(&composition.scene).expect("the PDF renders");
    assert_rasters_match("spike", &svg, page.width, page.height, pdf, CENTRE_TOLERANCE);
}

/// An imposed eight-page fold sheet: whole pages placed through transforms,
/// the top row turned upside down, each clipped to its panel.
#[test]
fn svg_and_pdf_of_an_imposed_sheet_match() {
    let (mut fonts, families) = common::fixture_fonts();
    let document = ginos_core::spike::fold_test_document(&families, ginos_core::model::Fold::EightPageFold, 8)
        .expect("the fold test document builds");
    let mut images = ginos_core::images::ImageCache::new(std::path::Path::new("."));
    let composition = compose(&mut fonts, &document, &mut images).expect("the fold test composes");
    assert!(composition.overflows.is_empty(), "{:?}", composition.overflows);
    let scene = ginos_core::impose::imposed_scene(&document, &composition, document.document.sheet).unwrap();
    assert_eq!(scene.pages.len(), 1);
    let page = &scene.pages[0];
    let svg = page_to_svg(page);
    let pdf = scene_to_pdf(&scene).expect("the PDF renders");
    // This page is mostly thin grey panel borders, which hayro paints about
    // 6 % darker than resvg; that alone moves the ink centre by under a pixel.
    assert_rasters_match("eight-page-fold", &svg, page.width, page.height, pdf, 1.0);
}

fn assert_rasters_match(name: &str, svg: &str, width_mm: f64, height_mm: f64, pdf: Vec<u8>, centre_tolerance: f64) {
    let from_svg = rasterise_svg(svg, width_mm, height_mm);
    let from_pdf = rasterise_pdf(pdf, 0);

    let folder = common::output_folder("raster-compare");
    from_svg.save_png(&folder.join(format!("{name}-from-svg.png")));
    from_pdf.save_png(&folder.join(format!("{name}-from-pdf.png")));

    assert!(
        from_svg.width.abs_diff(from_pdf.width) <= 1 && from_svg.height.abs_diff(from_pdf.height) <= 1,
        "raster sizes differ: SVG {}x{}, PDF {}x{}",
        from_svg.width,
        from_svg.height,
        from_pdf.width,
        from_pdf.height
    );
    let width = from_svg.width.min(from_pdf.width);
    let height = from_svg.height.min(from_pdf.height);

    let (ink_svg, cx_svg, cy_svg) = from_svg.ink();
    let (ink_pdf, cx_pdf, cy_pdf) = from_pdf.ink();
    assert!(ink_svg > 0.0 && ink_pdf > 0.0, "both rasters must contain ink");
    let ink_difference = (ink_svg - ink_pdf).abs() / ink_svg.max(ink_pdf);
    let centre_distance = ((cx_svg - cx_pdf).powi(2) + (cy_svg - cy_pdf).powi(2)).sqrt();

    let mismatches = count_mismatches(&from_svg, &from_pdf, width, height);
    let mismatch_share = mismatches as f64 / (width * height) as f64;

    println!(
        "raster {}x{} px at {} px/mm: {} mismatched pixels ({:.4} %), ink difference {:.2} %, centre distance {:.3} px",
        width,
        height,
        PIXELS_PER_MM,
        mismatches,
        mismatch_share * 100.0,
        ink_difference * 100.0,
        centre_distance
    );
    assert!(
        mismatch_share <= MISMATCH_TOLERANCE,
        "{mismatches} pixels mismatch ({:.4} %), more than {:.4} %; see {}",
        mismatch_share * 100.0,
        MISMATCH_TOLERANCE * 100.0,
        folder.display()
    );
    assert!(
        ink_difference <= INK_TOLERANCE,
        "total ink differs by {:.2} %, more than {:.2} %",
        ink_difference * 100.0,
        INK_TOLERANCE * 100.0
    );
    assert!(
        centre_distance <= centre_tolerance,
        "ink centres are {centre_distance:.3} px apart, more than {centre_tolerance} px"
    );
}
