# Milestone 2: static canvas

Written 15 November 2026. Built and run on Windows 11 with Rust 1.99.1,
Node 26.13, Tauri 2.12.5 and WebView2 157.

## What exists

- **Shell** (`src-tauri`): one window, one `Session` behind a mutex. Every
  command forwards to the core and returns an `Update`: the SVG of each frame
  whose drawing or geometry changed, selection geometry and inspector data,
  the snap lines used, overflows and image problems, dirty and undo state.
- **Interface** (`ui`, Svelte 5 and TypeScript): page strip of spreads,
  canvas (one `<svg>` per spread whose user unit is a millimetre), inspector
  for the selected frame, status bar. The canvas draws fragments the core
  sends and forwards pointer events in page millimetres; it computes no
  position of its own. Zoom and scrolling are the only geometry it owns.
- **Core additions** (`ginos-core`): `edit` (commands with inverses and an
  undo history), `hit` (frame and handle under a point), `snap` (page edges,
  margins, column grid, spine, other frames' edges and centres), `images`
  (PNG, JPEG, TIFF decoded once, re-encoded as PNG for both renderers; fill,
  fit and manual crop), `zine` (open a folder, save through a temporary file
  and rename), `spreads` (reader spreads), `guides` (margin, column, printer
  margin, bleed and spine guides as SVG), `export` (reader-spread PDF),
  `session` (intents to commands to patches).
- **Command line**: `ginos-cli export <folder.zine> --mode reader --out <file.pdf>`.
- **Fixture**: `fixtures\documents\sample.zine`, eight A5 pages with a story
  threaded through four frames, a picture, a rule and a grey block.

## Protocol between interface and core

| Intent | Core method | Result |
| --- | --- | --- |
| pointer down (page, x, y, tolerance) | `pointer_down` | selects a frame or starts a resize on a handle |
| pointer move (x, y) | `pointer_move` | provisional geometry with snapping, patches for every re-laid-out frame, snap lines |
| pointer up | `pointer_up` | the whole drag becomes one `SetFrameRect` command |
| inspector value | `set_frame_rect` | one command |
| arrow keys | `nudge` | one command per press |
| place image, fit, crop | `place_image`, `set_image_fit`, `adjust_crop` | one `SetImage` command each |
| undo, redo, save, export | the same names | |

The tolerance is sent by the interface as millimetres (6 screen pixels at the
current zoom), so snapping and handles feel the same at every zoom.

Each `FramePatch` carries the frame's rectangle as well as its SVG, so an
empty frame that moves still produces a patch. Patches are sent only for
frames whose patch differs from the last one sent, which is how a move of one
frame re-sends only that frame and the frames of its thread that reflowed.

## What the core re-lays-out

Everything, on every change, then diffs the per-frame output. For milestone 2
documents this is immediate (the sample zine recomposes in a few
milliseconds). Milestone 3 measures typing on a 32-page document and limits
re-layout to the edited frame and the rest of its thread, as the prompt asks.

## Images

An image is decoded once with the `image` crate and kept as PNG bytes; the
SVG embeds them as a data URI inside a `<clipPath>`, the PDF embeds them with
krilla under the same clip. Re-encoding JPEG as PNG keeps the pixels exact
but makes PDFs larger than they need to be; passing JPEG through is a later
improvement. SVG images and resolution warnings come with later milestones.

Fit modes: `fill` scales to cover the frame, centred; `fit` scales to sit
inside it, centred; `manual` stores an offset and a scale (millimetres per
image pixel) and starts from whatever the previous mode produced.

## Tests

- `edit`: apply then undo returns the identical document; no-op commands are
  not recorded; a missing frame is a plain error.
- `hit`, `snap`, `images`, `spreads`, `guides`, `zine`: unit tests.
- `session_tests`: the fixture zine opens with every frame patched and no
  problems; a drag snaps, commits one command and undoes exactly; resizing a
  text frame re-renders the next frame of its thread and nothing else;
  placing an image, changing its fit and cropping are commands; reader
  spreads export as five PDF pages and render (PNGs in
  `target\reader-export\`).
- The milestone 1 pixel gate still runs.

## Korean input test

The window has a **Korean input test** button opening a throwaway frame.
Two approaches are built and switchable:

1. **Hidden `<textarea>`** positioned at the caret, forwarding
   `compositionstart`, `compositionupdate`, `compositionend` and `input` to
   the core, which lays out the committed text plus the composing text with
   an underline and returns the caret.
2. **EditContext** on a plain `<div>`, when the WebView2 in use supports it
   (the panel says whether it does), with `updateControlBounds` and
   `updateSelectionBounds` so the candidate window follows the caret.

The composing text is laid out by the core with Parley's underline
decoration, so it uses the real fonts and the real line breaking. Which
approach to keep is decided from Grant's test of both with the Windows
Korean input method; the panel logs every event it receives to make the
comparison concrete.

## Known gaps and limitations

- The Windows resource compiler cannot handle the apostrophe in the
  repository folder name `gino's`; `src-tauri\build.rs` copies the window icon
  to the system temporary folder to work around it. Renaming the clone to
  `ginos` would remove the workaround.
- Frames cannot yet be created, deleted, locked or reordered from the window;
  the fixture is edited by hand. Those arrive with milestone 3's tools.
- Rotation is stored but not drawn.
- Page numbers and the master page are not drawn until milestone 4.
- The window does not yet warn about unsaved changes on close.
