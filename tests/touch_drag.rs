//! Touch drags as a real finger makes them: many small moves, not one jump
//! to the target cell. Also swipes that begin on a group, which must scroll
//! the surrounding scroll area instead of being swallowed by the cells.

use egui::{
    Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, ScrollArea, TouchPhase,
    Ui, pos2, vec2,
};
use egui_control_groups::{FieldGroup, ToggleGroup};

struct H {
    ctx: Context,
    time: f64,
    toggles: [bool; 4],
    exclusive: bool,
    fields: [f32; 3],
    /// Browser event order: the pointer press arrives a frame before the touch.
    web: bool,
    rects: [Rect; 2],
    offset: f32,
}

impl H {
    fn new(web: bool) -> Self {
        let mut h = Self {
            ctx: Context::default(),
            time: 0.0,
            toggles: [false; 4],
            exclusive: false,
            fields: [1.0, 2.0, 3.0],
            web,
            rects: [Rect::NOTHING; 2],
            offset: 0.0,
        };
        for _ in 0..3 {
            h.frame(vec![]);
        }
        // Drag-to-scroll only switches on once a touch screen has been seen.
        h.swipe(pos2(300.0, 250.0), pos2(300.0, 249.0));
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let (toggles, fields, rects) = (&mut self.toggles, &mut self.fields, &mut self.rects);
        let (exclusive, web) = (self.exclusive, self.web);
        let mut offset = 0.0;
        let out = self.ctx.run_ui(input, |ui: &mut Ui| {
            let scroll = ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                ui.add_space(40.0);
                rects[0] = ui
                    .add(
                        ToggleGroup::new("t")
                            .cells(["A", "B", "C", "D"], toggles)
                            .equal_width()
                            .exclusive(exclusive)
                            .settle_press(web),
                    )
                    .rect;
                rects[1] = ui
                    .add(
                        FieldGroup::new("f")
                            .fields(["X", "Y", "Z"], fields)
                            .cell_width(80.0),
                    )
                    .rect;
                ui.add_space(1000.0);
            });
            offset = scroll.state.offset.y;
        });
        self.offset = offset;
        out.drop_without_applying_deltas();
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

    fn button(p: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    /// A finger down at `from`, moved to `to` in ten small steps, and lifted.
    fn swipe(&mut self, from: Pos2, to: Pos2) {
        if self.web {
            self.frame(vec![Event::PointerMoved(from), Self::button(from, true)]);
            self.frame(vec![Self::touch(from, TouchPhase::Start)]);
        } else {
            self.frame(vec![
                Self::touch(from, TouchPhase::Start),
                Event::PointerMoved(from),
                Self::button(from, true),
            ]);
        }
        for k in 1..=10 {
            let p = from + (to - from) * (k as f32 / 10.0);
            self.frame(vec![
                Self::touch(p, TouchPhase::Move),
                Event::PointerMoved(p),
            ]);
        }
        self.frame(vec![
            Self::touch(to, TouchPhase::End),
            Self::button(to, false),
        ]);
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    fn toggle_cell(&self, i: usize) -> Pos2 {
        let r = self.rects[0];
        pos2(r.left() + r.width() / 4.0 * (i as f32 + 0.5), r.center().y)
    }

    fn field_cell(&self, i: usize) -> Pos2 {
        let r = self.rects[1];
        pos2(r.left() + 80.0 * (i as f32 + 0.5), r.center().y)
    }

    fn type_and_commit(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.to_owned())]);
        let key = |pressed| Event::Key {
            key: Key::Enter,
            physical_key: Some(Key::Enter),
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![key(true), key(false)]);
        self.frame(vec![]);
    }
}

fn both(test: impl Fn(H)) {
    for web in [false, true] {
        test(H::new(web));
    }
}

#[test]
fn slow_touch_drag_paints_toggles() {
    both(|mut h| {
        // A little sideways drift, as a finger has.
        h.swipe(h.toggle_cell(0), h.toggle_cell(2) + vec2(0.0, 5.0));
        assert_eq!(h.toggles, [true, true, true, false], "web: {}", h.web);
        assert_eq!(h.offset, 0.0, "a drag along the group does not scroll");
    });
}

#[test]
fn slow_touch_drag_moves_an_exclusive_selection() {
    both(|mut h| {
        h.exclusive = true;
        h.toggles = [true, false, false, false];
        h.swipe(h.toggle_cell(0), h.toggle_cell(3));
        assert_eq!(h.toggles, [false, false, false, true], "web: {}", h.web);
    });
}

#[test]
fn slow_touch_drag_selects_fields() {
    both(|mut h| {
        h.swipe(h.field_cell(0), h.field_cell(2) + vec2(0.0, 5.0));
        h.type_and_commit("7");
        assert_eq!(h.fields, [7.0, 7.0, 7.0], "web: {}", h.web);
    });
}

#[test]
fn touch_drag_that_stays_in_its_cell_is_not_a_tap() {
    both(|mut h| {
        let from = h.toggle_cell(0) - vec2(12.0, 0.0);
        h.swipe(from, from + vec2(20.0, 0.0));
        assert_eq!(h.toggles, [false; 4], "web: {}", h.web);
    });
}

#[test]
fn swipe_across_toggles_scrolls_and_toggles_nothing() {
    both(|mut h| {
        let from = h.toggle_cell(1);
        h.swipe(from, from - vec2(0.0, 80.0));
        assert_eq!(h.toggles, [false; 4], "web: {}", h.web);
        // The swipe goes on scrolling after the group has left the view.
        assert_eq!(h.offset, 80.0, "web: {}", h.web);
    });
}

#[test]
fn swipe_across_fields_scrolls_and_opens_nothing() {
    both(|mut h| {
        let from = h.field_cell(1);
        h.swipe(from, from - vec2(0.0, 80.0));
        assert_eq!(h.offset, 80.0, "web: {}", h.web);
        h.type_and_commit("7");
        assert_eq!(h.fields, [1.0, 2.0, 3.0], "no editor was opened");
    });
}
