use crate::{color::Color, output::pixel::Pixel};
use core::marker::PhantomData;

pub mod pixel;

pub trait RenderTarget {
    type Color;

    fn draw(&mut self, pixels: impl Iterator<Item = Pixel<Self::Color>>);
}

// NOTE (WS6.4d): `trait FinishRender<C>` lived here — `finish_frame` plus
// WS6.3's `finish_frame_regions` override — and was removed when the renderer
// stopped being owned by `UI`.
//
// It existed so rsact could *drive* the flush: `UI` held the renderer, so it
// also had to know how to get pixels out of one. Once the caller owns the
// renderer it owns the transport too — an embedded app takes its buffer back
// through the backend's own `detach` and ships it over its own SPI/DMA, with
// every `.await` on its side of the boundary. A trait rsact never calls is not
// an abstraction; it is a tax on every backend with no `RenderTarget` to flush
// *to* (a GPU, a command encoder, a direct-to-panel renderer).
//
// The flush code is NOT gone, only un-trait-ed: `PackedFramebuf::output` /
// `output_region` and the backends' inherent `output_regions` still stream a
// packed surface into any `RenderTarget`, which is what the simulator and the
// host goldens use. Something like this may return as a *convenience* once the
// N-buffered path has settled (roadmap 6.4d) — the point of removing it now is
// to keep the render path free of a flush concept it does not need.

pub trait MapColor<O> {
    fn map_color(&self) -> O;
}

impl<O: Clone> MapColor<O> for O {
    fn map_color(&self) -> O {
        self.clone()
    }
}

pub struct ColorMapper<C: Color, O: Color, T: RenderTarget<Color = O>> {
    target: T,
    _input: PhantomData<C>,
    _output: PhantomData<O>,
}

impl<C: Color, O: Color, T: RenderTarget<Color = O>> ColorMapper<C, O, T> {
    pub fn new(target: T) -> Self {
        Self { target, _input: PhantomData, _output: PhantomData }
    }
}

impl<C: Color, O: Color, T: RenderTarget<Color = O>> RenderTarget
    for ColorMapper<C, O, T>
where
    C: MapColor<O>,
{
    type Color = C;

    fn draw(&mut self, pixels: impl Iterator<Item = Pixel<Self::Color>>) {
        self.target
            .draw(pixels.map(|p| Pixel(p.0, p.1.map_color())));
    }
}
