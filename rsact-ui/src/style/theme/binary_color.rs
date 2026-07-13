//! Theme for [`BinaryColor`] (1-bit / monochrome displays).
//!
//! `BinaryColor` is a very special case: only two colors exist, `Off` and `On`,
//! so the RGB strategy of dimming/tinting backgrounds for muted surfaces and
//! state feedback is impossible. It therefore gets a dedicated [`BinaryTheme`]
//! type (rather than reusing the RGB [`Theme`](super::Theme)), designed from
//! scratch around a single rule:
//!
//! > **Never fill a background with `On` behind `On` content.**
//!
//! A widget cannot currently recolor the text/icons of its children (we only
//! control direct widget styling), so content is always drawn in the
//! foreground color. If we inverted a widget's background to `On`, its `On`
//! text would become invisible. Instead, structure and interaction state are
//! expressed with **borders** and **outlines** — both of which we fully
//! control — rather than by inverting backgrounds.
//!
//! Layout:
//! - Background: `Off`, foreground/content: `On`.
//! - Borders and widget-custom elements (bar/knob fills, slider track,
//!   scrollbar thumb): `On`, so they contrast against the `Off` background.
//! - Hover/focus: an `On` outline is added, keeping content readable.
//!
//! TODO: Once a widget can cascade a foreground (text) color to its children,
//! revisit "inverted active" states (active button = background `On`,
//! foreground `Off`). Until then it would white-out labels, so it is omitted.

#[cfg(feature = "tiny-icons")]
use crate::widget::icon::IconStyle;
use crate::{
    style::{
        StyleSelector,
        stylist::{InternalStylist, Stylist},
    },
    widget::{
        bar::BarStyle,
        button::ButtonStyle,
        checkbox::CheckboxStyle,
        container::ContainerStyle,
        edge::EdgeStyle,
        knob::KnobStyle,
        label::LabelStyle,
        scrollable::{ScrollableStyle, ScrollbarShow},
        select::SelectStyle,
        slider::{SliderStyle, SliderThumbShape},
    },
};
use embedded_graphics::pixelcolor::BinaryColor;
use rsact_render::{
    geometry::Angle,
    style::block::{BlockStyle, BorderStyle, OutlineStyle, Radius},
};

/// Theme for 1-bit / monochrome ([`BinaryColor`]) displays.
///
/// Construct with [`BinaryTheme::default()`] (background `Off`, foreground
/// `On`). See the [module docs](self) for the design rationale.
#[derive(Clone, Copy, PartialEq)]
pub struct BinaryTheme {
    /// Foreground: everything drawn on top of the background (text, icons,
    /// borders, fills).
    fg: BinaryColor,
    /// Background.
    bg: BinaryColor,
    border_radius: Radius,
}

impl Default for BinaryTheme {
    fn default() -> Self {
        Self {
            fg: BinaryColor::On,
            bg: BinaryColor::Off,
            // Crisper on small monochrome panels than the RGB default of 5.
            border_radius: Radius::SizeEqual(2),
        }
    }
}

impl BinaryTheme {
    pub fn border_radius(mut self, border_radius: impl Into<Radius>) -> Self {
        self.border_radius = border_radius.into();
        self
    }

    fn mono_border(&self) -> BorderStyle<BinaryColor> {
        BorderStyle::base()
            .color(self.fg)
            .radius(self.border_radius)
    }

    /// `bg` background with an `fg` border. Content drawn on top (in `fg`)
    /// stays visible.
    fn mono_container(&self) -> BlockStyle<BinaryColor> {
        BlockStyle::base()
            .background_color(self.bg)
            .border(self.mono_border())
    }

    /// `fg` outline used to signal hover/focus. Unlike inverting the
    /// background, an outline keeps the widget's content readable.
    fn mono_outline(&self) -> OutlineStyle<BinaryColor> {
        OutlineStyle::base()
            .color(self.fg)
            .radius(self.border_radius)
            .offset(1)
            .width(1)
    }

    /// Bordered container that gains an outline while hovered or focused, and a
    /// bolder (width-2) outline while pressed. `pressed` is checked first: a
    /// held pointer stays "hovered" (hover freezes on the pressed widget), so
    /// the states co-occur. A bolder outline is the strongest feedback possible
    /// without inverting the background (which would white-out `On` content).
    fn interactive_container(
        &self,
        selector: &StyleSelector,
    ) -> BlockStyle<BinaryColor> {
        let container = self.mono_container();
        if selector.pseudoclass.pressed {
            container.outline(self.mono_outline().width(2))
        } else if selector.pseudoclass.hovered || selector.pseudoclass.focused {
            container.outline(self.mono_outline())
        } else {
            container
        }
    }
}

