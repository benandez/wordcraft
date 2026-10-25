use super::*;
use wordcraft_doc::props::{Border, Borders, HeightRule, LineSpacing, ParaProps};
use wordcraft_doc::{Paragraph, Table, para_block};

const BODY_HEIGHT: f32 = 210.0;
const PAGE_MARGIN: f32 = 20.0;
const FILLER_HEIGHT: f32 = 60.0;
const LINE_HEIGHT: f32 = 10.0;
const DATA_LINES: usize = 40;
const HEADER_HEIGHT: f32 = 20.0;

#[derive(Clone, Copy)]
enum Header {
    None,
    Repeat,
}

fn minimum_probe(minimum: f32, inset: f32, header: Header) -> (DocLayout, usize) {
    let props = ParaProps {
        line_spacing: Some(LineSpacing::Exactly(LINE_HEIGHT)),
        space_before: Some(0.0),
        space_after: Some(0.0),
        widow_control: Some(false),
        keep_lines: Some(false),
        keep_next: Some(false),
        ..Default::default()
    };
    let mut doc = Document::from_text("Filler");
    doc.styles.default_chr.font = Some("Source Sans 3".into());
    doc.styles.default_chr.size = Some(12.0);
    doc.last_section.page_h = BODY_HEIGHT + 2.0 * PAGE_MARGIN;
    doc.last_section.margin_top = PAGE_MARGIN;
    doc.last_section.margin_bottom = PAGE_MARGIN;
    doc.para_mut(StoryRef::Body, &wordcraft_doc::Path::top(0)).unwrap().props =
        ParaProps { line_spacing: Some(LineSpacing::Exactly(FILLER_HEIGHT)), ..props.clone() };
    let data_row = match header {
        Header::None => 0,
        Header::Repeat => 1,
    };
    let mut table = Table::new(data_row + 1, 1, 300.0);
    table.props.style = None;
    table.props.cell_margins = Some([inset, 0.0, inset, 0.0]);
    let none = Some(Border::default());
    table.props.borders = Some(Borders { top: none, bottom: none, left: none, right: none, between: none, inside_v: none });
    if data_row == 1 {
        table.rows[0].props.header = true;
        table.rows[0].props.height = Some(HEADER_HEIGHT);
        table.rows[0].props.height_rule = HeightRule::AtLeast;
        let mut paragraph = Paragraph::with_text("Header", Default::default());
        paragraph.props = props.clone();
        table.rows[0].cells[0].blocks = vec![para_block(paragraph)];
    }
    table.rows[data_row].props.height = Some(minimum);
    table.rows[data_row].props.height_rule = HeightRule::AtLeast;
    let mut paragraph = Paragraph::with_text(&vec!["Data"; DATA_LINES].join("\n"), Default::default());
    paragraph.props = props;
    table.rows[data_row].cells[0].blocks = vec![para_block(paragraph)];
    doc.body.push(std::sync::Arc::new(Block::Table(table)));
    (layout(&doc, &mut LayoutCache::new(), &LayoutOptions::default()), data_row)
}

fn assert_minimum_start(minimum: f32, inset: f32, header: Header, expected: usize) {
    let (laid, data_row) = minimum_probe(minimum, inset, header);
    let lines: Vec<_> = laid
        .pages
        .iter()
        .flat_map(|page| &page.items)
        .flat_map(|item| match item {
            Placed::Lines { path, l0, l1, .. } if path.0 == [1, data_row as u32, 0, 0] => *l0..*l1,
            _ => 0..0,
        })
        .collect();
    assert_eq!(lines, (0..DATA_LINES).collect::<Vec<_>>(), "every data line survives exactly once in source order");
    let cells: Vec<_> = laid
        .pages
        .iter()
        .enumerate()
        .flat_map(|(page, laid)| {
            laid.items.iter().filter_map(move |item| match item {
                Placed::Cell { table, row, rect, .. } if table.0 == [1] && *row == data_row => Some((page, *rect)),
                _ => None,
            })
        })
        .collect();
    assert!(cells.iter().all(|(_, rect)| rect.bottom() <= PAGE_MARGIN + BODY_HEIGHT + 0.01), "data stays inside each body: {cells:?}");
    assert_eq!(cells.first().map(|(page, _)| *page), Some(expected), "minimum={minimum}, inset={inset}, cells={cells:?}");
}

#[test]
fn oversized_minimum_uses_room() {
    assert_minimum_start(220.0, 0.0, Header::None, 0);
}

#[test]
fn header_oversized_uses_room() {
    assert_minimum_start(220.0, 0.0, Header::Repeat, 0);
}

#[test]
fn header_floor_over_body_room() {
    assert_minimum_start(200.0, 0.0, Header::Repeat, 0);
}

#[test]
fn insets_push_floor_past_page() {
    assert_minimum_start(200.0, 10.0, Header::None, 0);
}

#[test]
fn fitting_minimum_still_moves() {
    assert_minimum_start(180.0, 0.0, Header::None, 1);
    assert_minimum_start(160.0, 10.0, Header::None, 1);
    assert_minimum_start(160.0, 0.0, Header::Repeat, 1);
}
