//! A row (or column) of toggle buttons drawn as one rounded object, where
//! pressing one cell and dragging across the others paints them all to the
//! same state, like Blender's toggle rows.

use std::hash::Hash;
use std::rc::Rc;

use egui::style::ScrollAnimation;
use egui::{
    Color32, Id, Key, Modifiers, Popup, Pos2, Rect, Response, Sense, Shape, Stroke, TextStyle, Ui,
    Vec2, Widget, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::content::{CellContent, columns_width};
use crate::geometry::corner_radius_for;
use crate::geometry::{Orientation, index_at, layout_cells, range_between};
use crate::paint::frame_shapes;
use crate::style::{GroupStyle, Resolved};
use crate::width::{Width, distribute};

/// The binding of a dropdown cell, with the value's type erased so that
/// `ToggleGroup` itself needn't be generic.
trait DropdownBinding<'a> {
    /// Index of the option equal to the bound value, if any.
    fn selected(&self) -> Option<usize>;
    /// Contents of the options, in order.
    fn options(&self) -> &[Rc<dyn CellContent + 'a>];
    /// Make option `i` the bound value. Call at most once, and read nothing
    /// else from the binding afterwards: it swaps the option's value into the
    /// binding (so the value type needn't be `Clone`).
    fn select(&mut self, i: usize);
}

struct Bound<'a, T> {
    value: &'a mut T,
    /// The option values (`values[i]` belongs to `contents[i]`).
    values: Vec<T>,
    contents: Vec<Rc<dyn CellContent + 'a>>,
}

impl<'a, T: PartialEq> DropdownBinding<'a> for Bound<'a, T> {
    fn selected(&self) -> Option<usize> {
        self.values.iter().position(|v| v == &*self.value)
    }

    fn options(&self) -> &[Rc<dyn CellContent + 'a>] {
        &self.contents
    }

    fn select(&mut self, i: usize) {
        std::mem::swap(self.value, &mut self.values[i]);
    }
}

enum CellKind<'a> {
    Toggle {
        off: Rc<dyn CellContent + 'a>,
        on: Rc<dyn CellContent + 'a>,
        value: &'a mut bool,
    },
    Dropdown(Box<dyn DropdownBinding<'a> + 'a>),
}

struct Cell<'a> {
    kind: CellKind<'a>,
    tooltip: Option<String>,
}

/// What a cell shows, once the group is being drawn.
enum Content<'a> {
    Toggle {
        off: Rc<dyn CellContent + 'a>,
        on: Rc<dyn CellContent + 'a>,
    },
    Dropdown {
        binding: Box<dyn DropdownBinding<'a> + 'a>,
        /// The option matching the bound value, found before drawing.
        selected: Option<usize>,
    },
}

/// The state of every cell that is a toggle; `None` marks a dropdown cell,
/// which is not a value and which every helper below leaves alone.
type Values<'a> = [Option<&'a mut bool>];

