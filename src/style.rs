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
    /// Stroke between neighbouring cells. Unset, it is 1 point wide in the
    /// outline's colour, so a divider's colour only differs when you ask.
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
        /// Set the divider stroke, to give dividers their own colour or width.
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
        let outline = self.outline.unwrap_or(Stroke::new(1.0, border));
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
            outline,
            divider: self.divider.unwrap_or(Stroke::new(1.0, outline.color)),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(style: GroupStyle) -> Resolved {
        let mut out = None;
        let output = egui::Context::default().run_ui(egui::RawInput::default(), |ui| {
            out = Some(style.resolve(ui));
        });
        output.drop_without_applying_deltas();
        out.unwrap()
    }

    #[test]
    fn divider_follows_the_outline_colour_by_default() {
        let red = Stroke::new(2.0, Color32::RED);
        let r = resolve(GroupStyle::default().outline(red));
        assert_eq!(r.divider, Stroke::new(1.0, Color32::RED));
    }

    #[test]
    fn divider_can_have_its_own_colour() {
        let r = resolve(
            GroupStyle::default()
                .outline(Stroke::new(1.0, Color32::RED))
                .divider(Stroke::new(1.0, Color32::BLUE)),
        );
        assert_eq!(r.outline.color, Color32::RED);
        assert_eq!(r.divider.color, Color32::BLUE);
    }
}
