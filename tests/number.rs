//! `Number`: precision, range, prefix / suffix and its use inside a `FieldGroup`.

use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, pos2, vec2};
use egui_control_groups::{FieldGroup, FieldValue, Number};

#[test]
fn precision_fixes_decimals() {
    let mut v = 1.5_f64;
    assert_eq!(Number::new(&mut v).precision(3).to_field_text(), "1.500");
    let mut v = 1.23456_f32;
    assert_eq!(Number::new(&mut v).precision(2).to_field_text(), "1.23");
    let mut v = 2.0_f64;
    assert_eq!(Number::new(&mut v).precision(0).to_field_text(), "2");
    let mut v = -0.001_f64;
    assert_eq!(Number::new(&mut v).precision(2).to_field_text(), "0.00");
}

#[test]
fn default_matches_plain_float_text() {
    let mut v = 1.5_f64;
    assert_eq!(Number::new(&mut v).to_field_text(), v.to_field_text());
    let mut w = 0.1234567891_f64;
    let plain = w.to_field_text();
    assert_eq!(Number::new(&mut w).to_field_text(), plain);
}

#[test]
fn precision_rounds_on_commit() {
    let mut v = 0.0_f64;
    let mut n = Number::new(&mut v).precision(2);
    assert!(n.set_from_field_text("1.2349"));
    assert_eq!(v, 1.23);
}

#[test]
fn range_clamps_on_commit() {
    let mut v = 5.0_f32;
    let mut n = Number::new(&mut v).range(0.0..=10.0);
    assert!(n.set_from_field_text("42"));
    assert_eq!(v, 10.0);
    let mut n = Number::new(&mut v).range(0.0..=10.0);
    assert!(n.set_from_field_text("-3"));
    assert_eq!(v, 0.0);
    let mut i = 5_i32;
    let mut n = Number::new(&mut i).range(1..=9);
    assert!(n.set_from_field_text("100"));
    assert_eq!(i, 9);
}

#[test]
fn invalid_text_is_rejected() {
    let mut v = 5.0_f64;
    let mut n = Number::new(&mut v).suffix(" px");
    assert!(!n.set_from_field_text("abc px"));
    assert!(!n.set_from_field_text(""));
    assert_eq!(v, 5.0);
}

#[test]
fn suffix_and_prefix_round_trip() {
    let mut v = 12.5_f64;
    let mut n = Number::new(&mut v).precision(1).prefix("$").suffix(" px");
    let text = n.to_field_text();
    assert_eq!(text, "$12.5 px");
    assert!(n.set_from_field_text(&text));
    assert_eq!(n.to_field_text(), text, "own text parses back unchanged");
    for input in ["7.5", "7.5 px", "$7.5", "$7.5px", " 7,5 "] {
        assert!(n.set_from_field_text(input), "{input}");
        assert_eq!(n.value_for_test(), 7.5, "{input}");
        assert_eq!(n.to_field_text(), "$7.5 px");
    }
}

#[test]
fn integers_work() {
    let mut a = 3_u8;
    let mut n = Number::new(&mut a).suffix("%").range(0..=100);
    assert_eq!(n.to_field_text(), "3%");
    assert!(n.set_from_field_text("250%"));
    assert!(!n.set_from_field_text("1.5"));
    assert_eq!(a, 100);
    let mut b = -4_i64;
    assert_eq!(Number::new(&mut b).precision(3).to_field_text(), "-4");
}

trait ValueForTest {
    fn value_for_test(&self) -> f64;
}
impl ValueForTest for Number<'_, f64> {
    fn value_for_test(&self) -> f64 {
        let t = self.to_field_text();
        t.trim_start_matches('$')
            .trim_end_matches(" px")
            .parse()
            .unwrap()
    }
}

struct Fields {
    ctx: Context,
    time: f64,
    vals: [f64; 3],
    rect: Rect,
    changed: bool,
}

