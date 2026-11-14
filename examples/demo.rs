//! `cargo run --example demo`
//!
//! Joined toggle rows and multi-edit fields. Try:
//!  * pressing a toggle and dragging across its neighbours,
//!  * pressing in one field and dragging across others, then typing a value.

use egui::vec2;
use egui_control_groups::{FieldGroup, GroupStyle, PaintCell, ToggleGroup};

fn main() -> eframe::Result<()> {
    let mut visibility = [true, false, true, false];
    let mut axes = [true, false, false];
    let mut icons = [true, false, true];
    let mut grid = false;
    let mut position = [0.0_f32, 1.5, -2.0];
    let mut name = [String::from("Part"), String::from("A")];

    eframe::run_ui_native(
        "egui-control-groups",
        eframe::NativeOptions::default(),
        move |ui, _frame| {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading("Toggles");
                ui.label("Press and drag across the buttons.");
                ui.add(
                    ToggleGroup::new("visibility")
                        .cells(["Solid", "Wire", "Axes", "Grid"], &mut visibility)
                        .equal_width(),
                );
                ui.add_space(8.0);
                ui.add(
                    ToggleGroup::new("axes")
                        .cells(["X", "Y", "Z"], &mut axes)
                        .equal_width(),
                );

                ui.add_space(16.0);
                ui.heading("Plain icon buttons");
                ui.label("No active fill: only the icon changes with the state.");
                let [lock, show, snap] = &mut icons;
                ui.add(
                    ToggleGroup::new("icons")
                        .state_cell("🔓", "🔒", lock)
                        .tooltip("Lock")
                        .state_cell("🙈", "👁", show)
                        .tooltip("Visible")
                        .state_cell("⊘", "🧲", snap)
                        .tooltip("Snap")
                        // Your own icons: any painter code works.
                        .state_cell(ring(false), ring(true), &mut grid)
                        .tooltip("Grid")
                        .highlight_active(false),
                );

                ui.add_space(16.0);
                ui.heading("Fields");
                ui.label("Click to edit. Drag across several to edit them together.");
                ui.horizontal(|ui| {
                    let [x, y, z] = &mut position;
                    ui.add(
                        FieldGroup::new("position")
                            .labeled("X", x)
                            .labeled("Y", y)
                            .labeled("Z", z)
                            .vertical(),
                    );
                    // The same values again, laid out as a row.
                    let [x, y, z] = &mut position;
                    ui.add(
                        FieldGroup::new("row")
                            .labeled("X", x)
                            .labeled("Y", y)
                            .labeled("Z", z),
                    );
                });

                ui.add_space(16.0);
                ui.heading("Custom style");
                let [a, b] = &mut name;
                ui.add(
                    FieldGroup::new("name")
                        .field(a)
                        .field(b)
                        .cell_width(110.0)
                        .style(GroupStyle::default().corner_radius(10.0)),
                );
                ui.label(format!("position = {position:?}"));
            });
        },
    )
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
