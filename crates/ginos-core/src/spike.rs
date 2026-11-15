//! The milestone 1 spike document: mixed Korean and English in a 100 mm frame
//! in both Korean break modes, and one story threaded across two frames of
//! different widths. Shared by `ginos-cli spike` and the raster test.

use crate::model::{
    Alignment, Columns, Document, Fold, FontSettings, Frame, KoreanBreak, Margins, MasterPage,
    Page, Paragraph, ParagraphStyle, Settings, Size, Story, FORMAT_VERSION,
};
use crate::units::paper;

/// Family names of the two fonts the spike is set in.
#[derive(Clone, Debug)]
pub struct SpikeFamilies {
    pub latin: String,
    pub korean: String,
}

pub const BREAK_SAMPLE: &str = "서울의 겨울밤은 길고 조용하다. Gino's sets Korean and English together on one page, \
한국어와 영어를 한 페이지에 나란히 놓는다. 줄바꿈이 음절 사이에서 일어나는지, 아니면 단어 사이에서만 \
일어나는지 두 상자를 비교해 보자. 띄어쓰기없이아주길게이어지는한국어문장도하나넣어둔다. The quick brown fox \
jumps over the lazy dog; 다람쥐 헌 쳇바퀴에 타고파. 끝.";

pub const THREAD_PARAGRAPHS: [&str; 4] = [
    "진(zine)은 작은 책이다. A zine is a small book: a few sheets, folded and stapled, made by one person \
or a few friends and handed around rather than sold. 한 사람이, 혹은 몇몇 친구가 만들어서 팔기보다는 \
나누어 주는 책이다.",
    "Gino's starts from the fold, not from the page. 접는 방식을 먼저 고르면 판형과 쪽수, 면 배치가 그 선택을 \
따라온다. Choose A5 on A4, and the program knows the trim size, that pages come in fours, and which page \
sits beside which on the printed sheet.",
    "이 글은 한 상자에서 다음 상자로 이어진다. This story begins in a narrow frame and continues in a wider \
one, and the line breaks in each frame are computed for that frame's width. 화면에 보이는 줄바꿈이 곧 \
인쇄되는 줄바꿈이다.",
    "The screen never lies about the print. 코어가 모든 글리프를 한 번 배치하고, 같은 위치가 화면의 SVG와 \
PDF에 그대로 쓰인다. Nothing on the canvas is a guess about the paper.",
];

pub fn spike_document(families: &SpikeFamilies) -> Document {
    let style = |id: &str, size: f64, leading: f64, korean_break: KoreanBreak, korean_scale: f64| {
        ParagraphStyle {
            id: id.to_owned(),
            latin_font: families.latin.clone(),
            korean_font: families.korean.clone(),
            korean_scale,
            size,
            leading,
            tracking: 0.0,
            alignment: Alignment::Left,
            space_before: 0.0,
            space_after: 0.0,
            first_line_indent: 0.0,
            korean_break,
        }
    };
    let mut thread_style = style("thread", 10.0, 15.0, KoreanBreak::Word, 0.95);
    thread_style.first_line_indent = 4.0;
    thread_style.space_after = 1.5;

    let frame_grey = 0.55;
    let edge = 0.2;

    Document {
        format_version: FORMAT_VERSION,
        document: Settings {
            fold: Fold::Custom,
            trim: Size {
                width: paper::A4.0,
                height: paper::A4.1,
            },
            sheet: Size {
                width: paper::A4.0,
                height: paper::A4.1,
            },
            bleed: 0.0,
            printer_margin: 5.0,
            margins: Margins {
                top: 15.0,
                bottom: 15.0,
                inside: 20.0,
                outside: 20.0,
            },
            columns: Columns {
                count: 1,
                gutter: 5.0,
            },
            creep_per_sheet: 0.0,
            page_count: 1,
        },
        fonts: FontSettings::default(),
        paragraph_styles: vec![
            style("syllable", 10.0, 15.0, KoreanBreak::Syllable, 1.0),
            style("word", 10.0, 15.0, KoreanBreak::Word, 1.0),
            thread_style,
            style("label", 7.0, 9.0, KoreanBreak::Word, 1.0),
        ],
        character_styles: Vec::new(),
        stories: vec![
            Story {
                id: "break_syllable".to_owned(),
                paragraphs: vec![Paragraph::plain("syllable", BREAK_SAMPLE)],
            },
            Story {
                id: "break_word".to_owned(),
                paragraphs: vec![Paragraph::plain("word", BREAK_SAMPLE)],
            },
            Story {
                id: "thread".to_owned(),
                paragraphs: THREAD_PARAGRAPHS
                    .iter()
                    .map(|text| Paragraph::plain("thread", text))
                    .collect(),
            },
            Story {
                id: "label_syllable".to_owned(),
                paragraphs: vec![Paragraph::plain(
                    "label",
                    "1. Korean break: syllable (default Unicode rules), 100 mm frame",
                )],
            },
            Story {
                id: "label_word".to_owned(),
                paragraphs: vec![Paragraph::plain(
                    "label",
                    "2. Korean break: word (keep-all), 100 mm frame",
                )],
            },
            Story {
                id: "label_thread".to_owned(),
                paragraphs: vec![Paragraph::plain(
                    "label",
                    "3. One story threaded from frame A (60 mm wide) into frame B (100 mm wide); word mode, Korean at 95 %",
                )],
            },
        ],
        master: MasterPage::default(),
        pages: vec![Page {
            frames: vec![
                Frame::text("label_syllable", 20.0, 16.0, 170.0, 5.0, "label_syllable", 0),
                Frame::rectangle("edge_syllable", 20.0, 22.0, 100.0, 70.0, edge, frame_grey),
                Frame::text("break_syllable", 20.0, 22.0, 100.0, 70.0, "break_syllable", 0),
                Frame::text("label_word", 20.0, 100.0, 170.0, 5.0, "label_word", 0),
                Frame::rectangle("edge_word", 20.0, 106.0, 100.0, 70.0, edge, frame_grey),
                Frame::text("break_word", 20.0, 106.0, 100.0, 70.0, "break_word", 0),
                Frame::text("label_thread", 20.0, 186.0, 170.0, 5.0, "label_thread", 0),
                Frame::rectangle("edge_a", 20.0, 192.0, 60.0, 85.0, edge, frame_grey),
                Frame::text("thread_a", 20.0, 192.0, 60.0, 85.0, "thread", 0),
                Frame::rectangle("edge_b", 90.0, 192.0, 100.0, 85.0, edge, frame_grey),
                Frame::text("thread_b", 90.0, 192.0, 100.0, 85.0, "thread", 1),
                Frame::rule("link", 80.0, 234.5, 10.0, 0.0, 0.3, 0.55),
            ],
        }],
    }
}

