//! The Format Shape pane (Shape Fill › More Gradients…, a shape's Format Shape…): the selected
//! shapes' fill, as Word's Fill section: no fill, a solid colour with its transparency, or a
//! gradient (preset, type, direction, angle, stops, rotate with shape). Every change runs
//! `shape.fill` with the whole fill; a drag applies as it goes and undoes in one step.

use std::sync::Arc;

use egui::{Color32, Id, Mesh, Pos2, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use wordcraft_doc::Rgb;
use wordcraft_doc::fill::{Fill, Gradient, GradientKind, PathShape};
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::wordart::GradStop;
use wordcraft_engine::cmd::shape_fill::{self, LINEAR_DIRECTIONS, PATH_DIRECTIONS, stops};
use wordcraft_layout::display::{Paint, paint_for};

use crate::WordApp;
use crate::theme::Tokens;

fn mark_id() -> Id {
    Id::new("format_shape.drag_mark")
}

fn stop_id() -> Id {
    Id::new("format_shape.stop")
}

/// The first selected shape's or canvas's fill: `None` with no shape selected.
fn current(app: &WordApp) -> Option<Option<Fill>> {
    let (_, o) = wordcraft_engine::cmd::objects::selected(&app.session)?;
    match o {
        InlineObject::Shape { fill, .. } => Some(fill),
        o => o.canvas_style().map(|c| c.fill.clone()),
    }
}

/// Sets the selected shapes' fill. While `resp` is being dragged each change is applied and
/// the drag becomes one undo step when the pointer is released.
fn apply(app: &mut WordApp, ui: &Ui, fill: Option<&Fill>, resp: Option<&Response>) {
    if resp.is_some_and(Response::dragged) && ui.data(|d| d.get_temp::<u64>(mark_id())).is_none() {
        let mark = app.session.undo_mark();
        ui.data_mut(|d| d.insert_temp(mark_id(), mark));
    }
    let _ = app.run("shape.fill", shape_fill::params(fill));
}

/// The pane, while open.
pub fn pane(app: &mut WordApp, ui: &mut Ui) {
    if crate::panes::header(ui, "Format Shape") {
        let _ = app.run("view.formatShapePane", serde_json::json!({"value": false}));
        return;
    }
    // A drag that applied as it went ends as one undo step.
    if !ui.input(|i| i.pointer.any_down())
        && let Some(mark) = ui.data_mut(|d| {
            let m = d.get_temp::<u64>(mark_id());
            d.remove::<u64>(mark_id());
            m
        })
    {
        app.session.merge_undo_since(mark);
    }
    let Some(fill) = current(app) else {
        ui.label(egui::RichText::new(tl!("Select a shape, text box or drawing canvas to format its fill.")).weak());
        return;
    };
    let theme = app.session.doc.settings.theme_colors.clone();
    ui.label(egui::RichText::new(tl!("Fill")).strong());
    ui.add_space(2.0);
    let kind = match &fill {
        None => 0,
        Some(Fill::Solid { .. }) => 1,
        Some(Fill::Gradient(_)) => 2,
    };
    let mut pick = kind;
    ui.radio_value(&mut pick, 0, tl!("No fill"));
    ui.radio_value(&mut pick, 1, tl!("Solid fill"));
    ui.radio_value(&mut pick, 2, tl!("Gradient fill"));
    if pick != kind {
        let main = fill.as_ref().map_or(theme.get(4).copied().unwrap_or(Rgb(68, 114, 196)), Fill::main_color);
        let new = match pick {
            1 => Some(Fill::solid(main)),
            2 => Some(shape_fill::default_gradient(main)),
            _ => None,
        };
        apply(app, ui, new.as_ref(), None);
        return;
    }
    ui.add_space(6.0);
    match fill {
        Some(Fill::Solid { color, transparency }) => solid(app, ui, &theme, color, transparency),
        Some(Fill::Gradient(g)) => gradient(app, ui, &theme, &g),
        None => {}
    }
}

fn solid(app: &mut WordApp, ui: &mut Ui, theme: &[Rgb], color: Rgb, transparency: f32) {
    let mut c = color;
    if color_field(ui, theme, Id::new("format_shape.solid_color"), &mut c) {
        apply(app, ui, Some(&Fill::Solid { color: c, transparency }), None);
    }
    let mut t = transparency;
    if let Some(r) = percent(ui, tl!("Transparency"), &mut t) {
        apply(app, ui, Some(&Fill::Solid { color, transparency: t }), Some(&r));
    }
}

/// A labelled 0–100% slider with its value box; the response when it changed.
fn percent(ui: &mut Ui, label: &str, v: &mut f32) -> Option<Response> {
    ui.horizontal(|ui| {
        ui.label(label);
        let s = ui.add(egui::Slider::new(v, 0.0..=100.0).suffix("%").fixed_decimals(0));
        (s.changed()).then_some(s)
    })
    .inner
}

/// A colour button with the theme colour drop-down; true when a colour was picked.
fn color_field(ui: &mut Ui, theme: &[Rgb], id: Id, c: &mut Rgb) -> bool {
    let mut hex = c.hex();
    crate::colors::take_field(ui.ctx(), id, &mut hex);
    ui.horizontal(|ui| {
        ui.label(tl!("Color"));
        let (r, _) = ui.allocate_exact_size(vec2(18.0, 14.0), Sense::hover());
        ui.painter().rect_filled(r, 2.0, crate::theme::c32(*c));
        ui.painter().rect_stroke(r, 2.0, Stroke::new(1.0, Tokens::get(ui.ctx()).border), egui::StrokeKind::Inside);
        ui.menu_button("▾", |ui| crate::colors::field_menu(ui, theme, id, &mut hex));
    });
    match Rgb::parse(&hex) {
        Some(n) if n != *c => {
            *c = n;
            true
        }
        _ => false,
    }
}

fn with(g: &Gradient, f: impl FnOnce(&mut Gradient)) -> Fill {
    let mut g = g.clone();
    f(&mut g);
    Fill::Gradient(Arc::new(g))
}

const TYPES: [(&str, &str); 4] = [("Linear", "linear"), ("Radial", "radial"), ("Rectangular", "rectangular"), ("Path", "path")];

fn type_of(k: &GradientKind) -> usize {
    match k {
        GradientKind::Linear { .. } => 0,
        GradientKind::Path { shape: PathShape::Circle, .. } => 1,
        GradientKind::Path { shape: PathShape::Rect, .. } => 2,
        GradientKind::Path { shape: PathShape::Shape, .. } => 3,
    }
}

fn kind_of(t: usize, focus: [f32; 4], angle: f32) -> GradientKind {
    match t {
        1 => GradientKind::Path { shape: PathShape::Circle, focus },
        2 => GradientKind::Path { shape: PathShape::Rect, focus },
        3 => GradientKind::Path { shape: PathShape::Shape, focus: [50.0; 4] },
        _ => GradientKind::Linear { angle, scaled: false },
    }
}

fn gradient(app: &mut WordApp, ui: &mut Ui, theme: &[Rgb], g: &Gradient) {
    egui::Grid::new("format_shape.gradient").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
        ui.label(tl!("Preset gradients"));
        ui.menu_button("▾", |ui| {
            if let Some(f) = presets_gallery(ui, theme) {
                apply(app, ui, Some(&f), None);
                ui.close();
            }
        });
        ui.end_row();

        ui.label(tl!("Type"));
        let t = type_of(&g.kind);
        let mut pick = t;
        egui::ComboBox::from_id_salt("format_shape.type").selected_text(tl!(TYPES[t].0)).show_ui(ui, |ui| {
            for (i, (label, _)) in TYPES.iter().enumerate() {
                ui.selectable_value(&mut pick, i, tl!(label));
            }
        });
        if pick != t {
            let (angle, focus) = match g.kind {
                GradientKind::Linear { angle, .. } => (angle, [50.0; 4]),
                GradientKind::Path { focus, .. } => (90.0, focus),
            };
            apply(app, ui, Some(&with(g, |g| g.kind = kind_of(pick, focus, angle))), None);
        }
        ui.end_row();

        if t != 3 {
            ui.label(tl!("Direction"));
            ui.menu_button("▾", |ui| {
                if let Some(kind) = directions(ui, g, t) {
                    apply(app, ui, Some(&with(g, |g| g.kind = kind)), None);
                    ui.close();
                }
            });
            ui.end_row();
        }

        if let GradientKind::Linear { angle, scaled } = g.kind {
            ui.label(tl!("Angle"));
            let mut a = angle;
            let r = ui.add(egui::DragValue::new(&mut a).range(0.0..=359.9).suffix("°").speed(1.0));
            if r.changed() {
                apply(app, ui, Some(&with(g, |g| g.kind = GradientKind::Linear { angle: a, scaled })), Some(&r));
            }
            ui.end_row();
        }
    });
    ui.add_space(6.0);
    ui.label(tl!("Gradient stops"));
    let sel = ui.data(|d| d.get_temp::<usize>(stop_id())).unwrap_or(0).min(g.stops.len().saturating_sub(1));
    stops_bar(app, ui, g, sel);
    let sel = ui.data(|d| d.get_temp::<usize>(stop_id())).unwrap_or(0).min(g.stops.len().saturating_sub(1));
    ui.horizontal(|ui| {
        if crate::widgets::icon_button(ui, "plus", tl!("Add gradient stop")).clicked() {
            let next = g.stops.get(sel + 1).map_or(100.0, |s| s.pos);
            let here = g.stops.get(sel).map_or(0.0, |s| s.pos);
            let mut st = g.stops.clone();
            if let Some(i) = stops::add(&mut st, (here + next) / 2.0) {
                ui.data_mut(|d| d.insert_temp(stop_id(), i));
                apply(app, ui, Some(&with(g, |g| g.stops = st)), None);
            }
        }
        if crate::widgets::icon_button(ui, "minus", tl!("Remove gradient stop")).clicked() {
            let mut st = g.stops.clone();
            if stops::remove(&mut st, sel) {
                ui.data_mut(|d| d.insert_temp(stop_id(), sel.saturating_sub(1)));
                apply(app, ui, Some(&with(g, |g| g.stops = st)), None);
            }
        }
    });
    let Some(stop) = g.stops.get(sel).copied() else { return };
    let set = |s: GradStop| with(g, |g| g.stops[sel] = s);
    let mut c = stop.color;
    if color_field(ui, theme, Id::new("format_shape.stop_color"), &mut c) {
        apply(app, ui, Some(&set(GradStop { color: c, ..stop })), None);
    }
    ui.horizontal(|ui| {
        ui.label(tl!("Position"));
        let mut p = stop.pos;
        let r = ui.add(egui::DragValue::new(&mut p).range(0.0..=100.0).suffix("%").speed(0.5).fixed_decimals(0));
        if r.changed() {
            let mut st = g.stops.clone();
            let i = stops::move_to(&mut st, sel, p);
            ui.data_mut(|d| d.insert_temp(stop_id(), i));
            apply(app, ui, Some(&with(g, |g| g.stops = st)), Some(&r));
        }
    });
    let mut t = stop.transparency;
    if let Some(r) = percent(ui, tl!("Transparency"), &mut t) {
        apply(app, ui, Some(&set(GradStop { transparency: t, ..stop })), Some(&r));
    }
    let mut rot = g.rot_with_shape;
    if ui.checkbox(&mut rot, tl!("Rotate with shape")).changed() {
        apply(app, ui, Some(&with(g, |g| g.rot_with_shape = rot)), None);
    }
}

