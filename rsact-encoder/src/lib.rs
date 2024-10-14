use rsact_ui::{
    prelude::{ButtonEvent, ButtonStyle, MonoTextStyle},
    style::Styler,
    widget::WidgetCtx,
};

extern crate alloc;

pub mod widget;

pub trait EncoderWidgetCtx: WidgetCtx
where
    // We use buttons
    Self::Styler: Styler<ButtonStyle<Self::Color>, Class = ()>,
    Self::Event: ButtonEvent,
    // We use text
    Self::Styler: Styler<MonoTextStyle<Self::Color>, Class = ()>,
{
}

impl<W, S, E> EncoderWidgetCtx for W
where
    W: WidgetCtx<Styler = S, Event = E>,
    S: Styler<ButtonStyle<Self::Color>, Class = ()>,
    E: ButtonEvent,
    S: Styler<MonoTextStyle<Self::Color>, Class = ()>,
{
}