/// A test document for checking a fold on paper: every page shows its number
/// large, the word "top" at its top edge, and a thin border just inside the
/// trim. Printed imposed, cut and folded, the pages must read 1 to n, upright.
pub fn fold_test_document(families: &SpikeFamilies, fold: Fold, pages: u32) -> Result<Document, crate::Error> {
    let request = crate::presets::NewDocument {
        starter_frames: false,
        fold,
        custom_trim: None,
        page_count: pages,
        sheet: None,
        margins: crate::presets::default_margins(fold),
        columns: Columns { count: 1, gutter: 5.0 },
        latin_font: families.latin.clone(),
        korean_font: families.korean.clone(),
        size: 10.0,
        leading: 12.0,
    };
    let mut document = crate::presets::new_document(&request)?;
    let trim = document.document.trim;
    let number_size = (trim.height * 0.35 / 25.4 * 72.0).min(160.0);
    let base = document.paragraph_styles[0].clone();
    document.paragraph_styles.push(ParagraphStyle {
        id: "number".to_owned(),
        size: number_size,
        leading: number_size * 1.1,
        alignment: Alignment::Center,
        ..base.clone()
    });
    document.paragraph_styles.push(ParagraphStyle {
        id: "top".to_owned(),
        size: 9.0,
        leading: 11.0,
        alignment: Alignment::Center,
        ..base
    });
    let count = document.pages.len();
    for (index, page) in document.pages.iter_mut().enumerate() {
        let number = index + 1;
        let number_story = format!("number_{number}");
        let top_story = format!("top_{number}");
        let number_height = number_size / 72.0 * 25.4 * 1.2;
        page.frames.push(Frame::rectangle(&format!("border_{number}"), 2.0, 2.0, trim.width - 4.0, trim.height - 4.0, 0.2, 0.6));
        page.frames.push(Frame::text(&format!("top_{number}"), 4.0, 4.0, trim.width - 8.0, 6.0, &top_story, 0));
        page.frames.push(Frame::text(
            &format!("number_{number}"),
            4.0,
            (trim.height - number_height) / 2.0,
            trim.width - 8.0,
            number_height,
            &number_story,
            0,
        ));
        document.stories.push(Story {
            id: top_story,
            paragraphs: vec![Paragraph::plain("top", &format!("top · 위 · page {number} of {count}"))],
        });
        document.stories.push(Story {
            id: number_story,
            paragraphs: vec![Paragraph::plain("number", &number.to_string())],
        });
    }
    Ok(document)
}
