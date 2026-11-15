//! Milestone 3 through the session: text editing with the caret and
//! selection, input-method composition, frame tools, threading, styles,
//! pages, new documents and imposed export.

mod common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use ginos_core::fonts::{fixture_font_folder, Fonts};
use ginos_core::model::{Columns, Fold, FrameKind, Frame, Paragraph, Size, Story};
use ginos_core::presets::{default_margins, NewDocument};
use ginos_core::session::{Motion, Session, Tool};
use ginos_core::text::TextPosition;
use hayro::hayro_syntax::Pdf;

fn sample_zine() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("documents")
        .join("sample.zine")
}

fn fixture_fonts() -> Fonts {
    let mut fonts = Fonts::without_system_fonts();
    fonts.add_folder(&fixture_font_folder());
    fonts
}

fn open_sample() -> Session {
    let mut session = Session::open(&sample_zine(), Fonts::without_system_fonts()).expect("the sample zine opens");
    session.view().unwrap();
    session
}

fn story_text(session: &Session, id: &str) -> Vec<String> {
    session
        .document()
        .story(id)
        .unwrap()
        .paragraphs
        .iter()
        .map(Paragraph::text)
        .collect()
}

/// Draws a text frame on the empty page 7 (index 6) and returns its id and story.
fn draw_text_frame(session: &mut Session, x: f64, y: f64, w: f64, h: f64) -> (String, String) {
    session.pointer_down(6, x, y, 1.0, Tool::Text, false, 1).unwrap();
    session.pointer_move(x + w, y + h).unwrap();
    let update = session.pointer_up().unwrap();
    let selection = update.selection.expect("the new frame is selected");
    assert_eq!(selection.kind, FrameKind::Text);
    assert_eq!(selection.rect, ginos_core::model::Rect::new(x, y, w, h));
    let text = update.text.expect("a new text frame is ready for typing");
    (selection.frame_id, text.story)
}

#[test]
fn typing_into_a_new_frame_with_korean_composition() {
    let mut session = open_sample();
    let (frame, story) = draw_text_frame(&mut session, 20.0, 30.0, 100.0, 60.0);
    assert!(session.is_editing_text());

    // An empty frame still shows a caret at its top, after the body style's
    // 4 mm first-line indent.
    let update = session.insert_text("").unwrap();
    let caret = update.text.unwrap().caret.expect("a caret in an empty frame");
    assert_eq!((caret.page, caret.top), (6, 30.0));
    assert!((caret.x - 24.0).abs() < 1e-6, "caret at {}", caret.x);

    for letter in ["H", "i", " "] {
        session.insert_text(letter).unwrap();
    }
    // The Windows Korean input method composes 한 as ㅎ, 하, 한, then commits.
    for composing in ["ㅎ", "하", "한"] {
        let update = session.set_preedit(composing).unwrap();
        let text = update.text.unwrap();
        assert!(text.composing);
        assert!(update.patches.iter().any(|patch| patch.frame_id == frame), "the frame redraws while composing");
        assert!(update.patches[0].svg.contains("<line"), "the composing syllable is underlined");
    }
    assert_eq!(story_text(&session, &story), vec!["Hi "], "composition is not in the document yet");
    session.commit_text("한").unwrap();
    session.set_preedit("ㄱ").unwrap();
    session.set_preedit("그").unwrap();
    session.set_preedit("글").unwrap();
    let update = session.commit_text("글").unwrap();
    assert_eq!(story_text(&session, &story), vec!["Hi 한글"]);
    let text = update.text.unwrap();
    assert!(!text.composing);
    let caret = text.caret.unwrap();
    assert!(caret.x > 25.0, "the caret sits after the text, at {}", caret.x);

    // Typing is one undo step; the frame and story stay.
    let update = session.undo().unwrap();
    assert_eq!(story_text(&session, &story), vec![""]);
    assert!((update.text.unwrap().caret.unwrap().x - 24.0).abs() < 1e-6);
    session.redo().unwrap();
    assert_eq!(story_text(&session, &story), vec!["Hi 한글"]);
}

