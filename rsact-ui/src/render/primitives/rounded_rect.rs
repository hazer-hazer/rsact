use embedded_graphics::{
    prelude::{Dimensions, Point, Primitive, Transform},
    primitives::{PrimitiveStyle, Rectangle, StyledDrawable},
};

use crate::{
    prelude::{BorderRadius, Color},
    render::alpha::StyledAlphaDrawable,
};

#[derive(Debug, Clone, Copy)]
pub struct RoundedRect {
    rect: Rectangle,
    corners: BorderRadius,
}

impl RoundedRect {
    pub fn new(rect: Rectangle, corners: BorderRadius) -> Self {
        Self { rect, corners }
    }
}

impl Dimensions for RoundedRect {
    fn bounding_box(&self) -> Rectangle {
        self.rect
    }
}

impl Primitive for RoundedRect {}

impl Transform for RoundedRect {
    fn translate(&self, by: Point) -> Self {
        Self::new(self.rect.translate(by), self.corners)
    }

    fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.rect.translate_mut(by);
        self
    }
}

impl<C: Color> StyledDrawable<PrimitiveStyle<C>> for RoundedRect {
    type Color = C;
    type Output = ();

    fn draw_styled<D>(
        &self,
        style: &PrimitiveStyle<C>,
        target: &mut D,
    ) -> Result<Self::Output, D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        embedded_graphics::primitives::RoundedRectangle::new(
            self.rect,
            self.corners.into_corner_radii(self.rect.size),
        )
        .draw_styled(style, target)
    }
}

impl<C: Color> StyledAlphaDrawable<PrimitiveStyle<C>> for RoundedRect {
    type Color = C;
    type Output = ();

    fn draw_styled_alpha<D>(
        &self,
        style: &PrimitiveStyle<C>,
        target: &mut D,
    ) -> crate::prelude::DrawResult
    where
        D: crate::render::alpha::AlphaDrawTarget<Color = Self::Color>,
    {
        // TODO
        embedded_graphics::primitives::RoundedRectangle::new(
            self.rect,
            self.corners.into_corner_radii(self.rect.size),
        )
        .draw_styled(style, target)
        .ok()
        .unwrap();
        Ok(())
    }
}
