use crate::{
    el::ElId,
    event::EventResponse,
    widget::{MetaTree, Widget, WidgetCtx, prelude::*},
};
use rsact_reactive::signal::{IntoSignal, Signal};

use super::ctx::EventCtx;

#[cfg(feature = "embedded-graphics")]
pub use eg_image::Image;

#[cfg(feature = "embedded-graphics")]
mod eg_image {
    use super::EventCtx;
    use crate::{
        el::ElId,
        event::EventResponse,
        widget::{MetaTree, Widget, WidgetCtx, prelude::*},
    };
    use embedded_graphics::{
        image::ImageRaw, iterator::raw::RawDataSlice,
        pixelcolor::raw::ByteOrder, prelude::*,
    };
    use rsact_reactive::signal::{IntoSignal, Signal};

    /// Static Image
    pub struct Image<'a, W: WidgetCtx, BO: ByteOrder>
    where
        W::Color: embedded_graphics::prelude::PixelColor
            + From<<W::Color as embedded_graphics::prelude::PixelColor>::Raw>,
    {
        // TODO: Reactive?
        data: ImageRaw<'a, W::Color, BO>,
        layout: Layout,
    }

    impl<'a, W: WidgetCtx, BO: ByteOrder> Image<'a, W, BO>
    where
        W::Color: embedded_graphics::prelude::PixelColor
            + From<<W::Color as embedded_graphics::prelude::PixelColor>::Raw>,
    {
        pub fn new(data: ImageRaw<'a, W::Color, BO>) -> Self {
            let size = data.size().into();

            Self { data, layout: Layout::edge(size) }
        }
    }

    impl<'a, W: WidgetCtx, BO: ByteOrder> Widget<W> for Image<'a, W, BO>
    where
        W::Color: embedded_graphics::prelude::PixelColor
            + From<<W::Color as embedded_graphics::prelude::PixelColor>::Raw>,
        RawDataSlice<'a, <W::Color as PixelColor>::Raw, BO>:
            IntoIterator<Item = <W::Color as PixelColor>::Raw>,
    {
        fn meta(&self, _: ElId) -> MetaTree {
            MetaTree::none()
        }

        fn layout(&self) -> Layout {
            self.layout
        }

        fn render(
            &self,
            ctx: &mut crate::widget::RenderCtx<'_, W>,
        ) -> crate::widget::RenderResult {
            ctx.render_self("Image", |ctx| {
                use embedded_graphics::prelude::DrawTarget as _;
                let eg_top_left: embedded_graphics::geometry::Point =
                    ctx.layout.inner.top_left.into();
                embedded_graphics::image::Image::new(&self.data, eg_top_left)
                    .draw(ctx.renderer())
                    .map_err(|_| ())
            })
        }

        fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
            ctx.ignore()
        }
    }
}
