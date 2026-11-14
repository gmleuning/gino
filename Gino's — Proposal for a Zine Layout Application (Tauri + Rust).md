# Gino's — Proposal for a Zine Layout Application (Tauri + Rust)

Nov 15, 2026 · @no

## Summary

Gino's is a small, free, offline desktop program for laying out zines: place text and images on pages, see them as spreads, and print a correctly imposed PDF that folds into a booklet on the first try. It does a narrow job well, where Scribus and InDesign do a wide job with a heavy interface.

The proposal rests on one architectural choice: the Rust core owns all layout and draws the same glyph positions to the screen and to the PDF. That is what makes the preview trustworthy, and it is the main reason to build this in Rust rather than as a web page with a print button.

The first usable version (milestone 3 below) is built by Claude Code on Grant's Windows machine, so its pace is set by the testing rounds at the end of each milestone, not by coding time. No time estimate is given; the first useful figure will come from milestone 1.

## Principles and scope

"Simple and clear" is enforced by four rules, each of which rules features out.

1. **The page is the interface.** Panels appear only for the selected object; no ribbon, no docked palettes by default.
2. **One measurement system per document.** Millimetres throughout, with points only for type size and leading.
3. **The screen never lies about the print.** Whatever is on the canvas is, glyph for glyph, what the PDF contains.
4. **Folding is a first-class concept.** The user chooses a fold format; page count, imposition and creep follow from it rather than being configured by hand.

| In version 1 | Deferred or excluded |
| --- | --- |
| Fixed page sizes from fold presets (A5 saddle-stitch, A6 saddle-stitch, eight-page single-sheet fold) plus custom size | Arbitrary multi-section books, perfect binding |
| Text frames, image frames, rectangles and rules | Bézier drawing tools (use Inkscape or Krita and import SVG) |
| Text threaded between frames | Footnotes, indexes, cross-references, automatic hyphenation (planned for version 2) |
| Paragraph and character styles (a short list, not a hierarchy), each pairing a changeable Latin font with a changeable Korean font | Style inheritance trees, GREP styles |
| One master page per document, with automatic page numbers | Multiple masters |
| Imposed PDF and reader-spread PDF export; bleed and crop marks when the sheet is larger than the pages | CMYK colour management, spot-colour separations |
| Photocopy preview (greyscale and one-bit threshold) | Plugin system, scripting |
| Fonts from the system and from user-chosen folders | Cloud sync, collaboration |

## Core features

**Document setup.** A new document starts from a fold format, not a blank page size. Choosing "A5 saddle-stitch on A4 sheets" fixes the trim size, enforces a page count divisible by four, and sets default margins, gutter and sheet size; bleed applies only to oversize sheets (see Export). Adding pages adds them in fours and says so.

**Frames.** Everything on a page is a frame with a position, size and rotation in millimetres: text, image, rectangle, rule. Frames snap to margins, the column grid, the spine and to each other. Locking and a simple stacking order (bring forward, send back) are enough; there are no layers panel and no groups in version 1.

**Text.** Text frames can be threaded so a long piece flows across pages. Styling is a short list of named paragraph styles (Latin font, Korean font, Korean size scale, size, leading, tracking, alignment, space before and after, first-line indent) and character styles for italic, bold, small capitals and similar. Both fonts of a style can be changed at any time, and the pickers list installed fonts plus the user's font folders. Text is edited directly on the canvas. Line breaking follows the Unicode line-breaking rules, so Korean, Japanese and Chinese break correctly without manual intervention; whether Korean breaks between syllables or only at spaces should be a per-style choice.

**Images.** Raster images (PNG, JPEG, TIFF) and SVG are placed by reference, not copied, and fitted by fill, fit or manual crop. The program warns when an image's effective resolution at its printed size falls below a threshold (300 pixels per inch for greyscale, 600 for line art).

**Master page and page numbers.** A page-number frame placed on the master page prints each page's number automatically. The user chooses the first numbered page, so a cover or an inside cover can stay unnumbered, and the position of the number.

**Imposition.** This is the feature that distinguishes Gino's from a general layout program. Three presets:

- **Saddle-stitch booklet** — pages arranged so folded, nested sheets read in order (for 8 pages on two sheets: 8 | 1 and 2 | 7 on the first sheet, 6 | 3 and 4 | 5 on the second). Optional creep compensation shifts inner pages toward the spine by a small amount per sheet so the trimmed edge stays even.
- **Eight-page single-sheet fold** — the classic one-sheet mini zine with one slit; the top row is rotated 180 degrees.
- **Reader spreads** — no imposition, for screen PDFs and for print shops that impose themselves.

