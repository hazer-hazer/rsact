use super::WidgetStylist;
use crate::{
    prelude::TextStyle,
    render::color::Color,
    widget::{
        bar::BarStyle,
        button::{ButtonState, ButtonStyle},
        checkbox::{CheckboxState, CheckboxStyle},
        knob::{KnobState, KnobStyle},
        scrollable::{ScrollableState, ScrollableStyle},
        select::{SelectState, SelectStyle},
        slider::{SliderState, SliderStyle},
    },
};

#[derive(Clone, Copy, PartialEq)]
pub struct AccentStyler<C: Color> {
    accent: C,
}

impl<C: Color> AccentStyler<C> {
    pub fn new(accent: C) -> Self {
        Self { accent }
    }
}

impl<C: Color + 'static> WidgetStylist<BarStyle<C>> for AccentStyler<C> {
    fn style(
        self,
    ) -> impl Fn(
        BarStyle<C>,
        <BarStyle<C> as super::WidgetStyle>::Inputs,
    ) -> BarStyle<C>
           + 'static {
        move |base, ()| base.color(self.accent)
    }
}

impl<C: Color + 'static> WidgetStylist<ButtonStyle<C>> for AccentStyler<C> {
    fn style(
        self,
    ) -> impl Fn(
        ButtonStyle<C>,
        <ButtonStyle<C> as super::WidgetStyle>::Inputs,
    ) -> ButtonStyle<C>
           + 'static {
        move |base, state| match state {
            ButtonState { pressed: true } => base.container(
                base.container.border(base.container.border.color(self.accent)),
            ),
            _ => base,
        }
    }
}

impl<C: Color + 'static> WidgetStylist<CheckboxStyle<C>> for AccentStyler<C> {
    fn style(
        self,
    ) -> impl Fn(
        CheckboxStyle<C>,
        <CheckboxStyle<C> as super::WidgetStyle>::Inputs,
    ) -> CheckboxStyle<C>
           + 'static {
        move |base, state| match state {
            CheckboxState { pressed: true } => base.container(
                base.container.border(base.container.border.color(self.accent)),
            ),
            _ => base,
        }
    }
}

impl<C: Color + 'static> WidgetStylist<KnobStyle<C>> for AccentStyler<C> {
    fn style(
        self,
    ) -> impl Fn(
        KnobStyle<C>,
        <KnobStyle<C> as super::WidgetStyle>::Inputs,
    ) -> KnobStyle<C>
           + 'static {
        move |base, state| match state {
            KnobState { pressed: _, active: true } => base.color(self.accent),
            _ => base,
        }
    }
}

// TODO: Useless?
impl<C: Color + 'static> WidgetStylist<TextStyle<C>> for AccentStyler<C> {
    fn style(
        self,
    ) -> impl Fn(
        TextStyle<C>,
        <TextStyle<C> as super::WidgetStyle>::Inputs,
    ) -> TextStyle<C>
           + 'static {
        move |base, ()| base
    }
}

impl<C: Color + 'static> WidgetStylist<ScrollableStyle<C>> for AccentStyler<C> {
    fn style(
        self,
    ) -> impl Fn(
        ScrollableStyle<C>,
        <ScrollableStyle<C> as super::WidgetStyle>::Inputs,
    ) -> ScrollableStyle<C>
           + 'static {
        move |base, state| match state {
            ScrollableState { offset: _, focus_pressed: _, active: true } => {
                base.thumb_color(self.accent)
            },
            _ => base,
        }
    }
}

impl<C: Color + 'static> WidgetStylist<SelectStyle<C>> for AccentStyler<C> {
    fn style(
        self,
    ) -> impl Fn(
        SelectStyle<C>,
        <SelectStyle<C> as super::WidgetStyle>::Inputs,
    ) -> SelectStyle<C>
           + 'static {
        move |base, state| match state {
            SelectState { pressed: _, active: true, selected: _ } => {
                base.selected_border_color(self.accent)
            },
            _ => base,
        }
    }
}

impl<C: Color + 'static> WidgetStylist<SliderStyle<C>> for AccentStyler<C> {
    fn style(
        self,
    ) -> impl Fn(
        SliderStyle<C>,
        <SliderStyle<C> as super::WidgetStyle>::Inputs,
    ) -> SliderStyle<C>
           + 'static {
        move |base, state| match state {
            SliderState { pressed: _, active: true } => {
                base.thumb_color(self.accent)
            },
            _ => base,
        }
    }
}
