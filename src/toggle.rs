//! A row (or column) of toggle buttons drawn as one rounded object, where
//! pressing one cell and dragging across the others paints them all to the
//! same state, like Blender's toggle rows.

use std::hash::Hash;
use std::rc::Rc;

use egui::{
    Color32, Id, Modifiers, Pos2, Response, Sense, Shape, Ui, Vec2, Widget, WidgetInfo, WidgetType,
    vec2,
};

use crate::content::CellContent;
use crate::geometry::{Orientation, index_at, layout_cells, range_between};
use crate::paint::frame_shapes;
use crate::style::GroupStyle;

struct Cell<'a> {
    off: Rc<dyn CellContent + 'a>,
    on: Rc<dyn CellContent + 'a>,
    value: &'a mut bool,
    tooltip: Option<String>,
}

/// What the current pointer gesture does as it moves across cells.
#[derive(Clone, Copy)]
enum Mode {
    /// Set every cell touched to `target`.
    Paint(bool),
    /// Exclusive mode: the cell under the pointer becomes the only one on.
    Move,
    /// Touch in exclusive mode, not yet decided: a tap selects, a long press
    /// extends, moving to another cell moves the selection.
    Pending { start: f64, pos: Pos2 },
    /// An extending press that changed nothing (it would have switched off
    /// the last active cell); dragging does nothing.
    Inert,
}

/// Pointer gesture in progress, kept in egui memory between frames.
#[derive(Clone, Copy)]
struct Gesture {
    mode: Mode,
    /// Last cell index visited, so skipped cells can be filled in.
    last: usize,
}

fn count_on(values: &[&mut bool]) -> usize {
    values.iter().filter(|v| ***v).count()
}

/// Set one cell, refusing to switch off the last active cell when
/// `require_one` is set. Returns whether anything changed.
fn set_cell(values: &mut [&mut bool], i: usize, target: bool, require_one: bool) -> bool {
    if *values[i] == target || (!target && require_one && count_on(values) <= 1) {
        return false;
    }
    *values[i] = target;
    true
}

/// Make `i` the only active cell.
fn select_only(values: &mut [&mut bool], i: usize) -> bool {
    let mut changed = false;
    for k in 0..values.len() {
        changed |= set_cell(values, k, k == i, false);
    }
    changed
}

/// A plain (non-extending) press in exclusive mode. Pressing the only active
/// cell switches it off unless `require_one` is set. Returns the change flag
/// and the gesture mode to continue in.
fn plain_press(values: &mut [&mut bool], i: usize, require_one: bool) -> (bool, Mode) {
    let only = *values[i] && count_on(values) == 1;
    if only {
        (set_cell(values, i, false, require_one), Mode::Move)
    } else {
        (select_only(values, i), Mode::Move)
    }
}

/// An extending press: flip `i`, leave the others, and paint from there.
fn extend_press(values: &mut [&mut bool], i: usize, require_one: bool) -> (bool, Mode) {
    let target = !*values[i];
    let changed = set_cell(values, i, target, require_one);
    (changed, if changed { Mode::Paint(target) } else { Mode::Inert })
}

/// Whether every modifier required by `want` is held in `have`.
fn held(have: Modifiers, want: Modifiers) -> bool {
    (!want.shift || have.shift)
        && (!want.alt || have.alt)
        && (!want.ctrl || have.ctrl)
        && (!want.command || have.command)
        && (!want.mac_cmd || have.mac_cmd)
}

