use crate::{color::Color, geometry::Rect, output::pixel::Pixel};
use core::marker::PhantomData;

pub mod pixel;

pub trait RenderTarget {
    type Color;

    fn draw(&mut self, pixels: impl Iterator<Item = Pixel<Self::Color>>);
}

pub trait FinishRender<C> {
    fn finish_frame(&mut self, target: &mut impl RenderTarget<Color = C>);

    /// WS6.3: flush only the given damage `regions` to `target` (the damage-
    /// driven flush). The default **ignores `regions` and flushes the whole
    /// frame** — always correct (a full flush is a superset of any damage set),
    /// just not the SPI win; region-aware backends (EG, tiny-skia) override this
    /// to stream only the pixels inside the regions. Each region is clamped to
    /// the viewport by the backend. Overlapping regions may flush a pixel more
    /// than once (harmless — same value); callers that care pre-join them (the
    /// LVGL-style joined-areas list, WS6.2). An empty slice flushes nothing.
    fn finish_frame_regions(
        &mut self,
        target: &mut impl RenderTarget<Color = C>,
        regions: &[Rect],
    ) {
        let _ = regions;
        self.finish_frame(target);
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Point, Size};

    struct NoopTarget;
    impl RenderTarget for NoopTarget {
        type Color = ();
        fn draw(&mut self, _pixels: impl Iterator<Item = Pixel<()>>) {}
    }

    /// Only implements `finish_frame`; leans on the defaulted
    /// `finish_frame_regions`.
    struct FullFlushOnly {
        frames: u32,
    }
    impl FinishRender<()> for FullFlushOnly {
        fn finish_frame(
            &mut self,
            _target: &mut impl RenderTarget<Color = ()>,
        ) {
            self.frames += 1;
        }
    }

    /// WS6.3 safety contract: a backend that does NOT override
    /// `finish_frame_regions` still flushes correctly — the default ignores the
    /// regions and does one full `finish_frame` (a full flush is a superset of
    /// any damage set, so it can never be wrong, only unoptimised).
    #[test]
    fn default_regions_flush_falls_back_to_full_frame() {
        let mut finisher = FullFlushOnly { frames: 0 };
        let mut target = NoopTarget;
        let regions = [Rect::new(Point::zero(), Size::new(2, 2)), Rect::zero()];

        finisher.finish_frame_regions(&mut target, &regions);

        assert_eq!(
            finisher.frames, 1,
            "default finish_frame_regions must delegate to one full finish_frame"
        );
    }
}
