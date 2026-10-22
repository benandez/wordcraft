//! Hostile documents whose layout work or memory would otherwise grow far past their size: each
//! is bounded by a cap, and real documents stay well under it.

use super::*;
use wordcraft_doc::Block;
use wordcraft_doc::table::{MAX_COLS, Table};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

#[test]
fn a_huge_grid_after_is_bounded_like_the_docx_readers() {
    // A native document can say a row leaves four billion columns empty after its cells.
    let mut t = Table::new(1, 2, 468.0);
    t.rows[0].props.grid_after = u32::MAX;
    t.grid.clear();
    assert!(t.cols() <= 2 + MAX_COLS, "{} columns", t.cols());
    let mut d = Document::from_text("");
    d.insert_block(StoryRef::Body, &Path::top(1), Block::Table(t)).unwrap();
    let l = lay(&d);
    assert!(l.pages[0].items.iter().any(|i| matches!(i, Placed::Cell { row: 0, cell: 1, .. })));
}
