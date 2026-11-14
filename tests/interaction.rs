//! Headless interaction tests: drive the widgets with scripted pointer and
//! keyboard events, one frame at a time, exactly as a backend would.

use egui::{
    Color32, ColorImage, Context, Event, Image, Key, Modifiers, PointerButton, Pos2, RawInput,
    Rect, Shape, TextureOptions, Ui, Vec2, pos2, vec2,
};
use egui_control_groups::{CellContent, FieldGroup, Icon, Orientation, PaintCell, ToggleGroup};
use std::cell::RefCell;

/// Text strings and rectangle fills found in one frame's output.
#[derive(Default)]
struct Painted {
    texts: Vec<String>,
    fills: Vec<Color32>,
}

fn collect(shape: &Shape, out: &mut Painted) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
        Shape::Text(t) => out.texts.push(t.galley.text().to_owned()),
        Shape::Rect(r) => out.fills.push(r.fill),
        _ => {}
    }
}

struct Harness {
    ctx: Context,
    time: f64,
    modifiers: Modifiers,
    sent_modifiers: Modifiers,
    painted: Painted,
}

impl Harness {
    fn new() -> Self {
        Self {
            ctx: Context::default(),
            time: 0.0,
            modifiers: Modifiers::NONE,
            sent_modifiers: Modifiers::NONE,
            painted: Painted::default(),
        }
    }

    fn frame(&mut self, events: Vec<Event>, mut ui_fn: impl FnMut(&mut Ui)) {
        self.time += 1.0 / 60.0;
        let mut events = events;
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
        let output = self.ctx.run_ui(input, |ui| ui_fn(ui));
        self.painted = Painted::default();
        for clipped in &output.shapes {
            collect(&clipped.shape, &mut self.painted);
        }
        output.drop_without_applying_deltas();
    }

    fn moved(&mut self, pos: Pos2, ui_fn: impl FnMut(&mut Ui)) {
        self.frame(vec![Event::PointerMoved(pos)], ui_fn);
    }

    fn press(&mut self, pos: Pos2, ui_fn: impl FnMut(&mut Ui)) {
        self.frame(button(pos, true), ui_fn);
    }

    fn release(&mut self, pos: Pos2, ui_fn: impl FnMut(&mut Ui)) {
        self.frame(button(pos, false), ui_fn);
    }

    fn idle(&mut self, mut ui_fn: impl FnMut(&mut Ui)) {
        self.frame(vec![], &mut ui_fn);
    }

    fn type_text(&mut self, text: &str, ui_fn: impl FnMut(&mut Ui)) {
        self.frame(vec![Event::Text(text.to_owned())], ui_fn);
    }

