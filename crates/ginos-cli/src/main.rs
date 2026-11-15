//! Gino command-line program: the milestone 1 spike, PDF export (reader
//! spreads or imposed sheets) and the fold test sheet.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ginos_core::compose::compose;
use ginos_core::export::{imposed_pdf, reader_pdf, write_pdf};
use ginos_core::impose::{impose, sheet_mode, SheetMode};
use ginos_core::model::{Fold, Size};
use ginos_core::units::paper;
use ginos_core::images::ImageCache;
use ginos_core::fonts::{fixture_font_folder, Fonts};
use ginos_core::render_pdf::scene_to_pdf;
use ginos_core::render_svg::page_to_svg;
use ginos_core::spike::{fold_test_document, spike_document, SpikeFamilies};
use ginos_core::zine::Zine;

#[derive(Parser)]
#[command(name = "ginos-cli", about = "Gino command-line program", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Writes spike.svg and spike.pdf: mixed Korean and English in both break
    /// modes, and one story threaded across two frames.
    Spike {
        /// Folder to write into; created if missing.
        #[arg(long)]
        out: PathBuf,
        /// Latin font file. Default: GT Cinetype from the user font folder,
        /// else Crimson Text from the fixtures.
        #[arg(long)]
        latin_font: Option<PathBuf>,
        /// Korean font file. Default: Gowun Dodum from the user font folder,
        /// else from the fixtures.
        #[arg(long)]
        korean_font: Option<PathBuf>,
    },
    /// Exports a .zine folder as a PDF. `--mode reader` writes spreads for
    /// screens; `--mode imposed` writes printer sheets.
    Export {
        /// The .zine folder (or its document.json).
        document: PathBuf,
        #[arg(long, default_value = "reader")]
        mode: String,
        /// Sheet for imposed export: A3, A4, A5, A6 or a size such as 320x450
        /// (millimetres). Default: the document's sheet. A sheet larger than
        /// the imposed pages adds bleed and crop marks.
        #[arg(long)]
        sheet: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Writes an imposed test sheet with large page numbers, to print, cut
    /// and fold to check a fold's page order.
    FoldTest {
        /// eight-page, a5 (A5 saddle-stitch on A4) or a6 (A6 on A5).
        #[arg(long, default_value = "eight-page")]
        fold: String,
        #[arg(long)]
        sheet: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Spike {
            out,
            latin_font,
            korean_font,
        } => spike(&out, latin_font, korean_font),
        Command::Export {
            document,
            mode,
            sheet,
            out,
        } => export(&document, &mode, sheet.as_deref(), &out),
        Command::FoldTest { fold, sheet, out } => fold_test(&fold, sheet.as_deref(), &out),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn user_font_folder() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| {
        Path::new(&base)
            .join("Microsoft")
            .join("Windows")
            .join("Fonts")
    })
}

/// The first existing candidate: user font folder files first, then fixtures.
fn default_font(user_file: &str, fixture_file: &str) -> PathBuf {
    if let Some(folder) = user_font_folder() {
        let candidate = folder.join(user_file);
        if candidate.is_file() {
            return candidate;
        }
    }
    fixture_font_folder().join(fixture_file)
}

fn spike(out: &Path, latin_font: Option<PathBuf>, korean_font: Option<PathBuf>) -> Result<(), String> {
    let latin_path =
        latin_font.unwrap_or_else(|| default_font("GT-UCLA-CINETYPE-REGULAR.OTF", "CrimsonText-Regular.ttf"));
    let korean_path =
        korean_font.unwrap_or_else(|| default_font("GOWUNDODUM-REGULAR.TTF", "GowunDodum-Regular.ttf"));

    let mut fonts = Fonts::with_system_fonts();
    let latin_families = fonts.add_file(&latin_path).map_err(|error| error.to_string())?;
    let korean_families = fonts.add_file(&korean_path).map_err(|error| error.to_string())?;
    let families = SpikeFamilies {
        latin: latin_families[0].clone(),
        korean: korean_families[0].clone(),
    };
    println!("Latin font:  {} ({})", families.latin, latin_path.display());
    println!("Korean font: {} ({})", families.korean, korean_path.display());

    let document = spike_document(&families);
    let mut images = ImageCache::new(out);
    let composition = compose(&mut fonts, &document, &mut images).map_err(|error| error.to_string())?;
    for overflow in &composition.overflows {
        println!(
            "Warning: story \"{}\" overflows its frames by {} characters.",
            overflow.story, overflow.characters
        );
    }

    std::fs::create_dir_all(out).map_err(|error| format!("could not create {}: {error}", out.display()))?;
    let svg_path = out.join("spike.svg");
    let pdf_path = out.join("spike.pdf");
    let json_path = out.join("spike.json");

    let svg = page_to_svg(&composition.scene.pages[0]);
    std::fs::write(&svg_path, svg).map_err(|error| format!("could not write {}: {error}", svg_path.display()))?;
    let pdf = scene_to_pdf(&composition.scene).map_err(|error| error.to_string())?;
    std::fs::write(&pdf_path, pdf).map_err(|error| format!("could not write {}: {error}", pdf_path.display()))?;
    let json = document.to_json().map_err(|error| error.to_string())?;
    std::fs::write(&json_path, json).map_err(|error| format!("could not write {}: {error}", json_path.display()))?;

    println!("Glyphs placed: {}", composition.scene.glyph_count());
    println!("Wrote {}", svg_path.display());
    println!("Wrote {}", pdf_path.display());
    println!("Wrote {}", json_path.display());
    Ok(())
}

