use crate::{
    render::color::Color,
    widget::{
        bar::BarStyle, button::ButtonStyle, knob::KnobStyle,
        scrollable::ScrollableStyle, select::SelectStyle, slider::SliderStyle,
        text::TextStyle,
    },
};

/// Application-level theme: provides default styles for all built-in widgets.
///
/// Construct with [`Theme::default()`] and optionally customise with
/// [`Theme::with_accent`].
#[derive(Clone, Copy, PartialEq)]
pub struct Theme<C: Color> {
    pub bar: BarStyle<C>,
    pub button: ButtonStyle<C>,
    #[cfg(feature = "tiny-icons")]
    pub checkbox: crate::widget::checkbox::CheckboxStyle<C>,
    #[cfg(feature = "tiny-icons")]
    pub icon: crate::widget::icon::IconStyle<C>,
    pub knob: KnobStyle<C>,
    pub scrollable: ScrollableStyle<C>,
    pub select: SelectStyle<C>,
    pub slider: SliderStyle<C>,
    pub text: TextStyle<C>,
}

impl<C: Color> Default for Theme<C> {
    fn default() -> Self {
        Self {
            bar: BarStyle::base(),
            button: ButtonStyle::base(),
            #[cfg(feature = "tiny-icons")]
            checkbox: crate::widget::checkbox::CheckboxStyle::base(),
            #[cfg(feature = "tiny-icons")]
            icon: crate::widget::icon::IconStyle::base(),
            knob: KnobStyle::base(),
            scrollable: ScrollableStyle::base(),
            select: SelectStyle::base(),
            slider: SliderStyle::base(),
            text: TextStyle::base(),
        }
    }
}

impl<C: Color> Theme<C> {
    // TODO: Flutter-like seed color

    /// Apply an accent colour to all widgets that support it.
    pub fn with_accent(mut self, accent: C) -> Self {
        self.bar.color.set_high_priority(Some(accent));
        self.button.container.border.color.set_high_priority(Some(accent));
        #[cfg(feature = "tiny-icons")]
        self.checkbox.container.border.color.set_high_priority(Some(accent));
        self.knob.color.set_high_priority(Some(accent));
        self
    }
}
