use super::{alpha::AlphaDrawTarget, color::Color, Renderer};
use crate::{layout::size::Size, widget::DrawResult};
use alloc::{collections::BTreeMap, vec::Vec};
use core::{
    convert::Infallible,
    f32::{self},
};
use embedded_canvas::CanvasAt;
use embedded_graphics::{
    prelude::{Dimensions, DrawTarget, DrawTargetExt, Point},
    primitives::Rectangle,
    Pixel,
};
use embedded_graphics_core::Drawable as _;
use rsact_reactive::prelude::IntoMaybeReactive;

#[derive(Clone, Copy, Debug)]
pub enum ViewportKind {
    Fullscreen,
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
        Self { layer: 0, kind: ViewportKind::Fullscreen }
    }
}

#[derive(PartialEq, Clone)]
pub enum AntiAliasing {
    Disabled,
    Enabled,
}

#[derive(Default, Clone, PartialEq, IntoMaybeReactive)]
pub struct LayeringRendererOptions {
    anti_aliasing: Option<AntiAliasing>,
}

impl LayeringRendererOptions {
    pub fn new() -> Self {
        Self { anti_aliasing: None }
    }

    pub fn anti_aliasing(mut self, aa: AntiAliasing) -> Self {
        self.anti_aliasing = Some(aa);
        self
    }
}

// Note: Real alpha channel is not supported. Now, alpha channel is more like blending parameter for drawing on a single layer, so each layer is not transparent and alpha parameter only affects blending on current layer.
// TODO: Real alpha-channel
struct Layer<C: Color> {
    // TODO: Custom Canvas, `embedded_canvas` doesn't effectively store pixels. This is because pixels are optional, but I think some kind of packing is possible, for example 2 bits per one pixel.
    canvas: CanvasAt<C>,
}

impl<C: Color> Layer<C> {
    fn fullscreen(size: Size) -> Self {
        Self { canvas: CanvasAt::new(Point::zero(), size.into()) }
    }
}

// TODO: Possibly we can only use 2 layers for now, main and the overlaying
// one
pub struct LayeringRenderer<C: Color> {
    viewport_stack: Vec<Viewport>,
    layers: BTreeMap<usize, Layer<C>>,
    main_viewport: Size,
    options: LayeringRendererOptions,
}

impl<C: Color> LayeringRenderer<C> {
    fn viewport(&self) -> &Viewport {
        // No need for checked last, there must be always at least a single layer
        self.viewport_stack.last().unwrap()
    }

    fn layer_index(&self) -> usize {
        self.viewport().layer
    }

    fn sub_viewport(&self, kind: ViewportKind) -> Viewport {
        Viewport { layer: self.layer_index(), kind }
    }
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
        let index = self.layer_index();
        let viewport = self.viewport_stack.get(index).unwrap();
        let layer = self.layers.get_mut(&index).unwrap();
        let canvas = &mut layer.canvas;

        match viewport.kind {
            ViewportKind::Fullscreen => canvas.draw_iter(pixels),
            ViewportKind::Clipped(area) => {
                canvas.clipped(&area).draw_iter(pixels)
            },
            ViewportKind::Cropped(area) => {
                canvas.cropped(&area).draw_iter(pixels)
            },
        }
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
        let index = self.layer_index();
        let viewport = self.viewport_stack.get(index).unwrap();
        let layer = self.layers.get_mut(&index).unwrap();
        let canvas = &mut layer.canvas;

        let current = canvas.get_pixel(pixel.0);
        let color = current
            .map(|current| current.mix(blend, pixel.1))
            .unwrap_or(pixel.1);

        let pixels = core::iter::once(Pixel(pixel.0, color));
        match viewport.kind {
            ViewportKind::Fullscreen => canvas.draw_iter(pixels),
            ViewportKind::Clipped(area) => {
                canvas.clipped(&area).draw_iter(pixels)
            },
            ViewportKind::Cropped(area) => {
                canvas.cropped(&area).draw_iter(pixels)
            },
        }
        .unwrap();

        Ok(())
    }
}

impl<C: Color> Renderer for LayeringRenderer<C>
where
    C: Default,
{
    type Color = C;
    type Options = LayeringRendererOptions;

    fn new(viewport: Size) -> Self {
        Self {
            viewport_stack: vec![Viewport::root()],
            layers: BTreeMap::from([(0, Layer::fullscreen(viewport.into()))]),
            // TODO: Can avoid storing by getting main viewport from the first
            // layer in the stack
            main_viewport: viewport,
            options: LayeringRendererOptions::default(),
        }
    }

    fn set_options(&mut self, options: Self::Options) {
        self.options = options;
    }

    // TODO: Real alpha channels
    fn finish_frame(&self, target: &mut impl DrawTarget<Color = C>) {
        self.layers.iter().for_each(|(_, layer)| {
            layer.canvas.draw(target).ok().unwrap();
        });
    }

    fn clear(&mut self, color: Self::Color) -> DrawResult {
        DrawTarget::clear(self, color).ok().unwrap();
        Ok(())
    }

    fn clear_rect(
        &mut self,
        rect: Rectangle,
        color: Self::Color,
    ) -> DrawResult {
        self.fill_solid(&rect, color).ok().unwrap();
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
        self.layers.insert(index, Layer::fullscreen(self.main_viewport.into()));

        self.viewport_stack
            .push(Viewport { layer: index, kind: ViewportKind::Fullscreen });
        let result = f(self);
        self.viewport_stack.pop();

        result
    }

    fn render(
        &mut self,
        renderable: &impl super::Renderable<Self::Color>,
    ) -> DrawResult {
        if matches!(self.options.anti_aliasing, Some(AntiAliasing::Enabled)) {
            renderable.draw_alpha(self).ok().unwrap();
        } else {
            renderable.draw(self).ok().unwrap();
        }

        Ok(())
    }

    fn pixel(&mut self, pixel: Pixel<Self::Color>) -> DrawResult {
        pixel.draw(self).unwrap();
        Ok(())
    }
}
