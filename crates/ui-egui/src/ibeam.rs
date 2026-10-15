//! The text pointer over the page on Windows (#1746, #1417, #1126). Windows' I-beam inverts what
//! lies under it, but over the GPU-drawn window it stays white, so it vanishes on white paper.
//! There the system pointer hides wherever it would be the I-beam and an I-beam is drawn in its
//! place: black on light paper, white on dark (as the caret, #299), edged in the other colour so
//! it shows on anything. Other systems' I-beams show on any page and are left alone.

use egui::{Color32, CursorIcon, Id, LayerId, Order, Pos2, Stroke};
use wordcraft_doc::props::Rgb;

/// Whether this system's I-beam is replaced by a drawn one.
pub(crate) const DRAWN: bool = cfg!(target_os = "windows");

/// Once the canvas has chosen its pointer: if it is the I-beam, hide it and draw one at `p` that
/// shows against `bg`. Whether it did.
pub(crate) fn replace(ctx: &egui::Context, p: Pos2, bg: Rgb) -> bool {
    if ctx.output(|o| o.cursor_icon) != CursorIcon::Text || !(p.x.is_finite() && p.y.is_finite()) {
        return false;
    }
    ctx.set_cursor_icon(CursorIcon::None);
    // Above the page and whatever floats over it, as the system pointer would be.
    let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("wordcraft_ibeam")));
    paint(&painter, p, crate::canvas::caret_contrast(bg));
    true
}

/// The I-beam centred on `p`, in `fill`, edged in the opposite colour.
fn paint(painter: &egui::Painter, p: Pos2, fill: Color32) {
    let edge = if fill == Color32::BLACK { Color32::WHITE } else { Color32::BLACK };
    let x = p.x.round() + 0.5;
    let (top, bottom) = (p.y.round() - 8.0, p.y.round() + 8.0);
    let bar = |grow: f32| {
        [
            [egui::pos2(x, top), egui::pos2(x, bottom)],
            [egui::pos2(x - 3.0 - grow, top), egui::pos2(x + 3.0 + grow, top)],
            [egui::pos2(x - 3.0 - grow, bottom), egui::pos2(x + 3.0 + grow, bottom)],
        ]
    };
    for seg in bar(1.0) {
        painter.line_segment(seg, Stroke::new(3.0, edge));
    }
    for seg in bar(0.0) {
        painter.line_segment(seg, Stroke::new(1.0, fill));
    }
}

#[cfg(test)]
mod tests {
    use crate::WordApp;
    use egui::{Color32, CursorIcon, Shape};

    type H = egui_kittest::Harness<'static, WordApp>;

    fn window(drawn: bool, dark: bool) -> H {
        let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Hello WordCraft")), Default::default());
        a.canvas.drawn_ibeam = drawn;
        a.session.view.dark_mode = dark;
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(3);
        h
    }

    /// Hovers the middle of the first page: the pointer then, and the drawn I-beam's colour if any.
    fn hover_page(h: &mut H) -> (CursorIcon, Option<Color32>) {
        let at = crate::canvas::page_to_screen(h.state_mut(), 0, 300.0, 400.0).unwrap();
        h.event(egui::Event::PointerMoved(at));
        h.run_steps(2);
        let out = h.output();
        let x = at.x.round() + 0.5;
        let stem = out.shapes.iter().find_map(|c| match &c.shape {
            Shape::LineSegment { points: [a, b], stroke } if a.x == x && b.x == x && stroke.width == 1.0 && (b.y - a.y - 16.0).abs() < 0.01 => {
                Some(stroke.color)
            }
            _ => None,
        });
        (out.platform_output.cursor_icon, stem)
    }

    /// #1746: where the system I-beam would vanish (Windows), it hides over the page and a black
    /// I-beam is drawn on white paper, a white one on the dark page.
    #[test]
    fn a_drawn_i_beam_shows_on_white_and_dark_paper() {
        let mut h = window(true, false);
        assert_eq!(hover_page(&mut h), (CursorIcon::None, Some(Color32::BLACK)), "white paper");
        let mut h = window(true, true);
        assert_eq!(hover_page(&mut h), (CursorIcon::None, Some(Color32::WHITE)), "dark page");
    }

    /// Elsewhere the system's own I-beam stays.
    #[test]
    fn the_system_i_beam_stays_where_it_shows() {
        let mut h = window(false, false);
        assert_eq!(hover_page(&mut h), (CursorIcon::Text, None));
    }
}
