//! Lookups that once rescanned the page, the paragraph or the document for every item they
//! looked up (#3692, #3706): an input 8x the size must cost about 8x as much, not 64x.

use super::tests::{SCALE, cost_ratio, layout_time_ratio};
use super::*;
use wordcraft_doc::para::Run;
use wordcraft_doc::{Pos, Table};

fn lay(d: &Document, opts: &LayoutOptions) -> DocLayout {
    layout(d, &mut LayoutCache::new(), opts)
}

fn assert_linear(what: &str, (t1, tk, ratio): (f64, f64, f64)) {
    eprintln!("{what}: t(n) {t1:.4} s, t(8n) {tk:.4} s, ratio {ratio:.1}");
    assert!(ratio < 24.0, "{what}: 8x the input took {ratio:.1}x the time ({t1:.4} s → {tk:.4} s): not linear");
}

/// [`cost_ratio`] of `f(big)` against `f(small)`, each repeated `reps` times so the timings are
/// long against the clock's tick.
fn ratio_of<T>(reps: usize, small: &T, big: &T, f: impl Fn(&T)) -> (f64, f64, f64) {
    cost_ratio(SCALE, || (0..reps).for_each(|_| f(small)), || (0..reps).for_each(|_| f(big)))
}

/// The first laid-out body paragraph on page 0.
fn first_para(l: &DocLayout) -> Option<&ParaLayout> {
    l.pages.first()?.items.iter().find_map(|it| match it {
        Placed::Lines { para, story: StoryRef::Body, .. } => Some(para.as_ref()),
        _ => None,
    })
}

/// Finding 29: proofing text found each character's run from the paragraph's first run.
#[test]
fn proof_text_walks_the_runs_once() {
    // One run per character, every other one deleted.
    let para = |n: usize| {
        let mut p = Paragraph::with_text(&"a".repeat(n), CharProps::default());
        p.runs = (0..n).map(|i| Run { len: 1, props: CharProps { del: (i % 2 == 1).then_some(0), ..Default::default() } }).collect();
        p
    };
    assert_eq!(para::proof_text(&para(5)), "a\u{1}a\u{1}a");
    let (small, big) = (para(50_000), para(SCALE * 50_000));
    assert_linear("proof_text", ratio_of(8, &small, &big, |p| drop(std::hint::black_box(para::proof_text(p)))));
}

/// A right-to-left paragraph whose one line holds `n` hidden letters: caret offsets with no width.
fn bidi_hidden(n: usize) -> Document {
    let head = "سلام ";
    let mut d = Document::from_text(&format!("{head}{} world", "x".repeat(n)));
    let at = Pos::body(0, 0);
    d.format_paragraphs(&at, &at, &|p| p.bidi = Some(true)).unwrap();
    d.format_range(&Pos::body(0, head.len()), &Pos::body(0, head.len() + n), &|c| c.hidden = Some(true)).unwrap();
    d
}

/// Finding 5: a click on a bidirectional line and a visual caret step measured every caret
/// offset from the line's first cluster.
#[test]
fn bidi_clicks_and_caret_steps_take_linear_time() {
    let opts = LayoutOptions { show_hidden: false, ..Default::default() };
    let (ds, db) = (bidi_hidden(5000), bidi_hidden(SCALE * 5000));
    let (ls, lb) = (lay(&ds, &opts), lay(&db, &opts));
    let (Some(ps), Some(pb)) = (first_para(&ls), first_para(&lb)) else { panic!("no paragraph") };
    assert_eq!(pb.lines.len(), 1, "the hidden letters stay on one line");
    let ratio = ratio_of(32, &(ps, &ls), &(pb, &lb), |(p, l)| {
        std::hint::black_box(p.off_at_x(0, 1.0));
        std::hint::black_box(l.visual_step(&Pos::body(0, 0), true, 0));
    });
    assert_linear("bidi hit test", ratio);
}

/// The carets of a line walked in one pass are where `x_of` puts each of them, on bidirectional
/// and left-to-right lines alike.
#[test]
fn line_carets_match_x_of() {
    let docs = [bidi_hidden(20), Document::from_text("Plain left-to-right text, long enough to wrap. ".repeat(6).as_str()), {
        let mut d = Document::from_text("سلام world 123 دنیا (مرحبا) end");
        let at = Pos::body(0, 0);
        d.format_paragraphs(&at, &at, &|p| p.bidi = Some(true)).unwrap();
        d
    }];
    for d in &docs {
        for show_hidden in [false, true] {
            let l = lay(d, &LayoutOptions { show_hidden, ..Default::default() });
            let Some(p) = first_para(&l) else { panic!("no paragraph") };
            for li in 0..p.lines.len() {
                let want: Vec<_> = p.line_offsets(li).into_iter().map(|off| (off, p.x_of(li, off))).collect();
                assert!(!want.is_empty());
                assert_eq!(p.line_carets(li), want, "line {li}");
            }
        }
    }
}

