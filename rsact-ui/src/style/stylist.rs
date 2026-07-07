use crate::{
    style::{Style, StyleSelector},
    widget::{
        bar::BarStyle, button::ButtonStyle, checkbox::CheckboxStyle,
        container::ContainerStyle, edge::EdgeStyle, knob::KnobStyle,
        label::LabelStyle, scrollable::ScrollableStyle, select::SelectStyle,
        slider::SliderStyle,
    },
};
use core::marker::PhantomData;
use rsact_render::{color::Color, renderer::NullColor};

// TODO: We can implement Stylist<S: Style> for T where T:
// ReadSignal<Stylist<S>> so that user can make reactive styles without much
// effort. But the problem is that this would make coarse-grained reactivity, so
// whole page will reload on any change. But root styles updates are not
// expected to happen frequently, it is common only for light/dark theme change
// or user preferences changes, for real fine-grained reactive styles user sets
// style functions in widgets.

pub trait Stylist<S: Style> {
    fn style(&self, base: &S, selector: &StyleSelector) -> S;
}

pub trait InternalStylist<C: Color>:
    Stylist<BarStyle<C>>
    + Stylist<ButtonStyle<C>>
    + Stylist<CheckboxStyle<C>>
    + Stylist<ContainerStyle<C>>
    + Stylist<EdgeStyle<C>>
    + Stylist<KnobStyle<C>>
    + Stylist<LabelStyle<C>>
    + Stylist<ScrollableStyle<C>>
    + Stylist<SelectStyle<C>>
    + Stylist<SliderStyle<C>>
{
}

pub struct InheritedStylist<S: Style, PS, CS>
where
    PS: Stylist<S>,
    CS: Stylist<S>,
{
    parent: PS,
    child: CS,
    _style: PhantomData<S>,
}

impl<S, PS, CS> Stylist<S> for InheritedStylist<S, PS, CS>
where
    S: Style,
    PS: Stylist<S>,
    CS: Stylist<S>,
{
    fn style(&self, base: &S, selector: &StyleSelector) -> S {
        self.child
            .style(&self.parent.style(base, selector), selector)
    }
}

// Test harness //

macro_rules! declare_null_stylist {
    ($($style: ty),* $(,)?) => {
        $(
            impl Stylist<$style> for () {
                fn style(&self, _base: &$style, _selector: &StyleSelector) -> $style {
                    <$style as $crate::style::Style>::base()
                }
            }
        )*
    };
}

declare_null_stylist!(
    BarStyle<NullColor>,
    ButtonStyle<NullColor>,
    CheckboxStyle<NullColor>,
    ContainerStyle<NullColor>,
    EdgeStyle<NullColor>,
    KnobStyle<NullColor>,
    LabelStyle<NullColor>,
    ScrollableStyle<NullColor>,
    SelectStyle<NullColor>,
    SliderStyle<NullColor>,
);

impl InternalStylist<NullColor> for () {}
