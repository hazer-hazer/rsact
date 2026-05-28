use crate::{
    color::Color,
    image::{ImageOwned, ImageRef},
};
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

    fn draw<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        todo!()
    }

    fn draw_sub_image<D>(
        &self,
        target: &mut D,
        area: &embedded_graphics::primitives::Rectangle,
    ) -> Result<(), D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        todo!()
    }
}
