use crate::{
    color::Color, primitives::sector::Sector, renderer::RenderResult,
    style::DrawStyle,
};
use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::PixelColor, primitives::StyledDrawable,
};

/// embedded-graphics' sector. See [`super::arc::draw`] for why this is a free
/// function.
pub fn draw<C: Color + PixelColor, D: DrawTarget<Color = C, Error = ()>>(
    target: &mut D,
    sector: &Sector,
    style: &DrawStyle<C>,
) -> RenderResult {
    embedded_graphics::primitives::Sector::new(
        sector.top_left.into(),
        sector.diameter,
        sector.start.into(),
        sector.sweep.into(),
    )
    .draw_styled(&style.into_primitive_style(), target)
}
