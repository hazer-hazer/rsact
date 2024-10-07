use super::Styler;
use crate::{
    render::color::Color,
    widget::{
        button::{ButtonState, ButtonStyle},
        checkbox::{CheckboxState, CheckboxStyle},
        mono_text::MonoTextStyle,
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

impl<C: Color + 'static> Styler<ButtonStyle<C>> for AccentStyler<C> {
    type Class = ();

    fn default() -> Self::Class {
        ()
    }

    fn style(
        self,
        _class: Self::Class,
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

impl<C: Color + 'static> Styler<CheckboxStyle<C>> for AccentStyler<C> {
    type Class = ();

    fn default() -> Self::Class {
        ()
    }

    fn style(
        self,
        _class: Self::Class,
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

// TODO: Useless?
impl<C: Color + 'static> Styler<MonoTextStyle<C>> for AccentStyler<C> {
    type Class = ();

    fn default() -> Self::Class {
        ()
    }

    fn style(
        self,
        _class: Self::Class,
    ) -> impl Fn(
        MonoTextStyle<C>,
        <MonoTextStyle<C> as super::WidgetStyle>::Inputs,
    ) -> MonoTextStyle<C>
           + 'static {
        move |base, ()| base
    }
}

impl<C: Color + 'static> Styler<ScrollableStyle<C>> for AccentStyler<C> {
    type Class = ();

    fn default() -> Self::Class {
        ()
    }

    fn style(
        self,
        _class: Self::Class,
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

impl<C: Color + 'static> Styler<SelectStyle<C>> for AccentStyler<C> {
    type Class = ();

    fn default() -> Self::Class {
        ()
    }

    fn style(
        self,
        _class: Self::Class,
    ) -> impl Fn(
        SelectStyle<C>,
        <SelectStyle<C> as super::WidgetStyle>::Inputs,
    ) -> SelectStyle<C>
           + 'static {
        move |base, state| match state {
            SelectState { pressed: _, active: true } => {
                base.selected_border_color(self.accent)
            },
            _ => base,
        }
    }
}

impl<C: Color + 'static> Styler<SliderStyle<C>> for AccentStyler<C> {
    type Class = ();

    fn default() -> Self::Class {
        ()
    }

    fn style(
        self,
        _class: Self::Class,
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
