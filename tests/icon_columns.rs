//! A vertical group whose cells have icons lays them out like a list: one
//! icon column, text left-aligned after it. Rows, and columns without icons,
//! keep their content centred.

use egui::{Context, Pos2, RawInput, Rect, Shape, Ui, vec2};
use egui_control_groups::{PaintCell, RadioGroup, ToggleGroup};

fn texts(shape: &Shape, out: &mut Vec<(String, f32)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        Shape::Text(t) => out.push((t.galley.text().to_owned(), t.pos.x)),
        _ => {}
    }
}

thread_local! {
    /// Centre x of every icon painted in the current frame.
    static CENTRES: std::cell::RefCell<Vec<f32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// An icon `width` points wide that records where it is painted.
fn icon(width: f32) -> PaintCell<impl Fn(&Ui, Rect, egui::Color32)> {
    PaintCell::new(vec2(width, 10.0), |_: &Ui, r: Rect, _| {
        CENTRES.with(|c| c.borrow_mut().push(r.center().x));
    })
}

/// Show `add` and return the left edge of each text painted, the centre of
/// each icon painted, and the group's rect.
fn show(add: impl Fn(&mut Ui) -> Rect) -> (Vec<(String, f32)>, Vec<f32>, Rect) {
    let ctx = Context::default();
    let (mut found, mut rect) = (Vec::new(), Rect::NOTHING);
    for _ in 0..2 {
        CENTRES.with(|c| c.borrow_mut().clear());
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            ..Default::default()
        };
        let out = ctx.run_ui(input, |ui| rect = add(ui));
        found.clear();
        for clipped in &out.shapes {
            texts(&clipped.shape, &mut found);
        }
        out.drop_without_applying_deltas();
    }
    (found, CENTRES.with(|c| c.borrow().clone()), rect)
}

fn x(found: &[(String, f32)], text: &str) -> f32 {
    found.iter().find(|(t, _)| t == text).unwrap().1
}

#[test]
fn vertical_toggles_with_icons_share_a_column() {
    let (found, centres, rect) = show(|ui| {
        let (mut a, mut b, mut c) = (false, true, false);
        ui.add(
            ToggleGroup::new("t")
                .cell((icon(8.0), "Snap"), &mut a)
                .cell((icon(20.0), "Align to grid"), &mut b)
                .cell("Plain", &mut c)
                .vertical(),
        )
        .rect
    });
    // Every text starts at the same place, including the cell with no icon.
    let start = x(&found, "Snap");
    assert_eq!(x(&found, "Align to grid"), start);
    assert_eq!(x(&found, "Plain"), start);
    // Icons of different widths are centred in one column, left of the text.
    assert_eq!(centres.len(), 2);
    assert_eq!(centres[0], centres[1]);
    assert!(rect.left() < centres[0] && centres[0] < start);
}

#[test]
fn vertical_radio_group_with_icons_shares_a_column() {
    let (found, centres, _) = show(|ui| {
        let mut v = 1u8;
        ui.add(
            RadioGroup::new("r", &mut v)
                .option(0, (icon(8.0), "Flat"))
                .option(1, (icon(20.0), "Smooth"))
                .vertical(),
        )
        .rect
    });
    assert_eq!(x(&found, "Flat"), x(&found, "Smooth"));
    assert_eq!(centres[0], centres[1]);
}

#[test]
fn vertical_group_without_icons_stays_centred() {
    let (found, _, rect) = show(|ui| {
        let (mut a, mut b) = (false, true);
        ui.add(
            ToggleGroup::new("t")
                .cell("X", &mut a)
                .cell("A much longer label", &mut b)
                .vertical(),
        )
        .rect
    });
    // Centred text: the short label starts well to the right of the long one.
    assert!(x(&found, "X") > x(&found, "A much longer label") + 20.0);
    assert!(x(&found, "X") > rect.center().x - 10.0);
}

#[test]
fn horizontal_group_with_icons_stays_centred() {
    let (found, centres, rect) = show(|ui| {
        let (mut a, mut b) = (false, true);
        ui.add(
            ToggleGroup::new("t")
                .cell((icon(8.0), "Snap"), &mut a)
                .cell((icon(20.0), "Align"), &mut b)
                .equal_width(),
        )
        .rect
    });
    // Each cell centres its own content: the first cell's stays in the left
    // half and the second's in the right half, not left-aligned in columns.
    let middle = rect.center().x;
    assert!(centres[0] < middle && x(&found, "Snap") < middle);
    assert!(centres[1] > middle && x(&found, "Align") > middle);
    assert!(
        centres[0] > rect.left() + 8.0,
        "not flush against the left edge"
    );
}
