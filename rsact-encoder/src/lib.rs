use rsact_ui::{
    prelude::{ButtonEvent, ButtonStyle, MonoTextStyle},
    style::WidgetStylist,
    widget::WidgetCtx,
};

extern crate alloc;

pub mod widget;

pub trait EncoderWidgetCtx: WidgetCtx
where
    // We use buttons
    Self::Styler: WidgetStylist<ButtonStyle<Self::Color>, Class = ()>,
    Self::Event: ButtonEvent,
    // We use text
    Self::Styler: WidgetStylist<MonoTextStyle<Self::Color>, Class = ()>,
{
}

impl<W, S, E> EncoderWidgetCtx for W
where
    W: WidgetCtx<Styler = S, Event = E>,
    S: WidgetStylist<ButtonStyle<Self::Color>, Class = ()>,
    E: ButtonEvent,
    S: WidgetStylist<MonoTextStyle<Self::Color>, Class = ()>,
{
}
