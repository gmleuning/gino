//! New documents: fold presets and the settings the new-document dialog sends.

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::impose::{footprint, padded_page_count, sheet_mode, SheetMode};
use crate::model::{
    Alignment, Columns, Document, FontSettings, Fold, Frame, KoreanBreak, MasterPage, Margins, Page,
    Paragraph, ParagraphStyle, Settings, Size, Story, FORMAT_VERSION,
};
use crate::snap::{column_edges, page_margins};
use crate::units::paper;

/// What the new-document dialog asks for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewDocument {
    pub fold: Fold,
    /// Only for the custom fold: the trim size of one page.
    #[serde(default)]
    pub custom_trim: Option<Size>,
    pub page_count: u32,
    /// None prints on the fold's own sheet (same-size). A larger sheet
    /// switches on bleed and crop marks.
    #[serde(default)]
    pub sheet: Option<Size>,
    pub margins: Margins,
    pub columns: Columns,
    pub latin_font: String,
    pub korean_font: String,
    /// Points.
    pub size: f64,
    /// Points.
    pub leading: f64,
    /// Start with an image frame on the front and back covers and one story
    /// threaded through the inside pages. Off gives empty pages.
    #[serde(default = "yes")]
    pub starter_frames: bool,
}

fn yes() -> bool {
    true
}

/// The trim size of a page and the same-size sheet for a fold.
pub fn fold_sizes(fold: Fold, custom_trim: Option<Size>) -> (Size, Size) {
    let a4 = landscape(size(paper::A4));
    let a5 = landscape(size(paper::A5));
    match fold {
        Fold::SaddleA5OnA4 => (size(paper::A5), a4),
        Fold::SaddleA6OnA5 => (size(paper::A6), a5),
        Fold::EightPageFold => (
            Size {
                width: a4.width / 4.0,
                height: a4.height / 2.0,
            },
            a4,
        ),
        Fold::Custom => {
            let trim = custom_trim.unwrap_or(size(paper::A5));
            (trim, footprint(Fold::Custom, trim))
        }
        Fold::SaddleLetter => (size(paper::LETTER), footprint(fold, size(paper::LETTER))),
        Fold::SaddleLegal => (size(paper::LEGAL), footprint(fold, size(paper::LEGAL))),
        Fold::SaddleTabloid => (size(paper::TABLOID), footprint(fold, size(paper::TABLOID))),
    }
}

fn size((width, height): (f64, f64)) -> Size {
    Size { width, height }
}

fn landscape(size: Size) -> Size {
    Size {
        width: size.width.max(size.height),
        height: size.width.min(size.height),
    }
}

/// Default margins for a trim size: smaller pages get smaller margins.
pub fn default_margins(fold: Fold) -> Margins {
    match fold {
        Fold::SaddleA5OnA4 | Fold::Custom => Margins {
            top: 12.0,
            bottom: 15.0,
            inside: 12.0,
            outside: 10.0,
        },
        Fold::SaddleLetter | Fold::SaddleLegal => Margins {
            top: 15.0,
            bottom: 18.0,
            inside: 15.0,
            outside: 12.0,
        },
        Fold::SaddleTabloid => Margins {
            top: 20.0,
            bottom: 24.0,
            inside: 20.0,
            outside: 16.0,
        },
        Fold::SaddleA6OnA5 => Margins {
            top: 9.0,
            bottom: 11.0,
            inside: 9.0,
            outside: 8.0,
        },
        Fold::EightPageFold => Margins {
            top: 6.0,
            bottom: 7.0,
            inside: 5.0,
            outside: 5.0,
        },
    }
}