#[test]
fn enter_backspace_arrows_and_selection() {
    let mut session = open_sample();
    let (_, story) = draw_text_frame(&mut session, 20.0, 30.0, 100.0, 60.0);
    session.insert_text("first line").unwrap();
    session.insert_text("\n").unwrap();
    session.insert_text("둘째 줄").unwrap();
    assert_eq!(story_text(&session, &story), vec!["first line", "둘째 줄"]);

    // Backspace at a paragraph start joins the paragraphs.
    session.move_caret(Motion::LineStart, false).unwrap();
    session.delete(true, false).unwrap();
    assert_eq!(story_text(&session, &story), vec!["first line둘째 줄"]);
    session.insert_text("\n").unwrap();

    // Up from the second paragraph lands in the first; Shift extends.
    session.move_caret(Motion::Up, false).unwrap();
    session.move_caret(Motion::LineStart, false).unwrap();
    let update = session.move_caret(Motion::WordRight, true).unwrap();
    let text = update.text.unwrap();
    assert_eq!(text.selected_text, "first");
    assert_eq!(text.selection.len(), 1);
    session.insert_text("1st").unwrap();
    assert_eq!(story_text(&session, &story), vec!["1st line", "둘째 줄"]);

    // Select all and delete leaves one empty paragraph.
    session.select_all().unwrap();
    let update = session.delete(false, false).unwrap();
    assert_eq!(story_text(&session, &story), vec![""]);
    assert!(update.text.unwrap().caret.is_some());
}

#[test]
fn double_click_edits_an_existing_story_and_clicks_place_the_caret() {
    let mut session = open_sample();
    // Double click inside essay_1 on page 2 (index 1).
    session.pointer_down(1, 40.0, 20.0, 1.0, Tool::Select, false, 1).unwrap();
    session.pointer_up().unwrap();
    let update = session.pointer_down(1, 40.0, 20.0, 1.0, Tool::Select, false, 2).unwrap();
    let text = update.text.expect("double click starts editing");
    assert_eq!(text.story, "essay");
    let caret = text.caret.unwrap();
    assert_eq!(caret.page, 1);
    assert!(caret.top <= 20.0 && caret.bottom >= 18.0, "the caret is on the clicked line: {caret:?}");

    // Shift-click further down extends a selection across lines.
    let update = session.pointer_down(1, 60.0, 50.0, 1.0, Tool::Select, true, 1).unwrap();
    session.pointer_up().unwrap();
    let text = update.text.unwrap();
    assert!(text.selection.len() > 3, "a selection over several lines: {}", text.selection.len());
    assert!(!text.selected_text.is_empty());

    // A press outside the story ends editing and selects normally.
    let update = session.pointer_down(2, 50.0, 150.0, 1.0, Tool::Select, false, 1).unwrap();
    session.pointer_up().unwrap();
    assert!(update.text.is_none());
    assert_eq!(update.selection.unwrap().frame_id, "picture");
}

#[test]
fn threading_two_frames_and_breaking_the_thread() {
    let mut session = open_sample();
    let (first, story) = draw_text_frame(&mut session, 15.0, 15.0, 50.0, 12.0);
    let long = "A sentence long enough to fill a small frame and overflow it. 한국어 문장도 넉넉하게 넣어서 넘치게 한다. ".repeat(4);
    let update = session.insert_text(&long).unwrap();
    let overflow = update.overflows.iter().find(|o| o.story == story).expect("the small frame overflows");
    assert_eq!((overflow.page, overflow.frame.as_str()), (6, first.as_str()));

    session.end_editing().unwrap();
    let (second, second_story) = draw_text_frame(&mut session, 15.0, 60.0, 110.0, 100.0);
    session.end_editing().unwrap();
    session.select(6, Some(&first)).unwrap();
    let update = session.thread_to(6, 50.0, 100.0).unwrap();
    assert!(update.overflows.iter().all(|o| o.story != story), "the second frame takes the rest");
    let selection = update.selection.unwrap();
    assert_eq!((selection.thread_position, selection.thread_length), (1, 2));
    assert!(session.document().story(&second_story).is_err(), "the empty story of the target is gone");
    assert!(update.patches.iter().any(|patch| patch.frame_id == second));

    // Threading into itself is refused with a sentence.
    let error = session.thread_to(6, 20.0, 20.0).unwrap_err();
    assert_eq!(error.to_string(), "that frame is already part of this story");

    let update = session.break_thread_after().unwrap();
    assert!(update.overflows.iter().any(|o| o.story == story), "the text overflows again");
    session.undo().unwrap();
    session.undo().unwrap();
    assert!(session.document().story(&second_story).is_ok(), "undo brings the story back");
}