/// Joined toggle buttons.
///
/// ```no_run
/// # use egui_control_groups::ToggleGroup;
/// # egui::__run_test_ui(|ui| {
/// let (mut x, mut y, mut z) = (true, false, false);
/// let response = ui.add(
///     ToggleGroup::new("axes")
///         .cell("X", &mut x)
///         .cell("Y", &mut y)
///         .cell("Z", &mut z),
/// );
/// if response.changed() { /* at least one value flipped */ }
/// # });
/// ```
///
/// Press a cell to flip it, then keep the button down and drag across
/// neighbours: each one is set to the state the first cell just took. Cells
/// the pointer skips between frames are filled in too, and the drag keeps
/// working when the pointer leaves the group sideways.
///
/// With [`exclusive`](ToggleGroup::exclusive), a plain press selects only the
/// pressed cell. Holding the extend modifier (Shift by default) flips cells
/// without touching the others, as in Blender. On a touch screen a tap
/// selects and a long press extends.
#[must_use = "Add with `ui.add(...)`."]
pub struct ToggleGroup<'a> {
    id_salt: Id,
    cells: Vec<Cell<'a>>,
    orientation: Orientation,
    equal_size: bool,
    highlight_active: bool,
    exclusive: bool,
    require_one: bool,
    extend: Modifiers,
    style: GroupStyle,
}

impl<'a> ToggleGroup<'a> {
    /// `id_salt` must be unique among groups in the same `Ui`.
    pub fn new(id_salt: impl Hash + std::fmt::Debug) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            cells: Vec::new(),
            orientation: Orientation::Horizontal,
            equal_size: false,
            highlight_active: true,
            exclusive: false,
            require_one: false,
            extend: Modifiers::SHIFT,
            style: GroupStyle::default(),
        }
    }

    /// Append one cell bound to `value`, showing the same content in both
    /// states (the active fill then tells them apart).
    ///
    /// `label` is anything implementing [`CellContent`]: a `&str`, `String`,
    /// `RichText`, an `egui::Image`, an [`Icon`](crate::Icon) or a
    /// [`PaintCell`](crate::PaintCell).
    pub fn cell(mut self, label: impl CellContent + 'a, value: &'a mut bool) -> Self {
        let content: Rc<dyn CellContent + 'a> = Rc::new(label);
        self.cells.push(Cell {
            off: content.clone(),
            on: content,
            value,
            tooltip: None,
        });
        self
    }

    /// Append a cell whose content changes with its state, for example an
    /// "eye closed" icon when `false` and an "eye open" icon when `true`.
    ///
    /// The cell is sized to the larger of the two, so toggling never shifts
    /// the layout. Combine with [`highlight_active(false)`](Self::highlight_active)
    /// for plain buttons where only the icon shows the state.
    pub fn state_cell(
        mut self,
        off: impl CellContent + 'a,
        on: impl CellContent + 'a,
        value: &'a mut bool,
    ) -> Self {
        self.cells.push(Cell {
            off: Rc::new(off),
            on: Rc::new(on),
            value,
            tooltip: None,
        });
        self
    }

    /// Append one cell per `(content, value)` pair.
    pub fn cells<L: CellContent + 'a>(
        mut self,
        labels: impl IntoIterator<Item = L>,
        values: &'a mut [bool],
    ) -> Self {
        for (label, value) in labels.into_iter().zip(values.iter_mut()) {
            self = self.cell(label, value);
        }
        self
    }

    /// Hover text for the most recently added cell. It is also the cell's
    /// accessible name, which icon-only cells need.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        if let Some(cell) = self.cells.last_mut() {
            cell.tooltip = Some(text.into());
        }
        self
    }

    /// Whether an "on" cell gets the active fill and text colour. Default
    /// `true`. Turn it off for plain buttons whose state is shown only by
    /// their content (see [`state_cell`](Self::state_cell)).
    pub fn highlight_active(mut self, highlight: bool) -> Self {
        self.highlight_active = highlight;
        self
    }

    /// Make the group exclusive, like Blender's toggle rows. A plain press
    /// turns the other cells off; dragging moves the selection. Holding the
    /// [extend modifier](Self::extend_modifier) (or long-pressing on touch)
    /// flips the pressed cell and leaves the rest, so several can be on.
    /// Pressing the only active cell switches it off, see
    /// [`require_one`](Self::require_one). Default `false`.
    pub fn exclusive(mut self, exclusive: bool) -> Self {
        self.exclusive = exclusive;
        self
    }

    /// In an exclusive group, never let the last active cell be switched
    /// off, which makes it behave like radio buttons. Default `false`.
    pub fn require_one(mut self, require_one: bool) -> Self {
        self.require_one = require_one;
        self
    }

    /// The modifier that makes a press extend instead of select in an
    /// [exclusive](Self::exclusive) group. Default [`Modifiers::SHIFT`]; use
    /// [`Modifiers::COMMAND`] for Cmd on macOS and Ctrl elsewhere. Does not
    /// apply to touch, where a long press extends.
    pub fn extend_modifier(mut self, modifier: Modifiers) -> Self {
        self.extend = modifier;
        self
    }

    /// Stack the cells top to bottom instead of left to right.
    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Give every cell the width of the widest one (horizontal groups).
    pub fn equal_width(mut self) -> Self {
        self.equal_size = true;
        self
    }

    /// Override parts of the look. Anything left unset follows `egui::Style`.
    pub fn style(mut self, style: GroupStyle) -> Self {
        self.style = style;
        self
    }
}

