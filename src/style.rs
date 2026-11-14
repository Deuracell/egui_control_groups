//! Styling. Every colour and metric defaults to whatever the surrounding
//! `egui::Style` says, so the widgets follow your app's look. Override
//! individual values on a [`GroupStyle`] only where you want to deviate.

use egui::{Color32, Stroke, Ui, Vec2};

/// Optional per-widget style overrides. `GroupStyle::default()` changes
/// nothing: everything is read from `ui.visuals()` / `ui.spacing()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupStyle {
    /// Outer corner radius in points.
    pub corner_radius: Option<f32>,
    /// Padding inside each cell.
    pub padding: Option<Vec2>,
    /// Resting cell fill.
    pub fill: Option<Color32>,
    /// Fill of the cell under the pointer.
    pub hover_fill: Option<Color32>,
    /// Fill of an on / selected cell.
    pub active_fill: Option<Color32>,
    /// Text colour of a resting cell.
    pub text: Option<Color32>,
    /// Text colour of an on / selected cell.
    pub active_text: Option<Color32>,
    /// Stroke around the whole group.
    pub outline: Option<Stroke>,
    /// Stroke between neighbouring cells.
    pub divider: Option<Stroke>,
}

macro_rules! setter {
    ($($(#[$m:meta])* $name:ident: $ty:ty),* $(,)?) => {
        $(
            $(#[$m])*
            #[must_use]
            pub fn $name(mut self, value: $ty) -> Self {
                self.$name = Some(value);
                self
            }
        )*
    };
}

impl GroupStyle {
    setter! {
        /// Set the outer corner radius.
        corner_radius: f32,
        /// Set the padding inside each cell.
        padding: Vec2,
        /// Set the resting fill.
        fill: Color32,
        /// Set the hover fill.
        hover_fill: Color32,
        /// Set the on / selected fill.
        active_fill: Color32,
        /// Set the resting text colour.
        text: Color32,
        /// Set the on / selected text colour.
        active_text: Color32,
        /// Set the outline stroke.
        outline: Stroke,
        /// Set the divider stroke.
        divider: Stroke,
    }

    pub(crate) fn resolve(&self, ui: &Ui) -> Resolved {
        let v = ui.visuals();
        let w = &v.widgets;
        let radius = self
            .corner_radius
            .map(|r| r.round().clamp(0.0, 255.0) as u8)
            .unwrap_or(w.inactive.corner_radius.nw);
        let border = w.noninteractive.bg_stroke.color;
        Resolved {
            radius,
            padding: self.padding.unwrap_or(ui.spacing().button_padding),
            fill: self.fill.unwrap_or(w.inactive.weak_bg_fill),
            hover_fill: self.hover_fill.unwrap_or(w.hovered.weak_bg_fill),
            active_fill: self.active_fill.unwrap_or(v.selection.bg_fill),
            text: self.text.unwrap_or(w.inactive.text_color()),
            active_text: self.active_text.unwrap_or(w.active.text_color()),
            muted_text: v.weak_text_color(),
            input_fill: v.extreme_bg_color,
            outline: self.outline.unwrap_or(Stroke::new(1.0, border)),
            divider: self.divider.unwrap_or(Stroke::new(1.0, border)),
        }
    }
}

/// A [`GroupStyle`] with every value filled in.
pub(crate) struct Resolved {
    pub radius: u8,
    pub padding: Vec2,
    pub fill: Color32,
    pub hover_fill: Color32,
    pub active_fill: Color32,
    pub text: Color32,
    pub active_text: Color32,
    pub muted_text: Color32,
    pub input_fill: Color32,
    pub outline: Stroke,
    pub divider: Stroke,
}

impl Resolved {
    /// Tint for cells that are selected but not the active one.
    pub fn selected_fill(&self) -> Color32 {
        self.active_fill.gamma_multiply(0.45)
    }
}
