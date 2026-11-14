//! What a [`ToggleGroup`](crate::ToggleGroup) cell shows.
//!
//! The group owns the frame, hit-testing and drag gesture. It knows nothing
//! about icons: a cell is anything that can report its size and paint itself
//! into a rectangle with a colour the group chooses. Text and
//! [`egui::Image`] work out of the box; [`Icon`] and [`PaintCell`] cover
//! sized images and custom drawing, and you can implement [`CellContent`]
//! for your own icon type.

use egui::{Color32, Image, Rect, RichText, TextStyle, Ui, Vec2, WidgetText};

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
