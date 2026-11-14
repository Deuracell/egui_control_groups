# egui-control-groups

Joined, Blender-style control groups for [egui](https://github.com/emilk/egui) 0.36.

Neighbouring controls are drawn as **one rounded object**: only the outer
corners are rounded and cells meet with a thin divider.

- `ToggleGroup`: a row or column of toggle buttons. Press one and drag across
  the others to set them all to the same state. Cells show anything that
  implements the small `CellContent` trait: text, glyphs, `egui::Image`
  (via `Icon`), custom painter code (`PaintCell`) or your own type. Content can
  differ per state (`state_cell`), and the active fill can be turned off
  (`highlight_active(false)`) for plain icon buttons. The crate knows nothing
  about icons.
- `FieldGroup`: a row or column of text or number fields. Click to edit one,
  or press in one and drag across others to select several, then type once to
  edit them together. Enter or clicking away commits, Escape cancels.

No theme of its own: colours, radius and padding come from the surrounding
`egui::Style`. Override single values with `GroupStyle` where needed.

```rust
use egui_control_groups::{FieldGroup, ToggleGroup};

ui.add(ToggleGroup::new("axes").cells(["X", "Y", "Z"], &mut axes).equal_width());

// Icon-only buttons whose icon swaps with the state, no active fill:
ui.add(ToggleGroup::new("vis").state_cell("🙈", "👁", &mut shown).tooltip("Visible").highlight_active(false));

let [x, y, z] = &mut position;
ui.add(FieldGroup::new("pos").labeled("X", x).labeled("Y", y).labeled("Z", z).vertical());
```

`FieldGroup` edits `String`, all integer types and `f32` / `f64` (decimal comma
accepted). Implement `FieldValue` for your own types.

Run the demo with `cargo run --example demo`.

## License

MIT, see [LICENSE](LICENSE).
