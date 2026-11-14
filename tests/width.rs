//! Whole-group width control: `width` and `fill_width` on both widgets.

use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, pos2, vec2};
use egui_control_groups::{FieldGroup, ToggleGroup};

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

    fn frame(&mut self, events: Vec<Event>, mut ui_fn: impl FnMut(&mut Ui)) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let output = self.ctx.run_ui(input, |ui| ui_fn(ui));
        output.drop_without_applying_deltas();
    }

    fn press(&mut self, pos: Pos2, ui_fn: impl FnMut(&mut Ui)) {
        self.frame(button(pos, true), ui_fn);
    }

    fn release(&mut self, pos: Pos2, ui_fn: impl FnMut(&mut Ui)) {
        self.frame(button(pos, false), ui_fn);
    }

    fn moved(&mut self, pos: Pos2, ui_fn: impl FnMut(&mut Ui)) {
        self.frame(vec![Event::PointerMoved(pos)], ui_fn);
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

#[derive(Clone, Copy, Debug)]
enum Mode {
    Auto,
    Fixed(f32),
    Fill,
}

/// Show a toggle group and return (its rect, available width before it).
fn toggles(ui: &mut Ui, v: &mut [bool; 3], mode: Mode, vertical: bool, equal: bool) -> (Rect, f32) {
    let avail = ui.available_width();
    let [a, b, c] = v;
    let mut g = ToggleGroup::new("t")
        .cell("A", a)
        .cell("Beta", b)
        .cell("C", c);
    g = match mode {
        Mode::Auto => g,
        Mode::Fixed(w) => g.width(w),
        Mode::Fill => g.fill_width(),
    };
    if vertical {
        g = g.vertical();
    }
    if equal {
        g = g.equal_width();
    }
    (ui.add(g).rect, avail)
}

fn fields(ui: &mut Ui, v: &mut [f32; 3], mode: Mode, vertical: bool) -> (Rect, f32) {
    let avail = ui.available_width();
    let [a, b, c] = v;
    let mut g = FieldGroup::new("f")
        .labeled("X", a)
        .labeled("Y", b)
        .labeled("Z", c)
        .cell_width(40.0);
    g = match mode {
        Mode::Auto => g,
        Mode::Fixed(w) => g.width(w),
        Mode::Fill => g.fill_width(),
    };
    if vertical {
        g = g.vertical();
    }
    (ui.add(g).rect, avail)
}

fn toggle_rect(mode: Mode, vertical: bool, equal: bool) -> (Rect, f32) {
    let mut h = Harness::new();
    let mut v = [false; 3];
    let mut out = (Rect::NOTHING, 0.0);
    h.frame(vec![], |ui| {
        out = toggles(ui, &mut v, mode, vertical, equal)
    });
    out
}

fn field_rect(mode: Mode, vertical: bool) -> (Rect, f32) {
    let mut h = Harness::new();
    let mut v = [0.0; 3];
    let mut out = (Rect::NOTHING, 0.0);
    h.frame(vec![], |ui| out = fields(ui, &mut v, mode, vertical));
    out
}

#[test]
fn fixed_width_is_exact() {
    for vertical in [false, true] {
        for w in [90.0, 250.0, 400.0] {
            let (r, _) = toggle_rect(Mode::Fixed(w), vertical, false);
            assert!((r.width() - w).abs() < 1e-3, "toggle {vertical} {w}: {r:?}");
            let (r, _) = field_rect(Mode::Fixed(w), vertical);
            assert!((r.width() - w).abs() < 1e-3, "field {vertical} {w}: {r:?}");
        }
    }
}

#[test]
fn fill_width_matches_available() {
    for vertical in [false, true] {
        let (r, avail) = toggle_rect(Mode::Fill, vertical, false);
        assert!(avail > 100.0);
        assert!((r.width() - avail).abs() < 1e-3, "{r:?} vs {avail}");
        let (r, avail) = field_rect(Mode::Fill, vertical);
        assert!((r.width() - avail).abs() < 1e-3, "{r:?} vs {avail}");
    }
}

#[test]
fn default_is_unchanged() {
    // Natural widths: not stretched to the available width.
    let (r, avail) = toggle_rect(Mode::Auto, false, false);
    assert!(r.width() < avail);
    let (r2, _) = toggle_rect(Mode::Auto, false, true);
    assert!(r2.width() >= r.width());
    let (f, _) = field_rect(Mode::Auto, false);
    assert!((f.width() - 120.0).abs() < 1e-3);
    let (f, _) = field_rect(Mode::Auto, true);
    assert!((f.width() - 40.0).abs() < 1e-3);
    // Height does not depend on the requested width.
    let (a, _) = toggle_rect(Mode::Auto, false, false);
    let (b, _) = toggle_rect(Mode::Fixed(300.0), false, false);
    assert!((a.height() - b.height()).abs() < 1e-3);
}

#[test]
fn width_wins_over_equal_width_and_keeps_cells_equal() {
    let (r, _) = toggle_rect(Mode::Fixed(300.0), false, true);
    assert!((r.width() - 300.0).abs() < 1e-3);
}

#[test]
fn narrow_width_still_exact() {
    let (r, _) = toggle_rect(Mode::Fixed(30.0), false, false);
    assert!((r.width() - 30.0).abs() < 1e-3);
}

#[test]
fn invalid_width_falls_back_to_auto() {
    let (auto, _) = toggle_rect(Mode::Auto, false, false);
    let (r, _) = toggle_rect(Mode::Fixed(f32::NAN), false, false);
    assert_eq!(r.width(), auto.width());
    let (r, _) = toggle_rect(Mode::Fixed(f32::INFINITY), false, false);
    assert_eq!(r.width(), auto.width());
}

#[test]
fn vertical_stretch_keeps_height() {
    let (a, _) = toggle_rect(Mode::Auto, true, false);
    let (b, _) = toggle_rect(Mode::Fixed(300.0), true, false);
    assert!((a.height() - b.height()).abs() < 1e-3);
    assert!((b.width() - 300.0).abs() < 1e-3);
}

#[test]
fn hit_testing_follows_stretched_cells() {
    let mut h = Harness::new();
    let mut v = [false; 3];
    let mut rect = Rect::NOTHING;
    let show = |v: &mut [bool; 3], rect: &mut Rect, ui: &mut Ui| {
        *rect = toggles(ui, v, Mode::Fixed(300.0), false, true).0;
    };
    h.frame(vec![], |ui| show(&mut v, &mut rect, ui));
    h.frame(vec![], |ui| show(&mut v, &mut rect, ui));
    let at = |rect: Rect, i: f32| {
        pos2(
            rect.left() + rect.width() * (i + 0.5) / 3.0,
            rect.center().y,
        )
    };
    // Click the last cell: far beyond where it would sit unstretched.
    let p = at(rect, 2.0);
    h.moved(p, |ui| show(&mut v, &mut rect, ui));
    h.press(p, |ui| show(&mut v, &mut rect, ui));
    h.release(p, |ui| show(&mut v, &mut rect, ui));
    assert_eq!(v, [false, false, true]);
    // Drag from the first cell across to the last.
    let (p0, p2) = (at(rect, 0.0), at(rect, 2.0));
    h.moved(p0, |ui| show(&mut v, &mut rect, ui));
    h.press(p0, |ui| show(&mut v, &mut rect, ui));
    h.moved(at(rect, 1.0), |ui| show(&mut v, &mut rect, ui));
    h.moved(p2, |ui| show(&mut v, &mut rect, ui));
    h.release(p2, |ui| show(&mut v, &mut rect, ui));
    assert_eq!(v, [true, true, true]);
}

#[test]
fn vertical_stretched_hit_testing() {
    let mut h = Harness::new();
    let mut v = [false; 3];
    let mut rect = Rect::NOTHING;
    let show = |v: &mut [bool; 3], rect: &mut Rect, ui: &mut Ui| {
        *rect = toggles(ui, v, Mode::Fill, true, false).0;
    };
    h.frame(vec![], |ui| show(&mut v, &mut rect, ui));
    h.frame(vec![], |ui| show(&mut v, &mut rect, ui));
    // Far right of a full-width column still hits the middle cell.
    let p = pos2(rect.right() - 5.0, rect.top() + rect.height() * 0.5);
    h.moved(p, |ui| show(&mut v, &mut rect, ui));
    h.press(p, |ui| show(&mut v, &mut rect, ui));
    h.release(p, |ui| show(&mut v, &mut rect, ui));
    assert_eq!(v, [false, true, false]);
}

#[test]
fn field_group_click_in_stretched_cell_edits_it() {
    let mut h = Harness::new();
    let mut v = [1.0_f32, 2.0, 3.0];
    let mut rect = Rect::NOTHING;
    let show = |v: &mut [f32; 3], rect: &mut Rect, ui: &mut Ui| {
        *rect = fields(ui, v, Mode::Fixed(450.0), false).0;
    };
    h.frame(vec![], |ui| show(&mut v, &mut rect, ui));
    h.frame(vec![], |ui| show(&mut v, &mut rect, ui));
    assert!((rect.width() - 450.0).abs() < 1e-3);
    let p = pos2(rect.left() + rect.width() * 5.0 / 6.0, rect.center().y);
    h.moved(p, |ui| show(&mut v, &mut rect, ui));
    h.press(p, |ui| show(&mut v, &mut rect, ui));
    h.release(p, |ui| show(&mut v, &mut rect, ui));
    h.frame(vec![Event::Text("9".into())], |ui| {
        show(&mut v, &mut rect, ui)
    });
    h.frame(
        vec![Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        |ui| show(&mut v, &mut rect, ui),
    );
    h.frame(vec![], |ui| show(&mut v, &mut rect, ui));
    assert_eq!(v, [1.0, 2.0, 9.0]);
}
