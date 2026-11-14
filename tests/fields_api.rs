//! Tab navigation and the bulk constructor of `FieldGroup`.

use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, pos2, vec2};
use egui_control_groups::{FieldGroup, TabBehavior};

struct Fields {
    ctx: Context,
    time: f64,
    vals: [f32; 3],
    rect: Rect,
    changed: bool,
    bulk: bool,
}

impl Fields {
    fn new(bulk: bool) -> Self {
        let mut f = Self {
            ctx: Context::default(),
            time: 0.0,
            vals: [1.0, 2.0, 3.0],
            rect: Rect::NOTHING,
            changed: false,
            bulk,
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
        let (vals, bulk) = (&mut self.vals, self.bulk);
        let (mut rect, mut changed) = (Rect::NOTHING, false);
        let out = self.ctx.run_ui(input, |ui: &mut Ui| {
            let g = if bulk {
                FieldGroup::new("f").fields(["X", "Y", "Z"], vals.as_mut_slice())
            } else {
                let [a, b, c] = vals;
                FieldGroup::new("f").field(a).field(b).field(c)
            };
            let r = ui.add(g.cell_width(80.0).tab_behavior(TabBehavior::Stop));
            rect = r.rect;
            changed = r.changed();
        });
        self.rect = rect;
        self.changed = changed;
        out.drop_without_applying_deltas();
    }

    fn center(&self, i: usize) -> Pos2 {
        pos2(
            self.rect.left() + self.rect.width() / 3.0 * (i as f32 + 0.5),
            self.rect.center().y,
        )
    }

    fn editing(&self) -> bool {
        self.ctx.text_edit_focused()
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
        assert!(self.editing());
    }

    fn type_text(&mut self, s: &str) {
        self.frame(vec![Event::Text(s.to_owned())]);
        self.frame(vec![]);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(vec![
            Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers,
            },
            Event::Key {
                key,
                physical_key: Some(key),
                pressed: false,
                repeat: false,
                modifiers,
            },
        ]);
        self.frame(vec![]);
    }
}

#[test]
fn tab_commits_and_opens_next_field() {
    let mut f = Fields::new(false);
    f.click(0);
    f.type_text("9");
    f.key(Key::Tab, Modifiers::NONE);
    assert_eq!(f.vals, [9.0, 2.0, 3.0]);
    assert!(f.editing(), "next field is being edited");
    f.type_text("8");
    f.key(Key::Tab, Modifiers::NONE);
    assert_eq!(f.vals, [9.0, 8.0, 3.0]);
    assert!(f.editing());
}

#[test]
fn tab_on_unchanged_text_does_not_change_anything() {
    let mut f = Fields::new(false);
    f.vals = [1.123_456_7, 2.0, 3.0];
    f.click(0);
    f.frame(vec![Event::Key {
        key: Key::Tab,
        physical_key: Some(Key::Tab),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    assert!(!f.changed);
    f.frame(vec![]);
    assert_eq!(f.vals, [1.123_456_7, 2.0, 3.0]);
    assert!(f.editing());
}

#[test]
fn shift_tab_goes_to_previous_field() {
    let mut f = Fields::new(false);
    f.click(2);
    f.type_text("7");
    f.key(Key::Tab, Modifiers::SHIFT);
    assert_eq!(f.vals, [1.0, 2.0, 7.0]);
    assert!(f.editing());
    f.type_text("6");
    f.key(Key::Enter, Modifiers::NONE);
    assert_eq!(f.vals, [1.0, 6.0, 7.0]);
    assert!(!f.editing());
}

#[test]
fn tab_on_last_field_commits_and_does_not_wrap() {
    let mut f = Fields::new(false);
    f.click(2);
    f.type_text("5");
    f.key(Key::Tab, Modifiers::NONE);
    assert_eq!(f.vals, [1.0, 2.0, 5.0]);
    assert!(!f.editing(), "no editor is open after leaving the last");
    assert!(f.ctx.memory(|m| m.focused()).is_none());
}

#[test]
fn shift_tab_on_first_field_commits_and_does_not_wrap() {
    let mut f = Fields::new(false);
    f.click(0);
    f.type_text("5");
    f.key(Key::Tab, Modifiers::SHIFT);
    assert_eq!(f.vals, [5.0, 2.0, 3.0]);
    assert!(!f.editing());
}

#[test]
fn fields_builds_one_field_per_value() {
    let mut f = Fields::new(true);
    f.click(1);
    f.type_text("4");
    f.key(Key::Enter, Modifiers::NONE);
    assert_eq!(f.vals, [1.0, 4.0, 3.0]);
    // Same geometry as three separate `field` calls.
    let g = Fields::new(false);
    assert_eq!(f.rect, g.rect);
}

#[test]
fn offscreen_group_still_allocates_its_space() {
    let ctx = Context::default();
    let mut v = [1.0_f32, 2.0];
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
        ..Default::default()
    };
    let mut rects = Vec::new();
    let out = ctx.run_ui(input, |ui: &mut Ui| {
        let [a, b] = &mut v;
        let on = ui.add(FieldGroup::new("on").field(a).cell_width(80.0)).rect;
        ui.allocate_space(vec2(1.0, 2000.0));
        let off = ui.add(FieldGroup::new("off").field(b).cell_width(80.0));
        rects.push((on, off.rect, off.changed()));
        rects.push((ui.min_rect(), Rect::NOTHING, false));
    });
    out.drop_without_applying_deltas();
    let (on, off, changed) = rects[0];
    assert!(!changed);
    assert_eq!(on.size(), off.size());
    assert!(off.top() > 2000.0);
    assert!(rects[1].0.bottom() >= off.bottom());
}
