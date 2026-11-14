//! Dropdown cells of `ToggleGroup`, and pair content.

use egui::{
    Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, Ui, pos2, vec2,
};
use egui_control_groups::{CellContent, ToggleGroup};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Grid,
    Vertex,
    Face,
}

const OPTIONS: [(Mode, &str); 3] = [
    (Mode::Grid, "Grid"),
    (Mode::Vertex, "Vertex"),
    (Mode::Face, "Face"),
];

/// Everything the group under test is bound to.
struct State {
    a: bool,
    c: bool,
    mode: Mode,
    exclusive: bool,
    /// Whether the dropdown sits between the toggles (else it comes last).
    middle: bool,
    changed: u32,
    rect: Rect,
}

impl State {
    fn new() -> Self {
        Self {
            a: false,
            c: false,
            mode: Mode::Grid,
            exclusive: false,
            middle: true,
            changed: 0,
            rect: Rect::NOTHING,
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        ui.add_space(20.0);
        let mut g = ToggleGroup::new("g");
        if self.middle {
            g = g
                .cell("A", &mut self.a)
                .dropdown(&mut self.mode, OPTIONS)
                .cell("C", &mut self.c);
        } else {
            g = g
                .cell("A", &mut self.a)
                .cell("C", &mut self.c)
                .dropdown(&mut self.mode, OPTIONS);
        }
        if self.exclusive {
            g = g.exclusive(true).require_one(true);
        }
        let r = ui.add(g);
        self.rect = r.rect;
        self.changed += u32::from(r.changed());
    }
}

struct Harness {
    ctx: Context,
    time: f64,
    s: State,
    /// Extra space above the group, to scroll it out of view.
    offset: f32,
    /// Text shapes of the last frame with their screen rectangles.
    texts: Vec<(String, Rect)>,
}

fn collect(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
        Shape::Text(t) => out.push((t.galley.text().to_owned(), t.visual_bounding_rect())),
        _ => {}
    }
}

impl Harness {
    fn new() -> Self {
        Self::with(State::new())
    }

    fn with(s: State) -> Self {
        Self {
            ctx: Context::default(),
            time: 0.0,
            s,
            offset: 0.0,
            texts: Vec::new(),
        }
    }

