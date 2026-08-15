use crate::{
    color::Color, primitives::arc::Arc, renderer::RenderResult,
    style::DrawStyle,
};
use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::PixelColor, primitives::StyledDrawable,
};

/// embedded-graphics' arc, drawn into any draw target.
///
/// **WS layer split, PR A: a free function, not a trait method.** `EgPrimitive`
/// existed to pair this with an anti-aliased twin and to demand `pixel_alpha`
/// from the renderer; with the AA half deleted the delegation needs only a
/// [`DrawTarget`], which `BlitTarget` is — and which is why the split's
/// `EgRasterizer::arc` is this same call with a different receiver.
pub fn draw<C: Color + PixelColor, D: DrawTarget<Color = C, Error = ()>>(
    target: &mut D,
    arc: &Arc,
    style: &DrawStyle<C>,
) -> RenderResult {
    embedded_graphics::primitives::Arc::new(
        arc.top_left.into(),
        arc.diameter,
        arc.start.into(),
        arc.sweep.into(),
    )
    .draw_styled(&style.into_primitive_style(), target)
}
