# Milestone 1: text spike

Written 15 November 2026. Everything here was built and run on Windows 11 with
Rust 1.99.1.

## What the spike does

`ginos-cli spike --out spike\` lays out one A4 page with three experiments and
writes it as `spike.svg` and `spike.pdf` (plus `spike.json`, the document that
produced them):

1. a mixed Korean and English paragraph in a 100 mm frame with `korean_break: syllable`;
2. the same paragraph with `korean_break: word`;
3. one story of four paragraphs threaded from a 60 mm frame into a 100 mm frame.

The spike defaults to GT Cinetype AUD and Gowun Dodum from the user font folder
and falls back to the fixture fonts (Crimson Text and Gowun Dodum, both SIL Open
Font License). `--latin-font` and `--korean-font` take any font file.

## Libraries

| Crate | Version | Licence | Role |
| --- | --- | --- | --- |
| parley | 0.13.0 | Apache-2.0 OR MIT | shaping (HarfRust), line breaking (ICU4X), font fallback |
| fontique | 0.13.0 | Apache-2.0 OR MIT | font database: system fonts, user files and folders |
| skrifa | 0.49.0 | Apache-2.0 OR MIT | glyph outlines for the SVG (same version Parley uses) |
| krilla | 0.8.2 | MIT OR Apache-2.0 | PDF writing, font embedding and subsetting |
| serde, serde_json | 1.0.229, 1.0.154 | MIT OR Apache-2.0 | document format |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | errors |
| clap | 4.6.7 | MIT OR Apache-2.0 | command line |
| hayro (tests only) | 0.9.0 | Apache-2.0 OR MIT | PDF rasteriser |
| resvg, usvg, tiny-skia (tests only) | 0.50.0, 0.50.0, 0.12.0 | MIT OR Apache-2.0; BSD-3-Clause | SVG rasteriser |
| image (tests only) | 0.25.10 | MIT OR Apache-2.0 | writing the comparison PNGs |

The full dependency tree of `ginos-core` is 122 crates. Every licence in it is
permissive (MIT, Apache-2.0, BSD, Zlib, Unicode-3.0 or a combination); nothing
is copyleft. Versions are pinned exactly in `Cargo.toml` and `Cargo.lock` is
committed.

## Korean break modes

Parley's line breaker supports the CSS `word-break` property directly, through
`StyleProperty::WordBreak`. `syllable` mode is Parley's default (`Normal`):
the Unicode line-breaking rules, which allow a break between any two Hangul
syllables. `word` mode sets `WordBreak::KeepAll` on the whole paragraph, which
forbids breaks inside runs of Hangul and Han characters while leaving breaks at
spaces and punctuation. No break-opportunity filtering of our own was needed.

Consequence to know about: in `word` mode a run of Korean longer than the frame
is wide cannot be broken at all and protrudes past the frame edge, exactly as
`keep-all` does on the web. The same is true of Latin words today, since there
is no hyphenation yet.

## Latin and Korean font pairing

The text of a paragraph is split into ranges by script (`layout::script_ranges`):
Hangul (jamo, compatibility jamo, syllables, extended blocks), Han characters,
and East Asian punctuation, symbols and fullwidth forms take the Korean font;
everything else takes the Latin font. Neutral characters (spaces, digits, ASCII
punctuation) join the range before them, so the space after a Korean word is
set in the Korean font. Each Korean range gets Parley's per-range
`FontFamily` and `FontSize` (size times `korean_scale`); the Latin font and size
are the paragraph defaults.

Parley aligns the runs of a line on a shared baseline, so the two faces sit on
the same line with no extra work. The line box height comes from the larger of
the two fonts' ascent and descent at their sizes, plus the half-leading needed
to reach the style's absolute leading; with a leading of 15 pt over 10 pt type
the Korean font's taller box made no visible difference. If a pairing ever
needs it, a baseline shift per script would be a one-line addition in
`break_lines`.

If a character is in neither font, Parley falls back to a system font when
system fonts are loaded; the PDF then embeds that font too.

## Line breaking and threading

All line breaking goes through one function, `layout::break_lines`, which
builds the Parley layout for a paragraph (or the remainder of one) at a given
width and indent. Hyphenation later means changing that function only.

Threading works per paragraph: a paragraph is broken at the current frame's
width; lines are placed until one would end below the frame's bottom edge; the
text from that line onward is broken again at the next frame's width, without
first-line indent. The frames' widths may differ freely. Text left over after
the last frame is counted as overflow and reported by `compose`.

## Scene and renderers

`layout` works in points relative to the frame. `compose` converts to
millimetres on the page and builds the `scene`: glyph runs (font, size, colour,
per-glyph origin and advance), rectangles and rules. Both renderers read only
the scene.

**SVG.** Glyphs are drawn as outlines. Each distinct glyph of each font becomes
one `<path>` in font units inside `<defs>`; each occurrence is a `<use>` with a
translate and a scale (negative in y). The trade-off: the webview never needs
the font file and shows the exact outline the PDF embeds, at the price of a
larger SVG than text elements would give (the spike page is about 280 KB,
about 1 150 glyph uses sharing about 420 outlines). Per-frame SVG in milestone
2 will share one outline cache across frames to keep that down.

**PDF.** krilla's `draw_glyphs` with one glyph list per run. krilla advances a
pen by each glyph's advance; the scene's absolute glyph positions are converted
into a per-glyph offset from that pen, so the PDF places every glyph exactly
where the scene says. Fonts are subset and embedded by krilla.

## The pixel gate

`cargo test -p ginos-core --test raster_compare` renders the spike page both
ways and rasterises each at 8 pixels per millimetre (1680 by 2376 pixels): the
SVG with resvg, the PDF with **hayro** 0.9, a pure Rust PDF rasteriser by the
author of krilla. hayro needs no setup and no downloaded binary, which is why it
was chosen over pdfium-render (which needs a separate Pdfium library).

The two rasterisers antialias edges differently, so an exact comparison is not
meaningful. The test counts a pixel as mismatched only when it differs by more
than 96 grey levels from every pixel in the other image's 3 by 3 neighbourhood,
in either direction, and requires:

- mismatched pixels at most 0.1 % of the page;
- total ink within 4 %;
- the centres of mass of the ink within 0.5 px of each other.

The last two catch what the neighbourhood tolerance would forgive: a global
offset, a wrong scale or a missing frame. The spike measures 818 mismatched
pixels (0.02 %), an ink difference of 0.35 % and a centre distance of 0.15 px.
The test writes `target\raster-compare\spike-from-svg.png` and
`spike-from-pdf.png` for inspection.

## Limitations found

- `word` mode lets an unbreakable Korean run protrude past the frame, as noted.
- Glyph text ranges for the PDF text layer come from Parley's clusters; if a
  style boundary ever splits a shaped run, the glyphs of that run map to the
  whole run's text instead. Copying text out of the PDF stays right; only the
  per-glyph mapping is coarser.
- Variable fonts have not been tried; the spike fonts are static.
- The SVG is one document per page. Per-frame patches come with milestone 2.