/// Finding 3: autofitting a table counted its grid's columns again for every spanning cell.
#[test]
fn autofit_tables_with_a_spanning_cell_in_every_row_lay_out_in_linear_time() {
    // Rows with no cells, only a column skipped before and after them with a width: two
    // spanning entries a row for next to no layout, so counting the grid per entry dominates.
    let doc = |n: usize| {
        let mut t = Table::new(n, 1, 468.0);
        for row in t.rows.iter_mut().skip(1) {
            row.cells.clear();
            row.props.grid_before = 1;
            row.props.w_before = Some(10.0);
            row.props.grid_after = 1;
            row.props.w_after = Some(10.0);
        }
        let mut d = Document::from_text("x");
        d.insert_block(StoryRef::Body, &Path::top(0), Block::Table(t)).unwrap();
        d
    };
    assert_linear("spanning cells", layout_time_ratio(doc, 2000, &LayoutOptions::default()));
}

/// `n` continuous sections of an empty paragraph each, each with a different first page. The
/// first section names every header and footer; the others name them too, or (`inherit`) name
/// none and link to the previous section's.
fn sections(n: usize, inherit: bool) -> Document {
    use wordcraft_doc::PartKind::{Footer, Header};
    let mut d = Document::from_text(&"\n".repeat(n.saturating_sub(1)));
    let mut part = |k| d.add_part(k, vec![wordcraft_doc::para_block(Paragraph::with_text("Head", CharProps::default()))]);
    let ids = [part(Header), part(Header), part(Header), part(Footer), part(Footer), part(Footer)];
    let named = SectionProps {
        start: SectionStart::Continuous,
        title_page: true,
        headers: HeaderSet { default: Some(ids[0]), first: Some(ids[1]), even: Some(ids[2]) },
        footers: HeaderSet { default: Some(ids[3]), first: Some(ids[4]), even: Some(ids[5]) },
        ..Default::default()
    };
    let linked = SectionProps { headers: HeaderSet::default(), footers: HeaderSet::default(), ..named.clone() };
    for i in 0..n.saturating_sub(1) {
        let s = if i == 0 || !inherit { named.clone() } else { linked.clone() };
        d.para_mut(StoryRef::Body, &Path::top(i)).unwrap().section = Some(Box::new(s));
    }
    d.last_section = if inherit { linked } else { named };
    d
}

/// Finding 13: each section's headers and footers were found by walking back through the
/// sections before it, for every section and again for every page. Linked to the previous
/// section's, they must cost no more to lay out than named in each section.
#[test]
fn headers_inherited_through_many_sections_lay_out_in_linear_time() {
    let l = lay(&sections(200, true), &LayoutOptions::default());
    assert!(l.pages.iter().all(|p| !p.header.is_empty() && !p.footer.is_empty()), "the header and footer are inherited");
    let n = 8000;
    let (named, inherited) = (sections(n, false), sections(n, true));
    let lay_once = |d: &Document| drop(std::hint::black_box(lay(d, &LayoutOptions::default())));
    let (t_named, t_inherited, ratio) = cost_ratio(1, || lay_once(&named), || lay_once(&inherited));
    eprintln!("{n} sections: named {t_named:.4} s, inherited {t_inherited:.4} s, ratio {ratio:.1}");
    assert!(ratio < 2.0, "{n} inheriting sections took {ratio:.1}x the time of named ones ({t_named:.4} s → {t_inherited:.4} s)");
}

/// A header of `n` short paragraphs.
fn tall_header(n: usize) -> (Document, u32) {
    let mut d = Document::from_text("Body");
    let paras = (0..n).map(|i| wordcraft_doc::para_block(Paragraph::with_text(&format!("h{i}"), CharProps::default()))).collect();
    let id = d.add_part(wordcraft_doc::PartKind::Header, paras);
    d.last_section.headers.default = Some(id);
    (d, id)
}

/// Finding 28: selecting across a header looked through the whole page for each paragraph.
#[test]
fn selecting_a_long_header_takes_linear_time() {
    let n = 300;
    let ((ds, ids), (db, idb)) = (tall_header(n), tall_header(SCALE * n));
    let (ls, lb) = (lay(&ds, &LayoutOptions::default()), lay(&db, &LayoutOptions::default()));
    let ends = |id: u32, n: usize| (Pos::new(StoryRef::Part(id), Path::top(0), 0), Pos::new(StoryRef::Part(id), Path::top(n - 1), 2));
    let ((a, b), (c, e)) = (ends(ids, n), ends(idb, SCALE * n));
    assert!(lb.selection_rects(&db, &c, &e, 0).len() >= SCALE * n, "a rectangle per paragraph");
    let ratio = ratio_of(32, &(&ds, &ls, a, b), &(&db, &lb, c, e), |(d, l, a, b)| drop(std::hint::black_box(l.selection_rects(d, a, b, 0))));
    assert_linear("header selection", ratio);
}

