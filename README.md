# Gino

A small, free, offline desktop program for laying out zines on Windows. Place text and images on pages, see them as spreads, and print a correctly imposed PDF that folds into a booklet on the first try.

Gino does one narrow job. It is not a general page-layout program and does not try to become one.

> **Status:** in development. Nothing below is guaranteed to work until milestone 3 ("first usable version") is reached. See [Roadmap](#roadmap).

---

## What it does

- **Starts from a fold, not a page size.** Choose a fold format (A5 saddle-stitch on A4, A6 saddle-stitch on A5, or the eight-page single-sheet mini zine). Trim size, page count rules, margins and gutter follow from that choice.
- **Frames on pages.** Text frames, image frames, rectangles and rules, positioned in millimetres, snapping to margins, the column grid, the spine and each other.
- **Bilingual text.** Korean and English set together, with correct line breaking for both and Korean input-method support while typing on the canvas. Each paragraph style chooses whether Korean breaks between syllables or only at spaces.
- **Changeable fonts.** Every paragraph style has a Latin font and a Korean font, both changeable at any time, plus a size scale for the Korean so the two faces sit well together.
- **Threaded text.** A long piece flows from frame to frame across pages.
- **Styles.** A short list of paragraph styles and character styles. No inheritance trees.
- **Page numbers.** A page-number frame on the master page numbers every page automatically; you choose the first numbered page, so covers stay clean.
- **Images.** PNG, JPEG, TIFF and SVG, linked rather than copied, with a warning when an image's printed resolution is too low.
- **Imposition and export.** One button produces either an imposed PDF (for a home printer's double-sided mode or a copy shop) or a reader-spread PDF for screens. Fonts are embedded and subset. Two sheet modes:
  - **Same-size sheet** (for example two A5 pages on A4): the pages fill the paper, so there is no bleed and no crop marks; the canvas draws a printer-margin guide showing what a home printer will clip.
  - **Oversize sheet** (for trimming at a copy shop): 3 mm bleed and crop marks.
- **Photocopy preview.** Greyscale and one-bit threshold views show what will survive a photocopier.

## What it deliberately does not do

Drawing tools (use Inkscape or Krita and import SVG), colour management for offset printing, Risograph separations, multiple master pages, footnotes and indexes, plugins, scripting, cloud sync, macOS or Linux builds. Automatic hyphenation is planned for a later version; until then, narrow justified columns of English text will show uneven spacing, so left-aligned is the safer setting.

## The one design rule that matters

The screen never lies about the print. The Rust core lays out every glyph once, and the same positioned glyphs are drawn to the canvas and written to the PDF. The interface never computes a line break of its own.

---

## Building from source (Windows)

### Requirements

| Tool | Notes |
| --- | --- |
| Rust (stable, via `rustup`) | Version 1.92 or later; the PDF library requires it |
| Microsoft C++ Build Tools | "Desktop development with C++" workload |
| Microsoft Edge WebView2 Runtime | Already present on current Windows 10 and 11 |
| Node.js | Long-term-support release |

### Build and run

```powershell
git clone <repository-url> ginos
cd ginos
npm install
npm run tauri dev        # development build with live reload
npm run tauri build      # release installer in src-tauri\target\release\bundle
```

### Run the core tests (no window needed)

```powershell
cargo test -p ginos-core
```

### Command-line export

```powershell
cargo run -p ginos-cli -- export path\to\my-zine.zine --mode imposed --out my-zine-print.pdf
```

`--mode` is `reader` (spreads for screens) or `imposed` (sheets for printing). For imposed export, `--sheet <size>` picks the sheet; by default it is the sheet of the document's fold format, and a larger sheet switches on bleed and crop marks.

---

## Using Gino

1. **New document** → choose a fold format and a page count (rounded to the nearest valid count for that fold).
2. **Place frames** with the keyboard: `T` text, `I` image, `R` rectangle, `L` rule, `V` select. Every command is also in the command palette (`Ctrl+K`).
3. **Thread text** by clicking a text frame's out-port, then clicking the next frame.
4. **Check for the photocopier** with the preview toggles under the canvas.
5. **Export** → *Imposed for printing* or *Reader spreads*.

### Printing an imposed saddle-stitch booklet at home

Print the imposed PDF double-sided, flipping on the **short edge**, at **100 % / actual size** (never "fit to page"). Stack the sheets in the order they come out, fold the stack in half, and staple on the fold.

If the printer has no automatic double-sided mode, print the odd-numbered pages of the PDF first (the sheet fronts), then reload the paper and print the even-numbered pages (the backs). How to reload depends on the printer, so test with pages 1–4 before printing the whole booklet.

### Fonts

Gino lists the fonts installed in Windows. Fonts that a font manager activates without installing may not appear; add their folders under **Settings → Font folders**. In the style editor, each paragraph style has a Latin font and a Korean font; change either at any time and the whole document reflows.

---

## Document format

A Gino document is a folder ending in `.zine`:

```
my-zine.zine\
  document.json   settings, styles, stories, pages, frames
  assets\         images and fonts copied by "Collect for print"
  exports\        last exported PDFs
```

`document.json` is plain, human-readable JSON. Lengths are in millimetres; type sizes and leading in points. Text ("stories") is stored separately from the frames it flows through. Image paths are relative to the document folder.

---

## Project layout

```
crates\ginos-core\   document model, layout, imposition, SVG and PDF rendering (no window code)
crates\ginos-cli\    command-line export and test harness
src-tauri\           the desktop shell: commands between interface and core
ui\                  interface: canvas, page strip, inspector
fixtures\            sample documents and test fonts
docs\                proposal and design notes
```

## Built on

[Tauri](https://v2.tauri.app/) (desktop shell) · [Parley](https://github.com/linebender/parley) (text layout, with HarfRust shaping and ICU4X line breaking) · [krilla](https://github.com/LaurenzV/krilla) (PDF writing) · [resvg](https://github.com/linebender/resvg) (SVG images) · [image](https://github.com/image-rs/image) (raster images). Each keeps its own licence.

## Roadmap

1. **Text spike** — mixed Korean and English laid out once, threaded across two frames, written identically to SVG and PDF.
2. **Static canvas** — documents open, frames move and snap, images place, reader-spread PDF exports; Korean input on the canvas tested in a throwaway frame.
3. **Editable text and imposition** — typing on the canvas with Korean input, threading, styles, imposed PDFs. *First usable version.*
4. **Finish** — undo and redo, photocopy preview, resolution warnings, collect for print, creep compensation, master page with page numbers, Windows installer.

## Licence

Ⓐ
