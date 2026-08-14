use crate::{
    color::Color, primitives::ellipse::Ellipse, renderer::RenderResult,
    style::DrawStyle,
};
use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::PixelColor, primitives::StyledDrawable,
};

/// embedded-graphics' ellipse. See [`super::arc::draw`] for why this is a free
/// function.
pub fn draw<C: Color + PixelColor, D: DrawTarget<Color = C, Error = ()>>(
    target: &mut D,
    ellipse: &Ellipse,
    style: &DrawStyle<C>,
) -> RenderResult {
    embedded_graphics::primitives::Ellipse::new(
        ellipse.top_left.into(),
        ellipse.size.into(),
    )
    .draw_styled(&style.into_primitive_style(), target)
}
