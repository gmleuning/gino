//! Reader spreads: page 1 alone, then pairs, then the last page alone when
//! the count is even. Used by the canvas and by reader-spread export.

use crate::model::Document;
use crate::scene::{FrameScene, Page, Scene};

/// Page indices of each spread, in reading order.
pub fn reader_spreads(page_count: usize) -> Vec<Vec<usize>> {
    let mut spreads = Vec::new();
    if page_count == 0 {
        return spreads;
    }
    spreads.push(vec![0]);
    let mut index = 1;
    while index < page_count {
        if index + 1 < page_count {
            spreads.push(vec![index, index + 1]);
        } else {
            spreads.push(vec![index]);
        }
        index += 2;
    }
    spreads
}

/// Where each page of a spread sits, as (page index, x offset in millimetres).
/// A single right-hand page sits to the right of an empty left half so the
/// cover and the back lie where the reader expects them.
pub fn spread_offsets(document: &Document, spread: &[usize]) -> (Vec<(usize, f64)>, f64, f64) {
    let size = document.page_size();
    if spread.len() == 1 {
        let page = spread[0];
        if Document::is_left_page(page) {
            (vec![(page, 0.0)], size.width, size.height)
        } else {
            (vec![(page, 0.0)], size.width, size.height)
        }
    } else {
        let mut x = 0.0;
        let mut offsets = Vec::new();
        for &page in spread {
            offsets.push((page, x));
            x += size.width;
        }
        (offsets, x, size.height)
    }
}

/// One scene page holding the pages of a spread side by side.
pub fn spread_page(document: &Document, scene: &Scene, spread: &[usize]) -> Page {
    let (offsets, width, height) = spread_offsets(document, spread);
    let mut frames = Vec::new();
    for (page_index, dx) in offsets {
        if let Some(page) = scene.pages.get(page_index) {
            for frame in &page.frames {
                frames.push(FrameScene {
                    id: format!("p{page_index}-{}", frame.id),
                    items: frame.items.iter().map(|item| item.translated(dx, 0.0)).collect(),
                });
            }
        }
    }
    Page {
        width,
        height,
        frames,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreads_follow_reading_order() {
        assert_eq!(reader_spreads(0), Vec::<Vec<usize>>::new());
        assert_eq!(reader_spreads(1), vec![vec![0]]);
        assert_eq!(reader_spreads(4), vec![vec![0], vec![1, 2], vec![3]]);
        assert_eq!(reader_spreads(5), vec![vec![0], vec![1, 2], vec![3, 4]]);
        assert_eq!(
            reader_spreads(8),
            vec![vec![0], vec![1, 2], vec![3, 4], vec![5, 6], vec![7]]
        );
    }
}
