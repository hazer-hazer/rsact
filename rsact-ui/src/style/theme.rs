use crate::{
    render::color::Color,
    style::stylist::{InternalStylist, Stylist},
    widget::{
        bar::BarStyle, button::ButtonStyle, knob::KnobStyle, label::LabelStyle,
        scrollable::ScrollableStyle, select::SelectStyle, slider::SliderStyle,
    },
};
use rsact_render::{color::Rgba, style::block::Radius};

/// Application-level theme: provides default styles for all built-in widgets.
///
/// Construct with [`Theme::default()`] and optionally customise with
/// [`Theme::with_accent`].
#[derive(Clone, Copy, PartialEq)]
pub struct Theme<C: Color> {
    bg: C,
    fg: C,
    primary: C,
    border_radius: Radius,
}

impl<C: Color> Stylist<ButtonStyle<C>> for Theme<C> {
    fn style(
        &self,
        base: &ButtonStyle<C>,
        selector: &super::StyleSelector,
    ) -> ButtonStyle<C> {
        todo!()
    }
}

impl<C: Color> InternalStylist<C> for Theme<C> {}

// impl<C: Color> Default for Theme<C> {
//     fn default() -> Self {
//         Self {
//             bg: C::default_background(),
//             fg: C::default_foreground(),
//             primary: C::accents()[0],
//             border_radius: Radius::circle(),
//         }
//     }
// }

impl<C: Color> Theme<C> {
    pub fn primary(mut self, primary: C) -> Self {
        self.primary = primary;
        self
    }

    pub fn background(mut self, bg: C) -> Self {
        self.bg = bg;
        self
    }

    pub fn foreground(mut self, fg: C) -> Self {
        self.fg = fg;
        self
    }

    pub fn border_radius(mut self, border_radius: impl Into<Radius>) -> Self {
        self.border_radius = border_radius.into();
        self
    }
}

// TODO: MaterialYou
