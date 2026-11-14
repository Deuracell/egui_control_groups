//! Joined text / number fields with Blender-style multi-editing: press in one
//! field and drag across its neighbours to select several, then type once and
//! the value is applied to all of them.

use std::hash::Hash;

use egui::text::{CCursor, CCursorRange, CharIndex};
use egui::{
    Align, Color32, Id, Key, Margin, Modifiers, Rect, Response, Sense, Shape, TextEdit, TextStyle,
    Ui, Vec2, Widget, WidgetText, vec2,
};

use crate::geometry::{Orientation, index_at, layout_cells};
use crate::paint::{centered_galley, frame_shapes, make_galley};
use crate::style::GroupStyle;

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

fn format_float(v: f64) -> String {
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

struct Entry<'a> {
    label: Option<WidgetText>,
    value: &'a mut dyn FieldValue,
}

#[derive(Clone, Default)]
struct State {
    /// Cell the pointer went down on, while the button is held.
    press: Option<usize>,
    edit: Option<Edit>,
}

#[derive(Clone)]
struct Edit {
    lo: usize,
    hi: usize,
    /// The cell hosting the real text editor. Other selected cells mirror it.
    host: usize,
    buffer: String,
    /// First frame: select all and take focus, ignore commit checks.
    fresh: bool,
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
///   commits, Escape cancels.
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
    style: GroupStyle,
}

impl<'a> FieldGroup<'a> {
    /// `id_salt` must be unique among groups in the same `Ui`.
    pub fn new(id_salt: impl Hash + std::fmt::Debug) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            entries: Vec::new(),
            orientation: Orientation::Horizontal,
            cell_width: None,
            style: GroupStyle::default(),
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

    /// Stack the fields top to bottom instead of left to right.
    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Width of every field. Defaults to half of `Spacing::text_edit_width`.
    pub fn cell_width(mut self, width: f32) -> Self {
        self.cell_width = Some(width);
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
            style,
        } = self;
        let n = entries.len();
        if n == 0 {
            return ui.allocate_response(Vec2::ZERO, Sense::hover());
        }
        let st = style.resolve(ui);
        let id = ui.make_persistent_id(id_salt);
        let font = TextStyle::Button;
        let text_h = ui.text_style_height(&font);

        let cell_w = cell_width.unwrap_or(ui.spacing().text_edit_width * 0.5);
        let cell_h = ui
            .spacing()
            .interact_size
            .y
            .max(text_h + st.padding.y * 2.0);
        let (mains, cross, total) = match orientation {
            Orientation::Horizontal => (vec![cell_w; n], cell_h, vec2(cell_w * n as f32, cell_h)),
            Orientation::Vertical => (vec![cell_h; n], cell_w, vec2(cell_w, cell_h * n as f32)),
        };
        let (group_rect, mut response) = ui.allocate_exact_size(total, Sense::hover());
        let cells = layout_cells(group_rect.min, &mains, cross, orientation);
        let responses: Vec<Response> = cells
            .iter()
            .enumerate()
            .map(|(i, r)| ui.interact(*r, id.with(i), Sense::click_and_drag()))
            .collect();

        let mut state: State = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        let mut changed = false;

        // Reserve a paint slot below the text editor, filled in at the end.
        let bg_slot = ui.painter().add(Shape::Noop);
        let te_id = id.with("editor");

        // 1. The live text editor, if a session is active.
        let mut finish: Option<bool> = None;
        if let Some(edit) = state.edit.as_mut() {
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
            let out = ui.place(area, editor);
            let escape = ui.input(|i| i.key_pressed(Key::Escape));
            if edit.fresh {
                edit.fresh = false;
            } else if escape {
                finish = Some(false);
            } else if out.lost_focus() {
                finish = Some(true);
            }
        }
        if let Some(commit) = finish
            && let Some(edit) = state.edit.take()
        {
            if commit {
                for entry in &mut entries[edit.lo..=edit.hi] {
                    changed |= entry.value.set_from_field_text(&edit.buffer);
                }
            }
            ui.ctx().request_repaint();
        }

        // 2. Press / drag / release while no editor is open.
        let (pressed, down, pointer) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()),
            )
        });
        let mut drag_range: Option<(usize, usize)> = None;
        if state.edit.is_none() {
            if state.press.is_none()
                && pressed
                && let Some(i) = responses.iter().position(Response::hovered)
            {
                state.press = Some(i);
            }
            if let Some(anchor) = state.press {
                match pointer {
                    Some(pos) => {
                        let j = index_at(&cells, pos, orientation);
                        let (lo, hi) = (anchor.min(j), anchor.max(j));
                        if down {
                            drag_range = Some((lo, hi));
                        } else {
                            if hi > lo || cells[anchor].contains(pos) {
                                state.edit = Some(Edit {
                                    lo,
                                    hi,
                                    host: anchor,
                                    buffer: entries[anchor].value.to_field_text(),
                                    fresh: true,
                                });
                                ui.ctx().request_repaint();
                            }
                            state.press = None;
                        }
                    }
                    None if !down => state.press = None,
                    None => {}
                }
            } else if let Some(i) = responses.iter().position(Response::has_focus)
                && ui.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::Enter))
            {
                state.edit = Some(Edit {
                    lo: i,
                    hi: i,
                    host: i,
                    buffer: entries[i].value.to_field_text(),
                    fresh: true,
                });
                ui.ctx().request_repaint();
            }
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
        let mut shapes = frame_shapes(group_rect, &cells, orientation, &st, &fills);

        // Are the editor's characters all selected? Then mirror that highlight.
        let all_selected = state.edit.as_ref().is_some_and(|e| {
            let len = e.buffer.chars().count();
            TextEdit::load_state(ui.ctx(), te_id)
                .and_then(|s| s.cursor.char_range())
                .is_some_and(|r| r.as_sorted_char_range() == (CharIndex(0)..CharIndex(len)))
        });

        for i in 0..n {
            let cell = cells[i];
            if let Some(label) = entries[i].label.take() {
                let galley = make_galley(ui, label);
                let pos = egui::pos2(
                    cell.left() + st.padding.x,
                    cell.center().y - galley.size().y / 2.0,
                );
                shapes.push(Shape::galley(pos, galley, st.muted_text));
            }
            let is_host = editing.is_some_and(|(_, _, h)| h == i);
            if is_host {
                continue; // the TextEdit draws this one
            }
            let mirrored = state
                .edit
                .as_ref()
                .filter(|e| (e.lo..=e.hi).contains(&i))
                .map(|e| e.buffer.clone());
            let mirror_active = mirrored.is_some();
            let text = mirrored.unwrap_or_else(|| entries[i].value.to_field_text());
            let galley = make_galley(ui, text.into());
            if mirror_active && all_selected {
                let r =
                    Rect::from_center_size(cell.center(), galley.size()).expand2(vec2(2.0, 1.0));
                shapes.push(Shape::rect_filled(r, 2.0, st.active_fill));
            }
            shapes.push(centered_galley(cell, &galley, st.text));
        }
        ui.painter().set(bg_slot, Shape::Vec(shapes));

        ui.data_mut(|d| d.insert_temp(id, state));
        if changed {
            response.mark_changed();
        }
        response
    }
}