**Export.** PDF only in version 1, with embedded and subset fonts and a page-range option. An imposed export has two sheet modes. On a same-size sheet (two A5 pages on A4) the pages already fill the paper, so there is no bleed and no crop marks, and the canvas draws a printer-margin guide showing what a home printer will clip. On an oversize sheet, chosen when the print will be trimmed at a copy shop, the export adds 3 mm bleed and crop marks. PNG export of single pages for social posting is cheap to add because the renderer already exists.

**Photocopy preview.** A toggle that renders the canvas in greyscale, and a second toggle for a one-bit threshold with an adjustable level. Most zines end up on a photocopier, and thin type, light tints and dark photographs fail there in predictable ways.

## Architecture

Recommendation: the Rust core does all text layout and page composition; the Tauri webview only displays what the core has drawn and sends back user gestures. The alternatives are cheaper to start and worse to finish.

| Option | How it works | Gain | Cost |
| --- | --- | --- | --- |
| **A. Rust owns layout (recommended)** | Rust shapes and breaks text, positions every glyph, and sends the result to the webview as SVG; the same positioned glyphs go to the PDF writer | Screen and print are identical by construction; full control over imposition, creep, bleed | Text editing on the canvas (cursor, selection, input methods for Korean) must be built against the Rust layout rather than borrowed from the browser |
| B. Webview owns layout | Pages are HTML and CSS; export uses the webview's print-to-PDF | Fastest prototype; browser handles text editing | Print output differs between WebView2 on Windows and WebKit on macOS; no real control over bleed, imposition or font subsetting |
| C. Typst as engine | The document is compiled to Typst markup; preview and PDF come from the Typst compiler as a library | Excellent typesetting for free | Typst is a flowing typesetter, not a free-placement canvas; every drag becomes a recompile, and the document model bends to Typst's |

Option C is worth keeping in reserve for one job: if threaded long-form text proves hard to build, a single text frame could be typeset by Typst while the canvas stays under Gino's control.

**Stack for option A.**

