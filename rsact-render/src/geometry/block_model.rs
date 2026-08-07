use super::padding::Padding;

/// The box model: what a widget's own geometry reserves around its content.
///
/// **WS5.5: `border_width` used to live here and no longer does.** The rule the
/// codebase follows is *does it change the box, or only the pixels inside it?* —
/// and a border, drawn `StrokeAlignment::Inside`, paints over the padding ring
/// without moving anything. It is a [`BorderStyle`] property now, which is what
/// lets it answer `hovered`/`pressed`/`focused` like every other style value;
/// from here it never could, because layout must not depend on the stylist (a
/// hover-driven relayout would be thrash).
///
/// The practical consequence for users is the SwiftUI one: a thick border over
/// content is fixed by adding padding, explicitly, rather than by the framework
/// silently reserving space.
///
/// [`BorderStyle`]: crate::style::block::BorderStyle
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockModel {
    pub padding: Padding,
}

impl BlockModel {
    pub fn zero() -> Self {
        Self { padding: Padding::zero() }
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }
}