impl Stylist<BarStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &BarStyle<BinaryColor>,
        _selector: &StyleSelector,
    ) -> BarStyle<BinaryColor> {
        base.color(self.fg)
    }
}

impl Stylist<ButtonStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &ButtonStyle<BinaryColor>,
        selector: &StyleSelector,
    ) -> ButtonStyle<BinaryColor> {
        base.container(self.interactive_container(selector))
    }
}

impl Stylist<CheckboxStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &CheckboxStyle<BinaryColor>,
        selector: &StyleSelector,
    ) -> CheckboxStyle<BinaryColor> {
        base.container(self.interactive_container(selector))
            .icon_color(self.fg)
    }
}

impl Stylist<ContainerStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &ContainerStyle<BinaryColor>,
        _selector: &StyleSelector,
    ) -> ContainerStyle<BinaryColor> {
        // Containers are layout grouping; left transparent to avoid drawing a
        // border around every group on an already sparse display.
        base.clone()
    }
}

impl Stylist<EdgeStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &EdgeStyle<BinaryColor>,
        _selector: &StyleSelector,
    ) -> EdgeStyle<BinaryColor> {
        base.clone()
    }
}

impl Stylist<KnobStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &KnobStyle<BinaryColor>,
        _selector: &StyleSelector,
    ) -> KnobStyle<BinaryColor> {
        base.color(self.fg)
            .angle_start(Angle::from_degrees(-120.0))
            .angle(Angle::from_degrees(-30.0))
    }
}

impl Stylist<LabelStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &LabelStyle<BinaryColor>,
        _selector: &StyleSelector,
    ) -> LabelStyle<BinaryColor> {
        base.text_color(self.fg)
    }
}

impl Stylist<ScrollableStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &ScrollableStyle<BinaryColor>,
        _selector: &StyleSelector,
    ) -> ScrollableStyle<BinaryColor> {
        // Track is invisible (no second color to tint it with); only the `fg`
        // thumb is drawn over the `bg` background.
        base.transparent_track_color()
            .thumb_color(self.fg)
            .scrollbar_width(3)
            .show(ScrollbarShow::Auto)
    }
}

impl Stylist<SelectStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &SelectStyle<BinaryColor>,
        _selector: &StyleSelector,
    ) -> SelectStyle<BinaryColor> {
        // Mark the selected option with a border instead of a filled
        // background, so its label (drawn in `fg`) stays readable.
        base.text_color(self.fg)
            .selected_border_color(self.fg)
            .selected_border_radius(self.border_radius)
    }
}

impl Stylist<SliderStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &SliderStyle<BinaryColor>,
        _selector: &StyleSelector,
    ) -> SliderStyle<BinaryColor> {
        // `fg` track line; the thumb is a hollow handle (`bg` fill punches out
        // the track behind it, `fg` border outlines it) so it reads clearly.
        base.track_width(4)
            .track_color(self.fg)
            .thumb_color(self.bg)
            .thumb_border_color(self.fg)
            .thumb_border_radius(self.border_radius)
            .thumb_size(10)
            .thumb_shape(SliderThumbShape::RoundedSquare)
    }
}

#[cfg(feature = "tiny-icons")]
impl Stylist<IconStyle<BinaryColor>> for BinaryTheme {
    fn style(
        &self,
        base: &IconStyle<BinaryColor>,
        _selector: &StyleSelector,
    ) -> IconStyle<BinaryColor> {
        // Icons are content, drawn in `fg` like text (see module docs: content
        // is always foreground-colored since a widget cannot yet recolor a
        // child's own drawing).
        base.color(self.fg)
    }
}

