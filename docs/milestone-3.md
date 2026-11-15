# Milestone 3: text editing, threading, styles, imposition

Written 15 November 2026. Built on Windows 11 with Rust 1.99.1, Node 26.13,
Tauri 2.12.5 and WebView2 157.

## What exists

- **Text editing on the canvas.** Double-click a text frame (or select it and
  press Enter, or draw a new one with the Text tool) to type into its story.
  Caret, selection (Shift with arrows, Home, End; Ctrl for words and the
  story's ends; drag; double-click a word, triple-click a paragraph),
  Backspace and Delete (Ctrl for a word), Enter for a new paragraph, Ctrl+A,
  cut, copy, paste, undo and redo inside the text. Escape stops editing.
  Typing merges into one undo step until the caret moves.
- **Korean input.** A hidden `<textarea>` sits at the caret, so the input
  method's candidate window opens beside it. `compositionupdate` sends the
  syllable being composed to the core, which lays it out in place,
  underlined, without changing the document; `compositionend` commits it as
  one edit. This is the route that worked in the milestone 2 test; the
  EditContext route showed only completed syllables and was dropped.
- **Threading.** A frame whose story does not fit shows a red `+` at its
  bottom right. Click it, then click another text frame, and the story
  continues there (the inspector's "Thread…" does the same). "Break thread
  after" gives the frames after the selected one a story of their own. An
  empty frame can join a thread; a frame that is part of another thread
  cannot until that thread is broken.
- **Styles.** Paragraph styles carry the Latin and Korean font, Korean
  scale, size, leading, tracking, alignment, space before and after,
  first-line indent, and whether Korean breaks between words or syllables.
  Character styles carry italic, weight and underline. Both are applied from
  the inspector while editing; editing a style reflows every paragraph that
  uses it. New styles are made from the current one.
- **Drawing tools.** V select, T text, I image, R rectangle, L rule. Drag to
  draw; a click without a drag makes a frame of a useful size. Arrange
  (forward, backward, front, back), delete, stroke, grey and fill.
- **New-document dialog.** Fold presets (A5 on A4, A6 on A5, eight-page fold
  on A4, custom page size), page count, the sheet to print on, margins,
  columns, body fonts, size and leading. The document's settings (margins,
  columns, sheet, bleed, printer margin, creep, pages) can be changed later
  from the inspector with nothing selected.
- **Imposed PDF.** "Export for print…" (Ctrl+Shift+E) or
  `ginos-cli export --mode imposed`. Saddle stitch places pages in folding
  order, two to a side; the eight-page fold places all eight pages on one
  side of one sheet, top row turned. On a sheet larger than the imposed pages
  the export adds bleed, crop marks, fold marks and creep (0.1 mm per sheet by
  default); on a same-size sheet it adds none of them. Sheets are taken
  landscape whatever way round they are given.
- **Fold test.** `ginos-cli fold-test --fold eight-page|a5|a6 --out <file.pdf>`
  prints big page numbers with "top · 위" on each page, to check the folding
  order before printing a real zine.

## Typing timing

Measured by `editing_tests::typing_in_a_long_threaded_story_is_fast`: a 32-page
document with one story of 59,750 characters threaded through every page,
typing at the start of the story so every following line has to move.

| Build | Median per keystroke | Worst | Frames redrawn per keystroke |
| --- | --- | --- | --- |
| release | 22.1 ms | 32.1 ms | at most 2 |
| dev (core at opt-level 3) | about the same | | |
| dev (core at opt-level 1) | 59 ms | | |

The core keeps a cache of broken paragraphs keyed by their text, styles and
width, so a keystroke re-breaks only the paragraph typed into; frames whose
drawing did not change are not re-sent. Opening that document takes 218 ms.
`Cargo.toml` builds `ginos-core` at opt-level 3 in dev builds, so
`npm run tauri dev` types at release speed.

## Raster checks

| Test | Mismatched pixels | Ink difference | Ink centre distance | Centre tolerance |
| --- | --- | --- | --- | --- |
| spike page | 818 (0.0205 %) | 0.35 % | 0.15 px | 0.5 px |
| imposed eight-page sheet | 6 | 1.27 % | 0.759 px | 1.0 px |

The imposed sheet is mostly thin grey panel borders, which hayro paints
about 6 % darker than resvg; that alone moves the ink centre by under a
pixel, so this test allows 1.0 px.

## Known gaps

- No faux italic. Parley's synthesis is ignored by both renderers, so an
  italic character style only slants when an italic face of that family is
  installed. The fixture fonts have none.
- The eight-page fold's page order follows the usual one-sheet zine fold
  (top row 5 4 3 2 turned, bottom row 6 7 8 1) and has not yet been
  confirmed on paper.
- Frame rotation is stored but not drawn.
- Frames cannot be dragged from one page to another.
- Page numbers come in milestone 4.
- The printer margin is still a 5 mm placeholder until the real printer is
  measured.
