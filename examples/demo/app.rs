//! The demo UI, shared by the native example and the web demo.

use egui::vec2;
use egui_control_groups::{FieldGroup, GroupStyle, PaintCell, ToggleGroup};

pub struct Demo {
    visibility: [bool; 4],
    axes: [bool; 3],
    icons: [bool; 3],
    grid: bool,
    position: [f32; 3],
    name: [String; 2],
}

impl Default for Demo {
    fn default() -> Self {
        Self {
            visibility: [true, false, true, false],
            axes: [true, false, false],
            icons: [true, false, true],
            grid: false,
            position: [0.0, 1.5, -2.0],
            name: [String::from("Part"), String::from("A")],
        }
    }
}

impl Demo {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Toggles");
        ui.label("Press and drag across the buttons.");
        ui.add(
            ToggleGroup::new("visibility")
                .cells(["Solid", "Wire", "Axes", "Grid"], &mut self.visibility)
                .equal_width(),
        );
        ui.add_space(8.0);
        ui.add(
            ToggleGroup::new("axes")
                .cells(["X", "Y", "Z"], &mut self.axes)
                .equal_width(),
        );

        ui.add_space(16.0);
        ui.heading("Plain icon buttons");
        ui.label("No active fill: only the icon changes with the state.");
        let [lock, show, snap] = &mut self.icons;
        ui.add(
            ToggleGroup::new("icons")
                .state_cell("🔓", "🔒", lock)
                .tooltip("Lock")
                .state_cell("🙈", "👁", show)
                .tooltip("Visible")
                .state_cell("⊘", "🧲", snap)
                .tooltip("Snap")
                // Your own icons: any painter code works.
                .state_cell(ring(false), ring(true), &mut self.grid)
                .tooltip("Grid")
                .highlight_active(false),
        );

        ui.add_space(16.0);
        ui.heading("Fields");
        ui.label("Click to edit. Drag across several to edit them together.");
        ui.horizontal(|ui| {
            let [x, y, z] = &mut self.position;
            ui.add(
                FieldGroup::new("position")
                    .labeled("X", x)
                    .labeled("Y", y)
                    .labeled("Z", z)
                    .vertical(),
            );
            // The same values again, laid out as a row.
            let [x, y, z] = &mut self.position;
            ui.add(
                FieldGroup::new("row")
                    .labeled("X", x)
                    .labeled("Y", y)
                    .labeled("Z", z),
            );
        });

        ui.add_space(16.0);
        ui.heading("Custom style");
        let [a, b] = &mut self.name;
        ui.add(
            FieldGroup::new("name")
                .field(a)
                .field(b)
                .cell_width(110.0)
                .style(GroupStyle::default().corner_radius(10.0)),
        );
        ui.label(format!("position = {:?}", self.position));
    }
}

/// A custom-painted icon: a ring, filled in when `filled`.
fn ring(filled: bool) -> PaintCell<impl Fn(&egui::Ui, egui::Rect, egui::Color32)> {
    PaintCell::new(vec2(14.0, 14.0), move |ui, rect, color| {
        let (c, r) = (rect.center(), rect.width() / 2.0 - 1.0);
        if filled {
            ui.painter().circle_filled(c, r, color);
        } else {
            ui.painter()
                .circle_stroke(c, r, egui::Stroke::new(1.5, color));
        }
    })
}
