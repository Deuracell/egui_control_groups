//! Options with an icon and text share one icon column, in the closed cell
//! and in the list, so nothing shifts when another option is chosen even if
//! the icons differ in size.

use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, Ui, pos2, vec2};
use egui_control_groups::{CellContent, PaintCell, ToggleGroup};

fn texts(shape: &Shape, out: &mut Vec<(String, f32)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        Shape::Text(t) => out.push((t.galley.text().to_owned(), t.pos.x)),
        _ => {}
    }
}

struct H {
    ctx: Context,
    time: f64,
    value: u8,
    rect: Rect,
    /// Text and left edge of every text shape painted last frame.
    texts: Vec<(String, f32)>,
    /// Left edge of every icon painted last frame, by icon width.
    icons: Vec<(f32, f32)>,
}

impl H {
    fn new(value: u8) -> Self {
        let mut h = Self {
            ctx: Context::default(),
            time: 0.0,
            value,
            rect: Rect::NOTHING,
            texts: Vec::new(),
            icons: Vec::new(),
        };
        for _ in 0..3 {
            h.frame(vec![]);
        }
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let icons = std::cell::RefCell::new(Vec::new());
        let (value, rect) = (&mut self.value, &mut self.rect);
        let out = self.ctx.run_ui(input, |ui: &mut Ui| {
            // Icons of different widths, then one option with no icon at all.
            let icon = |w: f32| {
                let icons = &icons;
                PaintCell::new(vec2(w, 10.0), move |_: &Ui, r: Rect, _| {
                    icons.borrow_mut().push((w, r.left()));
                })
            };
            let options: [(u8, Box<dyn CellContent + '_>); 3] = [
                (0, Box::new((icon(8.0), "Grid"))),
                (1, Box::new((icon(20.0), "Vertex"))),
                (2, Box::new("None")),
            ];
            *rect = ui.add(ToggleGroup::new("d").dropdown(value, options)).rect;
        });
        self.texts.clear();
        for clipped in &out.shapes {
            texts(&clipped.shape, &mut self.texts);
        }
        self.icons = icons.into_inner();
        out.drop_without_applying_deltas();
    }

    fn click(&mut self, p: Pos2) {
        self.frame(vec![Event::PointerMoved(p)]);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    fn text_x(&self, text: &str) -> Vec<f32> {
        let found = self.texts.iter().filter(|(t, _)| t == text);
        found.map(|(_, x)| *x).collect()
    }
}

#[test]
fn closed_cell_content_does_not_shift_between_options() {
    let (grid, vertex, none) = (H::new(0), H::new(1), H::new(2));
    assert_eq!(grid.rect, vertex.rect, "the cell keeps its size");
    assert_eq!(grid.rect, none.rect);
    // The text starts at the same place whatever the icon's width, and an
    // option without an icon lines up with it.
    let x = grid.text_x("Grid")[0];
    assert_eq!(vertex.text_x("Vertex")[0], x);
    assert_eq!(none.text_x("None")[0], x);
    // Each icon is centred in the shared column.
    let centre = |h: &H| h.icons[0].1 + h.icons[0].0 / 2.0;
    assert_eq!(centre(&grid), centre(&vertex));
    assert!(
        grid.icons[0].1 > grid.rect.left(),
        "inside the cell's padding"
    );
}

#[test]
fn list_rows_use_the_same_columns() {
    let mut h = H::new(0);
    let closed = h.text_x("Grid")[0];
    h.click(pos2(h.rect.left() + 4.0, h.rect.center().y));
    // The closed cell and its row in the list, then the other rows.
    let grid = h.text_x("Grid");
    assert_eq!(grid.len(), 2, "the list is open");
    assert_eq!(
        grid[0], closed,
        "opening the list moves nothing in the cell"
    );
    let row = grid[1];
    assert_eq!(h.text_x("Vertex"), vec![row]);
    assert_eq!(h.text_x("None"), vec![row]);
    let centres: Vec<f32> = h.icons.iter().skip(1).map(|(w, x)| x + w / 2.0).collect();
    assert_eq!(centres.len(), 2, "one icon per row that has one");
    assert_eq!(centres[0], centres[1], "icons are centred in one column");
}
