//! The demo UI, shared by the native example and the web demo.

use egui::{Color32, Stroke, Ui, Vec2, vec2};
use egui_control_groups::{FieldGroup, GroupStyle, PaintCell, ToggleGroup};

pub struct Demo {
    style: StyleEdit,
    visibility: [bool; 4],
    axes: [bool; 3],
    modes: [bool; 3],
    icons: [bool; 3],
    grid: bool,
    position: [f32; 3],
    name: [String; 2],
    panel_width: f32,
}

impl Default for Demo {
    fn default() -> Self {
        Self {
            style: StyleEdit::default(),
            visibility: [true, false, true, false],
            axes: [true, false, false],
            modes: [true, false, false],
            icons: [true, false, true],
            grid: false,
            position: [0.0, 1.5, -2.0],
            name: [String::from("Part"), String::from("A")],
            panel_width: 360.0,
        }
    }
}

impl Demo {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.style.ui(ui);
        let style = self.style.to_style(ui);
        ui.add_space(8.0);

        ui.heading("Toggles");
        ui.label("Press and drag across the buttons.");
        ui.add(
            ToggleGroup::new("visibility")
                .style(style.clone())
                .cells(["Solid", "Wire", "Axes", "Grid"], &mut self.visibility)
                .equal_width(),
        );
        ui.add_space(8.0);
        ui.add(
            ToggleGroup::new("axes")
                .style(style.clone())
                .cells(["X", "Y", "Z"], &mut self.axes)
                .equal_width(),
        );

        ui.add_space(16.0);
        ui.heading("Exclusive");
        ui.label("A press selects one. Hold Shift (long press on touch) to add more.");
        ui.add(
            ToggleGroup::new("modes")
                .style(style.clone())
                .cells(["Vertex", "Edge", "Face"], &mut self.modes)
                .equal_width()
                .exclusive(true),
        );

        ui.add_space(16.0);
        ui.heading("Plain icon buttons");
        ui.label("No active fill: only the icon changes with the state.");
        let [lock, show, snap] = &mut self.icons;
        ui.add(
            ToggleGroup::new("icons")
                .style(style.clone())
                .state_cell("🔓", "🔒", lock)
                .tooltip("Lock")
                .state_cell("🙈", "👁", show)
                .tooltip("Visible")
                .state_cell(square(false), square(true), snap)
                .tooltip("Snap")
                // Your own icons: any painter code works, and needs no font.
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
                    .style(style.clone())
                    .labeled("X", x)
                    .labeled("Y", y)
                    .labeled("Z", z)
                    .vertical(),
            );
            // The same values again, laid out as a row.
            let [x, y, z] = &mut self.position;
            ui.add(
                FieldGroup::new("row")
                    .style(style.clone())
                    .labeled("X", x)
                    .labeled("Y", y)
                    .labeled("Z", z),
            );
        });

        ui.add_space(8.0);
        let [a, b] = &mut self.name;
        ui.add(
            FieldGroup::new("name")
                .style(style.clone())
                .field(a)
                .field(b)
                .cell_width(110.0),
        );
        ui.label(format!("position = {:?}", self.position));

        ui.add_space(16.0);
        ui.heading("Width");
        ui.label("`fill_width` follows the space available; `width` is fixed.");
        ui.add(egui::Slider::new(&mut self.panel_width, 160.0..=600.0).text("Panel width"));
        ui.scope(|ui| {
            ui.set_max_width(self.panel_width);
            ui.add(
                ToggleGroup::new("fill_toggles")
                    .style(style.clone())
                    .cells(["X", "Y", "Z"], &mut self.axes)
                    .fill_width(),
            );
            ui.add_space(8.0);
            let [x, y, z] = &mut self.position;
            ui.add(
                FieldGroup::new("fill_fields")
                    .style(style.clone())
                    .labeled("X", x)
                    .labeled("Y", y)
                    .labeled("Z", z)
                    .fill_width(),
            );
        });
        ui.add_space(8.0);
        let [a, b] = &mut self.name;
        ui.add(
            FieldGroup::new("fixed_name")
                .style(style.clone())
                .field(a)
                .field(b)
                .width(200.0),
        );
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

/// Editable `GroupStyle` values. A colour left at `None` follows the
/// surrounding `egui::Style`, which is the crate's default.
struct StyleEdit {
    radius: f32,
    padding: Vec2,
    outline_width: f32,
    divider_width: f32,
    fill: Option<Color32>,
    hover_fill: Option<Color32>,
    active_fill: Option<Color32>,
    text: Option<Color32>,
    active_text: Option<Color32>,
    outline: Option<Color32>,
    divider: Option<Color32>,
}