/// Builds an empty document: pages, one body style, settings.
pub fn new_document(request: &NewDocument) -> Result<Document, Error> {
    if request.size <= 0.0 || request.leading <= 0.0 {
        return Err(Error::Invalid("type size and leading must be above zero".to_owned()));
    }
    if let Some(trim) = request.custom_trim {
        if request.fold == Fold::Custom && (trim.width < 20.0 || trim.height < 20.0) {
            return Err(Error::Invalid("a custom page must be at least 20 by 20 mm".to_owned()));
        }
    }
    let (trim, own_sheet) = fold_sizes(request.fold, request.custom_trim);
    let sheet = request.sheet.map(landscape).unwrap_or(own_sheet);
    let pages = padded_page_count(request.fold, request.page_count as usize);
    let mut document = Document {
        format_version: FORMAT_VERSION,
        document: Settings {
            fold: request.fold,
            trim,
            sheet,
            bleed: 3.0,
            printer_margin: 5.0,
            margins: request.margins,
            columns: Columns {
                count: request.columns.count.max(1),
                gutter: request.columns.gutter,
            },
            creep_per_sheet: 0.0,
            page_count: pages as u32,
        },
        fonts: FontSettings::default(),
        paragraph_styles: vec![ParagraphStyle {
            id: "body".to_owned(),
            latin_font: request.latin_font.clone(),
            korean_font: request.korean_font.clone(),
            korean_scale: 1.0,
            size: request.size,
            leading: request.leading,
            tracking: 0.0,
            alignment: Alignment::Left,
            space_before: 0.0,
            space_after: 0.0,
            first_line_indent: 0.0,
            korean_break: KoreanBreak::Word,
        }],
        character_styles: Vec::new(),
        stories: Vec::new(),
        master: MasterPage::default(),
        pages: vec![Page::default(); pages],
    };
    // Creep: none on a same-size sheet, 0.1 mm per sheet when trimming.
    if sheet_mode(&document, sheet)? == SheetMode::Oversize {
        document.document.creep_per_sheet = 0.1;
    }
    if request.starter_frames {
        add_starter_frames(&mut document);
    }
    Ok(document)
}