impl Widget for ToggleGroup<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self {
            id_salt,
            cells,
            orientation,
            equal_size,
            highlight_active,
            exclusive,
            require_one,
            extend,
            style,
        } = self;
        let n = cells.len();
        if n == 0 {
            return ui.allocate_response(Vec2::ZERO, Sense::hover());
        }
        let st = style.resolve(ui);
        let id = ui.make_persistent_id(id_salt);

        let mut offs = Vec::with_capacity(n);
        let mut ons = Vec::with_capacity(n);
        let mut tooltips = Vec::with_capacity(n);
        let mut values = Vec::with_capacity(n);
        for c in cells {
            offs.push(c.off);
            ons.push(c.on);
            tooltips.push(c.tooltip);
            values.push(c.value);
        }

        // Sizes: each cell fits the larger of its two states.
        let pad = st.padding;
        let content: Vec<Vec2> = offs
            .iter()
            .zip(&ons)
            .map(|(a, b)| a.size(ui).max(b.size(ui)) + pad * 2.0)
            .collect();
        let min = ui.spacing().interact_size;
        let max_w = content.iter().map(|s| s.x).fold(min.x, f32::max);
        let max_h = content.iter().map(|s| s.y).fold(min.y, f32::max);
        let (cross, mains): (f32, Vec<f32>) = match orientation {
            Orientation::Horizontal => (
                max_h,
                content
                    .iter()
                    .map(|s| if equal_size { max_w } else { s.x.max(min.x) })
                    .collect(),
            ),
            Orientation::Vertical => (max_w, vec![max_h; n]),
        };
        let total_main: f32 = mains.iter().sum();
        let total = match orientation {
            Orientation::Horizontal => vec2(total_main, cross),
            Orientation::Vertical => vec2(cross, total_main),
        };
        let (group_rect, mut response) = ui.allocate_exact_size(total, Sense::hover());
        let rects = layout_cells(group_rect.min, &mains, cross, orientation);
        let responses: Vec<Response> = rects
            .iter()
            .enumerate()
            .map(|(i, r)| ui.interact(*r, id.with(i), Sense::click_and_drag()))
            .collect();

        // Interaction.
        let (pressed, down, pointer, modifiers, touch, now) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()),
                i.modifiers,
                i.any_touches(),
                i.time,
            )
        });
        let (long_press, max_dist) = ui
            .ctx()
            .options(|o| (o.input_options.max_click_duration, o.input_options.max_click_dist));
        let extending = held(modifiers, extend);
        let mut gesture: Option<Gesture> = ui.data(|d| d.get_temp(id));
        let was_active = gesture.is_some();
        let mut began = false;
        let mut changed = false;

        if gesture.is_none() {
            if pressed && let Some(i) = responses.iter().position(Response::hovered) {
                let mode = if !exclusive {
                    let target = !*values[i];
                    changed |= set_cell(&mut values, i, target, false);
                    Mode::Paint(target)
                } else if touch {
                    Mode::Pending {
                        start: now,
                        pos: pointer.unwrap_or_else(|| rects[i].center()),
                    }
                } else {
                    let (c, mode) = if extending {
                        extend_press(&mut values, i, require_one)
                    } else {
                        plain_press(&mut values, i, require_one)
                    };
                    changed |= c;
                    mode
                };
                gesture = Some(Gesture { mode, last: i });
                began = true;
            }
        } else if down && let (Some(g), Some(pos)) = (gesture.as_mut(), pointer) {
            let idx = index_at(&rects, pos, orientation);
            match g.mode {
                Mode::Paint(target) => {
                    for k in range_between(g.last, idx) {
                        changed |= set_cell(&mut values, k, target, exclusive && require_one);
                    }
                    g.last = idx;
                }
                Mode::Move => {
                    if idx != g.last {
                        changed |= select_only(&mut values, idx);
                        g.last = idx;
                    }
                }
                Mode::Pending { start, pos: origin } => {
                    if idx != g.last {
                        // Moved to another cell: a drag, not a tap.
                        changed |= select_only(&mut values, idx);
                        g.mode = Mode::Move;
                        g.last = idx;
                    } else if pos.distance(origin) <= max_dist && now - start >= long_press {
                        let (c, mode) = extend_press(&mut values, g.last, require_one);
                        changed |= c;
                        g.mode = mode;
                    }
                }
                Mode::Inert => {}
            }
        }
        if !down {
            // A touch released without moving or waiting is a tap.
            if let Some(Gesture {
                mode: Mode::Pending { .. },
                last,
            }) = gesture
            {
                changed |= plain_press(&mut values, last, require_one).0;
            }
            gesture = None;
        }

        // Keyboard activation (Space / Enter on a focused cell). Pointer
        // presses are handled above, so skip clicks that belong to a gesture.
        if !was_active && !began {
            for (i, r) in responses.iter().enumerate() {
                if r.clicked() {
                    changed |= if !exclusive {
                        let target = !*values[i];
                        set_cell(&mut values, i, target, false)
                    } else if extending {
                        extend_press(&mut values, i, require_one).0
                    } else {
                        plain_press(&mut values, i, require_one).0
                    };
                }
            }
        }

        ui.data_mut(|d| match gesture {
            Some(g) => {
                d.insert_temp(id, g);
            }
            None => {
                d.remove::<Gesture>(id);
            }
        });

        // Accessibility.
        for (i, r) in responses.iter().enumerate() {
            let on = *values[i];
            let shown = if on { &ons[i] } else { &offs[i] };
            let label = tooltips[i]
                .clone()
                .or_else(|| shown.accessible_label())
                .unwrap_or_default();
            r.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, label.clone()));
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                if let Some(text) = &tooltips[i] {
                    r.clone().on_hover_text(text);
                }
            }
        }

        // Paint.
        let hovered = gesture
            .map(|g| g.last)
            .or_else(|| responses.iter().position(Response::hovered));
        let fills: Vec<Color32> = (0..n)
            .map(|i| {
                if highlight_active && *values[i] {
                    st.active_fill
                } else if hovered == Some(i) {
                    st.hover_fill
                } else {
                    st.fill
                }
            })
            .collect();
        let shapes: Vec<Shape> = frame_shapes(group_rect, &rects, orientation, &st, &fills);
        ui.painter().extend(shapes);
        for i in 0..n {
            let on = *values[i];
            let color = if highlight_active && on {
                st.active_text
            } else {
                st.text
            };
            let shown = if on { &ons[i] } else { &offs[i] };
            shown.paint(ui, rects[i], color);
        }

        if changed {
            response.mark_changed();
        }
        response
    }
}
