use super::{
    AntiAliasing, Renderer, RendererOptions, Viewport,
    alpha::AlphaDrawTarget,
    color::Color,
    framebuf::{Framebuf as _, PackedFramebuf},
};
use crate::prelude::Size;
use alloc::vec::Vec;
use embedded_graphics::{
    Drawable,
    prelude::{Dimensions, DrawTarget},
};

pub struct BufferRenderer<C: Color> {
    viewport_stack: Vec<Viewport>,
    buf: PackedFramebuf<C>,
    main_viewport: Size,
    options: RendererOptions,
}

impl<C: Color> BufferRenderer<C> {
    pub fn new(viewport: Size) -> Self {
        Self {
            viewport_stack: vec![Viewport::root()],
            buf: PackedFramebuf::new(viewport),
            main_viewport: viewport,
            options: RendererOptions::default(),
        }
    }
}

impl<C: Color> Drawable for BufferRenderer<C> {
    type Color = C;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.buf.draw(target)
    }
}

impl<C: Color> Renderer for BufferRenderer<C> {
    type Color = C;
    type Options = RendererOptions;

    fn set_options(&mut self, options: Self::Options) {
        self.options = options;
    }

    fn clipped(
        &mut self,
        area: embedded_graphics::primitives::Rectangle,
        f: impl FnOnce(&mut Self) -> crate::prelude::DrawResult,
    ) -> crate::prelude::DrawResult {
        self.viewport_stack.push(Viewport {
            layer: 0,
            kind: super::ViewportKind::Clipped(area),
        });
        let result = f(self);
        self.viewport_stack.pop();
        result
    }

    fn render(
        &mut self,
        renderable: &impl super::Renderable<C>,
    ) -> crate::prelude::DrawResult {
        if matches!(self.options.anti_aliasing, Some(AntiAliasing::Enabled)) {
            renderable.draw_alpha(self).ok().unwrap();
        } else {
            renderable.draw(self).ok().unwrap();
        }
        Ok(())
    }
}

impl<C: Color> Dimensions for BufferRenderer<C> {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        embedded_graphics::primitives::Rectangle::new(
            embedded_graphics::geometry::Point::zero(),
            self.main_viewport.into(),
        )
    }
}

impl<C: Color> DrawTarget for BufferRenderer<C> {
    type Color = C;
    type Error = <PackedFramebuf<C> as DrawTarget>::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        self.viewport().draw_in(&mut self.buf, pixels)
    }
}

impl<C: Color> AlphaDrawTarget for BufferRenderer<C> {
    fn pixel_alpha(
        &mut self,
        pixel: embedded_graphics::Pixel<Self::Color>,
        blend: f32,
    ) -> crate::prelude::DrawResult {
        let color = self
            .buf
            .pixel(pixel.0)
            .map(|current| current.mix(blend, pixel.1))
            .unwrap_or(pixel.1);

        self.viewport().draw_in(
            &mut self.buf,
            core::iter::once(embedded_graphics::Pixel(pixel.0, color)),
        )
    }
}

impl<C: Color> BufferRenderer<C> {
    fn viewport(&self) -> Viewport {
        self.viewport_stack.last().copied().unwrap()
    }

    pub fn draw_buffer(
        &self,
        f: impl FnOnce(&[<C as super::framebuf::PackedColor>::Storage]),
    ) {
        self.buf.draw_buffer(f);
    }
}
