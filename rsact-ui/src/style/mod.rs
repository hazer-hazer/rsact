use crate::render::color::Color;

pub mod block;
pub mod text;
pub mod theme;

// pub struct Style<S> {
//     style: Signal<S>,
// }

// impl<S: 'static> Style<S> {
//     pub fn new(style: S) -> Self {
//         Self { style: use_signal(style) }
//     }
// }

pub trait WidgetStyle: PartialEq + Clone {
    type Color: Color;
    type Inputs;
}

#[derive(Default)]
pub struct NullStyler;

impl<S: WidgetStyle> Styler<S> for NullStyler
where
    S: Clone,
{
    type Class = ();

    fn default() -> Self::Class {
        ()
    }

    fn style(
        self,
        _inputs: Self::Class,
    ) -> impl Fn(S, <S as WidgetStyle>::Inputs) -> S + 'static {
        move |style, _| style.clone()
    }
}

pub trait Styler<S: WidgetStyle> {
    type Class;

    fn default() -> Self::Class;
    fn style(self, class: Self::Class) -> impl Fn(S, S::Inputs) -> S + 'static;
}

// impl<S: WidgetStyle, F> Styler<S> for F
// where
//     F: Fn(S, S::Inputs) -> S + 'static,
// {
//     type Class = ();

//     fn default() -> Self::Class {
//         ()
//     }

//     fn style(
//         self,
//         _class: Self::Class,
//     ) -> impl Fn(S, S::Inputs) -> S + 'static {
//         self
//     }
// }

// impl<S: WidgetStyle> Styler<S> for S {}