fn is_on(values: &Values, i: usize) -> bool {
    matches!(values[i].as_deref(), Some(true))
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
    /// Touch that has moved along the group but is still on the cell it began
    /// on: no longer a tap or a long press, and a drag once it reaches
    /// another cell.
    Armed,
    /// Touch that moved across the group instead of along it: a swipe, which
    /// scrolls the surrounding scroll area and leaves the cells alone.
    Scroll,
    /// The rest of the gesture does nothing: an extending press that changed
    /// nothing (it would have switched off the last active cell).
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

fn count_on(values: &Values) -> usize {
    values
        .iter()
        .filter(|v| matches!(v.as_deref(), Some(true)))
        .count()
}

/// Set one cell, refusing to switch off the last active cell when
/// `require_one` is set. Returns whether anything changed.
fn set_cell(values: &mut Values, i: usize, target: bool, require_one: bool) -> bool {
    if values[i].is_none() || is_on(values, i) == target {
        return false;
    }
    if !target && require_one && count_on(values) <= 1 {
        return false;
    }
    if let Some(v) = values[i].as_deref_mut() {
        *v = target;
    }
    true
}

/// Make `i` the only active cell.
fn select_only(values: &mut Values, i: usize) -> bool {
    let mut changed = false;
    for k in 0..values.len() {
        changed |= set_cell(values, k, k == i, false);
    }
    changed
}

/// A plain (non-extending) press in exclusive mode. Pressing the only active
/// cell switches it off unless `require_one` is set. Returns the change flag
/// and the gesture mode to continue in.
fn plain_press(values: &mut Values, i: usize, require_one: bool) -> (bool, Mode) {
    let only = is_on(values, i) && count_on(values) == 1;
    if only {
        (set_cell(values, i, false, require_one), Mode::Move)
    } else {
        (select_only(values, i), Mode::Move)
    }
}

/// An extending press: flip `i`, leave the others, and paint from there.
fn extend_press(values: &mut Values, i: usize, require_one: bool) -> (bool, Mode) {
    let target = !is_on(values, i);
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
    values: &mut Values,
    i: usize,
    exclusive: bool,
    extending: bool,
    require_one: bool,
) -> (bool, Mode) {
    if !exclusive {
        let target = !is_on(values, i);
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
/// A group can also hold [dropdown](ToggleGroup::dropdown) cells, which show
/// the selected option of a list instead of an on / off state. They share the
/// frame and look of the toggles but take no part in dragging or in
/// `exclusive` selection.
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
    /// [`PaintCell`](crate::PaintCell). A pair of those, such as
    /// `(Icon::new(image), "Snap")`, shows both side by side.
    pub fn cell(mut self, label: impl CellContent + 'a, value: &'a mut bool) -> Self {
        let content: Rc<dyn CellContent + 'a> = Rc::new(label);
        self.cells.push(Cell {
            kind: CellKind::Toggle {
                off: content.clone(),
                on: content,
                value,
            },
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
            kind: CellKind::Toggle {
                off: Rc::new(off),
                on: Rc::new(on),
                value,
            },
            tooltip: None,
        });
        self
    }

    /// Append a dropdown cell bound to `value`, with one option per
    /// `(value, content)` pair.
    ///
    /// The closed cell shows the content of the selected option and a small
    /// caret, and is as wide as the widest option, so choosing another never
    /// shifts the layout. If `value` matches no option, only the caret shows.
    /// Pressing the cell (or Space / Enter once it has focus) opens a list
    /// below it, or above when there is no room; the response then reports
    /// [`changed`](Response::changed) when a different option is chosen.
    /// Escape or a click elsewhere closes the list unchanged. Within the list,
    /// the arrow keys move between options and Enter chooses one.
    ///
    /// A dropdown has the look of the other cells and may sit anywhere in the
    /// group, but it is not a toggle: dragging across it skips it,
    /// [`exclusive`](Self::exclusive) and [`require_one`](Self::require_one)
    /// ignore it, and a press on it only opens the list. Options are any
    /// [`CellContent`], so a pair `(Icon, &str)` shows an icon and text.
    /// [`tooltip`](Self::tooltip) applies to it as to any cell.
    ///
    /// ```no_run
    /// # use egui_control_groups::ToggleGroup;
    /// # egui::__run_test_ui(|ui| {
    /// #[derive(PartialEq)]
    /// enum SnapMode { Grid, Vertex }
    /// let (mut snap, mut mode) = (true, SnapMode::Grid);
    /// ui.add(
    ///     ToggleGroup::new("snap")
    ///         .cell("Snap", &mut snap)
    ///         .dropdown(&mut mode, [(SnapMode::Grid, "Grid"), (SnapMode::Vertex, "Vertex")]),
    /// );
    /// # });
    /// ```
    pub fn dropdown<T, C>(
        mut self,
        value: &'a mut T,
        options: impl IntoIterator<Item = (T, C)>,
    ) -> Self
    where
        T: PartialEq + 'a,
        C: CellContent + 'a,
    {
        let (values, contents) = options
            .into_iter()
            .map(|(v, c)| (v, Rc::new(c) as Rc<dyn CellContent + 'a>))
            .unzip();
        self.cells.push(Cell {
            kind: CellKind::Dropdown(Box::new(Bound {
                value,
                values,
                contents,
            })),
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
    ///
    /// If any cell has an icon before its text (a pair such as
    /// `(icon, "Snap")`), the column is laid out like a list: the icons share
    /// one column as wide as the widest and the text is left-aligned after
    /// it, so it lines up whatever the icons' sizes. Otherwise each cell's
    /// content is centred, as in a row.
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

        let mut contents = Vec::with_capacity(n);
        let mut tooltips = Vec::with_capacity(n);
        let mut values: Vec<Option<&mut bool>> = Vec::with_capacity(n);
        for c in cells {
            tooltips.push(c.tooltip);
            match c.kind {
                CellKind::Toggle { off, on, value } => {
                    contents.push(Content::Toggle { off, on });
                    values.push(Some(value));
                }
                CellKind::Dropdown(binding) => {
                    let selected = binding.selected();
                    contents.push(Content::Dropdown { binding, selected });
                    values.push(None);
                }
            }
        }
        let is_toggle: Vec<bool> = values.iter().map(Option::is_some).collect();
        let (caret, caret_gap) = (caret_size(ui), ui.spacing().icon_spacing);

        // A column of cells with icons is laid out like a list: one column
        // for the icons, as wide as the widest, and the text left-aligned
        // after it, so it lines up down the group whatever the icons' sizes.
        // Zero (cells centred, each on its own) for a row, or without icons.
        let icon_column = match orientation {
            Orientation::Horizontal => 0.0,
            Orientation::Vertical => contents
                .iter()
                .flat_map(|c| match c {
                    Content::Toggle { off, on } => {
                        vec![off.leading_width(ui), on.leading_width(ui)]
                    }
                    Content::Dropdown { binding, .. } => binding
                        .options()
                        .iter()
                        .map(|o| o.leading_width(ui))
                        .collect(),
                })
                .flatten()
                .fold(0.0, f32::max),
        };

        // Sizes: each cell fits the larger of its two states, or the widest
        // option of a dropdown.
        let pad = st.padding;
        let content: Vec<Vec2> = contents
            .iter()
            .map(|c| {
                let size = match c {
                    Content::Toggle { off, on } if icon_column > 0.0 => {
                        let (a, b) = (off.size(ui), on.size(ui));
                        let width = columns_width(off.as_ref(), ui, icon_column)
                            .max(columns_width(on.as_ref(), ui, icon_column));
                        vec2(width, a.y.max(b.y))
                    }
                    // `cell()` shares one Rc between both states: measure once.
                    Content::Toggle { off, on } if Rc::ptr_eq(off, on) => off.size(ui),
                    Content::Toggle { off, on } => off.size(ui).max(on.size(ui)),
                    Content::Dropdown { binding, .. } => {
                        let options = binding.options();
                        let (_, width) = option_columns(ui, options, icon_column);
                        let height = options.iter().fold(caret.y, |h, o| h.max(o.size(ui).y));
                        vec2(width + caret_gap + caret.x, height)
                    }
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
        // is painted and pointer gestures are dropped (but for a swipe, which
        // goes on scrolling).
        let visible = ui.is_rect_visible(group_rect);
        let rects = layout_cells(group_rect.min, &mains, cross, orientation);
        let responses: Vec<Response> = rects
            .iter()
            .enumerate()
            .map(|(i, r)| {
                // A dropdown only ever clicks: it has no part in dragging.
                let sense = if is_toggle[i] {
                    Sense::click_and_drag()
                } else {
                    Sense::click()
                };
                ui.interact(*r, id.with(i), sense)
            })
            .collect();

        // Interaction.
        let (pressed, down, pointer, modifiers, touch, now) = ui.input(|i| {
            (
                visible && i.pointer.primary_pressed(),
                i.pointer.primary_down(),
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
        let mut gesture: Option<Gesture> = ui.data(|d| d.get_temp::<Gesture>(id)).filter(|g| {
            // A swipe carries on after it has scrolled the group away.
            (visible || matches!(g.mode, Mode::Scroll)) && g.frame.saturating_add(1) >= frame_nr
        });
        let was_active = gesture.is_some();
        let mut began = false;
        let mut changed = false;

        if gesture.is_none()
            && pressed
            && let Some(i) = responses
                .iter()
                .enumerate()
                .position(|(i, r)| is_toggle[i] && r.hovered())
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
            let raw = index_at(&rects, pos, orientation);
            // Over a dropdown the pointer is "still on" the last toggle.
            let idx = if is_toggle[raw] { raw } else { g.last };
            match g.mode {
                Mode::Undecided { .. } => {}
                Mode::Paint(target) => {
                    // Fill up to the raw index, so cells beyond a dropdown at
                    // the edge are reached too; dropdowns themselves are
                    // skipped by `set_cell`.
                    for k in range_between(g.last, raw) {
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
                Mode::Pending { .. } | Mode::Armed if idx != g.last => {
                    // Moved to another cell: a drag, not a tap.
                    if exclusive {
                        changed |= select_only(&mut values, idx);
                        g.mode = Mode::Move;
                    } else {
                        let target = !is_on(&values, g.last);
                        for k in range_between(g.last, idx) {
                            changed |= set_cell(&mut values, k, target, false);
                        }
                        g.mode = Mode::Paint(target);
                    }
                    g.last = idx;
                }
                Mode::Pending { start, pos: origin } => {
                    let moved = pos - origin;
                    if moved.length() > max_dist {
                        // Too far for a tap or a long press. A finger crosses
                        // this long before it reaches the next cell, so tell
                        // a drag along the group from a swipe across it.
                        let (along, across) = match orientation {
                            Orientation::Horizontal => (moved.x, moved.y),
                            Orientation::Vertical => (moved.y, moved.x),
                        };
                        if across.abs() > along.abs() {
                            g.mode = Mode::Scroll;
                            ui.scroll_with_delta_animation(moved, ScrollAnimation::none());
                        } else {
                            g.mode = Mode::Armed;
                        }
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
                Mode::Armed | Mode::Inert => {}
                Mode::Scroll => {
                    // Cells take the drag from a scroll area they sit in, so
                    // pass the swipe on to it.
                    let delta = ui.input(|i| i.pointer.delta());
                    ui.scroll_with_delta_animation(delta, ScrollAnimation::none());
                }
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
                    let target = !is_on(&values, last);
                    set_cell(&mut values, last, target, false)
                };
            }
            gesture = None;
        }

        // Keyboard activation (Space / Enter on a focused cell). Pointer
        // presses are handled above, so skip clicks that belong to a gesture.
        if !was_active && !began {
            for (i, r) in responses.iter().enumerate() {
                // A dropdown opens its list by itself, see `Popup::menu` below.
                if is_toggle[i] && r.clicked() {
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
        let popup_ids: Vec<Id> = responses.iter().map(Popup::default_response_id).collect();
        let open: Vec<bool> = (0..n)
            .map(|i| !is_toggle[i] && Popup::is_id_open(ui.ctx(), popup_ids[i]))
            .collect();
        for (i, r) in responses.iter().enumerate() {
            let enabled = ui.is_enabled();
            match &contents[i] {
                Content::Toggle { off, on } => {
                    let on_now = is_on(&values, i);
                    let shown = if on_now { on } else { off };
                    r.widget_info(|| {
                        let label = tooltips[i]
                            .clone()
                            .or_else(|| shown.accessible_label())
                            .unwrap_or_default();
                        WidgetInfo::selected(WidgetType::Button, enabled, on_now, label)
                    });
                }
                Content::Dropdown { binding, selected } => {
                    r.widget_info(|| {
                        let label = tooltips[i]
                            .clone()
                            .or_else(|| {
                                selected.and_then(|s| binding.options()[s].accessible_label())
                            })
                            .unwrap_or_default();
                        WidgetInfo::labeled(WidgetType::ComboBox, enabled, label)
                    });
                }
            }
            if r.gained_focus() {
                r.scroll_to_me(None);
            }
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                if let Some(text) = &tooltips[i]
                    && !open[i]
                {
                    r.clone().on_hover_text(text);
                }
            }
        }

        if !visible {
            // Off-screen: a list would be left hanging, so close it.
            for i in (0..n).filter(|&i| open[i]) {
                Popup::close_id(ui.ctx(), popup_ids[i]);
            }
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
                if highlight_active && is_on(&values, i) {
                    st.active_fill
                } else if hovered == Some(i) || open[i] {
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
            ui.set_clip_rect(outer_clip.intersect(rects[i]));
            match &contents[i] {
                Content::Toggle { off, on } => {
                    let on_now = is_on(&values, i);
                    let color = if highlight_active && on_now {
                        st.active_text
                    } else {
                        st.text
                    };
                    let shown = if on_now { on } else { off };
                    if icon_column > 0.0 {
                        let inner = rects[i].shrink2(vec2(pad.x, 0.0));
                        shown.paint_in_columns(ui, inner, color, icon_column);
                    } else {
                        shown.paint(ui, rects[i], color);
                    }
                }
                Content::Dropdown { binding, selected } => {
                    // The caret sits at the right edge; the selected option
                    // is centred in the room left of it.
                    let inner = rects[i].shrink2(pad);
                    let caret_rect = Rect::from_center_size(
                        pos2(inner.right() - caret.x / 2.0, inner.center().y),
                        caret,
                    );
                    if let Some(s) = *selected {
                        let room = Rect::from_min_max(
                            inner.min,
                            pos2(caret_rect.left() - caret_gap, inner.max.y),
                        );
                        // Left-aligned in the list's columns, so the content
                        // stays put whichever option is chosen.
                        let (column, _) = option_columns(ui, binding.options(), icon_column);
                        binding.options()[s].paint_in_columns(ui, room, st.text, column);
                    }
                    paint_caret(ui, caret_rect, st.text);
                }
            }
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

        // Dropdown lists.
        for i in (0..n).filter(|&i| !is_toggle[i]) {
            if let Content::Dropdown { binding, selected } = &mut contents[i]
                && let Some(chosen) = show_list(
                    ui,
                    &responses[i],
                    binding.as_ref(),
                    *selected,
                    &st,
                    icon_column,
                )
                && *selected != Some(chosen)
            {
                binding.select(chosen);
                changed = true;
            }
        }

        if changed {
            response.mark_changed();
        }
        response
    }
}

/// The leading column shared by `options` (as wide as the widest leading
/// part, such as an icon) and the width of the widest option laid out in it.
///
/// `min_column` is the column of the group the options belong to, when its
/// cells are laid out in columns too.
fn option_columns(ui: &Ui, options: &[Rc<dyn CellContent + '_>], min_column: f32) -> (f32, f32) {
    let column = options
        .iter()
        .filter_map(|o| o.leading_width(ui))
        .fold(min_column, f32::max);
    let width = options
        .iter()
        .map(|o| columns_width(o.as_ref(), ui, column))
        .fold(0.0, f32::max);
    (column, width)
}

/// Size of the caret painted in a dropdown cell.
fn caret_size(ui: &Ui) -> Vec2 {
    let h = ui.text_style_height(&TextStyle::Button);
    vec2((h * 0.55).round(), (h * 0.3).round())
}

/// A small downward-pointing triangle filling `rect`.
fn paint_caret(ui: &Ui, rect: Rect, color: Color32) {
    ui.painter().add(Shape::convex_polygon(
        vec![rect.left_top(), rect.right_top(), rect.center_bottom()],
        color,
        Stroke::NONE,
    ));
}

/// Show the list of options of a dropdown whose cell is `r`, if it is open
/// (the cell's own click toggles it). Returns the option that was chosen.
fn show_list(
    ui: &Ui,
    r: &Response,
    binding: &dyn DropdownBinding<'_>,
    selected: Option<usize>,
    st: &Resolved,
    min_column: f32,
) -> Option<usize> {
    let popup_id = Popup::default_response_id(r);
    let was_open = Popup::is_id_open(ui.ctx(), popup_id);
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    let options = binding.options();
    let pad = st.padding;
    let min_row_h = ui.spacing().interact_size.y;
    let (column, widest) = option_columns(ui, options, min_column);
    let mut chosen = None;

    Popup::menu(r)
        .id(popup_id)
        .width(r.rect.width())
        .show(|ui| {
            ui.set_min_width(r.rect.width().max(widest + pad.x * 2.0));
            // Just opened: put the keyboard focus on the current option so the
            // arrow keys move from there.
            let fresh = !was_open;
            for (k, option) in options.iter().enumerate() {
                let size = option.size(ui);
                let row_size = vec2(ui.available_width(), (size.y + pad.y * 2.0).max(min_row_h));
                let (rect, row) = ui.allocate_exact_size(row_size, Sense::click());
                row.widget_info(|| {
                    WidgetInfo::selected(
                        WidgetType::SelectableLabel,
                        true,
                        selected == Some(k),
                        option.accessible_label().unwrap_or_default(),
                    )
                });
                if fresh && selected.unwrap_or(0) == k {
                    row.request_focus();
                }
                if ui.is_rect_visible(rect) {
                    let visuals = ui.style().interact_selectable(&row, selected == Some(k));
                    if selected == Some(k) {
                        ui.painter()
                            .rect_filled(rect, visuals.corner_radius, visuals.bg_fill);
                    } else if row.hovered() || row.has_focus() {
                        ui.painter().rect_filled(
                            rect,
                            visuals.corner_radius,
                            ui.visuals().widgets.hovered.weak_bg_fill,
                        );
                    }
                    if row.has_focus() {
                        ui.painter().rect_stroke(
                            rect,
                            visuals.corner_radius,
                            ui.visuals().selection.stroke,
                            egui::StrokeKind::Inside,
                        );
                    }
                    let color = if selected == Some(k) {
                        ui.visuals().selection.stroke.color
                    } else if row.hovered() {
                        ui.visuals().widgets.hovered.text_color()
                    } else {
                        ui.visuals().widgets.inactive.text_color()
                    };
                    // In the same columns as the closed cell, so icons and
                    // text line up down the list.
                    option.paint_in_columns(ui, rect.shrink2(vec2(pad.x, 0.0)), color, column);
                }
                if row.clicked() {
                    chosen = Some(k);
                }
            }
        });

    if chosen.is_some() {
        Popup::close_id(ui.ctx(), popup_id);
    }
    if was_open && (chosen.is_some() || escape) {
        // Closed by choosing or Escape: give the focus back to the cell.
        r.request_focus();
    }
    chosen
}