- **Shell:** [Tauri](https://v2.tauri.app/) version 2, which has been stable since late 2024 and is actively maintained, with [releases through mid-2026](https://v2.tauri.app/ja/release/). It uses WebView2 on Windows, so the installer stays small.
- **Interface:** plain TypeScript with a small framework (Svelte or Solid) for panels only. The canvas is one SVG element that the core redraws.
- **Text layout:** [Parley](https://github.com/linebender/parley), the Linebender rich-text layout library. It shapes text with HarfRust (a Rust port of HarfBuzz), uses ICU4X for line breaking and bidirectional text, enumerates fonts and handles fallback through its companion Fontique, and includes a plain-text editor that tracks cursor, selection and input-method preedit text but not styled, threaded text, which is ours to build.
- **PDF output:** [krilla](https://github.com/LaurenzV/krilla), the high-level PDF library that Typst itself now uses for export. It subsets both CFF and TrueType fonts, embeds bitmap and SVG images, and is tested against Ghostscript, MuPDF, Poppler, pdf.js and other viewers.
- **Images:** the `image` crate for raster decoding and resolution checks; `resvg`/`usvg` for SVG.
- **Storage:** `serde` with JSON for the document; `zip` for packaging.

The diagram below shows the flow: every edit goes to the core, the core re-lays-out only the affected frames, and the canvas receives fresh SVG for those frames.

&#91;embedded content: architecture · webview displays, Rust core lays out\]

The webview never computes a line break; it only shows the SVG the core sends and forwards input back.

## Data model and file format

A document is a plain JSON file inside a folder, so it can be read, diffed and versioned without Gino's. Units are stored in millimetres as numbers; type sizes in points.

```
my-zine.zine/
  document.json     -- settings, styles, pages, frames, stories
  assets/           -- optional copies of linked images and fonts
  exports/          -- last exported PDFs
```

The model, in order of containment:

1. **Document** — fold format, trim size, bleed, margins, column grid, page count, master page.
2. **Styles** — paragraph styles and character styles, each a flat list of properties.
3. **Stories** — the text itself, as runs of characters with style references, independent of where it is placed. A story can flow through several frames.
4. **Pages** — an ordered list; each page holds frames.
5. **Frames** — kind (text, image, rectangle, rule), position, size, rotation, and either a story reference with a thread position or an image reference with crop.

Separating stories from frames is what makes threading and later re-pagination cheap: moving a frame never touches the text. Image and font paths are stored relative to the document folder, with a "collect for print" command that copies everything into `assets/` before sending the folder to someone else.

Undo is a stack of document diffs held in the Rust core, so the interface never holds authoritative state.

## Interface sketch

One window, three regions, nothing docked that the user did not ask for.

- **Page strip (left, collapsible).** Thumbnails of spreads in reading order, with the imposed sheet order available as a second view. Drag to reorder; pages move in fours when the fold format requires it.
- **Canvas (centre).** The current spread at true proportion, with margins, gutter, bleed and the fold line drawn as thin guides that can be hidden with one key.
- **Inspector (right, appears on selection).** Shows only the properties of the selected frame: position and size in millimetres, then text style or image fit. With nothing selected, it shows document settings.

All tools have single-key shortcuts (T for text frame, I for image, R for rectangle, L for rule, V for select), and every command is reachable from a command palette opened with Ctrl+K, so the menus can stay short. The photocopy preview and guide visibility are toggles in a thin bar under the canvas.

## Milestones

Each milestone ends in something printable, and the two riskiest tests, Korean text layout and Korean input, come in milestones 1 and 2.

1. **Text spike.** A command-line Rust program, no interface: lay out one paragraph of mixed Korean and English with Parley in a chosen font, write it to PDF with krilla, and write the same glyph positions to SVG. Also thread one story across two frames of different widths. *Gate:* the SVG and the PDF match when both are rasterised and compared pixel by pixel within a small tolerance, the story threads correctly across the two frames, and Korean line breaks look right in both break modes. If this fails, reconsider option C before building anything else.
2. **Static canvas.** Tauri shell, document loaded from JSON, spreads drawn as SVG, frames selectable and movable with snapping, inspector for position and size. Images placed and cropped. Every edit goes through one command structure, which later carries undo. Korean input on the canvas is tested here in a throwaway frame: a hidden text input against the browser's EditContext interface. *Gate:* a hand-written JSON zine renders and exports a reader-spread PDF, and Korean typed with the Windows input method composes correctly in the test frame.
3. **Editable text and imposition.** Text typed on the canvas with cursor and selection, Korean input method support, threading, paragraph and character styles, saddle-stitch and eight-page fold impositions in both sheet modes (bleed and crop marks on oversize sheets only). *Gate:* a real 16-page zine is made in Gino's, printed double-sided, folded and stapled, and reads in order. This is the first usable version.
4. **Finish.** Undo and redo interface, photocopy preview, resolution warnings, collect-for-print, creep compensation, master page with automatic page numbers, installer for Windows. *Gate:* someone else installs it and makes a zine without help.

Version 2 candidates, in rough order of value: hyphenation, PNG export for posting, multiple master pages, and a scripting hook.

## Risks

| Risk | Why it matters | Mitigation |
| --- | --- | --- |
| Canvas text editing is the hardest part | Option A gives up the browser's free cursor, selection and input-method handling; Korean composition (jamo assembling into syllables before commit) is especially fiddly | Use a hidden text input in the webview to receive keystrokes and composition events, and render the composing text from Rust; compare it with the browser's EditContext interface in a throwaway test in milestone 2, and keep whichever composes Korean syllables correctly |
| Rust libraries are pre-1.0 | Parley and krilla both change their interfaces between minor versions | Pin versions; keep each behind a thin internal module so an upgrade touches one file |
| Redraw speed on large documents | Re-laying out a 32-page threaded story on every keystroke could lag | Lay out only from the edited frame onward; send SVG per frame, not per page |
| Fonts outside the system folder | A font manager that activates fonts without installing them may hide them from system font lists | Let the user add font folders directly in Gino's settings |
| Scope creep toward InDesign | Every "small" feature request erodes the four principles | Treat the scope table as a contract; additions go to the version 2 list |

## Decisions

Settled 15 November 2026; the plan above stands with these choices.

| Question | Decision | Effect on the plan |
| --- | --- | --- |
| Name | Gino's | Display name; code and folders use the plain form ginos |
| Architecture | Option A: Rust owns layout | As written |
| Audience | Grant and a small circle | No public installer polish, documentation kept to a README |
| Platforms | Windows only | Test on WebView2 only; no macOS work |
| Bilingual text | Korean–English is core | Korean line breaking and input-method composition are milestone gates |
| Fonts | Latin and Korean font per style, both changeable | Style editor has two font pickers and a Korean size scale |
| Threaded text | Required | Stays in milestone 3 |
| Page numbers | Included | Page-number frame on the master page, in milestone 4 |
| Hyphenation | Later | Version 2 candidate; narrow justified columns will look gappy until then |
| Print route | Home printer and copy shop | Imposed and reader-spread PDF only; same-size sheets get no bleed, oversize sheets get bleed and crop marks |
| Risograph | Not needed | Dropped from scope |
| Programmer | Claude Code on Grant's Windows machine, in a clone of a GitHub repository; Grant reviews and tests | The program is built and run on the target machine; the core is still a separate library testable without a window |
| Licence | None, marked with a circled A (Ⓐ) | No licence file is shipped |
