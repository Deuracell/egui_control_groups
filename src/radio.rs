//! A row (or column) of options bound to one value, drawn as one rounded
//! object. A thin wrapper over an exclusive [`ToggleGroup`].

use std::hash::Hash;

use egui::{Id, Response, Ui, Widget};

use crate::content::CellContent;
use crate::geometry::Orientation;
use crate::style::GroupStyle;
use crate::toggle::ToggleGroup;
use crate::width::Width;

struct Choice<'a, T> {
    value: T,
    content: Box<dyn CellContent + 'a>,
    tooltip: Option<String>,
}

/// What the group writes to.
enum Binding<'a, T> {
    /// Always one selected.
    Value(&'a mut T),
    /// The selected one can be cleared.
    Optional(&'a mut Option<T>),
}

impl<T: PartialEq> Binding<'_, T> {
    fn is(&self, value: &T) -> bool {
        match self {
            Self::Value(v) => **v == *value,
            Self::Optional(v) => v.as_ref() == Some(value),
        }
    }

    fn is_none(&self) -> bool {
        matches!(self, Self::Optional(None))
    }

    fn set(&mut self, value: T) {
        match self {
            Self::Value(v) => **v = value,
            Self::Optional(v) => **v = Some(value),
        }
    }

    /// Clear an optional binding; does nothing for a plain value.
    fn clear(&mut self) {
        if let Self::Optional(v) = self {
            **v = None;
        }
    }
}

/// Joined options bound to a single value, like
/// [`selectable_value`](egui::Ui::selectable_value) drawn as one object.
///
/// ```no_run
/// # use egui_control_groups::RadioGroup;
/// # egui::__run_test_ui(|ui| {
/// #[derive(PartialEq)]
/// enum Mode { Vertex, Edge, Face }
/// let mut mode = Mode::Vertex;
/// let response = ui.add(
///     RadioGroup::new("mode", &mut mode)
///         .option(Mode::Vertex, "Vertex")
///         .option(Mode::Edge, "Edge")
///         .option(Mode::Face, "Face"),
/// );
/// if response.changed() { /* `mode` was set to another option */ }
/// # });
/// ```
///
/// Exactly one option is selected. Press one to select it, or press and drag
/// across the others to move the selection. Unlike an exclusive
/// [`ToggleGroup`], nothing extends it to several options, and pressing the
/// selected option does nothing. Use [`optional`](Self::optional) for a value
/// that can also be "nothing": there, pressing the selected option clears it.
///
/// If the bound value matches none of the options, none is shown as selected
/// and the first press selects normally.
///
/// Options are anything implementing [`CellContent`], as for a toggle cell.
/// The group owns the option values and moves the chosen one into the bound
/// value, so `T` needs only `PartialEq`, not `Clone`.
#[must_use = "Add with `ui.add(...)`."]
pub struct RadioGroup<'a, T> {
    id_salt: Id,
    binding: Binding<'a, T>,
    options: Vec<Choice<'a, T>>,
    orientation: Orientation,
    equal_size: bool,
    width: Width,
    highlight_active: bool,
    settle_press: bool,
    style: GroupStyle,
}

impl<'a, T: PartialEq> RadioGroup<'a, T> {
    /// A group bound to `value`: always one option selected, and pressing the
    /// selected option does nothing.
    ///
    /// `id_salt` must be unique among groups in the same `Ui`. This includes
    /// [`ToggleGroup`]s, which share the same id space.
    pub fn new(id_salt: impl Hash + std::fmt::Debug, value: &'a mut T) -> Self {
        Self::with_binding(id_salt, Binding::Value(value))
    }

    /// A group bound to an `Option`: pressing the selected option clears it to
    /// `None`, and the value can start as `None`.
    ///
    /// `id_salt` must be unique among groups in the same `Ui`.
    pub fn optional(id_salt: impl Hash + std::fmt::Debug, value: &'a mut Option<T>) -> Self {
        Self::with_binding(id_salt, Binding::Optional(value))
    }

