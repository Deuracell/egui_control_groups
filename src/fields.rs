//! Joined text / number fields with Blender-style multi-editing: press in one
//! field and drag across its neighbours to select several, then type once and
//! the value is applied to all of them.

use std::hash::Hash;

use egui::text::{CCursor, CCursorRange, CharIndex};
use egui::{
    Align, Color32, Id, Key, Margin, Modifiers, Pos2, Rect, Response, Sense, Shape, TextEdit,
    TextStyle, Ui, Vec2, Widget, WidgetInfo, WidgetText, vec2,
};

use crate::geometry::{Orientation, corner_radius_for, index_at, layout_cells};
use crate::paint::{centered_galley, frame_shapes, make_galley};
use crate::style::GroupStyle;
use crate::width::{Width, distribute};

/// A value a [`FieldGroup`] can edit as text.
///
/// Implemented for `String`, the integer types and `f32` / `f64`. Numbers
/// accept a decimal comma as well as a decimal point. `set_from_field_text`
/// returns `false` (leaving the value untouched) when the text does not parse.
pub trait FieldValue {
    /// The text shown for the current value.
    fn to_field_text(&self) -> String;
    /// Apply edited text. Returns whether it was accepted.
    fn set_from_field_text(&mut self, text: &str) -> bool;
}

impl FieldValue for String {
    fn to_field_text(&self) -> String {
        self.clone()
    }
    fn set_from_field_text(&mut self, text: &str) -> bool {
        *self = text.to_owned();
        true
    }
}

pub(crate) fn format_float(v: f64) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

macro_rules! float_value {
    ($($t:ty),*) => {$(
        impl FieldValue for $t {
            fn to_field_text(&self) -> String {
                format_float(f64::from(*self))
            }
            fn set_from_field_text(&mut self, text: &str) -> bool {
                match text.trim().replace(',', ".").parse::<$t>() {
                    Ok(v) if v.is_finite() => { *self = v; true }
                    _ => false,
                }
            }
        }
    )*};
}
macro_rules! int_value {
    ($($t:ty),*) => {$(
        impl FieldValue for $t {
            fn to_field_text(&self) -> String {
                self.to_string()
            }
            fn set_from_field_text(&mut self, text: &str) -> bool {
                match text.trim().parse::<$t>() {
                    Ok(v) => { *self = v; true }
                    Err(_) => false,
                }
            }
        }
    )*};
}
float_value!(f32, f64);
int_value!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

/// What Tab (Shift+Tab) does at the last (first) field of a [`FieldGroup`].
///
/// Tab between fields inside the group always commits the field being edited
/// and opens the neighbour. This only decides what happens past either end;
/// every mode commits first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TabBehavior {
    /// Drop keyboard focus and stay put.
    Stop,
    /// Hand focus on to the next (previous) widget outside the group, as
    /// egui's own Tab navigation would. Shift+Tab goes to the widget added
    /// before the group; if there is none, egui's choice of widget applies.
    /// The default.
    #[default]
    Leave,
    /// Wrap around: Tab on the last field opens the first, Shift+Tab on the
    /// first opens the last.
    Wrap,
}

struct Entry<'a> {
    label: Option<WidgetText>,
    value: &'a mut dyn FieldValue,
}

#[derive(Clone, Default, PartialEq)]
struct State {
    /// Cell the pointer went down on, while the button is held.
    press: Option<usize>,
    /// A finger (not a mouse) was seen during the current press.
    press_touch: bool,
    /// Where and when the current press began (for touch gesture decisions).
    press_origin: Pos2,
    press_start: f64,
    touch_mode: TouchMode,
    edit: Option<Edit>,
}

impl State {
    fn clear_press(&mut self) {
        self.press = None;
        self.press_touch = false;
        self.touch_mode = TouchMode::Pending;
    }
}

/// What a touch that began on a cell is turning into.
#[derive(Clone, Copy, Default, PartialEq)]
enum TouchMode {
    /// A tap so far: release opens the editor.
    #[default]
    Pending,
    /// Drag-select across cells.
    Drag,
    /// Moved too far to be a tap without starting a drag-select (a swipe or
    /// scroll): the rest of the gesture does nothing.
    Cancelled,
}

