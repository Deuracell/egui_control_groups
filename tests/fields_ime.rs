//! The text input (`PlatformOutput::ime`) must stay claimed while the editor
//! moves from one field to another: a backend closes the soft keyboard on any
//! frame without it, which made the keyboard flicker.

use egui::{
    Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, TouchPhase, pos2, vec2,
};
use egui_control_groups::FieldGroup;

struct H {
    ctx: Context,
    time: f64,
    vals: [f32; 3],
    other: [f32; 2],
    rects: [Rect; 2],
    /// Whether each frame so far reported a text input.
    ime: Vec<bool>,
}

impl H {
    fn new() -> Self {
        let mut h = Self {
            ctx: Context::default(),
            time: 0.0,
            vals: [1.0, 2.0, 3.0],
            other: [4.0, 5.0],
            rects: [Rect::NOTHING; 2],
            ime: Vec::new(),
        };
        for _ in 0..3 {
            h.frame(vec![]);
        }
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let (vals, other, rects) = (&mut self.vals, &mut self.other, &mut self.rects);
        let out = self.ctx.run_ui(input, |ui| {
            rects[0] = ui
                .add(
                    FieldGroup::new("a")
                        .fields(["X", "Y", "Z"], vals)
                        .cell_width(80.0),
                )
                .rect;
            rects[1] = ui
                .add(
                    FieldGroup::new("b")
                        .fields(["U", "V"], other)
                        .cell_width(80.0),
                )
                .rect;
        });
        self.ime.push(out.platform_output.ime.is_some());
        out.drop_without_applying_deltas();
    }

    fn cell(&self, group: usize, i: usize) -> Pos2 {
        let r = self.rects[group];
        pos2(r.left() + 80.0 * (i as f32 + 0.5), r.center().y)
    }

    fn button(p: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn touch(p: Pos2, phase: TouchPhase) -> Event {
        Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(0),
            phase,
            pos: p,
            force: None,
        }
    }

    /// A finger tap, with touch and pointer events arriving together.
    fn tap(&mut self, p: Pos2) {
        self.frame(vec![
            Self::touch(p, TouchPhase::Start),
            Event::PointerMoved(p),
            Self::button(p, true),
        ]);
        self.frame(vec![]);
        self.frame(vec![
            Self::touch(p, TouchPhase::End),
            Self::button(p, false),
            Event::PointerGone,
        ]);
        for _ in 0..4 {
            self.frame(vec![]);
        }
    }

    fn click(&mut self, p: Pos2) {
        self.frame(vec![Event::PointerMoved(p)]);
        self.frame(vec![Self::button(p, true)]);
        self.frame(vec![Self::button(p, false)]);
        for _ in 0..4 {
            self.frame(vec![]);
        }
    }

    /// Whether the text input stayed claimed on every frame from `from` on.
    fn held_since(&self, from: usize) -> bool {
        self.ime[from..].iter().all(|&on| on)
    }
}

#[test]
fn tapping_a_neighbouring_field_keeps_the_text_input() {
    let mut h = H::new();
    h.tap(h.cell(0, 0));
    assert!(*h.ime.last().unwrap(), "the first field is being edited");
    let from = h.ime.len();
    h.tap(h.cell(0, 2));
    assert!(*h.ime.last().unwrap(), "the tapped field is being edited");
    assert!(
        h.held_since(from),
        "no frame without a text input: {:?}",
        &h.ime[from..]
    );
}

#[test]
fn clicking_a_neighbouring_field_keeps_the_text_input() {
    let mut h = H::new();
    h.click(h.cell(0, 0));
    let from = h.ime.len();
    h.click(h.cell(0, 1));
    assert!(
        h.held_since(from),
        "no frame without a text input: {:?}",
        &h.ime[from..]
    );
}

#[test]
fn tapping_a_field_in_another_group_keeps_the_text_input() {
    let mut h = H::new();
    h.tap(h.cell(0, 1));
    let from = h.ime.len();
    h.tap(h.cell(1, 0));
    assert!(
        *h.ime.last().unwrap(),
        "the other group's field is being edited"
    );
    assert!(
        h.held_since(from),
        "no frame without a text input: {:?}",
        &h.ime[from..]
    );
}

#[test]
fn tapping_away_releases_the_text_input() {
    let mut h = H::new();
    h.tap(h.cell(0, 0));
    h.tap(pos2(600.0, 400.0));
    assert!(!*h.ime.last().unwrap(), "nothing is being edited any more");
}
