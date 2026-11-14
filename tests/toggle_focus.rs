//! Keyboard focus ring and enabled state of `ToggleGroup`.

use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, Shape, Ui, vec2};
use egui_control_groups::ToggleGroup;

fn count_strokes(shape: &Shape, color: egui::Color32) -> usize {
    match shape {
        Shape::Vec(v) => v.iter().map(|s| count_strokes(s, color)).sum(),
        Shape::Rect(r) => usize::from(r.stroke.color == color && r.stroke.width > 0.0),
        _ => 0,
    }
}

fn frame(ctx: &Context, time: f64, events: Vec<Event>, ui_fn: impl FnMut(&mut Ui)) -> usize {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
        time: Some(time),
        events,
        ..Default::default()
    };
    let mut ui_fn = ui_fn;
    let out = ctx.run_ui(input, |ui| ui_fn(ui));
    let color = ctx.global_style().visuals.selection.stroke.color;
    let n = out
        .shapes
        .iter()
        .map(|c| count_strokes(&c.shape, color))
        .sum();
    out.drop_without_applying_deltas();
    n
}

#[test]
fn focused_cell_paints_ring() {
    let ctx = Context::default();
    let mut vals = [false; 3];
    let mut show = |ui: &mut Ui| {
        let [a, b, c] = &mut vals;
        ui.add(ToggleGroup::new("g").cell("A", a).cell("B", b).cell("C", c));
    };
    let none = frame(&ctx, 0.0, vec![], &mut show);
    let tab = Event::Key {
        key: Key::Tab,
        physical_key: Some(Key::Tab),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    frame(&ctx, 0.02, vec![tab], &mut show);
    let focused = frame(&ctx, 0.04, vec![], &mut show);
    assert_eq!(focused, none + 1);
}