    fn frame_with(&mut self, events: Vec<Event>, mut ui_fn: impl FnMut(&mut Ui)) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let output = self.ctx.run_ui(input, |ui| ui_fn(ui));
        self.texts.clear();
        for c in &output.shapes {
            collect(&c.shape, &mut self.texts);
        }
        output.drop_without_applying_deltas();
    }

    fn frame(&mut self, events: Vec<Event>) {
        let (offset, mut s) = (self.offset, std::mem::replace(&mut self.s, State::new()));
        self.frame_with(events, |ui| {
            ui.add_space(offset);
            s.show(ui);
        });
        self.s = s;
    }

    /// A few idle frames, for popups to size and settle.
    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame(vec![]);
        }
    }

    fn press(&mut self, pos: Pos2) {
        self.frame(button(pos, true));
    }

    fn release(&mut self, pos: Pos2) {
        self.frame(button(pos, false));
    }

    fn moved(&mut self, pos: Pos2) {
        self.frame(vec![Event::PointerMoved(pos)]);
    }

    fn click(&mut self, pos: Pos2) {
        self.press(pos);
        self.release(pos);
        self.settle();
    }

    fn key(&mut self, key: Key) {
        self.frame(vec![key_event(key, true), key_event(key, false)]);
        self.settle();
    }

    fn has(&self, text: &str) -> bool {
        self.texts.iter().any(|(t, _)| t == text)
    }

    fn count(&self, text: &str) -> usize {
        self.texts.iter().filter(|(t, _)| t == text).count()
    }

    /// Centre of the last-painted text `text` (the popup row, when listed
    /// below the cell, comes after the closed cell's own text).
    fn centre(&self, text: &str) -> Pos2 {
        self.texts
            .iter()
            .rev()
            .find(|(t, _)| t == text)
            .unwrap_or_else(|| panic!("no text {text:?} in {:?}", self.texts))
            .1
            .center()
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

fn key_event(key: Key, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// A harness with the group shown and the list open.
fn opened(s: State) -> Harness {
    let mut h = Harness::with(s);
    h.settle();
    let cell = h.centre("Grid");
    h.click(cell);
    h
}

#[test]
fn closed_cell_paints_the_selected_option_only() {
    let mut s = State::new();
    s.mode = Mode::Vertex;
    let mut h = Harness::with(s);
    h.settle();
    assert!(h.has("Vertex"), "{:?}", h.texts);
    assert!(!h.has("Grid") && !h.has("Face"), "{:?}", h.texts);
}

#[test]
fn value_matching_no_option_shows_only_the_caret() {
    struct Odd;
    impl PartialEq for Odd {
        fn eq(&self, _: &Self) -> bool {
            false
        }
    }
    let mut v = Odd;
    let mut h = Harness::new();
    h.frame_with(vec![], |ui| {
        ui.add(ToggleGroup::new("g").dropdown(&mut v, [(Odd, "One"), (Odd, "Two")]));
    });
    assert!(h.texts.is_empty(), "{:?}", h.texts);
}

#[test]
fn click_opens_a_list_with_every_option() {
    let h = opened(State::new());
    for (_, name) in OPTIONS {
        assert!(h.has(name), "missing {name}: {:?}", h.texts);
    }
    // The selected option is in the closed cell and in the list.
    assert_eq!(h.count("Grid"), 2, "{:?}", h.texts);
    assert_eq!(h.s.mode, Mode::Grid);
    assert_eq!(h.s.changed, 0);
}

#[test]
fn choosing_a_row_selects_it_closes_and_reports_changed_once() {
    let mut h = opened(State::new());
    let target = h.centre("Vertex");
    h.click(target);
    assert_eq!(h.s.mode, Mode::Vertex);
    assert_eq!(h.s.changed, 1);
    // Closed: only the cell's own "Vertex" remains.
    assert_eq!(h.count("Vertex"), 1, "{:?}", h.texts);
    assert!(!h.has("Grid") && !h.has("Face"), "{:?}", h.texts);
}

#[test]
fn choosing_the_current_option_closes_without_change() {
    let mut h = opened(State::new());
    let row = h.centre("Grid");
    h.click(row);
    assert_eq!(h.s.changed, 0);
    assert_eq!(h.count("Grid"), 1, "list still open: {:?}", h.texts);
    assert!(!h.has("Vertex"));
}

#[test]
fn click_outside_closes_without_change() {
    let mut h = opened(State::new());
    h.click(pos2(600.0, 500.0));
    assert_eq!(h.s.mode, Mode::Grid);
    assert_eq!(h.s.changed, 0);
    assert!(!h.has("Vertex"), "{:?}", h.texts);
}

#[test]
fn escape_closes_without_change() {
    let mut h = opened(State::new());
    assert!(h.has("Vertex"));
    h.key(Key::Escape);
    assert_eq!(h.s.mode, Mode::Grid);
    assert_eq!(h.s.changed, 0);
    assert!(!h.has("Vertex"), "{:?}", h.texts);
}

#[test]
fn choosing_another_option_does_not_change_the_layout() {
    let mut s = State::new();
    s.middle = false;
    let mut h = opened(s);
    let before = h.s.rect;
    let row = h.centre("Vertex");
    h.click(row);
    assert_eq!(h.s.mode, Mode::Vertex);
    assert_eq!(h.s.rect, before);
    // And on to a narrower option.
    let cell = h.centre("Vertex");
    h.click(cell);
    let row = h.centre("Face");
    h.click(row);
    assert_eq!(h.s.mode, Mode::Face);
    assert_eq!(h.s.rect, before);
}

#[test]
fn drag_from_toggle_across_the_dropdown_skips_it() {
    let mut h = Harness::new();
    h.settle();
    let (a, mid, c) = (h.centre("A"), h.centre("Grid"), h.centre("C"));
    h.press(a);
    h.moved(mid);
    h.moved(c);
    h.release(c);
    h.settle();
    assert!(h.s.a && h.s.c, "a={} c={}", h.s.a, h.s.c);
    assert_eq!(h.s.mode, Mode::Grid);
    assert!(!h.has("Vertex"), "list opened: {:?}", h.texts);
}

#[test]
fn drag_jumping_past_the_dropdown_still_paints_far_cells() {
    let mut h = Harness::new();
    h.settle();
    let (a, c) = (h.centre("A"), h.centre("C"));
    h.press(a);
    h.moved(c);
    h.release(c);
    assert!(h.s.a && h.s.c);
}

#[test]
fn drag_ending_on_a_trailing_dropdown_paints_the_toggles_before_it() {
    let mut s = State::new();
    s.middle = false;
    let mut h = Harness::with(s);
    h.settle();
    let (a, end) = (h.centre("A"), h.centre("Grid"));
    h.press(a);
    h.moved(end);
    h.release(end);
    assert!(h.s.a && h.s.c);
    assert!(!h.has("Vertex"));
}

#[test]
fn pressing_the_dropdown_flips_no_toggle() {
    for (exclusive, a, c) in [
        (false, false, false),
        (false, true, true),
        (true, true, false),
    ] {
        let mut s = State::new();
        s.exclusive = exclusive;
        s.a = a;
        s.c = c;
        let h = opened(s);
        assert_eq!((h.s.a, h.s.c), (a, c), "exclusive={exclusive}");
        assert_eq!(h.s.changed, 0);
    }
}

#[test]
fn exclusive_require_one_ignores_the_dropdown() {
    let mut s = State::new();
    s.exclusive = true;
    s.a = true;
    let mut h = Harness::with(s);
    h.settle();
    // The only "on" toggle can't be switched off.
    let a = h.centre("A");
    h.click(a);
    assert!(h.s.a && !h.s.c);
    // Dragging the selection across the dropdown moves it to C, and the
    // dropdown in between neither takes it nor loses it.
    let (mid, c) = (h.centre("Grid"), h.centre("C"));
    h.press(a);
    h.moved(mid);
    assert!(h.s.a && !h.s.c);
    h.moved(c);
    h.release(c);
    assert!(!h.s.a && h.s.c);
    assert_eq!(h.s.mode, Mode::Grid);
}

#[test]
fn swipe_starting_on_the_dropdown_does_not_open_it() {
    let mut h = Harness::new();
    h.settle();
    let cell = h.centre("Grid");
    h.press(cell);
    h.moved(cell + vec2(30.0, 0.0));
    h.release(cell + vec2(30.0, 0.0));
    h.settle();
    assert!(!h.has("Vertex"), "{:?}", h.texts);
}

#[test]
fn tab_then_space_or_enter_opens() {
    for open_key in [Key::Space, Key::Enter] {
        let mut s = State::new();
        s.middle = false;
        let mut h = Harness::with(s);
        h.settle();
        // A, C, then the dropdown.
        for _ in 0..3 {
            h.key(Key::Tab);
        }
        assert!(!h.has("Vertex"));
        h.key(open_key);
        assert!(
            h.has("Vertex") && h.has("Face"),
            "{open_key:?}: {:?}",
            h.texts
        );
        h.key(Key::Escape);
        assert!(!h.has("Vertex"));
        assert_eq!(h.s.changed, 0);
    }
}

#[test]
fn arrow_keys_and_enter_choose_in_the_list() {
    let mut s = State::new();
    s.middle = false;
    let mut h = Harness::with(s);
    h.settle();
    for _ in 0..3 {
        h.key(Key::Tab);
    }
    h.key(Key::Space);
    h.key(Key::ArrowDown);
    h.key(Key::Enter);
    assert_eq!(h.s.mode, Mode::Vertex);
    assert_eq!(h.s.changed, 1);
    assert!(!h.has("Face"), "{:?}", h.texts);
}

#[test]
fn open_list_closes_when_the_group_scrolls_away() {
    let mut h = opened(State::new());
    assert!(h.has("Vertex"));
    h.offset = 2000.0;
    h.settle();
    h.offset = 0.0;
    h.settle();
    assert!(!h.has("Vertex"), "list reopened by itself: {:?}", h.texts);
}

#[test]
fn pair_content_sums_widths_with_icon_spacing_and_paints_both() {
    let mut h = Harness::new();
    let mut sizes = None;
    let mut spacing = 0.0;
    h.frame_with(vec![], |ui| {
        spacing = ui.spacing().icon_spacing;
        let pair = ("Snap", String::from("Grid"));
        sizes = Some((
            "Snap".size(ui),
            String::from("Grid").size(ui),
            pair.size(ui),
        ));
        let mut on = false;
        ui.add(ToggleGroup::new("p").cell(pair, &mut on));
    });
    let (a, b, pair) = sizes.unwrap();
    assert!((pair.x - (a.x + spacing + b.x)).abs() < 1e-3, "{pair:?}");
    assert!((pair.y - a.y.max(b.y)).abs() < 1e-3);
    assert!(h.has("Snap") && h.has("Grid"), "{:?}", h.texts);
    assert!(h.centre("Snap").x < h.centre("Grid").x);
}

#[test]
fn pair_accessible_label_joins_labels() {
    assert_eq!(
        ("Snap", "Grid").accessible_label().as_deref(),
        Some("Snap Grid")
    );
}