/// The gradient stops bar: the gradient with a handle per stop. Click a handle to pick it, drag
/// it to move it, click elsewhere to add a stop there; with the bar focused, the arrow keys pick
/// the previous or next stop.
fn stops_bar(app: &mut WordApp, ui: &mut Ui, g: &Gradient, sel: usize) {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width().clamp(120.0, 400.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click_and_drag());
    let bar = Rect::from_min_size(r.min + vec2(6.0, 0.0), vec2(w - 12.0, 16.0));
    let linear = Fill::Gradient(Arc::new(Gradient { kind: GradientKind::Linear { angle: 0.0, scaled: false }, ..g.clone() }));
    paint_fill(ui.painter(), bar, &linear);
    ui.painter().rect_stroke(bar, 0.0, Stroke::new(1.0, t.border), egui::StrokeKind::Outside);
    let x_of = |pos: f32| bar.min.x + bar.width() * pos / 100.0;
    let pos_of = |x: f32| ((x - bar.min.x) / bar.width() * 100.0).clamp(0.0, 100.0);
    for (i, s) in g.stops.iter().enumerate() {
        let x = x_of(s.pos);
        let h = Rect::from_center_size(pos2(x, bar.max.y + 7.0), vec2(9.0, 11.0));
        ui.painter().rect_filled(h, 1.0, crate::theme::c32(s.color));
        let ring = if i == sel { Stroke::new(2.0, t.accent) } else { Stroke::new(1.0, t.border_strong) };
        ui.painter().rect_stroke(h, 1.0, ring, egui::StrokeKind::Outside);
    }
    if resp.has_focus() {
        ui.painter().rect_stroke(r, 2.0, Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
        let (left, right) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
        let next = if left {
            sel.saturating_sub(1)
        } else if right {
            (sel + 1).min(g.stops.len().saturating_sub(1))
        } else {
            sel
        };
        ui.data_mut(|d| d.insert_temp(stop_id(), next));
    }
    let near = |p: Pos2| {
        g.stops
            .iter()
            .enumerate()
            .map(|(i, s)| (i, (x_of(s.pos) - p.x).abs()))
            .filter(|(_, d)| *d <= 6.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let Some(p) = resp.interact_pointer_pos() else { return };
    if resp.drag_started() || resp.clicked() {
        resp.request_focus();
        match near(p) {
            Some(i) => {
                ui.data_mut(|d| d.insert_temp(stop_id(), i));
            }
            None if resp.clicked() => {
                let mut st = g.stops.clone();
                if let Some(i) = stops::add(&mut st, pos_of(p.x)) {
                    ui.data_mut(|d| d.insert_temp(stop_id(), i));
                    apply(app, ui, Some(&with(g, |g| g.stops = st)), None);
                }
            }
            None => {}
        }
    } else if resp.dragged() && resp.drag_delta().x != 0.0 {
        let mut st = g.stops.clone();
        let i = stops::move_to(&mut st, sel, pos_of(p.x));
        ui.data_mut(|d| d.insert_temp(stop_id(), i));
        apply(app, ui, Some(&with(g, |g| g.stops = st)), Some(&resp));
    }
}

/// Word's 30 preset gradients from the theme's colours; the one picked.
fn presets_gallery(ui: &mut Ui, theme: &[Rgb]) -> Option<Fill> {
    let presets = shape_fill::presets(theme);
    let mut out = None;
    egui::Grid::new("format_shape.presets").spacing(vec2(4.0, 4.0)).show(ui, |ui| {
        for (i, f) in presets.iter().enumerate() {
            if swatch(ui, f, &shape_fill::preset_name(i)) {
                out = Some(f.clone());
            }
            if i % 6 == 5 {
                ui.end_row();
            }
        }
    });
    out
}

/// The direction tiles for gradient type `t` (linear, radial or rectangular); the kind picked.
fn directions(ui: &mut Ui, g: &Gradient, t: usize) -> Option<GradientKind> {
    let kinds: Vec<(GradientKind, String)> = if t == 0 {
        LINEAR_DIRECTIONS.iter().map(|&a| (GradientKind::Linear { angle: a, scaled: false }, shape_fill::linear_direction_name(a))).collect()
    } else {
        PATH_DIRECTIONS.iter().map(|&f| (kind_of(t, f, 90.0), shape_fill::path_direction_name(f))).collect()
    };
    let mut out = None;
    egui::Grid::new("format_shape.directions").spacing(vec2(4.0, 4.0)).show(ui, |ui| {
        for (i, (k, name)) in kinds.into_iter().enumerate() {
            let f = with(g, |g| g.kind = k);
            if swatch(ui, &f, &name) {
                out = Some(k);
            }
            if i % 4 == 3 {
                ui.end_row();
            }
        }
    });
    out
}

/// A clickable square showing `fill`; true when clicked.
pub fn swatch(ui: &mut Ui, fill: &Fill, tip: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
    paint_fill(ui.painter(), r.shrink(2.0), fill);
    let ring = if resp.hovered() { Stroke::new(2.0, t.accent) } else { Stroke::new(1.0, t.border) };
    ui.painter().rect_stroke(r.shrink(2.0), 0.0, ring, egui::StrokeKind::Outside);
    let name = tl!(tip);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    resp.on_hover_text(name).clicked()
}

/// Paints `fill` over `r`: a grid of vertex colours, fine enough for previews.
pub fn paint_fill(p: &egui::Painter, r: Rect, fill: &Fill) {
    const N: u32 = 16;
    let frame = wordcraft_geom::Rect { x: r.min.x, y: r.min.y, w: r.width(), h: r.height() };
    let Some(paint) = paint_for(fill, frame) else { return };
    let at = |q: Pos2| -> Color32 {
        let (stops, t) = match &paint {
            Paint::Solid(c, a) => return rgba(*c, *a),
            Paint::Linear { p0, p1, stops } => {
                let d = (p1.0 - p0.0, p1.1 - p0.1);
                let len2 = (d.0 * d.0 + d.1 * d.1).max(1e-6);
                (stops, ((q.x - p0.0) * d.0 + (q.y - p0.1) * d.1) / len2)
            }
            Paint::Radial { c, r, stops } => (stops, ((q.x - c.0).hypot(q.y - c.1) / r.max(1e-6))),
        };
        let gs: Vec<GradStop> = stops.iter().map(|&(o, c, a)| GradStop { pos: o * 100.0, color: c, transparency: (1.0 - a) * 100.0 }).collect();
        let (c, tr) = stops::at(&gs, t * 100.0);
        rgba(c, 1.0 - tr / 100.0)
    };
    let mut mesh = Mesh::default();
    for j in 0..=N {
        for i in 0..=N {
            let q = pos2(r.min.x + r.width() * i as f32 / N as f32, r.min.y + r.height() * j as f32 / N as f32);
            mesh.colored_vertex(q, at(q));
        }
    }
    for j in 0..N {
        for i in 0..N {
            let k = j * (N + 1) + i;
            mesh.add_triangle(k, k + 1, k + N + 1);
            mesh.add_triangle(k + 1, k + N + 2, k + N + 1);
        }
    }
    p.add(mesh);
}

fn rgba(c: Rgb, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0, c.1, c.2, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use crate::WordApp;
    use egui_kittest::kittest::Queryable;
    use serde_json::json;
    use wordcraft_doc::fill::Fill;
    use wordcraft_doc::para::InlineObject;

    fn fill(app: &WordApp) -> Option<Fill> {
        match wordcraft_engine::cmd::objects::selected(&app.session) {
            Some((_, InlineObject::Shape { fill, .. })) => fill,
            o => panic!("{o:?}"),
        }
    }

    /// Shape Fill › Gradient applies a variation; the Format Shape pane switches the fill to
    /// solid and back to no fill, each one undo step.
    #[test]
    fn gradient_menu_and_format_shape_pane_set_the_fill() {
        let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
        a.run("insert.shape", json!({"kind": "rectangle"})).unwrap();
        // Shape Fill › Gradient's body (ribbon buttons are drawn without accessible names).
        let mut m = egui_kittest::Harness::builder().build_ui_state(
            |ui, app: &mut WordApp| {
                let f = fill(app);
                crate::ribbon::gradient_menu(ui, app, f.as_ref());
            },
            a,
        );
        m.run_steps(2);
        m.get_all_by_label("Linear Down").next().expect("a light variation").click();
        m.run_steps(2);
        let a = std::mem::replace(m.state_mut(), WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default()));
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(3);
        assert!(matches!(fill(h.state()), Some(Fill::Gradient(_))), "{:?}", fill(h.state()));
        h.state_mut().run("view.formatShapePane", json!({"value": true})).unwrap();
        h.run_steps(3);
        let depth = h.state().session.undo_depth();
        h.get_by_label("Solid fill").click();
        h.run_steps(3);
        assert!(matches!(fill(h.state()), Some(Fill::Solid { .. })));
        h.get_by_label("No fill").click();
        h.run_steps(3);
        assert_eq!(fill(h.state()), None);
        assert_eq!(h.state().session.undo_depth(), depth + 2);
    }
}
