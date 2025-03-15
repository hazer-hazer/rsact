use super::{
    AntiAliasing, LayerRenderer, Renderer, RendererOptions, Viewport,
    ViewportKind,
    alpha::AlphaDrawTarget,
    canvas::{PackedColor, RawCanvas},
    color::Color,
};
use crate::{layout::size::Size, widget::DrawResult};
use alloc::{collections::BTreeMap, vec::Vec};
use core::{
    convert::Infallible,
    f32::{self},
};
use embedded_graphics::{
    Drawable, Pixel,
    prelude::{Dimensions, DrawTarget, DrawTargetExt, Point},
    primitives::Rectangle,
};
use rsact_reactive::prelude::IntoMaybeReactive;

// Note: Real alpha channel is not supported. Now, alpha channel is more like blending parameter for drawing on a single layer, so each layer is not transparent and alpha parameter only affects blending on current layer.
// TODO: Real alpha-channel
struct Layer<C: Color> {
    canvas: RawCanvas<C>,
}

impl<C: Color> Layer<C> {
    fn fullscreen(size: Size) -> Self {
        Self { canvas: RawCanvas::new(size) }
    }
}

// TODO: Possibly we can only use 2 layers for now, main and the overlaying one
pub struct LayeringRenderer<C: Color> {
    // TODO: Use tinyvec? How often new viewports are created?
    viewport_stack: Vec<Viewport>,
    // TODO: Use signed int for underlayers
    layers: BTreeMap<usize, Layer<C>>,
    // TODO: Use first element of `viewport_stack`?
    main_viewport: Size,
    options: RendererOptions,
}

impl<C: Color> Drawable for LayeringRenderer<C> {
    type Color = C;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.layers.values().try_for_each(|layer| layer.canvas.draw(target))
    }
}

impl<C: Color> LayerRenderer for LayeringRenderer<C> {
    fn on_layer(
        &mut self,
        index: usize,
        f: impl FnOnce(&mut Self) -> DrawResult,
    ) -> DrawResult {
        self.layers.insert(index, Layer::fullscreen(self.main_viewport.into()));

        self.viewport_stack
            .push(Viewport { layer: index, kind: ViewportKind::Fullscreen });
        let result = f(self);
        self.viewport_stack.pop();

        result
    }
}

impl<C: Color> LayeringRenderer<C> {
    fn viewport(&self) -> Viewport {
        // No need for checked last, there must be always at least a single layer
        self.viewport_stack.last().copied().unwrap()
    }

    fn layer_index(&self) -> usize {
        self.viewport().layer
    }

    fn sub_viewport(&self, kind: ViewportKind) -> Viewport {
        Viewport { layer: self.layer_index(), kind }
    }

    // // TODO: Real alpha channels
    // pub async fn finish_frame(&self, f: impl AsyncFn(&[C::Storage])) {
    //     // self.layers.iter().for_each(|(_, layer)| {
    //     //     // TODO
    //     //     layer.canvas.draw_buffer(&f);
    //     // });
    //     // TODO: RawCanvas is only for BufferRenderer, here we need something like embedded_canvas
    //     self.layers.get(&0).unwrap().canvas.draw_buffer(f).await;
    // }
}

impl<C: Color> Dimensions for LayeringRenderer<C> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(Point::zero(), self.main_viewport.into())
    }
}

// TODO: DrawTarget is not the valid usage as one can mistakenly draw on LayeringRenderer, but this logic is intended only for layering handling.
impl<C: Color> DrawTarget for LayeringRenderer<C> {
    type Color = C;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let viewport = self.viewport();

        viewport
            .draw_in(
                &mut self.layers.get_mut(&viewport.layer).unwrap().canvas,
                pixels,
            )
            .unwrap();

        Ok(())
    }
}

impl<C: Color> AlphaDrawTarget for LayeringRenderer<C> {
    fn pixel_alpha(
        &mut self,
        pixel: Pixel<Self::Color>,
        blend: f32,
    ) -> DrawResult {
        let viewport = self.viewport();
        let canvas = &mut self.layers.get_mut(&viewport.layer).unwrap().canvas;

        // TODO: Custom default for rgb colors. For example white or black background
        let color = canvas
            .pixel(pixel.0)
            .map(|current| current.mix(blend, pixel.1))
            .unwrap_or(pixel.1);

        viewport.draw_in(canvas, core::iter::once(Pixel(pixel.0, color)))
    }
}

impl<C: Color> Renderer for LayeringRenderer<C>
where
    C: Default,
{
    type Color = C;
    type Options = RendererOptions;

    fn new(viewport: Size) -> Self {
        Self {
            viewport_stack: vec![Viewport::root()],
            layers: BTreeMap::from([(0, Layer::fullscreen(viewport.into()))]),
            // TODO: Can avoid storing by getting main viewport from the first
            // layer in the stack
            main_viewport: viewport,
            options: RendererOptions::default(),
        }
    }

    fn set_options(&mut self, options: Self::Options) {
        self.options = options;
    }

    // fn clear(&mut self, color: Self::Color) -> DrawResult {
    //     DrawTarget::clear(self, color).ok().unwrap();
    //     Ok(())
    // }

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

    fn render(&mut self, renderable: &impl super::Renderable<C>) -> DrawResult {
        if matches!(self.options.anti_aliasing, Some(AntiAliasing::Enabled)) {
            renderable.draw_alpha(self).ok().unwrap();
        } else {
            renderable.draw(self).ok().unwrap();
        }

        Ok(())
    }
}
