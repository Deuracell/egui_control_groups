//! `RadioGroup`: single-value selection driven by pointer, touch and keyboard
//! events, one frame at a time.

use egui::{
    Color32, Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, TouchPhase, Ui,
    pos2, vec2,
};
use egui_control_groups::{GroupStyle, RadioGroup};

const ACTIVE: Color32 = Color32::from_rgb(250, 10, 10);

fn style() -> GroupStyle {
    GroupStyle {
        active_fill: Some(ACTIVE),
        ..Default::default()
    }
}

/// Rectangles painted in the active fill.
fn count_active(shape: &egui::Shape) -> usize {
    match shape {
        egui::Shape::Vec(v) => v.iter().map(count_active).sum(),
        egui::Shape::Rect(r) => usize::from(r.fill == ACTIVE),
        _ => 0,
    }
}

struct Harness {
    ctx: Context,
    time: f64,
    modifiers: Modifiers,
    sent_modifiers: Modifiers,
}

impl Harness {
    fn new() -> Self {
        Self {
            ctx: Context::default(),
            time: 0.0,
            modifiers: Modifiers::NONE,
            sent_modifiers: Modifiers::NONE,
        }
    }

    /// Run one frame; returns how many cells were painted active.
    fn frame(&mut self, mut events: Vec<Event>, ui_fn: impl FnOnce(&mut Ui)) -> usize {
        self.time += 1.0 / 60.0;
        if self.modifiers != self.sent_modifiers {
            events.insert(0, Event::ModifiersChanged(self.modifiers));
            self.sent_modifiers = self.modifiers;
        }
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let mut ui_fn = Some(ui_fn);
        let output = self.ctx.run_ui(input, |ui| (ui_fn.take().unwrap())(ui));
        let active = output.shapes.iter().map(|c| count_active(&c.shape)).sum();
        output.drop_without_applying_deltas();
        active
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

fn touch(pos: Pos2, phase: TouchPhase) -> Event {
    Event::Touch {
        device_id: egui::TouchDeviceId(0),
        id: egui::TouchId(0),
        phase,
        pos,
        force: None,
    }
}

fn key(key: Key) -> Vec<Event> {
    [true, false]
        .into_iter()
        .map(|pressed| Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    A,
    B,
    C,
}

/// A `RadioGroup` over `Mode`, either plain or optional.
struct Radio {
    h: Harness,
    value: Option<Mode>,
    optional: bool,
    rect: Rect,
    changed: u32,
    active: usize,
}

impl Radio {
    fn new(value: Option<Mode>, optional: bool) -> Self {
        let mut r = Self {
            h: Harness::new(),
            value,
            optional,
            rect: Rect::NOTHING,
            changed: 0,
            active: 0,
        };
        for _ in 0..3 {
            r.idle();
        }
        r.changed = 0;
        r
    }

    fn show(&mut self, events: Vec<Event>) {
        let (optional, value) = (self.optional, &mut self.value);
        let (mut rect, mut changed) = (Rect::NOTHING, false);
        self.active = self.h.frame(events, |ui| {
            let response = if optional {
                ui.add(
                    RadioGroup::optional("r", value)
                        .option(Mode::A, "A")
                        .option(Mode::B, "B")
                        .option(Mode::C, "C")
                        .equal_width()
                        .style(style()),
                )
            } else {
                let mut dummy = Mode::A;
                let bound = match value {
                    Some(v) => v,
                    None => &mut dummy,
                };
                ui.add(
                    RadioGroup::new("r", bound)
                        .option(Mode::A, "A")
                        .option(Mode::B, "B")
                        .option(Mode::C, "C")
                        .equal_width()
                        .style(style()),
                )
            };
            rect = response.rect;
            changed = response.changed();
        });
        self.rect = rect;
        self.changed += u32::from(changed);
    }

    fn idle(&mut self) {
        self.show(vec![]);
    }

    fn center(&self, i: usize) -> Pos2 {
        pos2(
            self.rect.left() + self.rect.width() / 3.0 * (i as f32 + 0.5),
            self.rect.center().y,
        )
    }

    fn click(&mut self, i: usize) {
        let p = self.center(i);
        self.show(vec![Event::PointerMoved(p)]);
        self.show(button(p, true));
        self.show(button(p, false));
        self.idle();
    }
}

#[test]
fn click_selects_and_reports_changed_once() {
    let mut r = Radio::new(Some(Mode::A), false);
    r.click(1);
    assert_eq!(r.value, Some(Mode::B));
    assert_eq!(r.changed, 1);
}

#[test]
fn clicking_the_selected_option_changes_nothing() {
    let mut r = Radio::new(Some(Mode::B), false);
    r.click(1);
    assert_eq!(r.value, Some(Mode::B));
    assert_eq!(r.changed, 0);
}

#[test]
fn optional_clears_and_selects_again() {
    let mut r = Radio::new(Some(Mode::B), true);
    r.click(1);
    assert_eq!(r.value, None);
    assert_eq!(r.changed, 1);
    assert_eq!(r.active, 0);
    r.click(2);
    assert_eq!(r.value, Some(Mode::C));
    assert_eq!(r.changed, 2);
    r.click(0);
    assert_eq!(r.value, Some(Mode::A));
}

#[test]
fn optional_starting_as_none_stays_none_without_input() {
    let mut r = Radio::new(None, true);
    for _ in 0..3 {
        r.idle();
    }
    assert_eq!(r.value, None);
    assert_eq!(r.changed, 0);
}

#[test]
fn shift_click_does_not_extend() {
    let mut r = Radio::new(Some(Mode::A), false);
    r.h.modifiers = Modifiers::SHIFT;
    r.click(2);
    assert_eq!(r.value, Some(Mode::C));
    assert_eq!(r.active, 1, "still a single active cell");
    r.h.modifiers = Modifiers::NONE;
    r.click(0);
    assert_eq!(r.value, Some(Mode::A));
    assert_eq!(r.changed, 2);
}

#[test]
fn shift_click_on_selected_option_does_nothing() {
    let mut r = Radio::new(Some(Mode::A), false);
    r.h.modifiers = Modifiers::SHIFT;
    r.click(0);
    assert_eq!(r.value, Some(Mode::A));
    assert_eq!(r.changed, 0);
}

#[test]
fn drag_across_moves_the_selection() {
    let mut r = Radio::new(Some(Mode::A), false);
    let start = r.center(0);
    r.show(vec![Event::PointerMoved(start)]);
    r.show(button(start, true));
    let end = r.center(2);
    r.show(vec![Event::PointerMoved(end)]);
    assert_eq!(r.value, Some(Mode::C));
    r.show(button(end, false));
    r.idle();
    assert_eq!(r.value, Some(Mode::C));
}

#[test]
fn touch_tap_selects() {
    let mut r = Radio::new(Some(Mode::A), false);
    let p = r.center(1);
    let mut ev = vec![touch(p, TouchPhase::Start)];
    ev.extend(button(p, true));
    r.show(ev);
    assert_eq!(r.value, Some(Mode::A), "nothing before release");
    let mut ev = vec![touch(p, TouchPhase::End)];
    ev.extend(button(p, false));
    r.show(ev);
    r.idle();
    assert_eq!(r.value, Some(Mode::B));
    assert_eq!(r.changed, 1);
}

#[test]
fn touch_long_press_does_not_extend() {
    let mut r = Radio::new(Some(Mode::A), false);
    let p = r.center(2);
    let mut ev = vec![touch(p, TouchPhase::Start)];
    ev.extend(button(p, true));
    r.show(ev);
    r.h.time += 1.0;
    r.show(vec![touch(p, TouchPhase::Move)]);
    let mut ev = vec![touch(p, TouchPhase::End)];
    ev.extend(button(p, false));
    r.show(ev);
    r.idle();
    // The release selects only the pressed option; A is no longer on.
    assert_eq!(r.value, Some(Mode::C));
    assert_eq!(r.active, 1);
}

#[test]
fn keyboard_tab_then_space_selects() {
    let mut r = Radio::new(Some(Mode::A), false);
    r.show(key(Key::Tab));
    r.idle();
    r.show(key(Key::Tab));
    r.idle();
    // Focus is on the second option (after the first Tab reached the first).
    r.show(key(Key::Space));
    r.idle();
    assert_eq!(r.value, Some(Mode::B));
    assert_eq!(r.changed, 1);
}

#[test]
fn value_matching_no_option_shows_nothing_active_and_click_selects() {
    // `optional` with None: no option matches.
    let mut r = Radio::new(None, true);
    assert_eq!(r.active, 0);
    r.click(2);
    assert_eq!(r.value, Some(Mode::C));
    assert_eq!(r.changed, 1);
    assert_eq!(r.active, 1);
}

#[test]
fn unmatched_plain_value_shows_nothing_active_and_a_click_selects() {
    #[derive(PartialEq, Debug)]
    enum Other {
        X,
        Y,
        Z,
    }
    let mut value = Other::Z;
    let mut h = Harness::new();
    let show = |h: &mut Harness, ev: Vec<Event>, value: &mut Other| {
        let (mut rect, mut changed) = (Rect::NOTHING, false);
        let active = h.frame(ev, |ui| {
            let r = ui.add(
                RadioGroup::new("u", value)
                    .option(Other::X, "X")
                    .option(Other::Y, "Y")
                    .equal_width()
                    .style(style()),
            );
            rect = r.rect;
            changed = r.changed();
        });
        (rect, changed, active)
    };
    let mut rect = Rect::NOTHING;
    for _ in 0..3 {
        let (r, changed, active) = show(&mut h, vec![], &mut value);
        assert!(!changed);
        assert_eq!(active, 0);
        rect = r;
    }
    assert_eq!(value, Other::Z);
    let p = pos2(rect.left() + rect.width() * 0.25, rect.center().y);
    show(&mut h, vec![Event::PointerMoved(p)], &mut value);
    let (_, changed, _) = show(&mut h, button(p, true), &mut value);
    assert!(changed);
    show(&mut h, button(p, false), &mut value);
    assert_eq!(value, Other::X);
    let (_, _, active) = show(&mut h, vec![], &mut value);
    assert_eq!(active, 1);
    let _ = Other::Y;
}
