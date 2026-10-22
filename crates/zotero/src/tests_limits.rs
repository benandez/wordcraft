//! Hostile documents and calls stay cheap: work grows with the document, not with how often a
//! crafted file or call repeats something (#3692).

use std::time::{Duration, Instant};

use wordcraft_doc::para::{NoteKind, OBJ};
use wordcraft_doc::{CharProps, Document, InlineObject, Paragraph, PartKind, Run, StoryRef, para_block};

use crate::fields;

const CITE: &str = r#"ADDIN ZOTERO_ITEM CSL_CITATION {"citationID":"k1","citationItems":[{"id":7}]}"#;

/// A paragraph of `objects`, one marker each, in one plain run.
fn objects_para(objects: Vec<InlineObject>) -> Paragraph {
    let mut p = Paragraph::new();
    p.text = std::iter::repeat_n(OBJ, objects.len()).collect();
    p.runs = vec![Run { len: p.text.len(), props: CharProps::default() }];
    p.objects = objects;
    p
}

/// `n` empty Zotero citation fields in a row.
fn citations(n: usize) -> Vec<InlineObject> {
    (0..n).flat_map(|_| [InlineObject::FieldStart { instr: CITE.into(), locked: false, dirty: false }, InlineObject::FieldEnd]).collect()
}

// Finding 12: one endnote referenced many times is listed once, not once per reference.
#[test]
fn a_note_referenced_many_times_is_listed_once() {
    let mut doc = Document::from_text("");
    let note = doc.add_part(PartKind::Endnote, vec![para_block(objects_para(citations(200)))]);
    let refs = (0..5_000).map(|_| InlineObject::NoteRef { kind: NoteKind::Endnote, id: note, custom: String::new() }).collect();
    doc.body = vec![para_block(objects_para(refs))];
    let t = Instant::now();
    let list = fields::list(&doc);
    assert_eq!(list.len(), 200, "each field once, at the note's first reference");
    assert!(list.iter().all(|f| f.start.story == StoryRef::Part(note) && f.note_index == 1));
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
}

#[test]
fn listing_stops_at_the_field_budget() {
    let mut doc = Document::from_text("");
    // Many short paragraphs: the per-paragraph object scan is another finding's (#3614).
    doc.body = (0..fields::MAX_FIELDS / 100 + 1).map(|_| para_block(objects_para(citations(100)))).collect();
    assert_eq!(fields::list(&doc).len(), fields::MAX_FIELDS);
    // Codes count too: a few huge codes fill the byte budget before the field budget.
    let big = format!("{CITE}{}", "x".repeat(fields::MAX_CODE_BYTES / 4 - 1000));
    let huge = (0..5).flat_map(|_| [InlineObject::FieldStart { instr: big.clone(), locked: false, dirty: false }, InlineObject::FieldEnd]).collect();
    doc.body = vec![para_block(objects_para(huge))];
    assert_eq!(fields::list(&doc).len(), 4);
}

// Finding 16: rich text with alternating formatting is built in one pass, not run by run with
// every earlier run merged again.
#[test]
fn rich_text_with_many_runs_is_linear() {
    let html: String = "<i>a</i>b".repeat(50_000);
    let t = Instant::now();
    let frag = crate::rich::fragment(&html, true, &CharProps::default());
    let elapsed = t.elapsed();
    let Some(wordcraft_doc::Block::Para(p)) = frag.blocks.first() else { panic!("no paragraph") };
    assert_eq!(p.text, "ab".repeat(50_000));
    assert_eq!(p.runs.len(), 100_000);
    assert_eq!(p.runs.iter().map(|r| r.len).sum::<usize>(), p.text.len());
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    // Neighbours with the same formatting are one run.
    let frag = crate::rich::fragment("<i>a</i><i>b</i>c", true, &CharProps::default());
    let Some(wordcraft_doc::Block::Para(p)) = frag.blocks.first() else { panic!("no paragraph") };
    assert_eq!((p.text.as_str(), p.runs.len()), ("abc", 2));
    assert_eq!(p.props_of_char(1).italic, Some(true));
}
