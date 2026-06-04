use crate::{color::Color, image::ImageRef};
use embedded_graphics::{
    geometry::OriginDimensions, image::ImageDrawable, pixelcolor::PixelColor,
};

impl<'a, C: Color> OriginDimensions for ImageRef<'a, C> {
    fn size(&self) -> embedded_graphics::geometry::Size {
        self.size().into()
    }
}

impl<'a, C: Color + PixelColor> ImageDrawable for ImageRef<'a, C> {
    type Color = C;

    fn draw<D>(&self, _target: &mut D) -> Result<(), D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        todo!()
    }

    fn draw_sub_image<D>(
        &self,
        _target: &mut D,
        _area: &embedded_graphics::primitives::Rectangle,
    ) -> Result<(), D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        todo!()
    }
}
