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
        // TODO(unimplemented): blit the image into the target. Degrade to a
        // logged no-op instead of `todo!()` so drawing an Image does not abort.
        log::warn!(
            "ImageDrawable::draw is not implemented for the embedded-graphics \
             backend; skipping"
        );
        Ok(())
    }

    fn draw_sub_image<D>(
        &self,
        _target: &mut D,
        _area: &embedded_graphics::primitives::Rectangle,
    ) -> Result<(), D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        // TODO(unimplemented): see `draw`. Logged no-op rather than `todo!()`.
        log::warn!(
            "ImageDrawable::draw_sub_image is not implemented for the \
             embedded-graphics backend; skipping"
        );
        Ok(())
    }
}
