//! Keyboard handling of `FieldGroup`: reaching off-screen cells with Tab,
//! keeping focus after Enter / Escape, and a quick Tab on a fresh editor.

use egui::{Context, Event, Id, Key, Modifiers, Pos2, RawInput, Rect, ScrollArea, Ui, vec2};
use egui_control_groups::{FieldGroup, TabBehavior};

struct H {
    ctx: Context,
    time: f64,
    vals: [f32; 3],
    scroll: bool,
    group_id: Id,
    after_focus: bool,
    mode: TabBehavior,
}

impl H {
    fn new(scroll: bool) -> Self {
        let mut h = Self {
            ctx: Context::default(),
            time: 0.0,
            vals: [1.0, 2.0, 3.0],
            scroll,
            group_id: Id::NULL,
            after_focus: false,
            mode: TabBehavior::Leave,
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
        let (vals, scroll, mode) = (&mut self.vals, self.scroll, self.mode);
        let (mut gid, mut af) = (Id::NULL, false);
        let out = self.ctx.run_ui(input, |ui: &mut Ui| {
            let mut group = |ui: &mut Ui| {
                gid = ui.make_persistent_id(Id::new("g"));
                let [a, b, c] = &mut *vals;
                ui.add(
                    FieldGroup::new("g")
                        .field(a)
                        .field(b)
                        .field(c)
                        .tab_behavior(mode)
                        .cell_width(80.0),
                );
            };
            if scroll {
                ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                    let _ = ui.button("inside");
                    ui.add_space(1000.0);
                    group(ui);
                });
            } else {
                group(ui);
            }
            af = ui.button("after").has_focus();
        });
        self.group_id = gid;
        self.after_focus = af;
        out.drop_without_applying_deltas();
    }

    fn press(&mut self, key: Key, modifiers: Modifiers) {
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

    fn tab(&mut self) {
        self.press(Key::Tab, Modifiers::NONE);
    }

    fn type_text(&mut self, s: &str) {
        self.frame(vec![Event::Text(s.to_owned())]);
        self.frame(vec![]);
    }

    fn focused(&self) -> Option<Id> {
        self.ctx.memory(|m| m.focused())
    }

    fn cell(&self, i: usize) -> Id {
        self.group_id.with(i)
    }

    fn editing(&self) -> bool {
        self.ctx.text_edit_focused()
    }

    fn focus_cell(&mut self, i: usize) {
        let id = self.cell(i);
        self.ctx.memory_mut(|m| m.request_focus(id));
        self.frame(vec![]);
        assert_eq!(self.focused(), Some(id));
    }
}

#[test]
fn tab_reaches_offscreen_cells() {
    let mut h = H::new(true);
    let mut reached = vec![];
    for _ in 0..8 {
        h.tab();
        if let Some(f) = h.focused() {
            for i in 0..3 {
                if f == h.cell(i) && !reached.contains(&i) {
                    reached.push(i);
                }
            }
        }
        if h.after_focus {
            break;
        }
    }
    assert_eq!(reached, vec![0, 1, 2], "every cell is reachable with Tab");
    assert!(h.after_focus, "Tab continues on to the widget after");
}

#[test]
fn enter_commit_keeps_focus_on_cell_and_does_not_reopen() {
    let mut h = H::new(false);
    h.tab();
    assert_eq!(h.focused(), Some(h.cell(0)));
    h.press(Key::Enter, Modifiers::NONE);
    assert!(h.editing());
    h.type_text("9");
    h.press(Key::Enter, Modifiers::NONE);
    assert_eq!(h.vals, [9.0, 2.0, 3.0]);
    assert!(!h.editing(), "the committing Enter must not re-open it");
    assert_eq!(h.focused(), Some(h.cell(0)));
    h.tab();
    assert_eq!(h.focused(), Some(h.cell(1)), "Tab moves to the next cell");
    assert!(!h.editing());
}

#[test]
fn escape_cancel_keeps_focus_on_cell_and_does_not_reopen() {
    let mut h = H::new(false);
    h.tab();
    h.tab();
    assert_eq!(h.focused(), Some(h.cell(1)));
    h.press(Key::Enter, Modifiers::NONE);
    assert!(h.editing());
    h.type_text("9");
    h.press(Key::Escape, Modifiers::NONE);
    assert_eq!(h.vals, [1.0, 2.0, 3.0]);
    assert!(!h.editing());
    assert_eq!(h.focused(), Some(h.cell(1)));
    h.tab();
    assert_eq!(h.focused(), Some(h.cell(2)));
}

#[test]
fn enter_again_after_commit_reopens() {
    let mut h = H::new(false);
    h.focus_cell(2);
    h.press(Key::Enter, Modifiers::NONE);
    h.press(Key::Enter, Modifiers::NONE);
    assert!(!h.editing());
    h.press(Key::Enter, Modifiers::NONE);
    assert!(h.editing(), "Enter on the focused cell opens it again");
}

#[test]
fn tab_on_first_frame_of_fresh_editor_moves_on() {
    let mut h = H::new(false);
    h.focus_cell(0);
    // Enter and Tab arrive on consecutive frames: the Tab lands on the
    // editor's first frame.
    let key = |key, pressed| Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    h.frame(vec![key(Key::Enter, true), key(Key::Enter, false)]);
    h.frame(vec![key(Key::Tab, true), key(Key::Tab, false)]);
    h.frame(vec![]);
    h.frame(vec![]);
    assert!(h.editing());
    h.type_text("7");
    h.press(Key::Enter, Modifiers::NONE);
    assert_eq!(h.vals, [1.0, 7.0, 3.0], "the second cell was opened");
}

#[test]
fn tab_on_first_frame_of_fresh_editor_at_last_cell_leaves() {
    let mut h = H::new(false);
    h.focus_cell(2);
    let key = |key, pressed| Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    h.frame(vec![key(Key::Enter, true), key(Key::Enter, false)]);
    h.frame(vec![key(Key::Tab, true), key(Key::Tab, false)]);
    h.frame(vec![]);
    h.frame(vec![]);
    assert!(!h.editing());
    assert!(
        h.after_focus,
        "focus moved on to the widget after the group"
    );
}

#[test]
fn pointer_opened_edit_leaves_nothing_focused_after_enter() {
    // An edit opened with the mouse must not keep keyboard focus once it is
    // committed, or the app's own shortcuts would stay suppressed.
    let mut h = H::new(false);
    let p = egui::pos2(48.0, 18.0);
    let button = |pressed| Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    h.frame(vec![Event::PointerMoved(p)]);
    h.frame(vec![button(true)]);
    h.frame(vec![button(false)]);
    h.frame(vec![]);
    h.frame(vec![]);
    assert!(h.editing(), "the click opened the editor");
    h.type_text("7");
    h.press(Key::Enter, Modifiers::NONE);
    assert_eq!(h.vals, [7.0, 2.0, 3.0]);
    assert_eq!(h.focused(), None);
}
