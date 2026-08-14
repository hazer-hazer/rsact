use crate::{
    color::Color, primitives::line::Line, renderer::RenderResult,
    style::DrawStyle,
};
use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::PixelColor, primitives::StyledDrawable,
};

/// embedded-graphics' line. See [`super::arc::draw`] for why this is a free
/// function.
///
/// The Xiaolin Wu implementation that lived beside this is deleted, not moved:
/// `EgRasterizer` is embedded-graphics **as-is**, and rsact's own anti-aliased
/// line belongs to `RsactRasterizer`, which will emit spans rather than
/// blended pixels.
pub fn draw<C: Color + PixelColor, D: DrawTarget<Color = C, Error = ()>>(
    target: &mut D,
    line: &Line,
    style: &DrawStyle<C>,
) -> RenderResult {
    embedded_graphics::primitives::Line::new(line.from.into(), line.to.into())
        .draw_styled(&style.into_primitive_style(), target)
}