    fn with_binding(id_salt: impl Hash + std::fmt::Debug, binding: Binding<'a, T>) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            binding,
            options: Vec::new(),
            orientation: Orientation::Horizontal,
            equal_size: false,
            width: Width::Auto,
            highlight_active: true,
            settle_press: cfg!(target_arch = "wasm32"),
            style: GroupStyle::default(),
        }
    }

    /// Append an option: selecting it sets the bound value to `value`.
    ///
    /// `content` is anything implementing [`CellContent`]: a `&str`, `String`,
    /// `RichText`, an `egui::Image`, an [`Icon`](crate::Icon) or a
    /// [`PaintCell`](crate::PaintCell).
    pub fn option(mut self, value: T, content: impl CellContent + 'a) -> Self {
        self.options.push(Choice {
            value,
            content: Box::new(content),
            tooltip: None,
        });
        self
    }

    /// Append one option per `(value, content)` pair.
    pub fn options<L: CellContent + 'a>(
        mut self,
        options: impl IntoIterator<Item = (T, L)>,
    ) -> Self {
        for (value, content) in options {
            self = self.option(value, content);
        }
        self
    }

    /// Hover text for the most recently added option. It is also the option's
    /// accessible name, which icon-only options need.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        if let Some(option) = self.options.last_mut() {
            option.tooltip = Some(text.into());
        }
        self
    }

    /// Whether the selected option gets the active fill and text colour.
    /// Default `true`.
    pub fn highlight_active(mut self, highlight: bool) -> Self {
        self.highlight_active = highlight;
        self
    }

    /// Wait one frame after a press before acting on it, to tell a finger
    /// from a mouse. See [`ToggleGroup`]; on by default on wasm only.
    #[doc(hidden)]
    pub fn settle_press(mut self, settle: bool) -> Self {
        self.settle_press = settle;
        self
    }

    /// Stack the options top to bottom instead of left to right.
    ///
    /// Options with an icon before their text, such as `(icon, "Flat")`, then
    /// share one icon column with the text left-aligned after it, as in
    /// [`ToggleGroup::vertical`].
    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Give every option the width of the widest one (horizontal groups).
    ///
    /// This sets the natural size only: an explicit [`width`](Self::width) or
    /// [`fill_width`](Self::fill_width) wins, and then scales these equal
    /// options, which therefore stay equal.
    pub fn equal_width(mut self) -> Self {
        self.equal_size = true;
        self
    }

    /// Make the whole group exactly `width` points wide.
    ///
    /// Horizontal groups share the width between their options in proportion
    /// to their natural widths, down to a small minimum per option. Vertical
    /// groups get this as their cross-axis width. Takes precedence over
    /// `equal_width`. A non-finite or negative value is ignored. Default:
    /// natural size.
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

impl<T: PartialEq> Widget for RadioGroup<'_, T> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self {
            id_salt,
            mut binding,
            options,
            orientation,
            equal_size,
            width,
            highlight_active,
            settle_press,
            style,
        } = self;

        let before: Vec<bool> = options.iter().map(|o| binding.is(&o.value)).collect();
        let mut bools = before.clone();
        let mut values = Vec::with_capacity(options.len());
        let mut group = ToggleGroup::new(id_salt)
            .exclusive(true)
            .allow_extend(false)
            .require_one(matches!(binding, Binding::Value(_)))
            .highlight_active(highlight_active)
            .settle_press(settle_press)
            .style(style);
        group = match orientation {
            Orientation::Horizontal => group,
            Orientation::Vertical => group.vertical(),
        };
        if equal_size {
            group = group.equal_width();
        }
        group = match width {
            Width::Auto => group,
            Width::Fixed(w) => group.width(w),
            Width::Fill => group.fill_width(),
        };
        for (option, on) in options.into_iter().zip(bools.iter_mut()) {
            group = group.cell(option.content, on);
            if let Some(text) = option.tooltip {
                group = group.tooltip(text);
            }
            values.push(option.value);
        }
        let response = ui.add(group);

        if bools != before {
            match bools.iter().position(|&on| on) {
                Some(i) => {
                    if !binding.is(&values[i]) {
                        binding.set(values.swap_remove(i));
                    }
                }
                None => {
                    if !binding.is_none() {
                        binding.clear();
                    }
                }
            }
        }
        response
    }
}
