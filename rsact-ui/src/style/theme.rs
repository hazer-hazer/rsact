use crate::{
    render::color::Color,
    style::stylist::{InternalStylist, Stylist},
    widget::{
        bar::BarStyle, button::ButtonStyle, container::ContainerStyle,
        edge::EdgeStyle, knob::KnobStyle, label::LabelStyle,
        scrollable::ScrollableStyle, select::SelectStyle, slider::SliderStyle,
    },
};
use rsact_render::{
    color::{RgbColor, Rgba},
    geometry::Angle,
    style::block::{BlockStyle, BorderStyle, Radius},
};

/// Application-level theme: provides default styles for all built-in widgets.
///
/// Construct with [`Theme::default()`] and optionally customize with
/// [`Theme::with_accent`].
#[derive(Clone, Copy, PartialEq)]
pub struct Theme<C: Color> {
    bg: C,
    fg: C,
    primary: C,
    border_radius: Radius,

    // Derived //
    bg_muted: C,
    fg_muted: C,
}

impl<C: RgbColor> Default for Theme<C> {
    fn default() -> Self {
        let bg = C::default_background();
        let fg = C::default_foreground();

        let this = Self {
            bg,
            fg,
            primary: C::accents()[0],
            border_radius: Radius::SizeEqual(5),

            bg_muted: bg,
            fg_muted: fg,
        };

        this.background(bg).foreground(fg)
    }
}

// TODO: Special support for BinaryColor
impl<C: RgbColor> Theme<C> {
    fn border(&self) -> BorderStyle<C> {
        BorderStyle::base().color(self.fg).radius(self.border_radius)
    }

    fn container(&self) -> BlockStyle<C> {
        BlockStyle::base().background_color(self.bg).border(self.border())
    }
}

impl<C: RgbColor> Stylist<BarStyle<C>> for Theme<C> {
    fn style(
        &self,
        base: &BarStyle<C>,
        _selector: &super::StyleSelector,
    ) -> BarStyle<C> {
        base.color(self.fg)
    }
}

impl<C: RgbColor> Stylist<ButtonStyle<C>> for Theme<C> {
    fn style(
        &self,
        base: &ButtonStyle<C>,
        selector: &super::StyleSelector,
    ) -> ButtonStyle<C> {
        if selector.pseudoclass.hovered {
            base.container(self.container().background_color(self.bg_muted))
        } else {
            base.container(self.container())
        }
    }
}

impl<C: RgbColor> Stylist<ContainerStyle<C>> for Theme<C> {
    fn style(
        &self,
        base: &ContainerStyle<C>,
        _selector: &super::StyleSelector,
    ) -> ContainerStyle<C> {
        base.clone()
    }
}

impl<C: RgbColor> Stylist<EdgeStyle<C>> for Theme<C> {
    fn style(
        &self,
        base: &EdgeStyle<C>,
        _selector: &super::StyleSelector,
    ) -> EdgeStyle<C> {
        base.clone()
    }
}

impl<C: RgbColor> Stylist<KnobStyle<C>> for Theme<C> {
    fn style(
        &self,
        base: &KnobStyle<C>,
        _selector: &super::StyleSelector,
    ) -> KnobStyle<C> {
        base.color(self.primary)
            .angle_start(Angle::from_degrees(-120.0))
            .angle(Angle::from_degrees(-30.0))
    }
}

impl<C: RgbColor> Stylist<LabelStyle<C>> for Theme<C> {
    fn style(
        &self,
        base: &LabelStyle<C>,
        _selector: &super::StyleSelector,
    ) -> LabelStyle<C> {
        base.text_color(self.fg)
    }
}

impl<C: RgbColor> Stylist<ScrollableStyle<C>> for Theme<C> {
    fn style(
        &self,
        base: &ScrollableStyle<C>,
        _selector: &super::StyleSelector,
    ) -> ScrollableStyle<C> {
        base.track_color(self.bg)
            .thumb_color(self.bg_muted)
            .scrollbar_width(5)
            .show(crate::widget::scrollable::ScrollbarShow::Auto)
    }
}

impl<C: RgbColor> Stylist<SelectStyle<C>> for Theme<C> {
    fn style(
        &self,
        base: &SelectStyle<C>,
        _selector: &super::StyleSelector,
    ) -> SelectStyle<C> {
        base.selected_background_color(self.bg_muted)
            .selected_border_radius(self.border_radius)
    }
}

impl<C: RgbColor> Stylist<SliderStyle<C>> for Theme<C> {
    fn style(
        &self,
        base: &SliderStyle<C>,
        _selector: &super::StyleSelector,
    ) -> SliderStyle<C> {
        base.track_width(10)
            .track_color(self.bg_muted)
            .thumb_border_radius(self.border_radius)
            .thumb_size(10)
            .thumb_shape(crate::widget::slider::SliderThumbShape::RoundedSquare)
    }
}

impl<C: RgbColor> InternalStylist<C> for Theme<C> {}

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

impl<C: RgbColor> Theme<C> {
    pub fn primary(mut self, primary: C) -> Self {
        self.primary = primary;
        self
    }

    pub fn background(mut self, bg: C) -> Self {
        self.bg = bg;
        self.bg_muted = self.bg.dim(0.25);
        self
    }

    pub fn foreground(mut self, fg: C) -> Self {
        self.fg = fg;
        self.fg_muted = self.fg.dim(0.25);
        self
    }

    pub fn border_radius(mut self, border_radius: impl Into<Radius>) -> Self {
        self.border_radius = border_radius.into();
        self
    }
}

// TODO: MaterialYou? HCT it is too complex and heavy, better implement just
// something like a seed color scheme generation.
