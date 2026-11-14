//! Gino's command-line program: the milestone 1 spike, and later export and
//! debugging dumps.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ginos_core::compose::compose;
use ginos_core::export::{reader_pdf, write_pdf};
use ginos_core::images::ImageCache;
use ginos_core::fonts::{fixture_font_folder, Fonts};
use ginos_core::render_pdf::scene_to_pdf;
use ginos_core::render_svg::page_to_svg;
use ginos_core::spike::{spike_document, SpikeFamilies};
use ginos_core::zine::Zine;

#[derive(Parser)]
#[command(name = "ginos-cli", about = "Gino's command-line program", version)]
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
    /// screens; `--mode imposed` comes with milestone 3.
    Export {
        /// The .zine folder (or its document.json).
        document: PathBuf,
        #[arg(long, default_value = "reader")]
        mode: String,
        /// Sheet size for imposed export (milestone 3).
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

fn export(document: &Path, mode: &str, sheet: Option<&str>, out: &Path) -> Result<(), String> {
    if mode != "reader" {
        return Err(format!(
            "export mode \"{mode}\" is not available yet; milestone 3 adds \"imposed\"."
        ));
    }
    if sheet.is_some() {
        return Err("--sheet applies to imposed export, which comes with milestone 3.".to_owned());
    }
    let zine = Zine::open(document).map_err(|error| error.to_string())?;
    let mut fonts = Fonts::with_system_fonts();
    for folder in zine.font_folders() {
        fonts.add_folder(&folder);
    }
    let mut images = ImageCache::new(&zine.folder);
    let (pdf, composition) =
        reader_pdf(&mut fonts, &mut images, &zine.document).map_err(|error| error.to_string())?;
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
    let spreads = ginos_core::spreads::reader_spreads(zine.document.pages.len()).len();
    println!("Wrote {} ({spreads} spreads)", out.display());
    Ok(())
}
