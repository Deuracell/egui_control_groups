//! API-level tests: `cells()` length check, `CellContent` impls, culling and
//! stale gesture handling.

use std::borrow::Cow;

use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, Ui, pos2, vec2};
use egui_control_groups::{CellContent, ToggleGroup};

fn texts(shape: &Shape, out: &mut Vec<String>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        Shape::Text(t) => out.push(t.galley.text().to_owned()),
        _ => {}
    }
}

struct Harness {
    ctx: Context,
    time: f64,
    texts: Vec<String>,
}

impl Harness {
    fn new() -> Self {
        Self {
            ctx: Context::default(),
            time: 0.0,
            texts: Vec::new(),
        }
    }

    fn frame(&mut self, events: Vec<Event>, mut ui_fn: impl FnMut(&mut Ui)) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let output = self.ctx.run_ui(input, |ui| ui_fn(ui));
        self.texts.clear();
        for c in &output.shapes {
            texts(&c.shape, &mut self.texts);
        }
        output.drop_without_applying_deltas();
    }
}

fn button(pos: Pos2, pressed: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        },
    ]
}

#[test]
fn all_content_kinds_render() {
    let s = String::from("string");
    let owned_cow: Cow<'_, str> = Cow::Owned("cow-owned".into());
    let borrowed_cow: Cow<'_, str> = Cow::Borrowed("cow-borrowed");
    let boxed: Box<dyn CellContent> = Box::new("boxed");
    let mixed: Vec<Box<dyn CellContent>> = vec![Box::new("m1"), Box::new(String::from("m2"))];
    let by_ref = String::from("by-ref");
    let mut v = [false; 8];
    let mut h = Harness::new();
    for _ in 0..2 {
        h.frame(vec![], |ui| {
            let [a, b, c, d, e, f, g, i] = &mut v;
            ui.add(
                ToggleGroup::new("g")
                    .cell(&s, a)
                    .cell(owned_cow.clone(), b)
                    .cell(borrowed_cow.clone(), c)
                    .cell(&boxed, d)
                    .cell(&by_ref, e)
                    .cell(Box::new(egui::WidgetText::from("wt")), f)
                    .cells(mixed.iter(), &mut [*g, *i]),
            );
        });
    }
    for want in [
        "string",
        "cow-owned",
        "cow-borrowed",
        "boxed",
        "by-ref",
        "wt",
        "m1",
        "m2",
    ] {
        assert!(
            h.texts.iter().any(|t| t == want),
            "missing {want}: {:?}",
            h.texts
        );
    }
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "labels but")]
fn cells_length_mismatch_panics_in_debug() {
    let mut v = [false; 2];
    let mut h = Harness::new();
    h.frame(vec![], |ui| {
        ui.add(ToggleGroup::new("g").cells(["A", "B", "C"], &mut v));
    });
}

#[test]
fn culled_group_still_allocates_and_responds() {
    let mut h = Harness::new();
    let mut v = [false; 2];
    let mut rect = Rect::NOTHING;
    let mut after = Rect::NOTHING;
    h.frame(vec![], |ui| {
        ui.add_space(2000.0); // far below the 600pt screen
        let r = ui.add(ToggleGroup::new("g").cells(["A", "B"], &mut v));
        rect = r.rect;
        after = ui.min_rect();
    });
    assert!(rect.min.y >= 2000.0 && rect.height() > 0.0);
    assert!(after.max.y >= rect.max.y);
    assert!(h.texts.is_empty(), "culled group painted {:?}", h.texts);
}

#[test]
fn gesture_does_not_resume_after_group_was_hidden() {
    let mut h = Harness::new();
    let mut v = [false; 3];
    let mut rect = Rect::NOTHING;
    let show = |v: &mut [bool; 3], rect: &mut Rect, ui: &mut Ui| {
        *rect = ui.add(ToggleGroup::new("g").cells(["A", "B", "C"], v)).rect;
    };
    for _ in 0..3 {
        h.frame(vec![], |ui| show(&mut v, &mut rect, ui));
    }
    let cell: Vec<Pos2> = (0..3)
        .map(|i| {
            pos2(
                rect.left() + rect.width() / 3.0 * (i as f32 + 0.5),
                rect.center().y,
            )
        })
        .collect();
    h.frame(button(cell[0], true), |ui| show(&mut v, &mut rect, ui));
    assert_eq!(v, [true, false, false]);
    // Group stops being shown while the button is still down, then released.
    h.frame(vec![], |_| {});
    h.frame(vec![], |_| {});
    h.frame(button(cell[0], false), |_| {});
    h.frame(vec![], |ui| show(&mut v, &mut rect, ui));
    // A fresh press must act as a fresh press.
    h.frame(button(cell[2], true), |ui| show(&mut v, &mut rect, ui));
    assert_eq!(v, [true, false, true]);
}
