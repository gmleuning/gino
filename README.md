# Gino

A small, free, offline Windows program for laying out zines. Place text and images on pages, see them as spreads, and print pages that fold into a booklet on the first try.

## What it does today

- **Starts from a fold.** Choose A5 pages on A4, A6 pages on A5, the eight-page single-sheet fold, a custom page size, or a North American saddle stitch (Letter, Legal or Tabloid pages). Margins, gutter and sheet follow from the choice, and you can change them in the document panel.
- **Starter layout.** New zines begin with an image frame on the front and back covers, a blank inside front cover, and one text story threaded through every column from page 3 to the second-to-last page. A preference starts new zines empty instead.
- **Frames.** Text, image and line frames in millimetres, snapping to margins, columns, the spine and each other. Click to select and drag; handles appear for resizing; arrow keys nudge (Shift for 5 mm); Delete removes. Double-click a text frame, or press Enter, to type in it.
- **Bilingual text.** Korean and English set together, with Korean input-method support on the canvas. The Korean font, scale and line-breaking controls are hidden until you turn them on in Preferences. Paragraph styles pick a Latin font, a Korean font and whether Korean breaks between syllables or only at spaces.
- **Threaded text.** A story flows from frame to frame; overflow is marked, and you click the marker and then the next frame to continue it.
- **Images.** PNG, JPEG and TIFF, linked by path. The inspector shows the whole original, its pixel size, the resolution the file states and the resolution it prints at in the frame. Fill, fit or manual crop; **Crop to page** and **Crop to bleed** trim a frame that runs off the edge without moving the picture.
- **Guides, off by default.** Margins and columns, bleed (light red, outer edges only), frame outlines and crop marks at the text area can each be shown or hidden. The page always stays white.
- **Print.** Three kinds, each with a resolution choice (default 300 dpi; pictures finer than that are reduced, coarser ones are left alone):
  - *Paged*: every page on its own, at page size.
  - *Viewed*: spreads as laid out on screen.
  - *Folded*: imposed sheets in fold order, to print double-sided flipping on the short edge, at actual size.
  - A single page can also be exported alone. Fonts are embedded in every PDF.
- **Undo and redo** for every change, and **preferences** for the colour scheme (light, dark or follow Windows), interface font and size, what shows at start, new-zine fonts and sizes, and print resolution.
- **Every command has a changeable shortcut.** See Edit → Shortcut keys.

Not done yet: JPG and PNG export, hyphenation, page-number master page, photocopy preview, Windows installer.

## Document format

A zine is a folder ending in `.zine`, holding a human-readable `document.json` (settings, styles, stories, pages, frames) and an `assets` folder. Lengths are in millimetres, type in points. Image paths are relative to the folder.

## Building from source

You need Rust (stable, 1.92 or later), the Microsoft C++ Build Tools ("Desktop development with C++"), the Edge WebView2 Runtime (already on current Windows) and Node.js.

```powershell
npm install
npm run tauri dev       # run with live reload
npm run tauri build     # release build
cargo test -p ginos-core
```

Export from the command line, without the window:

```powershell
cargo run -p ginos-cli -- export my-zine.zine --mode imposed --dpi 300 --out my-zine.pdf
```

`--mode` is `reader` or `imposed`; `--sheet` picks a different sheet size.

## How it is built

The Rust core (`crates/ginos-core`) lays out every glyph once; the same positions are drawn on the canvas and written to the PDF, so the screen does not disagree with the print. The interface (`ui`, Svelte) sends intents and draws what the core returns. `src-tauri` joins the two, and `crates/ginos-cli` is the command-line front end.

Built on [Tauri](https://v2.tauri.app/), [Parley](https://github.com/linebender/parley), [krilla](https://github.com/LaurenzV/krilla), [resvg](https://github.com/linebender/resvg) and [image](https://github.com/image-rs/image). Each keeps its own licence.

Ⓐ