/// The starting layout: an image frame filling the type area of the front
/// and back covers, the inside front cover left blank, and one story
/// threaded through every column of pages 3 to the second-to-last.
fn add_starter_frames(document: &mut Document) {
    let count = document.pages.len();
    if count < 4 {
        return;
    }
    let size = document.page_size();
    let area = |document: &Document, page: usize| {
        let (left, top, right, bottom) = page_margins(document, page);
        (left, top, size.width - left - right, size.height - top - bottom)
    };
    for (index, page) in [(1, 0), (2, count - 1)] {
        let (x, y, w, h) = area(document, page);
        document.pages[page]
            .frames
            .push(Frame::image(&format!("image-{index}"), x, y, w, h, None));
    }
    let story = "story-1";
    document.stories.push(Story {
        id: story.to_owned(),
        paragraphs: vec![Paragraph::plain("body", "")],
    });
    let mut thread_index = 0;
    for page in 2..count - 1 {
        let (_, top, _, height) = area(document, page);
        let edges = column_edges(document, page);
        for column in edges.chunks(2) {
            thread_index += 1;
            let frame = Frame::text(
                &format!("text-{thread_index}"),
                column[0],
                top,
                column[1] - column[0],
                height,
                story,
                thread_index - 1,
            );
            document.pages[page].frames.push(frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(fold: Fold, pages: u32, sheet: Option<Size>) -> NewDocument {
        NewDocument {
            starter_frames: false,
            fold,
            custom_trim: None,
            page_count: pages,
            sheet,
            margins: default_margins(fold),
            columns: Columns { count: 1, gutter: 5.0 },
            latin_font: "Crimson Text".to_owned(),
            korean_font: "Gowun Dodum".to_owned(),
            size: 10.0,
            leading: 14.0,
        }
    }

    #[test]
    fn north_american_folds_fit_two_pages_on_their_sheet() {
        for (fold, trim, sheet) in [
            (Fold::SaddleLetter, (215.9, 279.4), (431.8, 279.4)),
            (Fold::SaddleLegal, (215.9, 355.6), (431.8, 355.6)),
            (Fold::SaddleTabloid, (279.4, 431.8), (558.8, 431.8)),
        ] {
            let document = new_document(&request(fold, 10, None)).unwrap();
            assert_eq!((document.document.trim.width, document.document.trim.height), trim);
            assert_eq!((document.document.sheet.width, document.document.sheet.height), sheet);
            assert_eq!(document.pages.len(), 12);
        }
    }

    #[test]
    fn presets_have_their_trim_and_sheet() {
        let document = new_document(&request(Fold::SaddleA5OnA4, 14, None)).unwrap();
        assert_eq!(document.document.trim, Size { width: 148.0, height: 210.0 });
        assert_eq!(document.document.sheet, Size { width: 297.0, height: 210.0 });
        assert_eq!(document.pages.len(), 16, "rounded up to a multiple of four");
        assert_eq!(document.document.creep_per_sheet, 0.0);

        let document = new_document(&request(Fold::SaddleA6OnA5, 8, None)).unwrap();
        assert_eq!(document.document.trim, Size { width: 105.0, height: 148.0 });
        assert_eq!(document.document.sheet, Size { width: 210.0, height: 148.0 });

        let document = new_document(&request(Fold::EightPageFold, 16, None)).unwrap();
        assert_eq!(document.pages.len(), 8, "always eight");
        assert_eq!(document.document.trim, Size { width: 74.25, height: 105.0 });
    }

    #[test]
    fn a_larger_sheet_turns_on_creep() {
        let a3 = Size { width: 297.0, height: 420.0 };
        let document = new_document(&request(Fold::SaddleA5OnA4, 16, Some(a3))).unwrap();
        assert_eq!(document.document.sheet, Size { width: 420.0, height: 297.0 });
        assert_eq!(document.document.creep_per_sheet, 0.1);
    }

    #[test]
    fn a_custom_trim_and_bad_values() {
        let mut custom = request(Fold::Custom, 8, None);
        custom.custom_trim = Some(Size { width: 120.0, height: 170.0 });
        let document = new_document(&custom).unwrap();
        assert_eq!(document.document.sheet, Size { width: 240.0, height: 170.0 });
        custom.size = 0.0;
        assert!(new_document(&custom).is_err());
    }

    #[test]
    fn the_starter_layout() {
        let mut with_frames = request(Fold::SaddleA5OnA4, 16, None);
        with_frames.starter_frames = true;
        with_frames.columns = Columns { count: 2, gutter: 4.0 };
        let document = new_document(&with_frames).unwrap();
        let kinds = |page: usize| -> Vec<crate::model::FrameKind> {
            document.pages[page].frames.iter().map(|frame| frame.kind).collect()
        };
        use crate::model::FrameKind::{Image, Text};
        assert_eq!(kinds(0), vec![Image], "front cover");
        assert!(kinds(1).is_empty(), "inside front cover is blank");
        for page in 2..15 {
            assert_eq!(kinds(page), vec![Text, Text], "page {} has one frame per column", page + 1);
        }
        assert_eq!(kinds(15), vec![Image], "back cover");
        // One story, threaded in page and column order.
        assert_eq!(document.stories.len(), 1);
        let order: Vec<u32> = (2..15)
            .flat_map(|page| document.pages[page].frames.iter().map(|frame| frame.thread_index.unwrap()))
            .collect();
        assert_eq!(order, (0..26).collect::<Vec<u32>>());
        // The cover image fills the type area.
        let cover = &document.pages[0].frames[0];
        assert_eq!((cover.y, cover.h), (with_frames.margins.top, 210.0 - with_frames.margins.top - with_frames.margins.bottom));

        let empty = new_document(&request(Fold::SaddleA5OnA4, 16, None)).unwrap();
        assert!(empty.pages.iter().all(|page| page.frames.is_empty()));
        assert!(empty.stories.is_empty());
    }
}
