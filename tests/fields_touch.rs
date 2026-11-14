//! Touch and mouse gestures on `FieldGroup`.

use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, pos2, vec2};
use egui_control_groups::FieldGroup;

struct Fields {
    ctx: Context,
    time: f64,
    vals: [f32; 3],
    rect: Rect,
    vertical: bool,
    shapes: usize,
}

impl Fields {
    fn new(vertical: bool) -> Self {
        let mut f = Self {
            ctx: Context::default(),
            time: 0.0,
            vals: [1.0, 2.0, 3.0],
            rect: Rect::NOTHING,
            vertical,
            shapes: 0,
        };
        for _ in 0..3 {
            f.frame(vec![]);
        }
        f
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let (vals, vertical) = (&mut self.vals, self.vertical);
        let mut rect = Rect::NOTHING;
        let out = self.ctx.run_ui(input, |ui: &mut Ui| {
            let [a, b, c] = vals;
            let mut g = FieldGroup::new("f")
                .field(a)
                .field(b)
                .field(c)
                .cell_width(80.0);
            if vertical {
                g = g.vertical();
            }
            rect = ui.add(g).rect;
        });
        self.rect = rect;
        self.shapes = out.shapes.len();
        out.drop_without_applying_deltas();
    }

    fn center(&self, i: usize) -> Pos2 {
        let n = 3.0;
        if self.vertical {
            pos2(
                self.rect.center().x,
                self.rect.top() + self.rect.height() / n * (i as f32 + 0.5),
            )
        } else {
            pos2(
                self.rect.left() + self.rect.width() / n * (i as f32 + 0.5),
                self.rect.center().y,
            )
        }
    }

    fn editing(&self) -> bool {
        self.ctx.egui_wants_keyboard_input()
    }
}

fn touch(pos: Pos2, phase: egui::TouchPhase) -> Event {
    Event::Touch {
        device_id: egui::TouchDeviceId(0),
        id: egui::TouchId(0),
        phase,
        pos,
        force: None,
    }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn touch_down(f: &mut Fields, p: Pos2) {
    f.frame(vec![
        touch(p, egui::TouchPhase::Start),
        Event::PointerMoved(p),
        button(p, true),
    ]);
}

fn touch_move(f: &mut Fields, p: Pos2) {
    f.frame(vec![
        touch(p, egui::TouchPhase::Move),
        Event::PointerMoved(p),
    ]);
}

fn touch_up(f: &mut Fields, p: Pos2) {
    f.frame(vec![
        touch(p, egui::TouchPhase::End),
        Event::PointerMoved(p),
        button(p, false),
    ]);
    f.frame(vec![Event::PointerGone]);
    f.frame(vec![]);
}

fn swipe(f: &mut Fields, from: Pos2, to: Pos2) {
    touch_down(f, from);
    for k in 1..=4 {
        touch_move(f, from + (to - from) * (k as f32 / 4.0));
    }
    touch_up(f, to);
}

#[test]
fn touch_tap_opens_editor() {
    let mut f = Fields::new(false);
    let p = f.center(1);
    touch_down(&mut f, p);
    touch_up(&mut f, p);
    assert!(f.editing());
}

#[test]
fn touch_swipe_within_a_cell_cancels() {
    for vertical in [false, true] {
        let mut f = Fields::new(vertical);
        let p = f.center(1);
        // Far enough to exceed the click distance, small enough to stay put.
        swipe(&mut f, p - vec2(0.0, 8.0), p + vec2(0.0, 8.0));
        assert!(!f.editing(), "vertical={vertical}");
    }
}

#[test]
fn touch_vertical_swipe_along_vertical_group_cancels() {
    let mut f = Fields::new(true);
    let (a, b) = (f.center(0), f.center(2));
    swipe(&mut f, a, b);
    assert!(!f.editing());
    assert_eq!(f.vals, [1.0, 2.0, 3.0]);
}

#[test]
fn touch_swipe_arriving_on_release_frame_cancels() {
    let mut f = Fields::new(true);
    let (a, b) = (f.center(0), f.center(2));
    touch_down(&mut f, a);
    touch_up(&mut f, b);
    assert!(!f.editing());
}

#[test]
fn touch_horizontal_drag_across_cells_selects() {
    let mut f = Fields::new(false);
    let (a, b) = (f.center(0), f.center(2));
    swipe(&mut f, a, b);
    assert!(f.editing());
}

#[test]
fn touch_long_press_then_vertical_drag_selects() {
    let mut f = Fields::new(true);
    let (a, b) = (f.center(0), f.center(2));
    touch_down(&mut f, a);
    for _ in 0..70 {
        f.frame(vec![]);
    }
    touch_move(&mut f, b);
    touch_up(&mut f, b);
    assert!(f.editing());
}

#[test]
fn mouse_drag_across_cells_selects_vertically_without_hold() {
    let mut f = Fields::new(true);
    let (a, b) = (f.center(0), f.center(2));
    f.frame(vec![Event::PointerMoved(a), button(a, true)]);
    f.frame(vec![Event::PointerMoved(b)]);
    f.frame(vec![Event::PointerMoved(b), button(b, false)]);
    f.frame(vec![]);
    assert!(f.editing());
}

#[test]
fn mouse_click_opens_editor_even_with_slight_movement() {
    let mut f = Fields::new(false);
    let p = f.center(1);
    f.frame(vec![Event::PointerMoved(p), button(p, true)]);
    f.frame(vec![Event::PointerMoved(p + vec2(0.0, 4.0))]);
    f.frame(vec![
        Event::PointerMoved(p + vec2(0.0, 4.0)),
        button(p + vec2(0.0, 4.0), false),
    ]);
    f.frame(vec![]);
    assert!(f.editing());
}