impl Default for StyleEdit {
    fn default() -> Self {
        Self {
            radius: 4.0,
            padding: vec2(6.0, 4.0),
            outline_width: 1.0,
            divider_width: 1.0,
            fill: None,
            hover_fill: None,
            active_fill: None,
            text: None,
            active_text: None,
            outline: None,
            divider: None,
        }
    }
}

impl StyleEdit {
    fn to_style(&self, ui: &Ui) -> GroupStyle {
        let outline = self
            .outline
            .unwrap_or(ui.visuals().widgets.noninteractive.bg_stroke.color);
        // Dividers follow the outline unless given a colour of their own.
        let divider = self.divider.unwrap_or(outline);
        let mut s = GroupStyle::default()
            .corner_radius(self.radius)
            .padding(self.padding)
            .outline(Stroke::new(self.outline_width, outline))
            .divider(Stroke::new(self.divider_width, divider));
        if let Some(c) = self.fill {
            s = s.fill(c);
        }
        if let Some(c) = self.hover_fill {
            s = s.hover_fill(c);
        }
        if let Some(c) = self.active_fill {
            s = s.active_fill(c);
        }
        if let Some(c) = self.text {
            s = s.text(c);
        }
        if let Some(c) = self.active_text {
            s = s.active_text(c);
        }
        s
    }

    fn ui(&mut self, ui: &mut Ui) {
        egui::CollapsingHeader::new("Style")
            .default_open(true)
            .show(ui, |ui| {
                let v = ui.visuals().clone();
                egui::Grid::new("style").num_columns(2).show(ui, |ui| {
                    ui.label("Corner radius");
                    ui.add(egui::Slider::new(&mut self.radius, 0.0..=24.0));
                    ui.end_row();
                    ui.label("Padding");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.padding.x).range(0.0..=30.0));
                        ui.add(egui::DragValue::new(&mut self.padding.y).range(0.0..=30.0));
                    });
                    ui.end_row();
                    ui.label("Outline width");
                    ui.add(egui::Slider::new(&mut self.outline_width, 0.0..=4.0));
                    ui.end_row();
                    ui.label("Divider width");
                    ui.add(egui::Slider::new(&mut self.divider_width, 0.0..=4.0));
                    ui.end_row();
                    let style = "from egui style";
                    let w = &v.widgets;
                    color_row(ui, "Fill", &mut self.fill, w.inactive.weak_bg_fill, style);
                    color_row(
                        ui,
                        "Hover fill",
                        &mut self.hover_fill,
                        w.hovered.weak_bg_fill,
                        style,
                    );
                    color_row(
                        ui,
                        "Active fill",
                        &mut self.active_fill,
                        v.selection.bg_fill,
                        style,
                    );
                    color_row(ui, "Text", &mut self.text, w.inactive.text_color(), style);
                    color_row(
                        ui,
                        "Active text",
                        &mut self.active_text,
                        w.active.text_color(),
                        style,
                    );
                    let border = w.noninteractive.bg_stroke.color;
                    color_row(ui, "Outline", &mut self.outline, border, style);
                    let outline = self.outline.unwrap_or(border);
                    color_row(ui, "Divider", &mut self.divider, outline, "same as outline");
                });
                if ui.button("Reset").clicked() {
                    *self = Self::default();
                }
            });
    }
}

/// A checkbox that switches a colour override on, and its picker.
fn color_row(ui: &mut Ui, label: &str, value: &mut Option<Color32>, default: Color32, unset: &str) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut custom = value.is_some();
        if ui.checkbox(&mut custom, "").changed() {
            *value = custom.then_some(default);
        }
        if let Some(c) = value {
            ui.color_edit_button_srgba(c);
        } else {
            ui.weak(unset);
        }
    });
    ui.end_row();
}

/// A custom-painted icon: a square outline, filled in when `filled`.
fn square(filled: bool) -> PaintCell<impl Fn(&egui::Ui, egui::Rect, egui::Color32)> {
    PaintCell::new(vec2(14.0, 14.0), move |ui, rect, color| {
        let r = rect.shrink(1.0);
        if filled {
            ui.painter().rect_filled(r, 2.0, color);
        } else {
            ui.painter().rect_stroke(
                r,
                2.0,
                egui::Stroke::new(1.5, color),
                egui::StrokeKind::Inside,
            );
        }
    })
}
