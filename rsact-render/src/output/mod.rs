pub mod pixel;

// NOTE (WS6.4d): `trait RenderTarget` and `struct ColorMapper` lived here, along
// with `trait FinishRender` before them. All three were one mistake wearing
// three hats: a **second seam**.
//
// The first seam is the real one — `Renderer`, which takes primitives and puts
// them somewhere. `RenderTarget` was a second: it took what a renderer had
// already produced and moved it somewhere else, so a backend both rendered AND
// flushed. Flushing is the application's prerogative. It owns the transport, it
// owns the timing, and (once the surface became a loan rather than a
// possession) it owns the pixels too — it takes them back from `detach` and
// ships them.
//
// `RenderTarget` was also just a mirror of embedded-graphics' `DrawTarget`,
// re-declared so rsact-render could name it without depending on that crate. We
// do not need a mirror. The three renderer shapes each meet their output
// directly and differently:
//
//   - a framebuffer renderer (a `FramebufBlitter` under `RasterRenderer`)
//     draws into storage the caller lends
//     it and hands it back;
//   - a direct renderer will take a `DrawTarget` as its own parameter, using the
//     real trait rather than a copy of it;
//   - a GPU renderer produces commands and never has pixels at all.
//
// `MapColor` survives because it is not part of that seam: it is a plain color
// conversion, and it is what a future `PixmapExt::map_to_framebuffer` would use
// to bring tiny-skia's `PremultipliedColorU8` down to an embedded-friendly
// color.

/// Convert one color representation into another.
///
/// Not an output abstraction — just a conversion, which is why it outlived the
/// `RenderTarget`/`ColorMapper` pair it used to serve.
pub trait MapColor<O> {
    fn map_color(&self) -> O;
}

impl<O: Clone> MapColor<O> for O {
    fn map_color(&self) -> O {
        self.clone()
    }
}