/// Reads a sheet size: A3, A4, A5 or A6, or a width and height in
/// millimetres such as `320x450`.
fn parse_sheet(text: &str) -> Result<Size, String> {
    let named = |(width, height): (f64, f64)| Size { width, height };
    match text.trim().to_ascii_uppercase().as_str() {
        "A3" => return Ok(named(paper::A3)),
        "A4" => return Ok(named(paper::A4)),
        "A5" => return Ok(named(paper::A5)),
        "A6" => return Ok(named(paper::A6)),
        _ => {}
    }
    let parts: Vec<&str> = text.split(['x', 'X', '×']).collect();
    if let [width, height] = parts.as_slice() {
        if let (Ok(width), Ok(height)) = (width.trim().parse::<f64>(), height.trim().parse::<f64>()) {
            if width > 0.0 && height > 0.0 {
                return Ok(Size { width, height });
            }
        }
    }
    Err(format!("\"{text}\" is not a sheet size; use A3, A4, A5, A6 or a size in millimetres such as 320x450"))
}

fn export(document: &Path, mode: &str, sheet: Option<&str>, out: &Path) -> Result<(), String> {
    let imposed = match mode {
        "reader" => false,
        "imposed" => true,
        other => return Err(format!("export mode \"{other}\" does not exist; use reader or imposed")),
    };
    if sheet.is_some() && !imposed {
        return Err("--sheet applies to imposed export only".to_owned());
    }
    let sheet = sheet.map(parse_sheet).transpose()?;
    let zine = Zine::open(document).map_err(|error| error.to_string())?;
    let mut fonts = Fonts::with_system_fonts();
    for folder in zine.font_folders() {
        fonts.add_folder(&folder);
    }
    let mut images = ImageCache::new(&zine.folder);
    let (pdf, composition) = if imposed {
        imposed_pdf(&mut fonts, &mut images, &zine.document, sheet)
    } else {
        reader_pdf(&mut fonts, &mut images, &zine.document)
    }
    .map_err(|error| error.to_string())?;
    for overflow in &composition.overflows {
        println!(
            "Warning: story \"{}\" overflows its frames by {} characters.",
            overflow.story, overflow.characters
        );
    }
    for problem in &composition.image_problems {
        println!("Warning: page {} frame \"{}\": {}", problem.page + 1, problem.frame, problem.message);
    }
    write_pdf(out, &pdf).map_err(|error| error.to_string())?;
    if imposed {
        let sheet = ginos_core::impose::landscape(sheet.unwrap_or(zine.document.document.sheet));
        let mode = sheet_mode(&zine.document, sheet).map_err(|error| error.to_string())?;
        let sides = impose(&zine.document, sheet).map_err(|error| error.to_string())?.len();
        let mode = match mode {
            SheetMode::SameSize => "same-size sheet, no bleed or crop marks",
            SheetMode::Oversize => "larger sheet, with bleed and crop marks",
        };
        println!("Wrote {} ({sides} sheet sides on {} by {} mm, {mode})", out.display(), sheet.width, sheet.height);
        if sides > 1 {
            println!("Print double-sided, flipping on the short edge.");
        }
    } else {
        let spreads = ginos_core::spreads::reader_spreads(zine.document.pages.len()).len();
        println!("Wrote {} ({spreads} spreads)", out.display());
    }
    Ok(())
}

fn fold_test(fold: &str, sheet: Option<&str>, out: &Path) -> Result<(), String> {
    let (fold, pages) = match fold {
        "eight-page" => (Fold::EightPageFold, 8),
        "a5" => (Fold::SaddleA5OnA4, 8),
        "a6" => (Fold::SaddleA6OnA5, 8),
        other => return Err(format!("fold \"{other}\" does not exist; use eight-page, a5 or a6")),
    };
    let sheet = sheet.map(parse_sheet).transpose()?;
    let mut fonts = Fonts::with_system_fonts();
    let latin = fonts
        .add_file(&default_font("GT-UCLA-CINETYPE-REGULAR.OTF", "CrimsonText-Regular.ttf"))
        .map_err(|error| error.to_string())?;
    let korean = fonts
        .add_file(&default_font("GOWUNDODUM-REGULAR.TTF", "GowunDodum-Regular.ttf"))
        .map_err(|error| error.to_string())?;
    let families = SpikeFamilies {
        latin: latin[0].clone(),
        korean: korean[0].clone(),
    };
    let document = fold_test_document(&families, fold, pages).map_err(|error| error.to_string())?;
    let mut images = ImageCache::new(Path::new("."));
    let (pdf, _) = imposed_pdf(&mut fonts, &mut images, &document, sheet).map_err(|error| error.to_string())?;
    write_pdf(out, &pdf).map_err(|error| error.to_string())?;
    println!("Wrote {}", out.display());
    match fold {
        Fold::EightPageFold => println!(
            "Print one side at 100 % (actual size), cut the slit and fold. The pages must read 1 to 8 upright."
        ),
        _ => println!(
            "Print double-sided, flipping on the short edge, at 100 % (actual size). Fold, nest and staple: pages 1 to 8 upright."
        ),
    }
    Ok(())
}
