use crate::{
    color::Color, primitives::circle::Circle, renderer::RenderResult,
    style::DrawStyle,
};
use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::PixelColor, primitives::StyledDrawable,
};

/// embedded-graphics' circle. See [`super::arc::draw`] for why this is a free
/// function.
///
/// Worth keeping in mind for the split: `Rasterizer::circle` has a *default*
/// (a full-sweep arc), and `EgRasterizer` must override it with this — eg has
/// its own circle algorithm and taking the default would silently discard it.
pub fn draw<C: Color + PixelColor, D: DrawTarget<Color = C, Error = ()>>(
    target: &mut D,
    circle: &Circle,
    style: &DrawStyle<C>,
) -> RenderResult {
    embedded_graphics::primitives::Circle::new(
        circle.top_left.into(),
        circle.diameter,
    )
    .draw_styled(&style.into_primitive_style(), target)
}
