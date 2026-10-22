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

