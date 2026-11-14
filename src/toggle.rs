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
use crate::geometry::corner_radius_for;
use crate::geometry::{Orientation, index_at, layout_cells, range_between};
use crate::paint::frame_shapes;
use crate::style::GroupStyle;
use crate::width::{Width, distribute};

struct Cell<'a> {
    off: Rc<dyn CellContent + 'a>,
    on: Rc<dyn CellContent + 'a>,
    value: &'a mut bool,
    tooltip: Option<String>,
}

/// What the current pointer gesture does as it moves across cells.
#[derive(Clone, Copy)]
enum Mode {
    /// Just pressed, and waiting one frame to tell a finger from a mouse.
    /// Only used when `settle_press` is on, see there.
    Undecided { start: f64, pos: Pos2 },
    /// Set every cell touched to `target`.
    Paint(bool),
    /// Exclusive mode: the cell under the pointer becomes the only one on.
    Move,
    /// Touch, not yet decided: a tap acts on release, moving to another cell
    /// starts a drag, and in an exclusive group a long press extends.
    Pending { start: f64, pos: Pos2 },
    /// The rest of the gesture does nothing: an extending press that changed
    /// nothing (it would have switched off the last active cell), or a touch
    /// that strayed too far from where it began to be a tap.
    Inert,
}

/// Pointer gesture in progress, kept in egui memory between frames.
#[derive(Clone, Copy)]
struct Gesture {
    mode: Mode,
    /// Last cell index visited, so skipped cells can be filled in.
    last: usize,
    /// Frame this was last updated in, to drop a gesture whose group stopped
    /// being shown (and so never saw the release).
    frame: u64,
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
    (
        changed,
        if changed {
            Mode::Paint(target)
        } else {
            Mode::Inert
        },
    )
}

