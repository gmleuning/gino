//! Imposition: reader pages onto printer sheets.
//!
//! Saddle-stitch, `n` pages (padded to a multiple of four with blanks),
//! sheets `s = 0 … n/4 − 1` from the outside in:
//!
//! - front of sheet `s`: left = page `n − 2s`, right = page `2s + 1`
//! - back of sheet `s`: left = page `2s + 2`, right = page `n − 2s − 1`
//!
//! Back sides are laid out for short-edge duplex printing, so they are not
//! turned. Creep shifts sheet `s` toward the spine by `s × creep_per_sheet`.
//!
//! The eight-page single-sheet fold puts eight panels on one side of a
//! landscape sheet; the table [`EIGHT_PAGE_FOLD`] says which page goes where.
//!
//! Sheet modes: when the sheet is the size of the imposed pages (A5 pairs on
//! A4) there is no bleed and no crop marks. When the sheet is larger (A5 on
//! A3), the pages are centred, run `bleed` millimetres past the trim on the
//! outer edges, and crop and fold marks sit outside the bleed.

use serde::{Deserialize, Serialize};

use crate::compose::Composition;
use crate::error::Error;
use crate::model::{Document, Fold, Rect, Size};
use crate::scene::{GroupItem, Item, Page, Rgb, RuleItem, Scene, Stroke, Transform};

/// Length of a crop mark, and its distance from the bleed edge, in millimetres.
const MARK_LENGTH: f64 = 5.0;
const MARK_GAP: f64 = 2.0;
/// A quarter point, in millimetres.
const MARK_WIDTH: f64 = 0.088;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetMode {
    /// The sheet equals the imposed pages: no bleed, no crop marks.
    SameSize,
    /// A larger sheet for trimming: bleed and crop marks.
    Oversize,
}

/// Which page sits in each of the eight panels, top row then bottom row,
/// left to right, as (page number from 1, turned upside down). This is the
/// commonly published layout; it is to be confirmed by printing and folding
/// the test sheet (`ginos-cli fold-test`) and fixed here if it is wrong.
pub const EIGHT_PAGE_FOLD: [[(usize, bool); 4]; 2] = [
    [(5, true), (4, true), (3, true), (2, true)],
    [(6, false), (7, false), (8, false), (1, false)],
];

/// The pages of one saddle-stitch sheet, numbered from 1: (left, right) on
/// the front and on the back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaddleSheet {
    pub front: (usize, usize),
    pub back: (usize, usize),
}

/// The sheets of a saddle-stitched booklet of `n` pages (a multiple of four).
pub fn saddle_sheets(n: usize) -> Vec<SaddleSheet> {
    (0..n / 4)
        .map(|s| SaddleSheet {
            front: (n - 2 * s, 2 * s + 1),
            back: (2 * s + 2, n - 2 * s - 1),
        })
        .collect()
}

/// The page count rounded up to what the fold needs.
pub fn padded_page_count(fold: Fold, pages: usize) -> usize {
    match fold {
        Fold::EightPageFold => 8,
        _ => pages.max(4).div_ceil(4) * 4,
    }
}

/// The area the imposed pages cover on one side of a sheet.
pub fn footprint(fold: Fold, trim: Size) -> Size {
    match fold {
        Fold::EightPageFold => Size {
            width: 4.0 * trim.width,
            height: 2.0 * trim.height,
        },
        _ => Size {
            width: 2.0 * trim.width,
            height: trim.height,
        },
    }
}

/// Whether a sheet is the size of the imposed pages or larger, or an error
/// when the pages do not fit on it.
pub fn sheet_mode(document: &Document, sheet: Size) -> Result<SheetMode, Error> {
    let settings = &document.document;
    let needed = footprint(settings.fold, settings.trim);
    // Up to a millimetre over is allowed: two A5 pages are 296 mm wide on a
    // 297 mm A4 sheet, and some trims round the other way.
    const SLACK: f64 = 1.0;
    let fits = |sheet: Size| needed.width <= sheet.width + SLACK && needed.height <= sheet.height + SLACK;
    if !fits(sheet) {
        // A portrait sheet holds the same pages turned; this program lays
        // sheets out landscape, so say what is needed.
        return Err(Error::Invalid(format!(
            "the imposed pages need a sheet of at least {} by {} mm; the sheet is {} by {} mm",
            needed.width, needed.height, sheet.width, sheet.height
        )));
    }
    let margin = 2.0 * (settings.bleed + MARK_GAP + MARK_LENGTH);
    if sheet.width >= needed.width + margin && sheet.height >= needed.height + margin {
        Ok(SheetMode::Oversize)
    } else {
        Ok(SheetMode::SameSize)
    }
}

