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

