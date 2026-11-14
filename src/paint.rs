use std::sync::Arc;

use egui::epaint::StrokeKind;
use egui::{
    Color32, CornerRadius, Galley, Pos2, Rect, Shape, TextStyle, TextWrapMode, Ui, WidgetText,
};

use crate::geometry::{Orientation, corner_radius_for};
use crate::style::Resolved;

/// Lay out `text` on one line using the button font.
pub(crate) fn make_galley(ui: &Ui, text: WidgetText) -> Arc<Galley> {
    text.into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Button,
    )
}

/// Shapes for the group body: per-cell fills (outer corners rounded only),
/// dividers between cells, and a rounded outline around everything.
pub(crate) fn frame_shapes(
    group: Rect,
    cells: &[Rect],
    orientation: Orientation,
    style: &Resolved,
    fills: &[Color32],
) -> Vec<Shape> {
    let n = cells.len();
    let mut shapes = Vec::with_capacity(n * 2 + 1);
    for (i, (cell, fill)) in cells.iter().zip(fills).enumerate() {
        let cr = corner_radius_for(i, n, orientation, style.radius);
        shapes.push(Shape::rect_filled(*cell, cr, *fill));
    }
    for pair in cells.windows(2) {
        let a = pair[0];
        let points = match orientation {
            Orientation::Horizontal => [a.right_top(), a.right_bottom()],
            Orientation::Vertical => [a.left_bottom(), a.right_bottom()],
        };
        shapes.push(Shape::line_segment(points, style.divider));
    }
    shapes.push(Shape::rect_stroke(
        group,
        CornerRadius::same(style.radius),
        style.outline,
        StrokeKind::Inside,
    ));
    shapes
}

/// A galley centred in `rect`.
pub(crate) fn centered_galley(rect: Rect, galley: &Arc<Galley>, color: Color32) -> Shape {
    let pos: Pos2 = rect.center() - galley.size() / 2.0;
    Shape::galley(pos, galley.clone(), color)
}