#[derive(Clone, PartialEq)]
struct Edit {
    lo: usize,
    hi: usize,
    /// The cell hosting the real text editor. Other selected cells mirror it.
    host: usize,
    buffer: String,
    /// First frame: select all and take focus, ignore commit checks.
    fresh: bool,
    /// The session was started or used by touch. A touch screen has no
    /// Escape key, so tapping away cancels there and only Enter commits.
    touch: bool,
    /// Opened from the keyboard (Enter / Space on a focused cell, or Tab), so
    /// Enter or Escape hands focus back to the cell. A pointer-opened edit
    /// leaves nothing focused, so the app's own shortcuts work again.
    keyboard: bool,
}

/// Joined text or number fields with multi-edit.
///
/// ```no_run
/// # use egui_control_groups::FieldGroup;
/// # egui::__run_test_ui(|ui| {
/// let mut pos = [0.0_f32, 1.5, -2.0];
/// let [x, y, z] = &mut pos;
/// let response = ui.add(
///     FieldGroup::new("position")
///         .labeled("X", x)
///         .labeled("Y", y)
///         .labeled("Z", z)
///         .vertical(),
/// );
/// if response.changed() { /* a value was committed */ }
/// # });
/// ```
///
/// * Click a field to edit it. All text is selected; Enter or clicking away
///   commits, Escape cancels. On a touch screen, where there is no Escape,
///   tapping away cancels instead and only Enter commits.
/// * Tab commits the field being edited and opens the next one (Shift+Tab the
///   previous one). What happens at the last or first field is set with
///   [`tab_behavior`](FieldGroup::tab_behavior): by default it just commits
///   and moves focus on to the next (previous) widget, like egui's own Tab
///   navigation.
/// * Press in one field and drag across others to select several. The
///   selected fields are highlighted; type a value and commit to write it to
///   every one of them.
///
/// Values are only written on commit. Numbers that fail to parse are left
/// unchanged.
#[must_use = "Add with `ui.add(...)`."]
pub struct FieldGroup<'a> {
    id_salt: Id,
    entries: Vec<Entry<'a>>,
    orientation: Orientation,
    cell_width: Option<f32>,
    width: Width,
    style: GroupStyle,
    tab_behavior: TabBehavior,
}

impl<'a> FieldGroup<'a> {
    /// `id_salt` must be unique among groups in the same `Ui`.
    pub fn new(id_salt: impl Hash + std::fmt::Debug) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            entries: Vec::new(),
            orientation: Orientation::Horizontal,
            cell_width: None,
            width: Width::Auto,
            style: GroupStyle::default(),
            tab_behavior: TabBehavior::default(),
        }
    }

    /// Append a field without a label.
    pub fn field(mut self, value: &'a mut dyn FieldValue) -> Self {
        self.entries.push(Entry { label: None, value });
        self
    }

    /// Append a field with a small label on its left edge.
    pub fn labeled(mut self, label: impl Into<WidgetText>, value: &'a mut dyn FieldValue) -> Self {
        self.entries.push(Entry {
            label: Some(label.into()),
            value,
        });
        self
    }

    /// Append one field per `(label, value)` pair, so a slice or array can be
    /// edited without destructuring it.
    ///
    /// In debug builds, panics if `labels` and `values` differ in length.
    pub fn fields<T: FieldValue + 'a>(
        mut self,
        labels: impl IntoIterator<Item = impl Into<WidgetText>>,
        values: &'a mut [T],
    ) -> Self {
        const MISMATCH: &str = "FieldGroup::fields: labels and values must have the same length";
        let mut labels = labels.into_iter();
        for value in values.iter_mut() {
            let Some(label) = labels.next() else {
                debug_assert!(false, "{MISMATCH}");
                break;
            };
            self = self.labeled(label, value);
        }
        debug_assert!(labels.next().is_none(), "{MISMATCH}");
        self
    }

    /// Stack the fields top to bottom instead of left to right.
    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Width of every field. Defaults to half of `Spacing::text_edit_width`.
    ///
    /// This sets the natural size only: an explicit [`width`](Self::width) or
    /// [`fill_width`](Self::fill_width) takes precedence over it.
    pub fn cell_width(mut self, width: f32) -> Self {
        self.cell_width = Some(width);
        self
    }

    /// Make the whole group exactly `width` points wide.
    ///
    /// Horizontal groups share it between the fields in proportion to their
    /// natural width (equal, so each gets an equal share); vertical groups
    /// get it as their cross-axis width. Takes precedence over
    /// [`cell_width`](Self::cell_width). A non-finite or negative value is
    /// ignored. Default: natural size.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Width::Fixed(width);
        self
    }

    /// Like [`width`](Self::width), using `ui.available_width()`. Falls back
    /// to the natural size when that is not finite.
    pub fn fill_width(mut self) -> Self {
        self.width = Width::Fill;
        self
    }

    /// What Tab does at the ends of the group. Default: [`TabBehavior::Leave`].
    pub fn tab_behavior(mut self, behavior: TabBehavior) -> Self {
        self.tab_behavior = behavior;
        self
    }

    /// Override parts of the look. Anything left unset follows `egui::Style`.
    pub fn style(mut self, style: GroupStyle) -> Self {
        self.style = style;
        self
    }
}