impl InternalStylist<BinaryColor> for BinaryTheme {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Style, StylePseudoClass};

    fn selector(hovered: bool, focused: bool) -> StyleSelector {
        StyleSelector {
            pseudoclass: StylePseudoClass::default()
                .hovered(hovered)
                .focused(focused),
        }
    }

    fn pressed_selector() -> StyleSelector {
        // A real press keeps the widget hovered too (hover freezes on it).
        StyleSelector {
            pseudoclass: StylePseudoClass::default()
                .hovered(true)
                .pressed(true),
        }
    }

    #[test]
    fn background_is_off_foreground_is_on() {
        let theme = BinaryTheme::default();
        assert_eq!(theme.bg, BinaryColor::Off);
        assert_eq!(theme.fg, BinaryColor::On);
    }

    #[test]
    fn button_has_off_bg_and_on_border() {
        let style = Stylist::<ButtonStyle<_>>::style(
            &BinaryTheme::default(),
            &ButtonStyle::base(),
            &selector(false, false),
        );

        assert_eq!(
            style.container.background_color.get(),
            Some(BinaryColor::Off)
        );
        assert_eq!(style.container.border.color.get(), Some(BinaryColor::On));
        // No outline at rest.
        assert_eq!(style.container.outline.width, 0);
    }

    #[test]
    fn button_gains_on_outline_when_hovered_or_focused() {
        for sel in [selector(true, false), selector(false, true)] {
            let style = Stylist::<ButtonStyle<_>>::style(
                &BinaryTheme::default(),
                &ButtonStyle::base(),
                &sel,
            );

            assert_eq!(style.container.outline.width, 1);
            assert_eq!(
                style.container.outline.color.get(),
                Some(BinaryColor::On)
            );
        }
    }

    #[test]
    fn button_pressed_gets_bolder_outline() {
        let style = Stylist::<ButtonStyle<_>>::style(
            &BinaryTheme::default(),
            &ButtonStyle::base(),
            &pressed_selector(),
        );

        // Pressed is stronger than hover/focus: a width-2 `On` outline.
        assert_eq!(style.container.outline.width, 2);
        assert_eq!(style.container.outline.color.get(), Some(BinaryColor::On));
        // Background stays `Off` — never inverted (would hide `On` content).
        assert_eq!(
            style.container.background_color.get(),
            Some(BinaryColor::Off)
        );
    }

    #[test]
    fn checkbox_icon_is_on() {
        let style = Stylist::<CheckboxStyle<_>>::style(
            &BinaryTheme::default(),
            &CheckboxStyle::base(),
            &selector(false, false),
        );

        assert_eq!(style.icon_color.get(), Some(BinaryColor::On));
        assert_eq!(style.container.border.color.get(), Some(BinaryColor::On));
    }

    #[test]
    fn bar_and_knob_fill_is_on() {
        let bar = Stylist::<BarStyle<_>>::style(
            &BinaryTheme::default(),
            &BarStyle::base(),
            &selector(false, false),
        );
        assert_eq!(bar.color.get(), Some(BinaryColor::On));

        let knob = Stylist::<KnobStyle<_>>::style(
            &BinaryTheme::default(),
            &KnobStyle::base(),
            &selector(false, false),
        );
        assert_eq!(knob.color.get(), Some(BinaryColor::On));
    }

    #[test]
    fn label_text_is_on() {
        let style = Stylist::<LabelStyle<_>>::style(
            &BinaryTheme::default(),
            &LabelStyle::base(),
            &selector(false, false),
        );
        assert_eq!(style.text_color.get(), Some(BinaryColor::On));
    }

    #[test]
    fn select_marks_selection_with_border_not_fill() {
        let style = Stylist::<SelectStyle<_>>::style(
            &BinaryTheme::default(),
            &SelectStyle::base(),
            &selector(false, false),
        );

        // Border highlights the selected row...
        assert_eq!(style.selected.border.color.get(), Some(BinaryColor::On));
        // ...and no background fill that would hide the option's text.
        assert_eq!(style.selected.background_color.get(), None);
        assert_eq!(style.text_color.get(), Some(BinaryColor::On));
    }

    #[test]
    fn slider_thumb_is_hollow_over_on_track() {
        let style = Stylist::<SliderStyle<_>>::style(
            &BinaryTheme::default(),
            &SliderStyle::base(),
            &selector(false, false),
        );

        assert_eq!(style.track_color.get(), Some(BinaryColor::On));
        // Hollow handle: Off fill, On border.
        assert_eq!(style.thumb.background_color.get(), Some(BinaryColor::Off));
        assert_eq!(style.thumb.border.color.get(), Some(BinaryColor::On));
    }
}
