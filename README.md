# egui-control-groups

Blender-style control groups for [egui](https://github.com/emilk/egui) 0.36.
Neighbouring controls are drawn as one rounded shape, with a thin divider
between cells.

- **`ToggleGroup`**: a row or column of toggle buttons. Press one and drag
  across the others to set them all to the same state. Cells can hold text,
  an `egui::Image` (via `Icon`), custom painting (`PaintCell`), or any type
  implementing `CellContent`. `.exclusive(true)` makes a press select only that
  cell; Shift (a long press on touch) adds to the selection, as in Blender.
- **`FieldGroup`**: a row or column of text or number fields. Click to edit
  one, or drag across several and type once to edit them all. Enter or
  clicking away commits; Escape cancels.

Colours, radius and padding come from the current `egui::Style`; use
`GroupStyle` to override them.

Release tags mirror the egui version they target: the minor version tracks egui (`0.36.x` targets egui 0.36).

> **Until 1.0, patch releases may contain breaking changes**: the minor version is reserved for the egui version. Read the commit log or release notes before moving to a newer tag. Since the crate is pinned by git tag, nothing updates without you changing the tag.

| Tag | egui / eframe |
|---|---|
| `v0.36.0` | 0.36 |

A tag is a fixed snapshot: it never moves. `main` targets the newest egui and may be ahead of the latest tag, with unreleased and possibly breaking changes, so depend on a tag rather than on `main`.

```rust
use egui_control_groups::{FieldGroup, ToggleGroup};

ui.add(ToggleGroup::new("axes").cells(["X", "Y", "Z"], &mut axes).equal_width());

let [x, y, z] = &mut position;
ui.add(FieldGroup::new("pos").labeled("X", x).labeled("Y", y).labeled("Z", z).vertical());
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
