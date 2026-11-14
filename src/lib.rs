//! Joined, Blender-style control groups for [egui](https://docs.rs/egui).
//!
//! Neighbouring controls are drawn as one rounded object: only the outer
//! corners of the group are rounded and cells meet with a thin divider.
//!
//! * [`ToggleGroup`] is a row or column of toggle buttons. Press one and drag
//!   across the others to set them all to the same state.
//! * [`FieldGroup`] is a row or column of text / number fields. Press in one
//!   and drag across others to select several, then type once to edit them
//!   together.
//!
//! The crate has no theme of its own. Colours, radius and padding come from
//! the `egui::Style` of the `Ui` it is shown in; use [`GroupStyle`] to
//! override individual values where you need to.

mod content;
mod fields;
mod geometry;
mod paint;
mod style;
mod toggle;

pub use content::{CellContent, Icon, PaintCell};
pub use fields::{FieldGroup, FieldValue};
pub use geometry::Orientation;
pub use style::GroupStyle;
pub use toggle::ToggleGroup;
