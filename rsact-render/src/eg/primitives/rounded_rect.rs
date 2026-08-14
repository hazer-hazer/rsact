use crate::{
    color::Color, primitives::rounded_rect::RoundedRect,
    renderer::RenderResult, style::DrawStyle,
};
use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::PixelColor, primitives::StyledDrawable,
};

/// embedded-graphics' rounded rectangle. See [`super::arc::draw`] for why this
/// is a free function.
///
/// The hand-written anti-aliased version deleted here carried a
/// `TODO: Bad ellipse drawing with stroke_width > 1` and a cross-fill it
/// described as "there must be a better way". Neither is lost work worth
/// migrating: `RsactRasterizer` is where rsact writes its own, and it will do
/// it span-wise.
pub fn draw<C: Color + PixelColor, D: DrawTarget<Color = C, Error = ()>>(
    target: &mut D,
    rounded_rect: &RoundedRect,
    style: &DrawStyle<C>,
) -> RenderResult {
    embedded_graphics::primitives::RoundedRectangle::new(
        rounded_rect.rect.into(),
        rounded_rect.corners.into(),
    )
    .draw_styled(&style.into_primitive_style(), target)
}
