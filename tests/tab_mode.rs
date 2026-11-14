//! `FieldGroup::tab_behavior`: what Tab does past the ends of the group.

use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, pos2, vec2};
use egui_control_groups::{FieldGroup, TabBehavior};

struct Fields {
    ctx: Context,
    time: f64,
    vals: [f32; 3],
    rect: Rect,
    mode: TabBehavior,
    before_focus: bool,
    after_focus: bool,
}

impl Fields {
    fn new(mode: TabBehavior) -> Self {
        let mut f = Self {
            ctx: Context::default(),
            time: 0.0,
            vals: [1.0, 2.0, 3.0],
            rect: Rect::NOTHING,
            mode,
            before_focus: false,
            after_focus: false,
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
        let (vals, mode) = (&mut self.vals, self.mode);
        let (mut rect, mut bf, mut af) = (Rect::NOTHING, false, false);
        let out = self.ctx.run_ui(input, |ui: &mut Ui| {
            bf = ui.button("before").has_focus();
            let [a, b, c] = vals;
            let g = FieldGroup::new("f")
                .field(a)
                .field(b)
                .field(c)
                .tab_behavior(mode);
            rect = ui.add(g.cell_width(80.0)).rect;
            af = ui.button("after").has_focus();
        });
        self.rect = rect;
        self.before_focus = bf;
        self.after_focus = af;
        out.drop_without_applying_deltas();
    }

    fn center(&self, i: usize) -> Pos2 {
        pos2(
            self.rect.left() + self.rect.width() / 3.0 * (i as f32 + 0.5),
            self.rect.center().y,
        )
    }

    fn editing(&self) -> bool {
        self.ctx.egui_wants_keyboard_input()
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
        for pressed in [true, false] {
            self.frame(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }]);
        }
        self.frame(vec![]);
        self.frame(vec![]);
    }

    fn focused(&self) -> bool {
        self.ctx.memory(|m| m.focused()).is_some()
    }
}

#[test]
fn default_is_leave() {
    assert_eq!(TabBehavior::default(), TabBehavior::Leave);
}

#[test]
fn group_without_explicit_mode_leaves() {
    let ctx = Context::default();
    let mut v = [1.0_f32, 2.0];
    let mut run = |events: Vec<Event>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            events,
            ..Default::default()
        };
        let mut after = false;
        let mut rect = Rect::NOTHING;
        let out = ctx.run_ui(input, |ui: &mut Ui| {
            let [a, b] = &mut v;
            rect = ui.add(FieldGroup::new("g").field(a).field(b)).rect;
            after = ui.button("after").has_focus();
        });
        out.drop_without_applying_deltas();
        (rect, after)
    };
    let (rect, _) = run(vec![]);
    let p = pos2(rect.right() - 10.0, rect.center().y);
    let b = |pressed| Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    run(vec![Event::PointerMoved(p), b(true)]);
    run(vec![Event::PointerMoved(p), b(false)]);
    run(vec![]);
    run(vec![]);
    let tab = |pressed| Event::Key {
        key: Key::Tab,
        physical_key: Some(Key::Tab),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    run(vec![tab(true)]);
    run(vec![tab(false)]);
    let (_, after) = run(vec![]);
    assert!(after);
}

#[test]
fn stop_commits_and_drops_focus_at_both_ends() {
    let mut f = Fields::new(TabBehavior::Stop);
    f.click(2);
    f.type_text("5");
    f.key(Key::Tab, Modifiers::NONE);
    assert_eq!(f.vals, [1.0, 2.0, 5.0]);
    assert!(!f.focused());
    f.click(0);
    f.type_text("4");
    f.key(Key::Tab, Modifiers::SHIFT);
    assert_eq!(f.vals, [4.0, 2.0, 5.0]);
    assert!(!f.focused());
}

#[test]
fn leave_moves_focus_to_next_widget() {
    let mut f = Fields::new(TabBehavior::Leave);
    f.click(2);
    f.type_text("5");
    f.key(Key::Tab, Modifiers::NONE);
    assert_eq!(f.vals, [1.0, 2.0, 5.0]);
    assert!(
        f.after_focus,
        "focus moved on to the widget after the group"
    );
    assert!(!f.before_focus);
}

#[test]
fn leave_moves_focus_to_previous_widget() {
    let mut f = Fields::new(TabBehavior::Leave);
    f.click(0);
    f.type_text("5");
    f.key(Key::Tab, Modifiers::SHIFT);
    assert_eq!(f.vals, [5.0, 2.0, 3.0]);
    assert!(f.before_focus, "focus moved to the widget before the group");
    assert!(!f.after_focus);
}

#[test]
fn leave_still_tabs_inside_the_group() {
    let mut f = Fields::new(TabBehavior::Leave);
    f.click(0);
    f.type_text("9");
    f.key(Key::Tab, Modifiers::NONE);
    assert_eq!(f.vals, [9.0, 2.0, 3.0]);
    assert!(f.editing());
    f.type_text("8");
    f.key(Key::Tab, Modifiers::SHIFT);
    assert_eq!(f.vals, [9.0, 8.0, 3.0]);
    assert!(f.editing());
    assert!(!f.before_focus && !f.after_focus);
}

#[test]
fn wrap_forward_from_last_opens_first() {
    let mut f = Fields::new(TabBehavior::Wrap);
    f.click(2);
    f.type_text("5");
    f.key(Key::Tab, Modifiers::NONE);
    assert_eq!(f.vals, [1.0, 2.0, 5.0]);
    assert!(f.editing());
    f.type_text("7");
    f.key(Key::Enter, Modifiers::NONE);
    assert_eq!(f.vals, [7.0, 2.0, 5.0]);
}

#[test]
fn wrap_backward_from_first_opens_last() {
    let mut f = Fields::new(TabBehavior::Wrap);
    f.click(0);
    f.type_text("5");
    f.key(Key::Tab, Modifiers::SHIFT);
    assert_eq!(f.vals, [5.0, 2.0, 3.0]);
    assert!(f.editing());
    f.type_text("7");
    f.key(Key::Enter, Modifiers::NONE);
    assert_eq!(f.vals, [5.0, 2.0, 7.0]);
}

#[test]
fn wrap_unchanged_text_does_not_change_values() {
    let mut f = Fields::new(TabBehavior::Wrap);
    f.click(2);
    f.key(Key::Tab, Modifiers::NONE);
    assert!(f.editing());
    assert_eq!(f.vals, [1.0, 2.0, 3.0]);
}