impl Fields {
    fn new(vals: [f64; 3]) -> Self {
        let mut f = Self {
            ctx: Context::default(),
            time: 0.0,
            vals,
            rect: Rect::NOTHING,
            changed: false,
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
        let vals = &mut self.vals;
        let (mut rect, mut changed) = (Rect::NOTHING, false);
        let out = self.ctx.run_ui(input, |ui: &mut Ui| {
            let [a, b, c] = vals;
            let mut a = Number::new(a).precision(2).range(0.0..=10.0).suffix(" px");
            let mut b = Number::new(b).precision(2).range(0.0..=10.0).suffix(" px");
            let mut c = Number::new(c).precision(2).range(0.0..=10.0).suffix(" px");
            let g = FieldGroup::new("n")
                .field(&mut a)
                .field(&mut b)
                .field(&mut c)
                .cell_width(80.0);
            let r = ui.add(g);
            rect = r.rect;
            changed |= r.changed();
        });
        self.rect = rect;
        self.changed |= changed;
        out.drop_without_applying_deltas();
    }

    fn center(&self, i: usize) -> Pos2 {
        pos2(
            self.rect.left() + self.rect.width() / 3.0 * (i as f32 + 0.5),
            self.rect.center().y,
        )
    }

    fn click(&mut self, i: usize) {
        let p = self.center(i);
        let b = |pressed| Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(p), b(true)]);
        self.frame(vec![Event::PointerMoved(p), b(false)]);
        self.frame(vec![]);
        self.frame(vec![]);
        assert!(self.ctx.egui_wants_keyboard_input());
    }

    fn drag(&mut self, from: usize, to: usize) {
        let (a, b) = (self.center(from), self.center(to));
        let ev = |p, pressed| Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(a), ev(a, true)]);
        self.frame(vec![Event::PointerMoved(b)]);
        self.frame(vec![Event::PointerMoved(b), ev(b, false)]);
        self.frame(vec![]);
        self.frame(vec![]);
        assert!(self.ctx.egui_wants_keyboard_input());
    }

    fn type_text(&mut self, s: &str) {
        self.frame(vec![Event::Text(s.to_owned())]);
        self.frame(vec![]);
    }

    fn key(&mut self, key: Key) {
        for pressed in [true, false] {
            self.frame(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            }]);
        }
        self.frame(vec![]);
    }

    fn done(&mut self) {
        self.changed = false;
        self.key(Key::Enter);
    }
}

#[test]
fn no_drift_when_clicking_in_and_out() {
    // Neither rounded nor clamped values may be touched by an empty edit.
    let start = [1.234_567, 55.5, -3.0];
    let mut f = Fields::new(start);
    for i in 0..3 {
        f.click(i);
        f.done();
        assert!(!f.changed, "field {i} reported a change");
        assert_eq!(f.vals, start);
    }
}

#[test]
fn edit_is_rounded_and_clamped() {
    let mut f = Fields::new([1.0, 2.0, 3.0]);
    f.click(0);
    f.type_text("2.3456");
    f.done();
    assert!(f.changed);
    assert_eq!(f.vals[0], 2.35);
    f.click(1);
    f.type_text("99");
    f.done();
    assert_eq!(f.vals[1], 10.0);
}

#[test]
fn changed_only_when_value_changes() {
    let mut f = Fields::new([1.0, 2.0, 3.0]);
    f.click(1);
    f.type_text("2.001"); // rounds to the same value
    f.done();
    assert!(!f.changed);
    assert_eq!(f.vals[1], 2.0);
    f.click(1);
    f.type_text("20"); // clamps to a new value
    f.done();
    assert!(f.changed);
    assert_eq!(f.vals[1], 10.0);
    f.click(1);
    f.type_text("10"); // clamps to the same value
    f.done();
    assert!(!f.changed);
}

#[test]
fn multi_edit_writes_to_every_number() {
    let mut f = Fields::new([1.0, 2.0, 3.0]);
    f.drag(0, 2);
    f.type_text("4.567");
    f.done();
    assert!(f.changed);
    assert_eq!(f.vals, [4.57, 4.57, 4.57]);
}

#[test]
fn multi_edit_applies_each_numbers_own_clamp() {
    let mut f = Fields::new([1.0, 2.0, 3.0]);
    f.drag(0, 1);
    f.type_text("50");
    f.done();
    assert_eq!(f.vals, [10.0, 10.0, 3.0]);
}

#[test]
fn integers_saturate_out_of_range_text() {
    let mut v = 5_u8;
    let mut n = Number::new(&mut v).range(0..=100);
    assert!(n.set_from_field_text("300"));
    drop(n);
    assert_eq!(v, 100);
    let mut n = Number::new(&mut v).range(10..=100);
    assert!(n.set_from_field_text("-5"));
    drop(n);
    assert_eq!(v, 10);
    let mut n = Number::new(&mut v);
    assert!(n.set_from_field_text("99999999999999999999999"));
    drop(n);
    assert_eq!(v, u8::MAX);
    let mut w = 0_i8;
    let mut n = Number::new(&mut w);
    assert!(n.set_from_field_text("-1000"));
    drop(n);
    assert_eq!(w, i8::MIN);
    let mut n = Number::new(&mut w);
    assert!(n.set_from_field_text("+1000"));
    drop(n);
    assert_eq!(w, i8::MAX);
}

#[test]
fn integers_still_reject_non_integers() {
    let mut v = 7_u8;
    let mut n = Number::new(&mut v);
    for bad in ["", "-", "+", "abc", "1.5", "3e2", "12x", "--1"] {
        assert!(!n.set_from_field_text(bad), "{bad:?}");
    }
    drop(n);
    assert_eq!(v, 7);
}
