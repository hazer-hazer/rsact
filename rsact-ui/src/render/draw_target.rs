use super::{color::Color, Block, Renderer};
use crate::{layout::size::Size, widget::DrawResult};
use alloc::{collections::BTreeMap, vec::Vec};
use core::convert::Infallible;
use embedded_canvas::CanvasAt;
use embedded_graphics::{
    image::{Image, ImageRaw},
    iterator::raw::RawDataSlice,
    pixelcolor::raw::ByteOrder,
    prelude::{
        Dimensions, DrawTarget, DrawTargetExt, PixelColor, Point, PointsIter,
    },
    primitives::{
        Arc, Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle,
        RoundedRectangle, Styled, StyledDrawable as _,
    },
    Pixel,
};
use embedded_graphics_core::Drawable as _;

#[derive(Clone, Copy, Debug)]
pub enum ViewportKind {
    FullScreen,
    /// Clipped part of parent layer with absolute positions relative to screen
    /// top-left point
    Clipped(Rectangle),
    /// Part of parent layer with positions relative to this layer top-left
    /// point
    Cropped(Rectangle),
}

pub struct Viewport {
    /// It's okay to have multiple Layers pointing to the same Canvas as it can
    /// be Clipped or Cropped but not for overlaying
    layer: usize,
    kind: ViewportKind,
}

impl Viewport {
    pub fn root() -> Self {
        Self { layer: 0, kind: ViewportKind::FullScreen }
    }
}

// TODO: Possibly we can only use 2 layers for now, main and the overlaying
// one
pub struct LayeringRenderer<C: Color> {
    viewport_stack: Vec<Viewport>,
    layers: BTreeMap<usize, CanvasAt<C>>,
    main_viewport: Size,
}

impl<C: Color> LayeringRenderer<C> {
    fn viewport(&self) -> &Viewport {
        self.viewport_stack.last().unwrap()
    }

    fn layer_index(&self) -> usize {
        // No need for checked sub, there must be always at least a single layer
        self.viewport().layer
    }

    fn sub_viewport(&self, kind: ViewportKind) -> Viewport {
        Viewport { layer: self.layer_index(), kind }
    }

    #[inline(always)]
    fn pixel(&self, point: Point) -> Option<Pixel<C>> {
        self.layers.iter().rev().find_map(|layer| {
            layer.1.get_pixel(point).map(|color| Pixel(point, color))
        })
    }
}

impl<C: Color> Dimensions for LayeringRenderer<C> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(Point::zero(), self.main_viewport.into())
    }
}

impl<C: Color> DrawTarget for LayeringRenderer<C> {
    type Color = C;

    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let index = self.layer_index();
        let viewport = self.viewport_stack.get(index).unwrap();
        let layer = self.layers.get_mut(&index).unwrap();

        match viewport.kind {
            ViewportKind::FullScreen => layer.draw_iter(pixels),
            ViewportKind::Clipped(area) => {
                layer.clipped(&area).draw_iter(pixels)
            },
            ViewportKind::Cropped(area) => {
                layer.cropped(&area).draw_iter(pixels)
            },
        }
    }
}

impl<C: Color> Renderer for LayeringRenderer<C>
where
    C: Default,
{
    type Color = C;

    fn new(viewport: Size) -> Self {
        Self {
            viewport_stack: vec![Viewport::root()],
            layers: BTreeMap::from([(
                0,
                CanvasAt::new(Point::zero(), viewport.into()),
            )]),
            // TODO: Can avoid storing by getting main viewport from the first
            // layer in the stack
            main_viewport: viewport,
        }
    }

    fn finish(&self, target: &mut impl DrawTarget<Color = C>) {
        // self.layers.iter().for_each(|(_, canvas)| {
        //     canvas.draw(target).ok().unwrap();
        // });
        target
            .draw_iter(
                Rectangle::new(Point::zero(), self.main_viewport.into())
                    .points()
                    .filter_map(|point| self.pixel(point)),
            )
            .ok()
            .unwrap()
    }

    fn clear(&mut self, color: Self::Color) -> DrawResult {
        DrawTarget::clear(self, color).ok().unwrap();
        Ok(())
    }

    fn clipped(
        &mut self,
        area: Rectangle,
        f: impl FnOnce(&mut Self) -> DrawResult,
    ) -> DrawResult {
        self.viewport_stack
            .push(self.sub_viewport(ViewportKind::Clipped(area)));
        let result = f(self);
        self.viewport_stack.pop();
        result
    }

    fn on_layer(
        &mut self,
        index: usize,
        f: impl FnOnce(&mut Self) -> DrawResult,
    ) -> DrawResult {
        self.layers.insert(
            index,
            CanvasAt::new(Point::zero(), self.main_viewport.into()),
        );

        self.viewport_stack
            .push(Viewport { layer: index, kind: ViewportKind::FullScreen });
        let result = f(self);
        self.viewport_stack.pop();

        result
    }

    fn line(
        &mut self,
        line: Styled<Line, PrimitiveStyle<Self::Color>>,
    ) -> DrawResult {
        line.draw(self).ok().unwrap();
        Ok(())
    }

    fn rect(
        &mut self,
        rect: Styled<RoundedRectangle, PrimitiveStyle<Self::Color>>,
    ) -> DrawResult {
        rect.draw(self).ok().unwrap();
        Ok(())
    }

    fn block(&mut self, block: Block<Self::Color>) -> DrawResult {
        let style =
            PrimitiveStyleBuilder::new().stroke_width(block.border.width);

        let style = if let Some(border_color) = block.border.color {
            style.stroke_color(border_color)
        } else {
            style
        };

        let style = if let Some(background) = block.background {
            style.fill_color(background)
        } else {
            style
        };

        RoundedRectangle::new(
            block.rect,
            block.border.radius.into_corner_radii(block.rect.size),
        )
        .draw_styled(&style.build(), self)
        .ok()
        .unwrap();

        Ok(())
    }

    fn arc(
        &mut self,
        arc: Styled<Arc, PrimitiveStyle<Self::Color>>,
    ) -> DrawResult {
        arc.draw(self).ok().unwrap();

        Ok(())
    }

    fn mono_text<'a>(
        &mut self,
        text_box: embedded_text::TextBox<
            'a,
            embedded_graphics::mono_font::MonoTextStyle<'a, Self::Color>,
        >,
    ) -> DrawResult {
        let residual = text_box.draw(self).unwrap();

        if !residual.is_empty() {
            log::warn!("Residual text: {residual}");
        }

        Ok(())
    }

    fn image<'a, BO: ByteOrder>(
        &mut self,
        image: Image<'_, ImageRaw<'a, Self::Color, BO>>,
    ) -> DrawResult
    where
        RawDataSlice<'a, <Self::Color as PixelColor>::Raw, BO>:
            IntoIterator<Item = <Self::Color as PixelColor>::Raw>,
    {
        image.draw(self).ok().unwrap();

        Ok(())
    }

    fn pixel(&mut self, pixel: Pixel<Self::Color>) -> DrawResult {
        pixel.draw(self).ok().unwrap();
        Ok(())
    }
}
