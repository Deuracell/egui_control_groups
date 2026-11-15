//! What a [`ToggleGroup`](crate::ToggleGroup) cell shows.
//!
//! The group owns the frame, hit-testing and drag gesture. It knows nothing
//! about icons: a cell is anything that can report its size and paint itself
//! into a rectangle with a colour the group chooses. Text and
//! [`egui::Image`] work out of the box; [`Icon`] and [`PaintCell`] cover
//! sized images and custom drawing, and you can implement [`CellContent`]
//! for your own icon type.
//!
//! A tuple `(A, B)` of two contents is itself content, laid out side by side,
//! so a cell can show an icon and a label: `(Icon::new(image), "Snap")`.

use std::borrow::Cow;

use egui::{Color32, Image, Rect, RichText, TextStyle, Ui, Vec2, WidgetText, pos2, vec2};

use crate::paint::make_galley;

/// Something a toggle cell can display.
pub trait CellContent {
    /// Natural size in points. A cell is sized to the larger of its "off" and
    /// "on" content, so toggling never shifts the layout.
    fn size(&self, ui: &Ui) -> Vec2;

    /// Paint centred in `rect`. `color` is the foreground colour the group
    /// picked for the cell's current state; honour it so the content follows
    /// the style.
    fn paint(&self, ui: &Ui, rect: Rect, color: Color32);

    /// Accessible name for the cell, if the content has one (text does).
    fn accessible_label(&self) -> Option<String> {
        None
    }

    /// Width of a leading part, such as the icon of an `(icon, text)` pair.
    ///
    /// A list of options (a [dropdown](crate::ToggleGroup::dropdown)) reserves
    /// one column as wide as the widest of these, so that the rest of every
    /// option starts at the same place whatever the size of its icon.
    /// `None`, the default, means the content has no such part.
    fn leading_width(&self, _ui: &Ui) -> Option<f32> {
        None
    }

    /// Paint left-aligned in `rect`, for a list of options: the leading part
    /// (see [`leading_width`](Self::leading_width)) centred in a column
    /// `column` points wide, and the rest after it.
    ///
    /// The default suits content without a leading part: it is painted whole
    /// after the column, in line with the other options' text.
    fn paint_in_columns(&self, ui: &Ui, rect: Rect, color: Color32, column: f32) {
        let size = self.size(ui);
        let at = pos2(
            rect.left() + column_offset(ui, column),
            rect.center().y - size.y / 2.0,
        );
        self.paint(ui, Rect::from_min_size(at, size), color);
    }
}

/// Where content without a leading part starts, past a leading column.
fn column_offset(ui: &Ui, column: f32) -> f32 {
    if column > 0.0 {
        column + ui.spacing().icon_spacing
    } else {
        0.0
    }
}

/// Width `content` takes when painted with
/// [`paint_in_columns`](CellContent::paint_in_columns).
pub(crate) fn columns_width(content: &dyn CellContent, ui: &Ui, column: f32) -> f32 {
    let width = content.size(ui).x;
    match content.leading_width(ui) {
        Some(leading) => column + width - leading,
        None => column_offset(ui, column) + width,
    }
}

fn text_size(ui: &Ui, text: &WidgetText) -> Vec2 {
    make_galley(ui, text.clone()).size()
}

fn text_paint(ui: &Ui, text: &WidgetText, rect: Rect, color: Color32) {
    let galley = make_galley(ui, text.clone());
    let pos = rect.center() - galley.size() / 2.0;
    ui.painter().galley(pos, galley, color);
}

macro_rules! text_content {
    ($($ty:ty => |$v:ident| $conv:expr),* $(,)?) => {$(
        impl CellContent for $ty {
            fn size(&self, ui: &Ui) -> Vec2 {
                let $v = self;
                text_size(ui, &$conv)
            }
            fn paint(&self, ui: &Ui, rect: Rect, color: Color32) {
                let $v = self;
                text_paint(ui, &$conv, rect, color);
            }
            fn accessible_label(&self) -> Option<String> {
                let $v = self;
                Some(WidgetText::from($conv).text().to_owned())
            }
        }
    )*};
}

text_content! {
    &str => |s| WidgetText::from(*s),
    String => |s| WidgetText::from(s.clone()),
    WidgetText => |s| s.clone(),
    RichText => |s| WidgetText::from(s.clone()),
    Cow<'_, str> => |s| WidgetText::from(s.as_ref()),
}

/// Borrowed content: covers `&String`, `&WidgetText`, `&Icon` and so on.
/// (`&str` has its own impl above, since `str` is unsized.)
impl<T: CellContent> CellContent for &T {
    fn size(&self, ui: &Ui) -> Vec2 {
        (**self).size(ui)
    }
    fn paint(&self, ui: &Ui, rect: Rect, color: Color32) {
        (**self).paint(ui, rect, color);
    }
    fn accessible_label(&self) -> Option<String> {
        (**self).accessible_label()
    }
    fn leading_width(&self, ui: &Ui) -> Option<f32> {
        (**self).leading_width(ui)
    }
    fn paint_in_columns(&self, ui: &Ui, rect: Rect, color: Color32, column: f32) {
        (**self).paint_in_columns(ui, rect, color, column);
    }
}