#[test]
fn paragraph_and_character_styles() {
    let mut session = open_sample();
    let (_, story) = draw_text_frame(&mut session, 20.0, 30.0, 100.0, 60.0);
    session.insert_text("Heading\nBody text with emphasis").unwrap();

    session.create_paragraph_style("heading", "body").unwrap();
    let mut heading = session.document().paragraph_style("heading").unwrap().clone();
    heading.size = 18.0;
    heading.leading = 22.0;
    heading.korean_scale = 0.9;
    session.update_paragraph_style(heading).unwrap();
    session.move_caret(Motion::StoryStart, false).unwrap();
    let update = session.apply_paragraph_style("heading").unwrap();
    assert_eq!(update.text.unwrap().paragraph_style, "heading");
    let paragraphs = &session.document().story(&story).unwrap().paragraphs;
    assert_eq!((paragraphs[0].style.as_str(), paragraphs[1].style.as_str()), ("heading", "body"));

    // A missing font is refused with a sentence and changes nothing.
    let mut broken = session.document().paragraph_style("heading").unwrap().clone();
    broken.latin_font = "No Such Font".to_owned();
    assert_eq!(
        session.update_paragraph_style(broken).unwrap_err().to_string(),
        "the font family \"No Such Font\" is not available"
    );

    session.create_character_style("emphasis").unwrap();
    session.move_caret(Motion::StoryEnd, false).unwrap();
    session.move_caret(Motion::WordLeft, true).unwrap();
    let update = session.apply_character_style(Some("emphasis")).unwrap();
    assert_eq!(update.text.unwrap().selected_text, "emphasis", "the selection stays");
    let runs = &session.document().story(&story).unwrap().paragraphs[1].runs;
    assert_eq!(runs.last().unwrap().character_style.as_deref(), Some("emphasis"));
    assert_eq!(runs.last().unwrap().text, "emphasis");

    session.undo().unwrap();
    let runs = &session.document().story(&story).unwrap().paragraphs[1].runs;
    assert_eq!(runs.len(), 1);
}

#[test]
fn frames_are_drawn_restacked_and_deleted_with_undo() {
    let mut session = open_sample();
    for (tool, kind) in [(Tool::Rectangle, FrameKind::Rectangle), (Tool::Image, FrameKind::Image), (Tool::Rule, FrameKind::Rule)] {
        let pressed = session.pointer_down(6, 30.0, 40.0, 1.0, tool, false, 1).unwrap();
        assert!(pressed.page_frames.iter().any(|page| page.page == 6), "page 7's frame list changes");
        let update = session.pointer_up().unwrap();
        let selection = update.selection.unwrap();
        assert_eq!(selection.kind, kind);
    }
    let frames: Vec<String> = session.document().pages[6].frames.iter().map(|f| f.id.clone()).collect();
    assert_eq!(frames, vec!["box-1", "image-1", "rule-1"]);
    // A click on a rule's hairline selects it.
    let update = session.pointer_down(6, 50.0, 40.4, 1.0, Tool::Select, false, 1).unwrap();
    session.pointer_up().unwrap();
    assert_eq!(update.selection.unwrap().frame_id, "rule-1");

    session.arrange(ginos_core::session::Arrange::Back).unwrap();
    assert_eq!(session.document().pages[6].frames[0].id, "rule-1");
    session.delete_frame().unwrap();
    assert_eq!(session.document().pages[6].frames.len(), 2);
    session.undo().unwrap();
    session.undo().unwrap();
    let frames: Vec<String> = session.document().pages[6].frames.iter().map(|f| f.id.clone()).collect();
    assert_eq!(frames, vec!["box-1", "image-1", "rule-1"]);
}

#[test]
fn pages_and_settings_change_the_view() {
    let mut session = open_sample();
    let update = session.add_pages(4).unwrap();
    assert!(update.view_changed);
    assert_eq!(session.view().unwrap().pages.len(), 12);
    session.remove_pages(4).unwrap();
    assert_eq!(session.document().pages.len(), 8);
    assert_eq!(session.document().document.page_count, 8);

    let original_top = session.document().document.margins.top;
    let mut settings = session.document().document.clone();
    settings.margins.top = 20.0;
    settings.sheet = Size { width: 420.0, height: 297.0 };
    let update = session.set_settings(settings).unwrap();
    assert!(update.view_changed);
    let view = session.view().unwrap();
    assert_eq!(view.sheet_mode, Some(ginos_core::impose::SheetMode::Oversize));
    let mut too_small = session.document().document.clone();
    too_small.sheet = Size { width: 200.0, height: 140.0 };
    assert!(session.set_settings(too_small).is_err());
    session.undo().unwrap();
    assert_eq!(session.document().document.margins.top, original_top);
}

