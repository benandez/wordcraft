//! Hostile documents whose layout work or memory would otherwise grow far past their size: each
//! is bounded by a cap, and real documents stay well under it.

use super::*;
use wordcraft_doc::props::HeightRule;
use wordcraft_doc::table::{MAX_COLS, Table};
use wordcraft_doc::{Block, PartKind, Pos, TextBody, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

/// A `w` × `h` text box showing `text`, inline at the end of body paragraph `para`; its story id.
fn text_box(d: &mut Document, para: usize, text: &str, w: f32, h: f32) -> u32 {
    let id = d.add_part(PartKind::TextBox, vec![para_block(Paragraph::with_text(text, Default::default()))]);
    if let Some(p) = d.parts.get_mut(&id) {
        p.body = TextBody::default();
    }
    let shape = InlineObject::Shape {
        kind: wordcraft_doc::para::ShapeKind::TextBox,
        w,
        h,
        fill: None,
        stroke: None,
        stroke_width: 1.0,
        float: Default::default(),
        story: Some(id),
        effects: Default::default(),
        freeform: None,
        extra: Default::default(),
    };
    let off = d.para(StoryRef::Body, &Path::top(para)).map_or(0, |p| p.len());
    d.insert_object(&Pos::body(para, off), shape, &Default::default()).unwrap();
    id
}

#[test]
fn a_linked_chain_lays_its_story_out_at_a_bounded_number_of_widths() {
    // Forty linked boxes, each a line deep and each a different width, over a long story: every
    // width would lay the whole story out again. Past the cap, the chain's later boxes show nothing.
    let n = 40;
    let mut d = Document::from_text(&vec!["p"; n].join("\n"));
    let story = "Words that run on and on through every box of the chain. ".repeat(200);
    let ids: Vec<u32> = (0..n).map(|i| text_box(&mut d, i, if i == 0 { &story } else { "" }, 100.0 + 3.0 * i as f32, 28.0)).collect();
    for w in ids.windows(2) {
        d.parts.get_mut(&w[0]).unwrap().body.next = Some(w[1]);
    }
    let l = lay(&d);
    let shown =
        l.pages.iter().flat_map(|p| &p.items).filter(|it| matches!(it, Placed::Lines { story: StoryRef::Part(s), .. } if *s == ids[0])).count();
    assert!(shown > 1, "the first boxes still show the story: {shown}");
    assert!(shown <= textbox::MAX_FLOW_WIDTHS, "{shown} boxes laid the story out");
}

#[test]
fn repeated_header_items_are_capped_across_the_table() {
    // An exact-height header row keeps thousands of overflowing paragraphs without growing, and
    // repeats on each of hundreds of pages: every page would copy all of them.
    let mut t = Table::new(201, 1, 468.0);
    t.rows[0].props.header = true;
    t.rows[0].props.height = Some(20.0);
    t.rows[0].props.height_rule = HeightRule::Exact;
    t.rows[0].cells[0].blocks = (0..3000).map(|_| para_block(Paragraph::with_text("x", Default::default()))).collect();
    for row in t.rows.iter_mut().skip(1) {
        row.props.height = Some(600.0);
        row.props.height_rule = HeightRule::Exact;
    }
    let mut d = Document::from_text("");
    d.insert_block(StoryRef::Body, &Path::top(1), Block::Table(t)).unwrap();
    let l = lay(&d);
    assert!(l.pages.len() >= 200, "{} pages", l.pages.len());
    let items: usize = l.pages.iter().map(|p| p.items.len()).sum();
    // The first page's header and each page's body row come on top of the repeats.
    assert!(items <= MAX_HEADER_ITEMS + 3000 + 50 * l.pages.len(), "{items} items placed");
    // An ordinary table's header still repeats on every page.
    let mut t = Table::new(80, 2, 468.0);
    t.rows[0].props.header = true;
    let mut d = Document::from_text("");
    d.insert_block(StoryRef::Body, &Path::top(1), Block::Table(t)).unwrap();
    let l = lay(&d);
    assert!(l.pages.len() >= 2);
    for p in &l.pages {
        assert!(p.items.iter().any(|i| matches!(i, Placed::Cell { row: 0, .. })), "the header repeats");
    }
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
