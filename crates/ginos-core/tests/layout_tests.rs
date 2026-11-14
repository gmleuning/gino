//! Layout behaviour with the fixture fonts: Korean break modes, Latin and
//! Korean font pairing, threading across frames and overflow.

mod common;

use ginos_core::layout::{layout_story, FrameBox, FrameLayout};
use ginos_core::model::{KoreanBreak, Paragraph, Story};
use ginos_core::spike::{spike_document, BREAK_SAMPLE};

const SPACELESS: &str = "띄어쓰기없이아주길게이어지는한국어문장도하나넣어둔다";

fn line_texts(layout: &FrameLayout) -> Vec<String> {
    layout
        .lines
        .iter()
        .map(|line| line.runs.iter().map(|run| run.text.as_str()).collect::<String>())
        .collect()
}

#[test]
fn word_mode_keeps_a_spaceless_korean_phrase_on_one_line() {
    let (mut fonts, families) = common::fixture_fonts();
    let document = spike_document(&families);
    // 60 mm is narrower than the spaceless phrase at 10 pt.
    let frames = [FrameBox {
        width: 60.0,
        height: 200.0,
    }];
    let results: Vec<Vec<String>> = ["break_syllable", "break_word"]
        .iter()
        .map(|id| {
            let story = document.story(id).unwrap();
            let layout = layout_story(&mut fonts, &document, story, &frames).unwrap();
            assert_eq!(layout.overflow_characters, 0);
            line_texts(&layout.frames[0])
        })
        .collect();
    let (syllable_lines, word_lines) = (&results[0], &results[1]);

    assert!(
        !syllable_lines.iter().any(|line| line.contains(SPACELESS)),
        "syllable mode should break inside the spaceless phrase: {syllable_lines:#?}"
    );
    assert!(
        word_lines.iter().any(|line| line.contains(SPACELESS)),
        "word mode should keep the spaceless phrase on one line: {word_lines:#?}"
    );
    assert_eq!(syllable_lines.concat(), BREAK_SAMPLE);
    assert_eq!(word_lines.concat(), BREAK_SAMPLE);
}

#[test]
fn korean_runs_use_the_korean_font_and_scale() {
    let (mut fonts, families) = common::fixture_fonts();
    let document = spike_document(&families);
    let style = document.paragraph_style("thread").unwrap();
    assert_eq!(style.korean_break, KoreanBreak::Word);
    let story = document.story("thread").unwrap();
    let frames = [FrameBox {
        width: 100.0,
        height: 300.0,
    }];
    let layout = layout_story(&mut fonts, &document, story, &frames).unwrap();

    let mut latin_faces = std::collections::HashSet::new();
    let mut korean_faces = std::collections::HashSet::new();
    for line in &layout.frames[0].lines {
        for run in &line.runs {
            let is_korean = run.text.chars().any(ginos_core::layout::is_korean_char);
            if is_korean {
                korean_faces.insert(run.font.id);
                assert!((run.size - style.size * style.korean_scale).abs() < 1e-6, "Korean run size {}", run.size);
                assert!(!run.text.chars().any(|c| c.is_ascii_alphabetic()), "mixed run: {:?}", run.text);
            } else if run.text.chars().any(|c| c.is_ascii_alphabetic()) {
                latin_faces.insert(run.font.id);
                assert!((run.size - style.size).abs() < 1e-6, "Latin run size {}", run.size);
            }
        }
    }
    assert_eq!(latin_faces.len(), 1, "one Latin face");
    assert_eq!(korean_faces.len(), 1, "one Korean face");
    assert!(latin_faces.is_disjoint(&korean_faces), "Latin and Korean use different faces");
}

#[test]
fn a_story_threads_across_frames_of_different_widths() {
    let (mut fonts, families) = common::fixture_fonts();
    let document = spike_document(&families);
    let story = document.story("thread").unwrap();
    let frames = [
        FrameBox {
            width: 60.0,
            height: 85.0,
        },
        FrameBox {
            width: 100.0,
            height: 85.0,
        },
    ];
    let layout = layout_story(&mut fonts, &document, story, &frames).unwrap();
    assert_eq!(layout.overflow_characters, 0);
    let a = &layout.frames[0];
    let b = &layout.frames[1];
    assert!(!a.lines.is_empty() && !b.lines.is_empty(), "both frames hold text");

    // Nothing is lost or duplicated across the two frames.
    let full: String = story.paragraphs.iter().map(Paragraph::text).collect();
    let placed = line_texts(a).concat() + &line_texts(b).concat();
    assert_eq!(placed, full);

    // Every line stays inside its frame (trailing spaces may hang past the
    // edge), and lines in the wide frame run longer.
    let height_pt = 85.0 * 72.0 / 25.4;
    for line in a.lines.iter().chain(b.lines.iter()) {
        assert!(line.bottom <= height_pt + 0.01, "line bottom {} exceeds the frame", line.bottom);
    }
    let widest = |frame: &FrameLayout| {
        frame
            .lines
            .iter()
            .flat_map(|line| {
                line.runs.iter().flat_map(|run| {
                    run.glyphs
                        .iter()
                        .filter(move |glyph| !run.text[glyph.text_range.clone()].trim().is_empty())
                })
            })
            .map(|glyph| glyph.x + glyph.advance)
            .fold(0.0_f64, f64::max)
    };
    assert!(widest(a) <= 60.0 * 72.0 / 25.4 + 0.01);
    assert!(widest(b) > widest(a) + 20.0, "the 100 mm frame holds longer lines");
}

#[test]
fn text_that_does_not_fit_is_reported_as_overflow() {
    let (mut fonts, families) = common::fixture_fonts();
    let document = spike_document(&families);
    let story = document.story("thread").unwrap();
    let frames = [FrameBox {
        width: 60.0,
        height: 30.0,
    }];
    let layout = layout_story(&mut fonts, &document, story, &frames).unwrap();
    let total: usize = story.paragraphs.iter().map(|p| p.text().chars().count()).sum();
    let placed: usize = line_texts(&layout.frames[0]).concat().chars().count();
    assert!(layout.overflow_characters > 0);
    assert_eq!(placed + layout.overflow_characters, total);

    // A frame too short for a single line places nothing and overflows everything.
    let tiny = [FrameBox {
        width: 60.0,
        height: 1.0,
    }];
    let short_story = Story {
        id: "short".to_owned(),
        paragraphs: vec![Paragraph::plain("thread", "한 줄. One line.")],
    };
    let layout = layout_story(&mut fonts, &document, &short_story, &tiny).unwrap();
    assert!(layout.frames[0].lines.is_empty());
    assert_eq!(layout.overflow_characters, "한 줄. One line.".chars().count());
}