/// A table of `n` rows of 60 cells, each row exactly half a point high, so every cell clips its
/// (empty) paragraph and the whole table fits on the first page.
fn clipped_table(n: usize) -> Document {
    let mut t = Table::new(n, 60, 468.0);
    for row in &mut t.rows {
        row.props.height = Some(0.5);
        row.props.height_rule = HeightRule::Exact;
    }
    let mut d = Document::from_text("x");
    d.insert_block(StoryRef::Body, &Path::top(0), Block::Table(t)).unwrap();
    d
}

/// Finding 1: drawing a line in a clipped cell looked through every clipped cell on the page.
#[test]
fn drawing_many_clipped_cells_takes_linear_time() {
    let (ds, db) = (clipped_table(16), clipped_table(SCALE * 16));
    let (ls, lb) = (lay(&ds, &LayoutOptions::default()), lay(&db, &LayoutOptions::default()));
    let clips = |d: &Document, l: &DocLayout| {
        l.pages
            .first()
            .map_or(0, |p| display::page_display(d, p, &Default::default()).iter().filter(|i| matches!(i, display::Draw::Clip { .. })).count())
    };
    assert_eq!(clips(&db, &lb), SCALE * 16 * 60, "every cell's line is clipped, all on the first page");
    let opts = display::DisplayOptions::default();
    let ratio = ratio_of(4, &(&ds, &ls), &(&db, &lb), |(d, l)| drop(std::hint::black_box(display::page_display(d, &l.pages[0], &opts))));
    assert_linear("clipped cells", ratio);
}

/// `n` words, each inserted by its own author, at a size that fits them all on the first page.
fn tracked_words(n: usize) -> Document {
    let mut d = Document::from_text("x");
    let p = Paragraph {
        text: "word ".repeat(n),
        runs: (0..n).map(|i| Run { len: 5, props: CharProps { ins: u32::try_from(i).ok(), size: Some(1.0), ..Default::default() } }).collect(),
        ..Paragraph::with_text("", CharProps::default())
    };
    d.body = vec![wordcraft_doc::para_block(p)];
    d.revisions = (0..n).map(|i| wordcraft_doc::Revision::new(wordcraft_doc::RevisionKind::Insert, format!("Author {i}"), "")).collect();
    d
}

/// Finding 30 (#3706): every tracked run ranked its author among all the document's revisions.
#[test]
fn drawing_tracked_changes_by_many_authors_takes_linear_time() {
    let (ds, db) = (tracked_words(1000), tracked_words(SCALE * 1000));
    let (ls, lb) = (lay(&ds, &LayoutOptions::default()), lay(&db, &LayoutOptions::default()));
    let opts = display::DisplayOptions { markup: true, ..Default::default() };
    assert_eq!(lb.pages.len(), 1);
    let ratio = ratio_of(4, &(&ds, &ls), &(&db, &lb), |(d, l)| drop(std::hint::black_box(display::page_display(d, &l.pages[0], &opts))));
    assert_linear("tracked runs", ratio);
}

/// Each author keeps the colour of their first revision's place among the authors.
#[test]
fn tracked_changes_colour_by_author_in_order_of_appearance() {
    let mut d = tracked_words(3);
    // Words 0 and 2 by Ana, word 1 by Ben: Ana first, Ben second.
    for (r, a) in d.revisions.iter_mut().zip(["Ana", "Ben", "Ana"]) {
        r.author = a.into();
    }
    let l = lay(&d, &LayoutOptions::default());
    let opts = display::DisplayOptions { markup: true, ..Default::default() };
    let colours: Vec<_> = display::page_display(&d, &l.pages[0], &opts)
        .into_iter()
        .filter_map(|i| if let display::Draw::Glyphs { color, .. } = i { Some(color) } else { None })
        .collect();
    let (ana, ben) = (display::revision_color(0), display::revision_color(1));
    assert_ne!(ana, ben);
    assert_eq!(colours, [ana, ben, ana]);
}

/// Finding 30 (#3706): every comment balloon ranked its author among all the comments.
#[test]
fn balloons_for_many_authors_lay_out_in_linear_time() {
    let doc = |n: usize| {
        let mut d = Document::from_text(&"word ".repeat(n));
        for i in (0..n).rev() {
            let part = d.add_part(wordcraft_doc::PartKind::Comment, vec![wordcraft_doc::para_block(Paragraph::with_text("c", CharProps::default()))]);
            let id = u32::try_from(i).unwrap();
            d.comments.insert(id, wordcraft_doc::Comment { author: format!("Author {i}"), part, ..Default::default() });
            let p = CharProps::default();
            d.insert_object(&Pos::body(0, 5 * i + 4), InlineObject::CommentEnd { id }, &p).unwrap();
            d.insert_object(&Pos::body(0, 5 * i), InlineObject::CommentStart { id }, &p).unwrap();
        }
        d
    };
    let opts = LayoutOptions { comment_balloons: true, ..Default::default() };
    assert_linear("comment balloons", layout_time_ratio(doc, 120, &opts));
}
