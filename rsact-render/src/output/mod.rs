pub mod pixel;

// NOTE: `RenderTarget`, `ColorMapper` and `FinishRender` lived here — a second
// seam, in which a backend both rendered and flushed. Flushing belongs to the
// application: it owns the transport, the timing, and (since the surface is a
// loan) the pixels it takes back from `detach`. `RenderTarget` was also a
// re-declaration of embedded-graphics' `DrawTarget`, so a direct renderer
// should take that trait rather than a copy of it.

/// Convert one color representation into another.
pub trait MapColor<O> {
    fn map_color(&self) -> O;
}

impl<O: Clone> MapColor<O> for O {
    fn map_color(&self) -> O {
        self.clone()
    }
}
