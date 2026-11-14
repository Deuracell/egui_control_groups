//! Joined, Blender-style control groups for [egui](https://docs.rs/egui).
//!
//! Neighbouring controls are drawn as one rounded object: only the outer
//! corners of the group are rounded and cells meet with a thin divider.
//!
//! * [`ToggleGroup`] is a row or column of toggle buttons. Press one and drag
//!   across the others to set them all to the same state.
//! * [`RadioGroup`] is a row or column of options bound to a single value, like
//!   `selectable_value`: exactly one is selected, or with
//!   [`RadioGroup::optional`] at most one.
//! * [`FieldGroup`] is a row or column of text / number fields. Press in one
//!   and drag across others to select several, then type once to edit them
//!   together. Tab commits a field and opens the next (see [`TabBehavior`]
//!   for what happens at the ends). Wrap a value in [`Number`] to give it a
//!   precision, range, prefix or suffix.
//!
//! Both widgets can be given a fixed width with `width` or stretched to the
//! available space with [`ToggleGroup::fill_width`] / [`FieldGroup::fill_width`],
//! and both work from the keyboard (Tab to a cell, Space or Enter to use it).
//!
//! The crate has no theme of its own. Colours, radius and padding come from
//! the `egui::Style` of the `Ui` it is shown in; use [`GroupStyle`] to
//! override individual values where you need to.

mod content;
mod fields;
mod geometry;
mod number;
mod paint;
mod radio;
mod style;
mod toggle;
mod width;

pub use content::{CellContent, Icon, PaintCell};
pub use fields::{FieldGroup, FieldValue, TabBehavior};
pub use geometry::Orientation;
pub use number::{Number, NumberValue};
pub use radio::RadioGroup;
pub use style::GroupStyle;
pub use toggle::ToggleGroup;