fn temporary_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("ginos-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

fn request(fold: Fold, pages: u32, sheet: Option<Size>) -> NewDocument {
    NewDocument {
        fold,
        custom_trim: None,
        page_count: pages,
        sheet,
        margins: default_margins(fold),
        columns: Columns { count: 2, gutter: 4.0 },
        latin_font: "Crimson Text".to_owned(),
        korean_font: "Gowun Dodum".to_owned(),
        size: 9.5,
        leading: 13.0,
    }
}

#[test]
fn a_new_document_is_created_saved_and_exported_imposed() {
    let parent = temporary_folder("new");
    let path = parent.join("첫 진");
    let mut session = Session::create(&path, &request(Fold::SaddleA5OnA4, 16, None), fixture_fonts()).unwrap();
    assert_eq!(session.zine.folder, parent.join("첫 진.zine"));
    assert!(session.zine.folder.join("document.json").is_file());
    let view = session.view().unwrap();
    assert_eq!(view.title, "첫 진");
    assert_eq!(view.pages.len(), 16);
    assert_eq!(view.sheet_mode, Some(ginos_core::impose::SheetMode::SameSize));

    // Fill page 1 so the imposed PDF has something on it.
    session.pointer_down(0, 12.0, 12.0, 1.0, Tool::Text, false, 1).unwrap();
    session.pointer_move(136.0, 195.0).unwrap();
    session.pointer_up().unwrap();
    session.insert_text("표지 Cover").unwrap();
    session.save().unwrap();
    let reopened = Session::open(&session.zine.folder, fixture_fonts()).unwrap();
    assert_eq!(reopened.document().stories.len(), 1);

    let out = common::output_folder("imposed");
    let a4 = out.join("new-a4.pdf");
    session.export_imposed_pdf(&a4, None).unwrap();
    let pdf = Pdf::new(std::fs::read(&a4).unwrap()).unwrap();
    assert_eq!(pdf.pages().len(), 8, "four sheets, front and back");
    let a3 = out.join("new-a3.pdf");
    session.export_imposed_pdf(&a3, Some(Size { width: 297.0, height: 420.0 })).unwrap();
    assert_eq!(Pdf::new(std::fs::read(&a3).unwrap()).unwrap().pages().len(), 8);

    // Creating into a folder that holds files is refused.
    let error = match Session::create(&path, &request(Fold::SaddleA5OnA4, 8, None), fixture_fonts()) {
        Err(error) => error,
        Ok(_) => panic!("creating over an existing document must fail"),
    };
    assert!(error.to_string().contains("already exists"), "{error}");
    let _ = std::fs::remove_dir_all(&parent);
}

/// Typing must feel immediate on a 32-page document with one long threaded
/// story. Each keystroke re-lays out only the edited paragraph (the rest come
/// from the cache) and renders only frames whose drawing changed.
#[test]
fn typing_in_a_long_threaded_story_is_fast() {
    let parent = temporary_folder("timing");
    let mut session = Session::create(&parent.join("long"), &request(Fold::SaddleA5OnA4, 32, None), fixture_fonts()).unwrap();
    let sentence = "Zines are small books made by hand. 진은 손으로 만드는 작은 책이다. ";
    let paragraphs: Vec<Paragraph> = (0..90).map(|i| Paragraph::plain("body", &format!("{i}. {}", sentence.repeat(12)))).collect();
    let characters: usize = paragraphs.iter().map(|p| p.text().chars().count()).sum();
    {
        let document = &mut session.zine.document;
        document.stories.push(Story {
            id: "long".to_owned(),
            paragraphs,
        });
        for (index, page) in document.pages.iter_mut().enumerate() {
            page.frames.push(Frame::text(&format!("t{index}"), 12.0, 12.0, 126.0, 183.0, "long", index as u32));
        }
    }
    let started = Instant::now();
    let view = session.view().unwrap();
    let open_time = started.elapsed();
    let glyph_patches: usize = view.update.patches.len();
    assert_eq!(glyph_patches, 32);

    session.edit_frame(10, "t10").unwrap();
    session.move_caret(Motion::StoryStart, false).unwrap();
    for _ in 0..30 {
        session.move_caret(Motion::Down, false).unwrap();
    }
    let mut times = Vec::new();
    let mut patched = 0;
    for letter in "typing here 여기에".chars() {
        let started = Instant::now();
        let update = session.insert_text(&letter.to_string()).unwrap();
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        patched = patched.max(update.patches.len());
    }
    times.sort_by(f64::total_cmp);
    let median = times[times.len() / 2];
    let worst = times[times.len() - 1];
    let (hits, misses) = session.fonts_cache_statistics();
    println!(
        "32 pages, {characters} characters in one thread: opened in {:.0} ms; keystroke median {median:.1} ms, worst {worst:.1} ms; at most {patched} frames redrawn per keystroke; cache {hits} hits, {misses} misses",
        open_time.as_secs_f64() * 1000.0
    );
    assert!(patched <= 32);
    assert!(median < 150.0, "a keystroke took {median:.1} ms");
    let _ = std::fs::remove_dir_all(&parent);
}