/// A press by a mouse (or the keyboard): acts at once.
fn press(
    values: &mut [&mut bool],
    i: usize,
    exclusive: bool,
    extending: bool,
    require_one: bool,
) -> (bool, Mode) {
    if !exclusive {
        let target = !*values[i];
        (set_cell(values, i, target, false), Mode::Paint(target))
    } else if extending {
        extend_press(values, i, require_one)
    } else {
        plain_press(values, i, require_one)
    }
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
/// without touching the others, as in Blender.
///
/// On a touch screen nothing happens at the moment of touch: a tap acts on
/// release, dragging onto another cell starts a drag, and in an exclusive
/// group a long press extends.
#[must_use = "Add with `ui.add(...)`."]
pub struct ToggleGroup<'a> {
    id_salt: Id,
    cells: Vec<Cell<'a>>,
    orientation: Orientation,
    equal_size: bool,
    width: Width,
    highlight_active: bool,
    exclusive: bool,
    require_one: bool,
    extend: Modifiers,
    allow_extend: bool,
    settle_press: bool,
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
            width: Width::Auto,
            highlight_active: true,
            exclusive: false,
            require_one: false,
            extend: Modifiers::SHIFT,
            allow_extend: true,
            settle_press: cfg!(target_arch = "wasm32"),
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
        let n_values = values.len();
        let mut n_labels = 0;
        let mut values = values.iter_mut();
        for label in labels {
            n_labels += 1;
            if let Some(value) = values.next() {
                self = self.cell(label, value);
            }
        }
        debug_assert_eq!(
            n_labels, n_values,
            "ToggleGroup::cells: {n_labels} labels but {n_values} values; the extra ones would be silently dropped",
        );
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

    /// Whether an [exclusive](Self::exclusive) group can be extended to
    /// several active cells with the [extend modifier](Self::extend_modifier)
    /// or a long press on touch. Default `true`. With `false` a press always
    /// selects only the pressed cell. For a group bound to a single value,
    /// see [`RadioGroup`](crate::RadioGroup).
    pub fn allow_extend(mut self, allow: bool) -> Self {
        self.allow_extend = allow;
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

    /// Wait one frame after a press before acting on it, to tell a finger
    /// from a mouse.
    ///
    /// This works around a browser quirk: a touch fires the pointer event
    /// first, and eframe runs a frame for it before the touch event arrives,
    /// so on that frame the two look the same. It is on by default on wasm
    /// and off elsewhere, where touch and pointer events arrive together.
    /// Exposed so the web behaviour can be tested natively.
    #[doc(hidden)]
    pub fn settle_press(mut self, settle: bool) -> Self {
        self.settle_press = settle;
        self
    }

    /// Stack the cells top to bottom instead of left to right.
    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Give every cell the width of the widest one (horizontal groups).
    ///
    /// This sets the natural size only: an explicit [`width`](Self::width) or
    /// [`fill_width`](Self::fill_width) wins, and then scales these equal
    /// cells, which therefore stay equal.
    pub fn equal_width(mut self) -> Self {
        self.equal_size = true;
        self
    }

    /// Make the whole group exactly `width` points wide.
    ///
    /// Horizontal groups share the width between their cells in proportion to
    /// their natural widths (so [`equal_width`](Self::equal_width) cells stay
    /// equal), down to a small minimum per cell; cells may become narrower
    /// than `interact_size.x`. Vertical groups get this as their cross-axis
    /// width. Takes precedence over `equal_width`. A non-finite or negative
    /// value is ignored. Default: natural size.
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
            width,
            highlight_active,
            exclusive,
            require_one,
            extend,
            allow_extend,
            settle_press,
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
            .map(|(a, b)| {
                // `cell()` shares one Rc between both states: measure once.
                let size = if Rc::ptr_eq(a, b) {
                    a.size(ui)
                } else {
                    a.size(ui).max(b.size(ui))
                };
                size + pad * 2.0
            })
            .collect();
        let min = ui.spacing().interact_size;
        let max_w = content.iter().map(|s| s.x).fold(min.x, f32::max);
        let max_h = content.iter().map(|s| s.y).fold(min.y, f32::max);
        let target = width.resolve(ui.available_width());
        let (cross, mut mains): (f32, Vec<f32>) = match orientation {
            Orientation::Horizontal => (
                max_h,
                content
                    .iter()
                    .map(|s| if equal_size { max_w } else { s.x.max(min.x) })
                    .collect(),
            ),
            Orientation::Vertical => (target.unwrap_or(max_w), vec![max_h; n]),
        };
        if let (Some(target), Orientation::Horizontal) = (target, orientation) {
            mains = distribute(&mains, target, (pad.x * 2.0).max(4.0));
        }
        let total_main: f32 = mains.iter().sum();
        let total = match orientation {
            Orientation::Horizontal => vec2(total_main, cross),
            Orientation::Vertical => vec2(cross, total_main),
        };
        let (group_rect, mut response) = ui.allocate_exact_size(total, Sense::hover());
        let frame_nr = ui.ctx().cumulative_frame_nr();
        // Scrolled out of view: the cells stay registered (so Tab reaches them
        // and a focused cell keeps focus and keyboard activation), but nothing
        // is painted and pointer gestures are dropped.
        let visible = ui.is_rect_visible(group_rect);
        let rects = layout_cells(group_rect.min, &mains, cross, orientation);
        let responses: Vec<Response> = rects
            .iter()
            .enumerate()
            .map(|(i, r)| ui.interact(*r, id.with(i), Sense::click_and_drag()))
            .collect();

        // Interaction.
        let (pressed, down, pointer, modifiers, touch, now) = ui.input(|i| {
            (
                visible && i.pointer.primary_pressed(),
                visible && i.pointer.primary_down(),
                i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()),
                i.modifiers,
                // A tap can start and end within one frame, leaving no touch
                // active at its end, so also look for touch events.
                i.any_touches()
                    || i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::Touch { .. })),
                i.time,
            )
        });
        let (long_press, max_dist) = ui.ctx().options(|o| {
            (
                o.input_options.max_click_duration,
                o.input_options.max_click_dist,
            )
        });
        let extending = allow_extend && held(modifiers, extend);
        // A gesture not touched last frame belongs to an earlier showing of
        // this group (hidden or culled mid-drag): discard it.
        let mut gesture: Option<Gesture> = ui
            .data(|d| d.get_temp::<Gesture>(id))
            .filter(|g| visible && g.frame.saturating_add(1) >= frame_nr);
        let was_active = gesture.is_some();
        let mut began = false;
        let mut changed = false;

        if gesture.is_none()
            && pressed
            && let Some(i) = responses.iter().position(Response::hovered)
        {
            let origin = pointer.unwrap_or_else(|| rects[i].center());
            let mode = if settle_press {
                Mode::Undecided {
                    start: now,
                    pos: origin,
                }
            } else if touch {
                Mode::Pending {
                    start: now,
                    pos: origin,
                }
            } else {
                let (c, mode) = press(&mut values, i, exclusive, extending, require_one);
                changed |= c;
                mode
            };
            gesture = Some(Gesture {
                mode,
                last: i,
                frame: frame_nr,
            });
            began = true;
        }
        // With `settle_press`: decide what kind of press it was, a frame later.
        if let Some(g) = gesture.as_mut()
            && let Mode::Undecided { start, pos } = g.mode
            && (!began || !down)
        {
            if touch {
                // Fingers can't hover and may be starting a scroll, so
                // nothing happens until release, a drag or a long press.
                g.mode = Mode::Pending { start, pos };
            } else {
                let (c, mode) = press(&mut values, g.last, exclusive, extending, require_one);
                changed |= c;
                g.mode = mode;
            }
        }
        if let Some(g) = gesture.as_mut()
            && matches!(g.mode, Mode::Undecided { .. })
        {
            ui.ctx().request_repaint();
        }
        if !began
            && down
            && let (Some(g), Some(pos)) = (gesture.as_mut(), pointer)
        {
            let idx = index_at(&rects, pos, orientation);
            match g.mode {
                Mode::Undecided { .. } => {}
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
                        if exclusive {
                            changed |= select_only(&mut values, idx);
                            g.mode = Mode::Move;
                        } else {
                            let target = !*values[g.last];
                            for k in range_between(g.last, idx) {
                                changed |= set_cell(&mut values, k, target, false);
                            }
                            g.mode = Mode::Paint(target);
                        }
                        g.last = idx;
                    } else if pos.distance(origin) > max_dist {
                        // Wandered off within the same cell (a scroll, say):
                        // neither a tap nor a long press any more.
                        g.mode = Mode::Inert;
                    } else if exclusive && allow_extend {
                        if now - start >= long_press {
                            let (c, mode) = extend_press(&mut values, g.last, require_one);
                            changed |= c;
                            g.mode = mode;
                        } else {
                            // No input arrives while the finger rests, so ask
                            // for a frame when the long press is due.
                            ui.ctx()
                                .request_repaint_after_secs((long_press - (now - start)) as f32);
                        }
                    }
                }
                Mode::Inert => {}
            }
        }
        if !down {
            // A touch released without moving or waiting is a tap.
            if let Some(Gesture {
                mode: Mode::Pending { pos: origin, .. },
                last,
                ..
            }) = gesture
                // The release frame can carry the whole movement, with no
                // `down` frame to notice it in.
                && !ui.input(|i| {
                    i.events.iter().any(|e| {
                        matches!(e, egui::Event::Touch { pos, .. } if pos.distance(origin) > max_dist)
                    })
                })
                && pointer.is_none_or(|p| p.distance(origin) <= max_dist)
            {
                changed |= if exclusive {
                    plain_press(&mut values, last, require_one).0
                } else {
                    let target = !*values[last];
                    set_cell(&mut values, last, target, false)
                };
            }
            gesture = None;
        }

        // Keyboard activation (Space / Enter on a focused cell). Pointer
        // presses are handled above, so skip clicks that belong to a gesture.
        if !was_active && !began {
            for (i, r) in responses.iter().enumerate() {
                if r.clicked() {
                    changed |= press(&mut values, i, exclusive, extending, require_one).0;
                }
            }
        }

        ui.data_mut(|d| match gesture {
            Some(g) => {
                d.insert_temp(
                    id,
                    Gesture {
                        frame: frame_nr,
                        ..g
                    },
                );
            }
            None => {
                d.remove::<Gesture>(id);
            }
        });

        // Accessibility.
        for (i, r) in responses.iter().enumerate() {
            let on = *values[i];
            let shown = if on { &ons[i] } else { &offs[i] };
            let enabled = ui.is_enabled();
            r.widget_info(|| {
                let label = tooltips[i]
                    .clone()
                    .or_else(|| shown.accessible_label())
                    .unwrap_or_default();
                WidgetInfo::selected(WidgetType::Button, enabled, on, label)
            });
            if r.gained_focus() {
                r.scroll_to_me(None);
            }
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                if let Some(text) = &tooltips[i] {
                    r.clone().on_hover_text(text);
                }
            }
        }

        if !visible {
            if changed {
                response.mark_changed();
            }
            return response;
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
        // Clip each cell's content to its own cell, so a width that squeezes
        // cells below their content can't spill into the neighbours.
        let outer_clip = ui.clip_rect();
        for i in 0..n {
            let on = *values[i];
            let color = if highlight_active && on {
                st.active_text
            } else {
                st.text
            };
            let shown = if on { &ons[i] } else { &offs[i] };
            ui.set_clip_rect(outer_clip.intersect(rects[i]));
            shown.paint(ui, rects[i], color);
        }
        ui.set_clip_rect(outer_clip);
        // Keyboard focus ring, following each cell's own corner rounding.
        for (i, r) in responses.iter().enumerate() {
            if r.has_focus() {
                ui.painter().add(Shape::rect_stroke(
                    rects[i],
                    corner_radius_for(i, n, orientation, st.radius),
                    ui.visuals().selection.stroke,
                    egui::StrokeKind::Inside,
                ));
            }
        }

        if changed {
            response.mark_changed();
        }
        response
    }
}