/// Boxed content, including `Box<dyn CellContent>` for mixed collections.
impl<T: CellContent + ?Sized> CellContent for Box<T> {
    fn size(&self, ui: &Ui) -> Vec2 {
        (**self).size(ui)
    }
    fn paint(&self, ui: &Ui, rect: Rect, color: Color32) {
        (**self).paint(ui, rect, color);
    }
    fn accessible_label(&self) -> Option<String> {
        (**self).accessible_label()
    }
    fn leading_width(&self, ui: &Ui) -> Option<f32> {
        (**self).leading_width(ui)
    }
    fn paint_in_columns(&self, ui: &Ui, rect: Rect, color: Color32, column: f32) {
        (**self).paint_in_columns(ui, rect, color, column);
    }
}

/// Two contents side by side, `A` on the left and `B` on the right, centred
/// vertically and separated by `spacing().icon_spacing`. This is how a cell
/// shows an icon and a label, for example `(Icon::new(image), "Snap")`.
///
/// In a list of options (a dropdown) the first part gets a column of its own,
/// as wide as the widest among the options, so the second parts line up.
///
/// Both parts are painted in the colour the group picked. The accessible
/// label is the parts' labels joined with a space (or whichever one has one).
impl<A: CellContent, B: CellContent> CellContent for (A, B) {
    fn size(&self, ui: &Ui) -> Vec2 {
        let (a, b) = (self.0.size(ui), self.1.size(ui));
        vec2(a.x + ui.spacing().icon_spacing + b.x, a.y.max(b.y))
    }

    fn paint(&self, ui: &Ui, rect: Rect, color: Color32) {
        let (a, b) = (self.0.size(ui), self.1.size(ui));
        let gap = ui.spacing().icon_spacing;
        let left = rect.center().x - (a.x + gap + b.x) / 2.0;
        let cy = rect.center().y;
        let a_rect = Rect::from_min_size(pos2(left, cy - a.y / 2.0), a);
        let b_rect = Rect::from_min_size(pos2(a_rect.right() + gap, cy - b.y / 2.0), b);
        self.0.paint(ui, a_rect, color);
        self.1.paint(ui, b_rect, color);
    }

    fn accessible_label(&self) -> Option<String> {
        match (self.0.accessible_label(), self.1.accessible_label()) {
            (Some(a), Some(b)) => Some(format!("{a} {b}")),
            (a, b) => a.or(b),
        }
    }

    fn leading_width(&self, ui: &Ui) -> Option<f32> {
        Some(self.0.size(ui).x)
    }

    fn paint_in_columns(&self, ui: &Ui, rect: Rect, color: Color32, column: f32) {
        let (a, b) = (self.0.size(ui), self.1.size(ui));
        let column = column.max(a.x);
        let cy = rect.center().y;
        let a_at = pos2(rect.left() + (column - a.x) / 2.0, cy - a.y / 2.0);
        let b_at = pos2(
            rect.left() + column + ui.spacing().icon_spacing,
            cy - b.y / 2.0,
        );
        self.0.paint(ui, Rect::from_min_size(a_at, a), color);
        self.1.paint(ui, Rect::from_min_size(b_at, b), color);
    }
}

/// An image shown as an icon, tinted to the cell's foreground colour.
///
/// Without [`sized`](Self::sized) the icon is as tall as the button text. As
/// with any [`egui::Image`], URI and byte sources need an image loader (for
/// example `egui_extras::install_image_loaders`); textures work without one.
#[derive(Clone)]
pub struct Icon {
    image: Image<'static>,
    size: Option<f32>,
}

impl Icon {
    /// Wrap an image.
    pub fn new(image: Image<'static>) -> Self {
        Self { image, size: None }
    }

    /// Edge length in points.
    #[must_use]
    pub fn sized(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    fn edge(&self, ui: &Ui) -> Vec2 {
        Vec2::splat(
            self.size
                .unwrap_or_else(|| ui.text_style_height(&TextStyle::Button)),
        )
    }
}

impl CellContent for Icon {
    fn size(&self, ui: &Ui) -> Vec2 {
        self.edge(ui)
    }

    fn paint(&self, ui: &Ui, rect: Rect, color: Color32) {
        let size = self.edge(ui);
        let target = Rect::from_center_size(rect.center(), size);
        self.image
            .clone()
            .fit_to_exact_size(size)
            .tint(color)
            .paint_at(ui, target);
    }
}

impl CellContent for Image<'static> {
    fn size(&self, ui: &Ui) -> Vec2 {
        Icon::new(self.clone()).edge(ui)
    }

    fn paint(&self, ui: &Ui, rect: Rect, color: Color32) {
        Icon::new(self.clone()).paint(ui, rect, color);
    }
}

/// Custom drawing: a fixed size and a closure that paints into the rectangle
/// it is given.
///
/// ```
/// # use egui_control_groups::PaintCell;
/// # use egui::vec2;
/// let dot = PaintCell::new(vec2(14.0, 14.0), |ui, rect, color| {
///     ui.painter().circle_filled(rect.center(), rect.width() / 2.0, color);
/// });
/// # let _ = dot;
/// ```
pub struct PaintCell<F> {
    size: Vec2,
    paint: F,
}

impl<F: Fn(&Ui, Rect, Color32)> PaintCell<F> {
    /// `size` is the content size; `paint` receives the `Ui`, a rectangle of
    /// that size centred in the cell, and the foreground colour.
    pub fn new(size: Vec2, paint: F) -> Self {
        Self { size, paint }
    }
}

impl<F: Fn(&Ui, Rect, Color32)> CellContent for PaintCell<F> {
    fn size(&self, _ui: &Ui) -> Vec2 {
        self.size
    }

    fn paint(&self, ui: &Ui, rect: Rect, color: Color32) {
        let target = Rect::from_center_size(rect.center(), self.size);
        (self.paint)(ui, target, color);
    }
}