impl Widget for FieldGroup<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self {
            id_salt,
            mut entries,
            orientation,
            cell_width,
            width,
            style,
            tab_behavior,
        } = self;
        let n = entries.len();
        if n == 0 {
            return ui.allocate_response(Vec2::ZERO, Sense::hover());
        }
        let st = style.resolve(ui);
        let id = ui.make_persistent_id(id_salt);
        let font = TextStyle::Button;
        let text_h = ui.text_style_height(&font);

        let mut cell_w = cell_width.unwrap_or(ui.spacing().text_edit_width * 0.5);
        let target = width.resolve(ui.available_width());
        let cell_h = ui
            .spacing()
            .interact_size
            .y
            .max(text_h + st.padding.y * 2.0);
        let (mains, cross, total) = match orientation {
            Orientation::Horizontal => {
                let mains = match target {
                    Some(t) => distribute(&vec![cell_w; n], t, (st.padding.x * 2.0).max(4.0)),
                    None => vec![cell_w; n],
                };
                let w = mains.iter().sum::<f32>();
                (mains, cell_h, vec2(w, cell_h))
            }
            Orientation::Vertical => {
                cell_w = target.unwrap_or(cell_w);
                (vec![cell_h; n], cell_w, vec2(cell_w, cell_h * n as f32))
            }
        };
        let (group_rect, mut response) = ui.allocate_exact_size(total, Sense::hover());

        let cells = layout_cells(group_rect.min, &mains, cross, orientation);
        let mut state: State = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        // Out of view and nothing in progress: the space is allocated, but
        // there is nothing to lay out or paint. The cells are still
        // registered so that Tab can reach them (and scroll them into view).
        if !ui.is_rect_visible(group_rect)
            && state.edit.is_none()
            && state.press.is_none()
            && !ui.memory(|m| (0..n).any(|i| m.has_focus(id.with(i))))
        {
            for (i, r) in cells.iter().enumerate() {
                ui.interact(*r, id.with(i), Sense::click_and_drag());
            }
            // Keep the auto ids of later widgets as stable as while editing.
            ui.skip_ahead_auto_ids(1);
            return response;
        }
        let te_id = id.with("editor");

        // Tab (Shift+Tab) while editing moves to the neighbouring field. The
        // editor's event filter (below) keeps egui's own focus navigation
        // away from it, so the key is handled here, before any cell is
        // registered: leaving backwards has to hand focus on before the
        // group's own cells take their place in egui's focus order.
        // `tab` is `Some(target)` when a Tab press ends the edit, the target
        // being the field to open next (`None` past an end).
        let mut tab: Option<Option<usize>> = None;
        let mut leave_next = false;
        if let Some(edit) = state.edit.as_ref() {
            // A fresh editor takes focus below; do it now so a Tab pressed on
            // its very first frame is handled like any other.
            if edit.fresh {
                ui.memory_mut(|m| m.request_focus(te_id));
            }
        }
        if let Some(edit) = state.edit.as_ref()
            && ui.memory(|m| m.has_focus(te_id))
        {
            let back = ui.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab));
            let fwd = !back && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
            let wrap = tab_behavior == TabBehavior::Wrap;
            if back {
                tab = Some(match edit.lo.checked_sub(1) {
                    Some(i) => Some(i),
                    None => wrap.then_some(n - 1),
                });
                if tab == Some(None) {
                    ui.memory_mut(|m| m.surrender_focus(te_id));
                    if tab_behavior == TabBehavior::Leave {
                        ui.memory_mut(|m| m.move_focus(egui::FocusDirection::Previous));
                    }
                }
            } else if fwd {
                tab = Some(match Some(edit.hi + 1).filter(|&i| i < n) {
                    Some(i) => Some(i),
                    None => wrap.then_some(0),
                });
                leave_next = tab == Some(None) && tab_behavior == TabBehavior::Leave;
            }
        }

        let responses: Vec<Response> = cells
            .iter()
            .enumerate()
            .map(|(i, r)| ui.interact(*r, id.with(i), Sense::click_and_drag()))
            .collect();
        // Idle state is small and cheap to compare; an active edit is always
        // written back, as its buffer is changing anyway.
        let before = state.edit.is_none().then(|| state.clone());
        let mut changed = false;

        // Reserve a paint slot below the text editor, filled in at the end.
        let bg_slot = ui.painter().add(Shape::Noop);

        // On the web the pointer event is handled a frame before the touch
        // event, so a touch is only noticed from the frame after the press.
        let touch_now = ui.input(|i| {
            i.any_touches()
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Touch { .. }))
        });

        // 1. The live text editor, if a session is active.
        let mut finish: Option<bool> = None;
        let mut tab_to: Option<usize> = None;
        // Cell to hand keyboard focus back to after Enter / Escape.
        let mut refocus: Option<usize> = None;
        if let Some(edit) = state.edit.as_mut() {
            edit.touch |= touch_now;
            if edit.fresh {
                let len = edit.buffer.chars().count();
                let mut te_state = TextEdit::load_state(ui.ctx(), te_id).unwrap_or_default();
                te_state
                    .cursor
                    .set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(len))));
                te_state.store(ui.ctx(), te_id);
                ui.memory_mut(|m| m.request_focus(te_id));
            }
            let host = cells[edit.host];
            let area = Rect::from_center_size(
                host.center(),
                vec2((host.width() - st.padding.x * 2.0).max(1.0), text_h),
            );
            let editor = TextEdit::singleline(&mut edit.buffer)
                .id(te_id)
                .font(font.clone())
                .frame(egui::Frame::NONE)
                .margin(Margin::ZERO)
                .horizontal_align(Align::Center)
                .desired_width(area.width());
            let editor = editor.event_filter(egui::EventFilter {
                tab: true,
                ..Default::default()
            });
            let out = ui.place(area, editor);
            let (escape, enter) =
                ui.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::Enter)));
            if edit.fresh && tab.is_none() {
                edit.fresh = false;
            } else if let Some(next) = tab {
                finish = Some(true);
                tab_to = next;
                if next.is_none() {
                    ui.memory_mut(|m| {
                        m.surrender_focus(te_id);
                        if leave_next {
                            m.move_focus(egui::FocusDirection::Next);
                        }
                    });
                }
            } else if escape {
                finish = Some(false);
                refocus = edit.keyboard.then_some(edit.host);
            } else if out.lost_focus() {
                finish = Some(enter || !edit.touch);
                if enter && edit.keyboard {
                    refocus = Some(edit.host);
                }
            }
        } else {
            // The editor is placed in a child `Ui`, which takes one auto id.
            // Take it here too so later widgets keep their ids across edits.
            ui.skip_ahead_auto_ids(1);
        }
        if let Some(commit) = finish
            && let Some(edit) = state.edit.take()
        {
            if commit {
                for entry in &mut entries[edit.lo..=edit.hi] {
                    // A field already showing this text is left alone: writing
                    // the displayed (rounded) text back would lose precision.
                    let before = entry.value.to_field_text();
                    if before != edit.buffer && entry.value.set_from_field_text(&edit.buffer) {
                        changed |= entry.value.to_field_text() != before;
                    }
                }
            }
            if let Some(next) = tab_to {
                state.edit = Some(Edit {
                    lo: next,
                    hi: next,
                    host: next,
                    buffer: entries[next].value.to_field_text(),
                    fresh: true,
                    touch: false,
                    keyboard: true,
                });
            }
            if tab_to.is_none()
                && let Some(host) = refocus
            {
                ui.memory_mut(|m| m.request_focus(id.with(host)));
            }
            ui.ctx().request_repaint();
        }
        // The Enter / Escape that ended the edit must not act on the cell it
        // just gave focus to.
        let refocused = refocus.is_some() && finish.is_some() && state.edit.is_none();

        // 2. Press / drag / release while no editor is open. A press on another
        // cell while an editor is open is remembered as the anchor, so the
        // release (after the edit has finished) opens that cell right away.
        let (pressed, down, pointer) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()),
            )
        });
        let (long_press, max_dist) = ui.ctx().options(|o| {
            (
                o.input_options.max_click_duration,
                o.input_options.max_click_dist,
            )
        });
        let mut touch_hold_pending = false;
        if state.press.is_some() && state.press_touch {
            // Latest touch position, which on a release frame may only be in
            // the events.
            let pos = ui
                .input(|i| {
                    i.events.iter().rev().find_map(|e| match e {
                        egui::Event::Touch { pos, .. } => Some(*pos),
                        _ => None,
                    })
                })
                .or(pointer);
            if let (TouchMode::Pending, Some(pos), Some(anchor)) =
                (state.touch_mode, pos, state.press)
            {
                let now = ui.input(|i| i.time);
                let held = now - state.press_start >= long_press;
                if pos.distance(state.press_origin) > max_dist {
                    // Crossing into a horizontal neighbour is a drag-select; a
                    // vertical move is only one after a hold, as otherwise it
                    // cannot be told from a swipe.
                    let crossed = index_at(&cells, pos, orientation) != anchor;
                    state.touch_mode =
                        if held || (crossed && orientation == Orientation::Horizontal) {
                            TouchMode::Drag
                        } else {
                            TouchMode::Cancelled
                        };
                } else if now - state.press_start < long_press {
                    touch_hold_pending = true;
                }
            }
        }
        if touch_hold_pending {
            ui.ctx().request_repaint_after_secs(
                (long_press - (ui.input(|i| i.time) - state.press_start)) as f32,
            );
        }
        let mut drag_range: Option<(usize, usize)> = None;
        if let Some(edit) = &state.edit {
            if state.press.is_none()
                && pressed
                && let Some(i) = responses.iter().position(Response::hovered)
                && i != edit.host
            {
                state.press = Some(i);
                state.press_origin = pointer.unwrap_or_else(|| cells[i].center());
                state.press_start = ui.input(|i| i.time);
                state.touch_mode = TouchMode::Pending;
            } else if !down {
                state.clear_press();
            }
            if state.press.is_some() {
                state.press_touch |= touch_now;
            }
        } else {
            if state.press.is_none()
                && pressed
                && let Some(i) = responses.iter().position(Response::hovered)
            {
                state.press = Some(i);
                state.press_origin = pointer.unwrap_or_else(|| cells[i].center());
                state.press_start = ui.input(|i| i.time);
                state.touch_mode = TouchMode::Pending;
            }
            if state.press.is_some() {
                state.press_touch |= touch_now;
            }
            if let Some(anchor) = state.press {
                match pointer {
                    Some(pos) => {
                        let j = index_at(&cells, pos, orientation);
                        let (lo, hi) = (anchor.min(j), anchor.max(j));
                        let cancelled = state.touch_mode == TouchMode::Cancelled;
                        let tentative = state.press_touch && state.touch_mode == TouchMode::Pending;
                        if down {
                            if !cancelled && !tentative {
                                drag_range = Some((lo, hi));
                            }
                        } else {
                            if !cancelled && (hi > lo || cells[anchor].contains(pos)) {
                                state.edit = Some(Edit {
                                    lo,
                                    hi,
                                    host: anchor,
                                    buffer: entries[anchor].value.to_field_text(),
                                    fresh: true,
                                    touch: state.press_touch,
                                    keyboard: false,
                                });
                                ui.ctx().request_repaint();
                            }
                            state.clear_press();
                        }
                    }
                    None if !down => state.clear_press(),
                    None => {}
                }
            } else if !refocused
                && let Some(i) = responses.iter().position(Response::has_focus)
                && ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::Enter))
            {
                state.edit = Some(Edit {
                    lo: i,
                    hi: i,
                    host: i,
                    buffer: entries[i].value.to_field_text(),
                    fresh: true,
                    touch: false,
                    keyboard: true,
                });
                ui.ctx().request_repaint();
            }
        }

        // An AccessKit click (or Space) on a cell acts like Enter on it. A
        // pointer click has opened the editor above already.
        if state.edit.is_none()
            && state.press.is_none()
            && !refocused
            && let Some(i) = responses.iter().position(Response::clicked)
        {
            state.edit = Some(Edit {
                lo: i,
                hi: i,
                host: i,
                buffer: entries[i].value.to_field_text(),
                fresh: true,
                touch: false,
                keyboard: true,
            });
            ui.ctx().request_repaint();
        }

        // 3. Paint everything that sits under the editor.
        let editing = state.edit.as_ref().map(|e| (e.lo, e.hi, e.host));
        let hovered = responses.iter().position(Response::hovered);
        let selected = |i: usize| {
            editing.is_some_and(|(lo, hi, _)| (lo..=hi).contains(&i))
                || drag_range.is_some_and(|(lo, hi)| (lo..=hi).contains(&i))
        };
        let fills: Vec<Color32> = (0..n)
            .map(|i| match editing {
                Some((_, _, host)) if host == i => st.input_fill,
                _ if selected(i) => st.selected_fill(),
                _ if hovered == Some(i) && editing.is_none() => st.hover_fill,
                _ => st.fill,
            })
            .collect();
        let shapes = frame_shapes(group_rect, &cells, orientation, &st, &fills);
        ui.painter().set(bg_slot, Shape::Vec(shapes));

        // Are the editor's characters all selected? Then mirror that highlight.
        let all_selected = state.edit.as_ref().is_some_and(|e| {
            let len = e.buffer.chars().count();
            TextEdit::load_state(ui.ctx(), te_id)
                .and_then(|s| s.cursor.char_range())
                .is_some_and(|r| r.as_sorted_char_range() == (CharIndex(0)..CharIndex(len)))
        });

        for i in 0..n {
            let cell = cells[i];
            // Text never leaves its own cell.
            let painter = ui.painter_at(cell);
            let label = entries[i].label.take();
            let enabled = ui.is_enabled();
            let value = &*entries[i].value;
            let label_text = label.as_ref().map(|l| l.text());
            responses[i].widget_info(|| {
                let text = value.to_field_text();
                let mut info = WidgetInfo::text_edit(enabled, text.clone(), text, String::new());
                info.label = label_text.map(str::to_owned);
                info
            });
            if responses[i].has_focus() {
                if responses[i].gained_focus() {
                    responses[i].scroll_to_me(None);
                }
                let stroke = ui.visuals().selection.stroke;
                painter.add(Shape::rect_stroke(
                    cell,
                    corner_radius_for(i, n, orientation, st.radius),
                    stroke,
                    egui::epaint::StrokeKind::Inside,
                ));
            }
            if let Some(label) = label {
                let galley = make_galley(ui, label);
                let pos = egui::pos2(
                    cell.left() + st.padding.x,
                    cell.center().y - galley.size().y / 2.0,
                );
                painter.add(Shape::galley(pos, galley, st.muted_text));
            }
            let is_host = editing.is_some_and(|(_, _, h)| h == i);
            if is_host {
                continue; // the TextEdit draws this one
            }
            let mirrored = state
                .edit
                .as_ref()
                .filter(|e| (e.lo..=e.hi).contains(&i))
                .map(|e| e.buffer.as_str());
            let mirror_active = mirrored.is_some();
            let text = mirrored.map_or_else(|| value.to_field_text(), str::to_owned);
            let galley = make_galley(ui, text.into());
            if mirror_active && all_selected {
                let r =
                    Rect::from_center_size(cell.center(), galley.size()).expand2(vec2(2.0, 1.0));
                painter.add(Shape::rect_filled(r, 2.0, st.active_fill));
            }
            painter.add(centered_galley(cell, &galley, st.text));
        }

        if before.as_ref() != Some(&state) {
            ui.data_mut(|d| d.insert_temp(id, state));
        }
        if changed {
            response.mark_changed();
        }
        response
    }
}
