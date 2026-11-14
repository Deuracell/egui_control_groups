//! Pure geometry shared by the widgets: corner rounding, cell layout and
//! pointer hit-testing. Nothing here touches a `Ui`, so it is easy to test.

use std::ops::RangeInclusive;

use egui::{CornerRadius, Pos2, Rect, Vec2, pos2, vec2};

/// Which way the cells of a group are stacked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// Cells sit side by side (left to right).
    #[default]
    Horizontal,
    /// Cells are stacked top to bottom.
    Vertical,
}

/// Corner radius for cell `index` of `count`, rounding only the corners that
/// lie on the outside of the group.
///
/// A single cell is rounded on all four corners. In a horizontal group the
/// first cell keeps its left corners and the last its right corners; in a
/// vertical group the first keeps its top corners and the last its bottom
/// corners. Every inner corner is square, so neighbours meet flush.
pub fn corner_radius_for(
    index: usize,
    count: usize,
    orientation: Orientation,
    radius: u8,
) -> CornerRadius {
    let first = index == 0;
    let last = index + 1 >= count;
    let (mut nw, mut ne, mut se, mut sw) = (0, 0, 0, 0);
    match orientation {
        Orientation::Horizontal => {
            if first {
                nw = radius;
                sw = radius;
            }
            if last {
                ne = radius;
                se = radius;
            }
        }
        Orientation::Vertical => {
            if first {
                nw = radius;
                ne = radius;
            }
            if last {
                sw = radius;
                se = radius;
            }
        }
    }
    CornerRadius { nw, ne, sw, se }
}

/// Lay cells out edge to edge starting at `origin`.
///
/// `main_sizes` are the cell extents along the stacking axis and `cross` is
/// the shared extent on the other axis.
pub fn layout_cells(
    origin: Pos2,
    main_sizes: &[f32],
    cross: f32,
    orientation: Orientation,
) -> Vec<Rect> {
    let mut cursor = 0.0;
    main_sizes
        .iter()
        .map(|&main| {
            let (offset, size): (Vec2, Vec2) = match orientation {
                Orientation::Horizontal => (vec2(cursor, 0.0), vec2(main, cross)),
                Orientation::Vertical => (vec2(0.0, cursor), vec2(cross, main)),
            };
            cursor += main;
            Rect::from_min_size(origin + offset, size)
        })
        .collect()
}

/// Index of the cell under `pos`, looking only at the stacking axis.
///
/// Positions before the first cell map to `0` and positions past the last map
/// to the last cell, so a drag keeps working when the pointer drifts off the
/// group sideways or overshoots its end. `cells` must be non-empty and laid
/// out contiguously (as [`layout_cells`] does).
pub fn index_at(cells: &[Rect], pos: Pos2, orientation: Orientation) -> usize {
    let coord = match orientation {
        Orientation::Horizontal => pos.x,
        Orientation::Vertical => pos.y,
    };
    for (i, cell) in cells.iter().enumerate() {
        let max = match orientation {
            Orientation::Horizontal => cell.max.x,
            Orientation::Vertical => cell.max.y,
        };
        if coord < max {
            return i;
        }
    }
    cells.len().saturating_sub(1)
}

/// Every index from `a` to `b` inclusive, whichever is larger. Used so a fast
/// drag that skips cells between two frames still touches all of them.
pub fn range_between(a: usize, b: usize) -> RangeInclusive<usize> {
    a.min(b)..=a.max(b)
}

#[allow(dead_code)]
pub(crate) fn origin(rect: Rect) -> Pos2 {
    pos2(rect.min.x, rect.min.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(nw: u8, ne: u8, sw: u8, se: u8) -> CornerRadius {
        CornerRadius { nw, ne, sw, se }
    }

    #[test]
    fn single_cell_is_rounded_everywhere() {
        for o in [Orientation::Horizontal, Orientation::Vertical] {
            assert_eq!(corner_radius_for(0, 1, o, 6), CornerRadius::same(6));
        }
    }

    #[test]
    fn horizontal_rounds_only_outer_ends() {
        let o = Orientation::Horizontal;
        assert_eq!(corner_radius_for(0, 3, o, 6), r(6, 0, 6, 0));
        assert_eq!(corner_radius_for(1, 3, o, 6), r(0, 0, 0, 0));
        assert_eq!(corner_radius_for(2, 3, o, 6), r(0, 6, 0, 6));
    }

    #[test]
    fn vertical_rounds_only_outer_ends() {
        let o = Orientation::Vertical;
        assert_eq!(corner_radius_for(0, 3, o, 6), r(6, 6, 0, 0));
        assert_eq!(corner_radius_for(1, 3, o, 6), r(0, 0, 0, 0));
        assert_eq!(corner_radius_for(2, 3, o, 6), r(0, 0, 6, 6));
    }

    #[test]
    fn layout_is_contiguous() {
        let cells = layout_cells(
            pos2(10.0, 20.0),
            &[30.0, 40.0],
            25.0,
            Orientation::Horizontal,
        );
        assert_eq!(
            cells[0],
            Rect::from_min_size(pos2(10.0, 20.0), vec2(30.0, 25.0))
        );
        assert_eq!(
            cells[1],
            Rect::from_min_size(pos2(40.0, 20.0), vec2(40.0, 25.0))
        );
        let cells = layout_cells(pos2(0.0, 0.0), &[10.0, 10.0], 5.0, Orientation::Vertical);
        assert_eq!(cells[1].min, pos2(0.0, 10.0));
        assert_eq!(cells[1].size(), vec2(5.0, 10.0));
    }

    #[test]
    fn index_at_clamps_and_ignores_cross_axis() {
        let cells = layout_cells(
            pos2(0.0, 0.0),
            &[10.0, 10.0, 10.0],
            20.0,
            Orientation::Horizontal,
        );
        let h = Orientation::Horizontal;
        assert_eq!(index_at(&cells, pos2(-50.0, 500.0), h), 0);
        assert_eq!(index_at(&cells, pos2(5.0, -500.0), h), 0);
        assert_eq!(index_at(&cells, pos2(10.0, 0.0), h), 1);
        assert_eq!(index_at(&cells, pos2(29.9, 0.0), h), 2);
        assert_eq!(index_at(&cells, pos2(999.0, 0.0), h), 2);
    }

    #[test]
    fn range_between_is_order_independent() {
        assert_eq!(range_between(1, 4).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
        assert_eq!(range_between(4, 1).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
        assert_eq!(range_between(2, 2).collect::<Vec<_>>(), vec![2]);
    }
}
