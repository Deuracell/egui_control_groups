# egui-control-groups

Blender-style control groups for [egui](https://github.com/emilk/egui) 0.36.
Neighbouring controls are drawn as one rounded shape, with a thin divider
between cells.

- **`ToggleGroup`**: a row or column of toggle buttons. Press one and drag
  across the others to set them all to the same state. Cells can hold text,
  an `egui::Image` (via `Icon`), custom painting (`PaintCell`), or any type
  implementing `CellContent`. `.exclusive(true)` makes a press select only that
  cell; Shift (a long press on touch) adds to the selection, as in Blender
  (`.allow_extend(false)` turns that off). A pair such as `(icon, "Snap")`
  shows an icon and text together. `.dropdown(&mut value, options)` adds a
  cell that shows the chosen option and a caret, and opens a list.
  `.width(w)` or `.fill_width()` sets the width of the whole group.
- **`RadioGroup`**: the same row bound to a single value, so exactly one
  option is selected. `RadioGroup::optional` binds an `Option` instead, and
  pressing the selected option clears it.
- **`FieldGroup`**: a row or column of text or number fields. Click to edit
  one, or drag across several and type once to edit them all. Enter or
  clicking away commits; Escape cancels. Tab commits and opens the next
  field (Shift+Tab the previous one); `.tab_behavior(TabBehavior::Wrap)` wraps
  around at the ends instead of moving focus on. `Number` wraps a value to
  give it a fixed precision, a range, a prefix or a suffix. `fields()` adds
  one field per array element, and `.width(w)` / `.fill_width()` set the width
  of the group.

Both groups work from the keyboard: Tab to a cell, then Space or Enter to
toggle it or open the field for editing.

Colours, radius and padding come from the current `egui::Style`; use
`GroupStyle` to override them.

Release tags mirror the egui version they target: the minor version tracks egui (`0.36.x` targets egui 0.36).

> **Until 1.0, patch releases may contain breaking changes**: the minor version is reserved for the egui version. Read the commit log or release notes before moving to a newer tag. Since the crate is pinned by git tag, nothing updates without you changing the tag.

| Tag | egui / eframe |
|---|---|
| `v0.36.0` | 0.36 |

A tag is a fixed snapshot: it never moves. `main` targets the newest egui and may be ahead of the latest tag, with unreleased and possibly breaking changes, so depend on a tag rather than on `main`.

```rust
use egui_control_groups::{FieldGroup, Number, ToggleGroup};

ui.add(ToggleGroup::new("axes").cells(["X", "Y", "Z"], &mut axes).equal_width());

ui.add(FieldGroup::new("pos").fields(["X", "Y", "Z"], &mut position).vertical());
```

`Number` borrows the value, so bind it to a variable first. Here the group
also fills the available width:

```rust
let mut w = Number::new(&mut width).precision(1).range(0.0..=500.0).suffix(" px");
let mut h = Number::new(&mut height).precision(1).range(0.0..=500.0).suffix(" px");
ui.add(FieldGroup::new("size").labeled("W", &mut w).labeled("H", &mut h).fill_width());
```

A `RadioGroup` picks one value, and a `ToggleGroup` can end in a dropdown:

```rust
use egui_control_groups::RadioGroup;

ui.add(
    RadioGroup::new("mode", &mut mode)
        .option(Mode::Vertex, "Vertex")
        .option(Mode::Edge, "Edge")
        .option(Mode::Face, "Face"),
);

ui.add(
    ToggleGroup::new("snap")
        .cell("Snap", &mut snap_on)
        .dropdown(&mut snap_mode, [(SnapMode::Grid, "Grid"), (SnapMode::Vertex, "Vertex")]),
);
```

Cells can show a different icon per state, with the active fill turned off:

```rust
ui.add(
    ToggleGroup::new("vis")
        .state_cell("🙈", "👁", &mut shown)
        .tooltip("Visible")
        .highlight_active(false),
);
```

`FieldGroup` edits `String`, the integer types, `f32` and `f64`. Implement
`FieldValue` for other types.

`cargo run --example demo` shows everything. A web build runs at
https://deuracell.github.io/egui_control_groups/ (source in `web-demo/`).

## License

MIT
