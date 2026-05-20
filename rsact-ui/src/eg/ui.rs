use crate::{
    page::id::PageId,
    render::color::Color,
    style::theme::Theme,
    ui::{NoPages, UI},
    widget::ctx::Wtf,
};
use core::fmt::Debug;
use rsact_reactive::prelude::*;
use rsact_render::{
    eg::{framebuf::PackedColor, renderer::EGRenderer},
    prelude::*,
};

impl<C, I, E> UI<Wtf<EGRenderer<C>, I, E>, NoPages>
where
    I: PageId + 'static,
    E: Debug + 'static,
    C: Color
        + PackedColor
        + embedded_graphics::prelude::PixelColor
        + From<<C as embedded_graphics::prelude::PixelColor>::Raw>
        + 'static,
{
    pub fn new_with_buffer_renderer<
        V: PartialEq + Into<Size> + Copy + 'static,
    >(
        viewport: impl IntoMaybeReactive<V>,
        theme: Theme<C>,
        default_background: C,
    ) -> Self {
        let viewport =
            viewport.maybe_reactive().map(|&viewport| viewport.into());

        Self::new(viewport, theme, EGRenderer::new(viewport.get()))
    }

    pub fn new_with_layer_renderer<
        V: PartialEq + Into<Size> + Copy + 'static,
    >(
        viewport: impl IntoMaybeReactive<V>,
        theme: Theme<C>,
    ) -> Self {
        let viewport =
            viewport.maybe_reactive().map(|&viewport| viewport.into());

        Self::new(viewport, theme, EGRenderer::new(viewport.get()))
    }
}
