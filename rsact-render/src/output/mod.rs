pub mod pixel;

// NOTE: rsact does not flush. A renderer paints into the surface you lend it
// and you take that surface back with `detach`, so the transport and the timing
// are yours. A renderer that writes straight to a display should take
// embedded-graphics' `DrawTarget` rather than a trait re-declaring it.

/// Convert one color representation into another.
pub trait MapColor<O> {
    fn map_color(&self) -> O;
}

impl<O: Clone> MapColor<O> for O {
    fn map_color(&self) -> O {
        self.clone()
    }
}