/// One page on one side of a sheet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Page index from 0, or none for a blank.
    pub page: Option<usize>,
    /// Where the page's trim box sits on the sheet, before any turn.
    pub x: f64,
    pub y: f64,
    pub turned: bool,
    /// Millimetres the content is shifted right (positive) or left, for creep.
    pub shift: f64,
    /// Bleed on each edge of the trim box, in page coordinates: left, top, right, bottom.
    pub bleed: [f64; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct SheetSide {
    pub width: f64,
    pub height: f64,
    pub placements: Vec<Placement>,
    /// Crop and fold marks as line segments on the sheet.
    pub marks: Vec<(f64, f64, f64, f64)>,
    /// A label for the person printing, such as "sheet 1 front".
    pub label: String,
}

/// Lays the document's pages out on sheet sides, front then back of each
/// sheet. The sheet is landscape: the longer side is its width.
pub fn impose(document: &Document, sheet: Size) -> Result<Vec<SheetSide>, Error> {
    let sheet = if sheet.height > sheet.width {
        Size {
            width: sheet.height,
            height: sheet.width,
        }
    } else {
        sheet
    };
    let mode = sheet_mode(document, sheet)?;
    let settings = &document.document;
    let trim = settings.trim;
    let fold = settings.fold;
    let needed = footprint(fold, trim);
    let origin_x = (sheet.width - needed.width) / 2.0;
    let origin_y = (sheet.height - needed.height) / 2.0;
    let bleed = if mode == SheetMode::Oversize { settings.bleed } else { 0.0 };
    let page_count = document.pages.len();
    let page_index = |number: usize| (number <= page_count).then(|| number - 1);

    let mut sides = Vec::new();
    match fold {
        Fold::EightPageFold => {
            let mut placements = Vec::new();
            for (row, panels) in EIGHT_PAGE_FOLD.iter().enumerate() {
                for (column, &(number, turned)) in panels.iter().enumerate() {
                    // Bleed only on the outer edges of the whole block.
                    let mut edges = [
                        if column == 0 { bleed } else { 0.0 },
                        if row == 0 { bleed } else { 0.0 },
                        if column == 3 { bleed } else { 0.0 },
                        if row == 1 { bleed } else { 0.0 },
                    ];
                    if turned {
                        // In the page's own coordinates a turned page's left is the sheet's right.
                        edges = [edges[2], edges[3], edges[0], edges[1]];
                    }
                    placements.push(Placement {
                        page: page_index(number),
                        x: origin_x + column as f64 * trim.width,
                        y: origin_y + row as f64 * trim.height,
                        turned,
                        shift: 0.0,
                        bleed: edges,
                    });
                }
            }
            let mut marks = Vec::new();
            if mode == SheetMode::Oversize {
                let block = Rect::new(origin_x, origin_y, needed.width, needed.height);
                marks.extend(corner_marks(&block, bleed));
                for column in 1..4 {
                    marks.extend(fold_marks_vertical(origin_x + column as f64 * trim.width, &block, bleed));
                }
                marks.extend(fold_marks_horizontal(origin_y + trim.height, &block, bleed));
            }
            sides.push(SheetSide {
                width: sheet.width,
                height: sheet.height,
                placements,
                marks,
                label: "eight-page fold".to_owned(),
            });
        }
        _ => {
            let padded = padded_page_count(fold, page_count);
            let block = Rect::new(origin_x, origin_y, needed.width, needed.height);
            let spine = origin_x + trim.width;
            let mut marks = Vec::new();
            if mode == SheetMode::Oversize {
                marks.extend(corner_marks(&block, bleed));
                marks.extend(fold_marks_vertical(spine, &block, bleed));
            }
            for (s, sheet_pages) in saddle_sheets(padded).iter().enumerate() {
                let shift = s as f64 * settings.creep_per_sheet;
                for (side, (left, right)) in [("front", sheet_pages.front), ("back", sheet_pages.back)] {
                    let placements = vec![
                        Placement {
                            page: page_index(left),
                            x: origin_x,
                            y: origin_y,
                            turned: false,
                            shift,
                            bleed: [bleed, bleed, 0.0, bleed],
                        },
                        Placement {
                            page: page_index(right),
                            x: spine,
                            y: origin_y,
                            turned: false,
                            shift: -shift,
                            bleed: [0.0, bleed, bleed, bleed],
                        },
                    ];
                    sides.push(SheetSide {
                        width: sheet.width,
                        height: sheet.height,
                        placements,
                        marks: marks.clone(),
                        label: format!("sheet {} {side}", s + 1),
                    });
                }
            }
        }
    }
    Ok(sides)
}

fn corner_marks(block: &Rect, bleed: f64) -> Vec<(f64, f64, f64, f64)> {
    let near = bleed + MARK_GAP;
    let far = near + MARK_LENGTH;
    let mut marks = Vec::new();
    for (x, sx) in [(block.x, -1.0), (block.right(), 1.0)] {
        for (y, sy) in [(block.y, -1.0), (block.bottom(), 1.0)] {
            // Horizontal mark level with the trim edge, outside the bleed.
            marks.push((x + sx * near, y, x + sx * far, y));
            // Vertical mark.
            marks.push((x, y + sy * near, x, y + sy * far));
        }
    }
    marks
}

fn fold_marks_vertical(x: f64, block: &Rect, bleed: f64) -> Vec<(f64, f64, f64, f64)> {
    let near = bleed + MARK_GAP;
    let far = near + MARK_LENGTH;
    vec![
        (x, block.y - near, x, block.y - far),
        (x, block.bottom() + near, x, block.bottom() + far),
    ]
}

fn fold_marks_horizontal(y: f64, block: &Rect, bleed: f64) -> Vec<(f64, f64, f64, f64)> {
    let near = bleed + MARK_GAP;
    let far = near + MARK_LENGTH;
    vec![
        (block.x - near, y, block.x - far, y),
        (block.right() + near, y, block.right() + far, y),
    ]
}

/// A scene with one page per sheet side, built from a composed document.
pub fn imposed_scene(document: &Document, composition: &Composition, sheet: Size) -> Result<Scene, Error> {
    let trim = document.document.trim;
    let sides = impose(document, sheet)?;
    let mut scene = Scene::default();
    for side in sides {
        let mut items = Vec::new();
        for placement in &side.placements {
            let Some(page) = placement.page.and_then(|index| composition.scene.pages.get(index)) else {
                continue;
            };
            let [left, top, right, bottom] = placement.bleed;
            // The clip stays put on the sheet while creep shifts the content,
            // so nothing crosses the spine.
            let clip = Rect::new(-left - placement.shift, -top, trim.width + left + right, trim.height + top + bottom);
            let transform = if placement.turned {
                Transform::half_turn(placement.x, placement.y, trim.width, trim.height)
            } else {
                Transform::translate(placement.x + placement.shift, placement.y)
            };
            items.push(Item::Group(GroupItem {
                transform,
                clip: Some(clip),
                items: page.items().cloned().collect(),
            }));
        }
        for (x1, y1, x2, y2) in &side.marks {
            items.push(Item::Rule(RuleItem {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                stroke: Stroke {
                    width: MARK_WIDTH,
                    color: Rgb::BLACK,
                },
            }));
        }
        scene.pages.push(Page {
            width: side.width,
            height: side.height,
            frames: vec![crate::scene::FrameScene {
                id: side.label.replace(' ', "-"),
                items,
            }],
        });
    }
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Columns, Margins, Page as ModelPage, Settings};
    use crate::spike::{spike_document, SpikeFamilies};

    #[test]
    fn eight_pages_saddle_stitched() {
        let sheets = saddle_sheets(8);
        assert_eq!(
            sheets,
            vec![
                SaddleSheet {
                    front: (8, 1),
                    back: (2, 7)
                },
                SaddleSheet {
                    front: (6, 3),
                    back: (4, 5)
                },
            ]
        );
    }

    #[test]
    fn every_page_appears_once_and_facing_pages_sum_to_n_plus_one() {
        for n in [4, 8, 12, 16, 32] {
            let mut seen = vec![0; n + 1];
            for sheet in saddle_sheets(n) {
                for (left, right) in [sheet.front, sheet.back] {
                    assert_eq!(left + right, n + 1, "pages on one side of a sheet face across the spine");
                    seen[left] += 1;
                    seen[right] += 1;
                }
                // The page behind the front right page is the next one.
                assert_eq!(sheet.back.0, sheet.front.1 + 1);
            }
            assert!(seen[1..].iter().all(|count| *count == 1), "n = {n}: {seen:?}");
        }
    }

    #[test]
    fn page_counts_pad_to_the_fold() {
        assert_eq!(padded_page_count(Fold::SaddleA5OnA4, 13), 16);
        assert_eq!(padded_page_count(Fold::SaddleA5OnA4, 16), 16);
        assert_eq!(padded_page_count(Fold::SaddleA6OnA5, 1), 4);
        assert_eq!(padded_page_count(Fold::EightPageFold, 3), 8);
    }

    fn a5_document(pages: usize, creep: f64) -> Document {
        let mut document = spike_document(&SpikeFamilies {
            latin: "A".to_owned(),
            korean: "B".to_owned(),
        });
        document.document = Settings {
            fold: Fold::SaddleA5OnA4,
            trim: Size { width: 148.0, height: 210.0 },
            sheet: Size { width: 297.0, height: 210.0 },
            bleed: 3.0,
            printer_margin: 5.0,
            margins: Margins {
                top: 12.0,
                bottom: 15.0,
                inside: 12.0,
                outside: 10.0,
            },
            columns: Columns { count: 1, gutter: 5.0 },
            creep_per_sheet: creep,
            page_count: pages as u32,
        };
        document.pages = vec![ModelPage::default(); pages];
        document
    }

    #[test]
    fn a5_on_a4_is_same_size_and_centred() {
        let document = a5_document(8, 0.0);
        let a4 = Size { width: 297.0, height: 210.0 };
        assert_eq!(sheet_mode(&document, a4).unwrap(), SheetMode::SameSize);
        let sides = impose(&document, a4).unwrap();
        assert_eq!(sides.len(), 4);
        let front = &sides[0];
        assert_eq!(front.label, "sheet 1 front");
        assert!(front.marks.is_empty());
        assert_eq!(front.placements[0].page, Some(7));
        assert_eq!(front.placements[1].page, Some(0));
        assert!((front.placements[0].x - 0.5).abs() < 1e-9, "half a millimetre each side");
        assert!((front.placements[1].x - 148.5).abs() < 1e-9);
        assert_eq!(front.placements[0].bleed, [0.0; 4]);
        // A portrait A4 is taken as landscape.
        assert_eq!(impose(&document, Size { width: 210.0, height: 297.0 }).unwrap().len(), 4);
    }

    #[test]
    fn a5_on_a3_has_bleed_marks_and_creep() {
        let document = a5_document(16, 0.1);
        let a3 = Size { width: 420.0, height: 297.0 };
        assert_eq!(sheet_mode(&document, a3).unwrap(), SheetMode::Oversize);
        let sides = impose(&document, a3).unwrap();
        assert_eq!(sides.len(), 8);
        let innermost_front = &sides[6];
        assert_eq!(innermost_front.label, "sheet 4 front");
        assert_eq!(innermost_front.placements[0].page, Some(9), "page 10 on the left");
        assert_eq!(innermost_front.placements[1].page, Some(6), "page 7 on the right");
        assert!((innermost_front.placements[0].shift - 0.3).abs() < 1e-9, "left page moves right toward the spine");
        assert!((innermost_front.placements[1].shift + 0.3).abs() < 1e-9);
        assert_eq!(sides[0].placements[0].shift, 0.0, "the outermost sheet does not move");
        assert_eq!(innermost_front.placements[0].bleed, [3.0, 3.0, 0.0, 3.0], "no bleed across the spine");
        assert_eq!(innermost_front.marks.len(), 8 + 2, "four corners and the fold");
        // Marks start outside the bleed.
        let block_x = (420.0 - 296.0) / 2.0;
        assert!(innermost_front.marks.iter().any(|m| (m.0 - (block_x - 5.0)).abs() < 1e-9));
    }

    #[test]
    fn too_small_a_sheet_is_a_plain_error() {
        let document = a5_document(8, 0.0);
        let error = sheet_mode(&document, Size { width: 210.0, height: 148.0 }).unwrap_err();
        assert!(error.to_string().contains("at least 296 by 210 mm"), "{error}");
    }

    #[test]
    fn eight_page_fold_puts_every_page_on_one_side() {
        let mut document = a5_document(8, 0.0);
        document.document.fold = Fold::EightPageFold;
        document.document.trim = Size { width: 74.25, height: 105.0 };
        let sides = impose(&document, Size { width: 297.0, height: 210.0 }).unwrap();
        assert_eq!(sides.len(), 1);
        let mut pages: Vec<usize> = sides[0].placements.iter().filter_map(|p| p.page).collect();
        pages.sort();
        assert_eq!(pages, (0..8).collect::<Vec<_>>());
        let turned = sides[0].placements.iter().filter(|p| p.turned).count();
        assert_eq!(turned, 4, "the top row is upside down");
        assert!(sides[0].marks.is_empty());
    }
}
