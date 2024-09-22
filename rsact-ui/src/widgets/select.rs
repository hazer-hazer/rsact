use rsact_core::signal::Signal;

use crate::{
    el::ElId, layout::Layout, render::color::Color, style::block::BoxStyle,
    widget::WidgetCtx,
};

#[derive(Clone, Copy)]
pub struct SelectState {
    pressed: bool,
    active: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub struct SelectStyle<C: Color> {
    block: BoxStyle<C>,
}

pub struct Select<W: WidgetCtx> {
    id: ElId,
    layout: Signal<Layout>,
    state: Signal<SelectState>,
    style: Signal<SelectStyle<W::Color>>,
}
