//! A row (or column) of toggle buttons drawn as one rounded object, where
//! pressing one cell and dragging across the others paints them all to the
//! same state, like Blender's toggle rows.

use std::hash::Hash;
use std::rc::Rc;

use egui::{Color32, Id, Response, Sense, Shape, Ui, Vec2, Widget, WidgetInfo, WidgetType, vec2};

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

/// Drag-paint gesture in progress, kept in egui memory between frames.
#[derive(Clone, Copy)]
struct DragPaint {
    /// The state every cell touched by this gesture is set to.
    target: bool,
    /// Last cell index visited, so skipped cells can be filled in.
    last: usize,
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
#[must_use = "Add with `ui.add(...)`."]
pub struct ToggleGroup<'a> {
    id_salt: Id,
    cells: Vec<Cell<'a>>,
    orientation: Orientation,
    equal_size: bool,
    highlight_active: bool,
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
        let (pressed, down, pointer) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()),
            )
        });
        let mut paint: Option<DragPaint> = ui.data(|d| d.get_temp(id));
        let was_painting = paint.is_some();
        let mut began = false;
        let mut changed = false;
        let mut set = |values: &mut [&mut bool], i: usize, target: bool| {
            if *values[i] != target {
                *values[i] = target;
                changed = true;
            }
        };

        if paint.is_none() {
            if pressed && let Some(i) = responses.iter().position(Response::hovered) {
                let target = !*values[i];
                set(&mut values, i, target);
                paint = Some(DragPaint { target, last: i });
                began = true;
            }
        } else if down && let (Some(p), Some(pos)) = (paint.as_mut(), pointer) {
            let idx = index_at(&rects, pos, orientation);
            for k in range_between(p.last, idx) {
                set(&mut values, k, p.target);
            }
            p.last = idx;
        }
        if !down {
            paint = None;
        }

        // Keyboard activation (Space / Enter on a focused cell). Pointer
        // presses are handled above, so skip clicks that belong to a gesture.
        if !was_painting && !began {
            for (i, r) in responses.iter().enumerate() {
                if r.clicked() {
                    let target = !*values[i];
                    set(&mut values, i, target);
                }
            }
        }

        ui.data_mut(|d| match paint {
            Some(p) => {
                d.insert_temp(id, p);
            }
            None => {
                d.remove::<DragPaint>(id);
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
        let hovered = paint
            .map(|p| p.last)
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
