//! Keyboard reachability of `ToggleGroup` cells when the group is off-screen,
//! and clipping of cell content when a width squeezes the cells.

use egui::{Context, Event, Id, Key, Modifiers, Pos2, RawInput, Rect, ScrollArea, Shape, Ui, vec2};
use egui_control_groups::ToggleGroup;

struct Harness {
    ctx: Context,
    time: f64,
}

impl Harness {
    fn new() -> Self {
        Self {
            ctx: Context::default(),
            time: 0.0,
        }
    }

    fn frame(
        &mut self,
        events: Vec<Event>,
        ui_fn: impl FnMut(&mut Ui),
    ) -> Vec<egui::epaint::ClippedShape> {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 200.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let mut ui_fn = ui_fn;
        let out = self.ctx.run_ui(input, |ui| ui_fn(ui));
        let shapes = out.shapes.clone();
        out.drop_without_applying_deltas();
        shapes
    }

    fn focused(&self) -> Option<Id> {
        self.ctx.memory(|m| m.focused())
    }
}

fn key(key: Key, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn tab(h: &mut Harness, ui_fn: &mut impl FnMut(&mut Ui)) {
    h.frame(vec![key(Key::Tab, true), key(Key::Tab, false)], &mut *ui_fn);
    h.frame(vec![], &mut *ui_fn);
}

#[test]
fn tab_reaches_cell_of_offscreen_group() {
    let mut h = Harness::new();
    let mut vals = [false; 2];
    let mut ui_fn = |ui: &mut Ui| {
        ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
            let _ = ui.button("before");
            ui.add_space(1000.0);
            let [a, b] = &mut vals;
            ui.add(ToggleGroup::new("g").cell("A", a).cell("B", b));
        });
        let _ = ui.button("after");
    };
    h.frame(vec![], &mut ui_fn);
    let mut seen = Vec::new();
    for _ in 0..6 {
        tab(&mut h, &mut ui_fn);
        if let Some(id) = h.focused()
            && !seen.contains(&id)
        {
            seen.push(id);
        }
    }
    // before, cell A, cell B, after.
    assert_eq!(seen.len(), 4, "tab order visited {} widgets", seen.len());
}

#[test]
fn focused_cell_keeps_focus_and_toggles_when_offscreen() {
    let mut h = Harness::new();
    let mut vals = [false; 2];
    let culled = std::cell::Cell::new(false);
    let mut ui_fn = |ui: &mut Ui| {
        if culled.get() {
            ui.set_clip_rect(Rect::from_min_size(Pos2::ZERO, vec2(10.0, 10.0)));
        }
        ui.add_space(100.0);
        let [a, b] = &mut vals;
        ui.add(ToggleGroup::new("g").cell("A", a).cell("B", b));
    };
    h.frame(vec![], &mut ui_fn);
    tab(&mut h, &mut ui_fn);
    let focus = h.focused().expect("a cell has focus");

    culled.set(true);
    h.frame(vec![], &mut ui_fn);
    assert_eq!(h.focused(), Some(focus), "focus lost while culled");

    h.frame(
        vec![key(Key::Space, true), key(Key::Space, false)],
        &mut ui_fn,
    );
    h.frame(vec![], &mut ui_fn);
    assert_eq!(h.focused(), Some(focus));
    assert_eq!(vals, [true, false], "Space must toggle while culled");
}

fn text_clips(shapes: &[egui::epaint::ClippedShape]) -> Vec<Rect> {
    fn walk(s: &Shape, clip: Rect, out: &mut Vec<Rect>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            Shape::Text(_) => out.push(clip),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, c.clip_rect, &mut out);
    }
    out
}

#[test]
fn content_is_clipped_to_its_cell_when_squeezed() {
    let mut h = Harness::new();
    let mut vals = [false; 3];
    let mut group = Rect::NOTHING;
    let mut ui_fn = |ui: &mut Ui| {
        let [a, b, c] = &mut vals;
        group = ui
            .add(
                ToggleGroup::new("g")
                    .cell("AAAAAAAAAAAA", a)
                    .cell("BBBBBBBBBBBB", b)
                    .cell("CCCCCCCCCCCC", c)
                    .width(30.0),
            )
            .rect;
    };
    let shapes = h.frame(vec![], &mut ui_fn);
    let clips = text_clips(&shapes);
    assert_eq!(clips.len(), 3);
    for clip in clips {
        assert!(
            clip.width() <= group.width() / 2.0,
            "clip {clip:?} wider than a cell"
        );
        assert!(
            group.expand(0.5).contains_rect(clip),
            "{clip:?} outside {group:?}"
        );
    }
}