    fn key(&mut self, key: Key, ui_fn: impl FnMut(&mut Ui)) {
        let ev = |pressed| Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![ev(true), ev(false)], ui_fn);
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

// ---------------------------------------------------------------- toggles

struct Toggles {
    v: Vec<bool>,
    rect: Rect,
    orientation: Orientation,
}

impl Toggles {
    fn new(v: Vec<bool>) -> Self {
        Self {
            v,
            rect: Rect::NOTHING,
            orientation: Orientation::Horizontal,
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        let labels = ["A", "B", "C", "D"];
        let n = self.v.len();
        let mut g = ToggleGroup::new("t")
            .cells(labels.into_iter().take(n), &mut self.v)
            .equal_width();
        if self.orientation == Orientation::Vertical {
            g = g.vertical();
        }
        self.rect = ui.add(g).rect;
    }

    fn center(&self, i: usize) -> Pos2 {
        let n = self.v.len() as f32;
        match self.orientation {
            Orientation::Horizontal => pos2(
                self.rect.left() + self.rect.width() / n * (i as f32 + 0.5),
                self.rect.center().y,
            ),
            Orientation::Vertical => pos2(
                self.rect.center().x,
                self.rect.top() + self.rect.height() / n * (i as f32 + 0.5),
            ),
        }
    }
}

fn warm(h: &mut Harness, t: &mut Toggles) {
    for _ in 0..3 {
        h.idle(|ui| t.show(ui));
    }
}

#[test]
fn click_flips_exactly_once() {
    let (mut h, mut t) = (Harness::new(), Toggles::new(vec![false, false, false]));
    warm(&mut h, &mut t);
    let p = t.center(1);
    h.moved(p, |ui| t.show(ui));
    h.press(p, |ui| t.show(ui));
    h.release(p, |ui| t.show(ui));
    h.idle(|ui| t.show(ui));
    assert_eq!(t.v, [false, true, false]);
}

#[test]
fn drag_paints_neighbours_on() {
    let (mut h, mut t) = (Harness::new(), Toggles::new(vec![false; 4]));
    warm(&mut h, &mut t);
    let start = t.center(0);
    h.moved(start, |ui| t.show(ui));
    h.press(start, |ui| t.show(ui));
    for i in 1..4 {
        let p = t.center(i);
        h.moved(p, |ui| t.show(ui));
    }
    let end = t.center(3);
    h.release(end, |ui| t.show(ui));
    assert_eq!(t.v, [true; 4]);
}

#[test]
fn drag_paints_off_when_first_cell_was_on() {
    let (mut h, mut t) = (Harness::new(), Toggles::new(vec![true, true, true, true]));
    warm(&mut h, &mut t);
    let start = t.center(1);
    h.moved(start, |ui| t.show(ui));
    h.press(start, |ui| t.show(ui));
    h.moved(t.center(0), |ui| t.show(ui));
    h.moved(t.center(3), |ui| t.show(ui));
    h.release(t.center(3), |ui| t.show(ui));
    assert_eq!(t.v, [false; 4]);
}

#[test]
fn mixed_states_are_set_to_one_target() {
    // Start on an OFF cell while a later cell is already ON: the ON one must
    // become ON (not flip back to OFF).
    let (mut h, mut t) = (
        Harness::new(),
        Toggles::new(vec![false, false, true, false]),
    );
    warm(&mut h, &mut t);
    let start = t.center(0);
    h.moved(start, |ui| t.show(ui));
    h.press(start, |ui| t.show(ui));
    h.moved(t.center(3), |ui| t.show(ui));
    h.release(t.center(3), |ui| t.show(ui));
    assert_eq!(t.v, [true; 4]);
}

#[test]
fn fast_drag_fills_skipped_cells() {
    let (mut h, mut t) = (Harness::new(), Toggles::new(vec![false; 4]));
    warm(&mut h, &mut t);
    let start = t.center(0);
    h.moved(start, |ui| t.show(ui));
    h.press(start, |ui| t.show(ui));
    // One big jump, no intermediate frames.
    h.moved(t.center(3), |ui| t.show(ui));
    h.release(t.center(3), |ui| t.show(ui));
    assert_eq!(t.v, [true; 4]);
}

#[test]
fn drag_survives_leaving_the_group_sideways() {
    let (mut h, mut t) = (Harness::new(), Toggles::new(vec![false; 4]));
    warm(&mut h, &mut t);
    let start = t.center(0);
    h.moved(start, |ui| t.show(ui));
    h.press(start, |ui| t.show(ui));
    // Pointer far below the group, but over the right-hand cells' x range.
    let below = |i: usize| pos2(t.center(i).x, t.rect.bottom() + 150.0);
    let (p2, p3) = (below(2), below(3));
    h.moved(p2, |ui| t.show(ui));
    h.moved(p3, |ui| t.show(ui));
    h.release(p3, |ui| t.show(ui));
    assert_eq!(t.v, [true; 4]);
}

#[test]
fn vertical_group_drags_along_y() {
    let mut t = Toggles::new(vec![false; 3]);
    t.orientation = Orientation::Vertical;
    let mut h = Harness::new();
    warm(&mut h, &mut t);
    let start = t.center(0);
    h.moved(start, |ui| t.show(ui));
    h.press(start, |ui| t.show(ui));
    h.moved(t.center(2), |ui| t.show(ui));
    h.release(t.center(2), |ui| t.show(ui));
    assert_eq!(t.v, [true; 3]);
}

#[test]
fn release_ends_the_gesture() {
    let (mut h, mut t) = (Harness::new(), Toggles::new(vec![false; 3]));
    warm(&mut h, &mut t);
    let start = t.center(0);
    h.moved(start, |ui| t.show(ui));
    h.press(start, |ui| t.show(ui));
    h.release(start, |ui| t.show(ui));
    // Moving afterwards must not paint anything.
    h.moved(t.center(2), |ui| t.show(ui));
    h.moved(t.center(1), |ui| t.show(ui));
    assert_eq!(t.v, [true, false, false]);
}

// ----------------------------------------------------------------- fields

struct Fields {
    v: [f32; 3],
    rect: Rect,
}

impl Fields {
    fn new() -> Self {
        Self {
            v: [1.0, 2.0, 3.0],
            rect: Rect::NOTHING,
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        let [x, y, z] = &mut self.v;
        let g = FieldGroup::new("f")
            .labeled("X", x)
            .labeled("Y", y)
            .labeled("Z", z)
            .cell_width(80.0);
        self.rect = ui.add(g).rect;
    }

    fn center(&self, i: usize) -> Pos2 {
        pos2(
            self.rect.left() + 80.0 * (i as f32 + 0.5),
            self.rect.center().y,
        )
    }
}

fn fwarm(h: &mut Harness, f: &mut Fields) {
    for _ in 0..3 {
        h.idle(|ui| f.show(ui));
    }
}

/// Press at `from`, move to `to`, release there, and let the editor open.
fn gesture(h: &mut Harness, f: &mut Fields, from_i: usize, to_i: usize) {
    let (from, to) = (f.center(from_i), f.center(to_i));
    h.moved(from, |ui| f.show(ui));
    h.press(from, |ui| f.show(ui));
    if from != to {
        h.moved(to, |ui| f.show(ui));
    }
    h.release(to, |ui| f.show(ui));
    for _ in 0..3 {
        h.idle(|ui| f.show(ui));
    }
}

#[test]
fn click_then_type_edits_a_single_field() {
    let (mut h, mut f) = (Harness::new(), Fields::new());
    fwarm(&mut h, &mut f);
    gesture(&mut h, &mut f, 1, 1);
    h.type_text("42", |ui| f.show(ui));
    // Nothing is written until the edit is committed.
    assert_eq!(f.v, [1.0, 2.0, 3.0]);
    h.key(Key::Enter, |ui| f.show(ui));
    h.idle(|ui| f.show(ui));
    assert_eq!(f.v, [1.0, 42.0, 3.0]);
}

#[test]
fn dragging_across_fields_edits_them_together() {
    let (mut h, mut f) = (Harness::new(), Fields::new());
    fwarm(&mut h, &mut f);
    gesture(&mut h, &mut f, 0, 2);
    h.type_text("7,5", |ui| f.show(ui)); // decimal comma accepted
    h.key(Key::Enter, |ui| f.show(ui));
    h.idle(|ui| f.show(ui));
    assert_eq!(f.v, [7.5, 7.5, 7.5]);
}

#[test]
fn drag_selection_works_right_to_left_and_partially() {
    let (mut h, mut f) = (Harness::new(), Fields::new());
    fwarm(&mut h, &mut f);
    gesture(&mut h, &mut f, 2, 1);
    h.type_text("9", |ui| f.show(ui));
    h.key(Key::Enter, |ui| f.show(ui));
    h.idle(|ui| f.show(ui));
    assert_eq!(f.v, [1.0, 9.0, 9.0]);
}

#[test]
fn escape_cancels_the_edit() {
    let (mut h, mut f) = (Harness::new(), Fields::new());
    fwarm(&mut h, &mut f);
    gesture(&mut h, &mut f, 0, 2);
    h.type_text("99", |ui| f.show(ui));
    h.key(Key::Escape, |ui| f.show(ui));
    h.idle(|ui| f.show(ui));
    assert_eq!(f.v, [1.0, 2.0, 3.0]);
}

#[test]
fn unparsable_numbers_leave_values_alone() {
    let (mut h, mut f) = (Harness::new(), Fields::new());
    fwarm(&mut h, &mut f);
    gesture(&mut h, &mut f, 0, 2);
    h.type_text("abc", |ui| f.show(ui));
    h.key(Key::Enter, |ui| f.show(ui));
    h.idle(|ui| f.show(ui));
    assert_eq!(f.v, [1.0, 2.0, 3.0]);
}

#[test]
fn text_fields_commit_strings() {
    let mut h = Harness::new();
    let (mut a, mut b) = (String::from("one"), String::from("two"));
    let mut rect = Rect::NOTHING;
    let show = |ui: &mut Ui, a: &mut String, b: &mut String, rect: &mut Rect| {
        *rect = ui
            .add(FieldGroup::new("s").field(a).field(b).cell_width(80.0))
            .rect;
    };
    for _ in 0..3 {
        h.idle(|ui| show(ui, &mut a, &mut b, &mut rect));
    }
    let p = |i: f32, r: Rect| pos2(r.left() + 80.0 * (i + 0.5), r.center().y);
    let (from, to) = (p(0.0, rect), p(1.0, rect));
    h.moved(from, |ui| show(ui, &mut a, &mut b, &mut rect));
    h.press(from, |ui| show(ui, &mut a, &mut b, &mut rect));
    h.moved(to, |ui| show(ui, &mut a, &mut b, &mut rect));
    h.release(to, |ui| show(ui, &mut a, &mut b, &mut rect));
    for _ in 0..3 {
        h.idle(|ui| show(ui, &mut a, &mut b, &mut rect));
    }
    h.type_text("hej", |ui| show(ui, &mut a, &mut b, &mut rect));
    h.key(Key::Enter, |ui| show(ui, &mut a, &mut b, &mut rect));
    h.idle(|ui| show(ui, &mut a, &mut b, &mut rect));
    assert_eq!((a.as_str(), b.as_str()), ("hej", "hej"));
}

// ----------------------------------------------------- state-dependent icons

/// Plain icon-style buttons: content swaps with the state, no active fill.
struct IconRow {
    v: [bool; 3],
    rect: Rect,
    highlight: bool,
}

impl IconRow {
    fn new(v: [bool; 3], highlight: bool) -> Self {
        Self {
            v,
            rect: Rect::NOTHING,
            highlight,
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        let [a, b, c] = &mut self.v;
        let g = ToggleGroup::new("icons")
            .state_cell("a-off", "a-ON-WIDER", a)
            .tooltip("A")
            .state_cell("b-off", "b-on", b)
            .state_cell("c-off", "c-on", c)
            .highlight_active(self.highlight);
        self.rect = ui.add(g).rect;
    }
}

#[test]
fn state_cell_shows_content_for_the_current_state() {
    let (mut h, mut r) = (Harness::new(), IconRow::new([false, true, false], false));
    for _ in 0..3 {
        h.idle(|ui| r.show(ui));
    }
    let t = &h.painted.texts;
    assert!(t.contains(&"a-off".to_owned()), "{t:?}");
    assert!(t.contains(&"b-on".to_owned()), "{t:?}");
    assert!(t.contains(&"c-off".to_owned()), "{t:?}");
    assert!(!t.contains(&"a-ON-WIDER".to_owned()), "{t:?}");
    assert!(!t.contains(&"b-off".to_owned()), "{t:?}");
}

#[test]
fn toggling_never_changes_the_layout() {
    let (mut h, mut r) = (Harness::new(), IconRow::new([false; 3], false));
    for _ in 0..3 {
        h.idle(|ui| r.show(ui));
    }
    let before = r.rect;
    // Flip the first cell, whose "on" content is much wider than "off".
    let p = pos2(r.rect.left() + 5.0, r.rect.center().y);
    h.moved(p, |ui| r.show(ui));
    h.press(p, |ui| r.show(ui));
    h.release(p, |ui| r.show(ui));
    h.idle(|ui| r.show(ui));
    assert!(r.v[0]);
    assert!(h.painted.texts.contains(&"a-ON-WIDER".to_owned()));
    assert_eq!(r.rect, before);
}

#[test]
fn highlight_active_controls_the_on_fill() {
    for highlight in [true, false] {
        let (mut h, mut r) = (Harness::new(), IconRow::new([true, true, true], highlight));
        for _ in 0..3 {
            h.idle(|ui| r.show(ui));
        }
        let active = h.ctx.global_style().visuals.selection.bg_fill;
        assert_eq!(
            h.painted.fills.contains(&active),
            highlight,
            "highlight={highlight}"
        );
    }
}

#[test]
fn state_cells_still_drag_paint() {
    let (mut h, mut r) = (Harness::new(), IconRow::new([false; 3], false));
    for _ in 0..3 {
        h.idle(|ui| r.show(ui));
    }
    let start = pos2(r.rect.left() + 5.0, r.rect.center().y);
    let end = pos2(r.rect.right() - 5.0, r.rect.center().y);
    h.moved(start, |ui| r.show(ui));
    h.press(start, |ui| r.show(ui));
    h.moved(end, |ui| r.show(ui));
    h.release(end, |ui| r.show(ui));
    assert_eq!(r.v, [true; 3]);
}

#[test]
fn image_cells_toggle_and_keep_a_stable_size() {
    let mut h = Harness::new();
    let tex = h
        .ctx
        .load_texture("icon", ColorImage::example(), TextureOptions::default());
    let sized = (tex.id(), vec2(16.0, 16.0));
    let mut v = [false, false];
    let mut rect = Rect::NOTHING;
    let show = |ui: &mut Ui, v: &mut [bool; 2], rect: &mut Rect| {
        let [a, b] = v;
        let g = ToggleGroup::new("img")
            .state_cell(
                Icon::new(Image::from_texture(sized)).sized(18.0),
                Icon::new(Image::from_texture(sized)).sized(18.0),
                a,
            )
            .tooltip("first")
            .state_cell(Image::from_texture(sized), Image::from_texture(sized), b)
            .highlight_active(false);
        *rect = ui.add(g).rect;
    };
    for _ in 0..3 {
        h.idle(|ui| show(ui, &mut v, &mut rect));
    }
    let before = rect;
    let start = pos2(rect.left() + 5.0, rect.center().y);
    let end = pos2(rect.right() - 5.0, rect.center().y);
    h.moved(start, |ui| show(ui, &mut v, &mut rect));
    h.press(start, |ui| show(ui, &mut v, &mut rect));
    h.moved(end, |ui| show(ui, &mut v, &mut rect));
    h.release(end, |ui| show(ui, &mut v, &mut rect));
    assert_eq!(v, [true, true]);
    assert_eq!(rect, before);
    // 18pt icon plus padding on each side.
    assert!(rect.height() >= 18.0);
}

// ------------------------------------------------------- custom content

#[test]
fn paint_cell_receives_rect_and_state_colour() {
    let mut h = Harness::new();
    let calls: RefCell<Vec<(Rect, Color32)>> = RefCell::new(Vec::new());
    let mut on = true;
    let mut group_rect = Rect::NOTHING;
    let mut show = |ui: &mut Ui, on: &mut bool| {
        let dot = PaintCell::new(vec2(14.0, 10.0), |_ui, rect, color| {
            calls.borrow_mut().push((rect, color));
        });
        group_rect = ui.add(ToggleGroup::new("p").cell(dot, on)).rect;
    };
    for _ in 0..3 {
        calls.borrow_mut().clear();
        h.idle(|ui| show(ui, &mut on));
    }
    let calls = calls.borrow();
    assert_eq!(calls.len(), 1);
    let (rect, color) = calls[0];
    assert_eq!(rect.size(), vec2(14.0, 10.0));
    assert!(group_rect.contains_rect(rect));
    // "on" with the default active fill uses the active text colour.
    let visuals = h.ctx.global_style().visuals.clone();
    assert_eq!(color, visuals.widgets.active.text_color());
}

/// A user-defined content type: the crate knows nothing about it.
struct Bar {
    width: f32,
}

impl CellContent for Bar {
    fn size(&self, _ui: &Ui) -> Vec2 {
        vec2(self.width, 4.0)
    }
    fn paint(&self, ui: &Ui, rect: Rect, color: Color32) {
        ui.painter().rect_filled(rect, 0.0, color);
    }
    fn accessible_label(&self) -> Option<String> {
        Some("bar".to_owned())
    }
}

#[test]
fn user_defined_content_sizes_the_cell_and_drag_paints() {
    let mut h = Harness::new();
    let mut v = [false, false];
    let mut rect = Rect::NOTHING;
    let show = |ui: &mut Ui, v: &mut [bool; 2], rect: &mut Rect| {
        let [a, b] = v;
        let g = ToggleGroup::new("bars")
            .state_cell(Bar { width: 10.0 }, Bar { width: 60.0 }, a)
            .cell(Bar { width: 10.0 }, b)
            .highlight_active(false);
        *rect = ui.add(g).rect;
    };
    for _ in 0..3 {
        h.idle(|ui| show(ui, &mut v, &mut rect));
    }
    // The first cell is as wide as its wider ("on") content plus padding.
    let pad_x = h.ctx.global_style().spacing.button_padding.x;
    assert!(rect.width() >= 60.0 + 2.0 * pad_x + 10.0);
    let before = rect;
    let start = pos2(rect.left() + 3.0, rect.center().y);
    let end = pos2(rect.right() - 3.0, rect.center().y);
    h.moved(start, |ui| show(ui, &mut v, &mut rect));
    h.press(start, |ui| show(ui, &mut v, &mut rect));
    h.moved(end, |ui| show(ui, &mut v, &mut rect));
    h.release(end, |ui| show(ui, &mut v, &mut rect));
    assert_eq!(v, [true, true]);
    assert_eq!(rect, before);
}

// ------------------------------------------------------ exclusive toggles

struct Exclusive {
    t: Toggles,
    require_one: bool,
}

impl Exclusive {
    fn new(v: Vec<bool>) -> Self {
        Self {
            t: Toggles::new(v),
            require_one: false,
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        let n = self.t.v.len();
        let g = ToggleGroup::new("t")
            .cells(["A", "B", "C", "D"].into_iter().take(n), &mut self.t.v)
            .equal_width()
            .exclusive(true)
            .require_one(self.require_one);
        self.t.rect = ui.add(g).rect;
    }
}

fn exclusive(v: Vec<bool>) -> (Harness, Exclusive) {
    let (mut h, mut e) = (Harness::new(), Exclusive::new(v));
    for _ in 0..3 {
        h.idle(|ui| e.show(ui));
    }
    (h, e)
}

fn click(h: &mut Harness, e: &mut Exclusive, i: usize) {
    let p = e.t.center(i);
    h.moved(p, |ui| e.show(ui));
    h.press(p, |ui| e.show(ui));
    h.release(p, |ui| e.show(ui));
    h.idle(|ui| e.show(ui));
}

#[test]
fn exclusive_press_turns_siblings_off() {
    let (mut h, mut e) = exclusive(vec![true, false, true, false]);
    click(&mut h, &mut e, 1);
    assert_eq!(e.t.v, [false, true, false, false]);
}

#[test]
fn exclusive_press_on_only_active_turns_it_off() {
    let (mut h, mut e) = exclusive(vec![false, true, false]);
    click(&mut h, &mut e, 1);
    assert_eq!(e.t.v, [false; 3]);
}

#[test]
fn exclusive_require_one_keeps_the_last_cell_on() {
    let (mut h, mut e) = exclusive(vec![false, true, false]);
    e.require_one = true;
    click(&mut h, &mut e, 1);
    assert_eq!(e.t.v, [false, true, false]);
    click(&mut h, &mut e, 2);
    assert_eq!(e.t.v, [false, false, true]);
}

#[test]
fn exclusive_plain_drag_moves_the_selection() {
    let (mut h, mut e) = exclusive(vec![true, false, false, false]);
    let start = e.t.center(0);
    h.moved(start, |ui| e.show(ui));
    h.press(start, |ui| e.show(ui));
    for i in 1..4 {
        h.moved(e.t.center(i), |ui| e.show(ui));
    }
    h.release(e.t.center(3), |ui| e.show(ui));
    assert_eq!(e.t.v, [false, false, false, true]);
}

#[test]
fn exclusive_shift_press_adds_without_clearing() {
    let (mut h, mut e) = exclusive(vec![true, false, false, false]);
    h.modifiers = Modifiers::SHIFT;
    click(&mut h, &mut e, 2);
    assert_eq!(e.t.v, [true, false, true, false]);
    // Shift on an active cell switches just that one off.
    click(&mut h, &mut e, 0);
    assert_eq!(e.t.v, [false, false, true, false]);
}

#[test]
fn exclusive_shift_drag_paints_additively() {
    let (mut h, mut e) = exclusive(vec![false, false, false, true]);
    h.modifiers = Modifiers::SHIFT;
    let start = e.t.center(0);
    h.moved(start, |ui| e.show(ui));
    h.press(start, |ui| e.show(ui));
    h.moved(e.t.center(1), |ui| e.show(ui));
    h.release(e.t.center(1), |ui| e.show(ui));
    assert_eq!(e.t.v, [true, true, false, true]);
}

#[test]
fn exclusive_shift_cannot_remove_the_last_cell_with_require_one() {
    let (mut h, mut e) = exclusive(vec![false, true, false]);
    e.require_one = true;
    h.modifiers = Modifiers::SHIFT;
    click(&mut h, &mut e, 1);
    assert_eq!(e.t.v, [false, true, false]);
}

#[test]
fn exclusive_custom_extend_modifier() {
    let (mut h, mut e) = (Harness::new(), vec![true, false, false]);
    let rect = std::cell::Cell::new(Rect::NOTHING);
    let show = |ui: &mut Ui, v: &mut Vec<bool>| {
        let g = ToggleGroup::new("t")
            .cells(["A", "B", "C"], v)
            .equal_width()
            .exclusive(true)
            .extend_modifier(Modifiers::CTRL);
        rect.set(ui.add(g).rect);
    };
    for _ in 0..3 {
        h.idle(|ui| show(ui, &mut e));
    }
    let r = rect.get();
    let p = pos2(r.right() - 5.0, r.center().y);
    // Shift is not the extend modifier here, so this is a plain press.
    h.modifiers = Modifiers::SHIFT;
    h.moved(p, |ui| show(ui, &mut e));
    h.press(p, |ui| show(ui, &mut e));
    h.release(p, |ui| show(ui, &mut e));
    assert_eq!(e, [false, false, true]);
    h.modifiers = Modifiers::CTRL;
    let q = pos2(r.left() + 5.0, r.center().y);
    h.moved(q, |ui| show(ui, &mut e));
    h.press(q, |ui| show(ui, &mut e));
    h.release(q, |ui| show(ui, &mut e));
    assert_eq!(e, [true, false, true]);
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

fn touch_down(h: &mut Harness, e: &mut Exclusive, p: Pos2) {
    let mut ev = vec![touch(p, egui::TouchPhase::Start)];
    ev.extend(button(p, true));
    h.frame(ev, |ui| e.show(ui));
}

fn touch_up(h: &mut Harness, e: &mut Exclusive, p: Pos2) {
    let mut ev = vec![touch(p, egui::TouchPhase::End)];
    ev.extend(button(p, false));
    h.frame(ev, |ui| e.show(ui));
}

#[test]
fn touch_tap_selects_exclusively() {
    let (mut h, mut e) = exclusive(vec![true, false, true, false]);
    let p = e.t.center(1);
    touch_down(&mut h, &mut e, p);
    // Nothing is decided until release.
    assert_eq!(e.t.v, [true, false, true, false]);
    touch_up(&mut h, &mut e, p);
    h.idle(|ui| e.show(ui));
    assert_eq!(e.t.v, [false, true, false, false]);
}

#[test]
fn touch_long_press_extends() {
    let (mut h, mut e) = exclusive(vec![true, false, false, false]);
    let p = e.t.center(2);
    touch_down(&mut h, &mut e, p);
    h.time += 1.0;
    h.frame(vec![touch(p, egui::TouchPhase::Move)], |ui| e.show(ui));
    touch_up(&mut h, &mut e, p);
    h.idle(|ui| e.show(ui));
    assert_eq!(e.t.v, [true, false, true, false]);
}

#[test]
fn touch_drag_moves_the_selection() {
    let (mut h, mut e) = exclusive(vec![true, false, false, false]);
    let start = e.t.center(0);
    touch_down(&mut h, &mut e, start);
    let p = e.t.center(2);
    let mut ev = vec![touch(p, egui::TouchPhase::Move)];
    ev.push(Event::PointerMoved(p));
    h.frame(ev, |ui| e.show(ui));
    touch_up(&mut h, &mut e, p);
    h.idle(|ui| e.show(ui));
    assert_eq!(e.t.v, [false, false, true, false]);
}
