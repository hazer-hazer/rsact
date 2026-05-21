use super::{FontSettingWidget, prelude::*};
use crate::font::{TextHorizontalAlign, TextVerticalAlign};
use alloc::string::{String, ToString};
use core::fmt::Display;
use layout::ContentLayout;
use rsact_reactive::{
    maybe::maybe_reactive::SignalMapMaybeReactive, signal::Signal,
};

declare_widget_style! {
    TextStyle () {
        color: color {
            transparent: transparent
        },
        horizontal_align: TextHorizontalAlign,
        vertical_align: TextVerticalAlign,
    }
}

impl<C: Color> TextStyle<C> {
    pub fn base() -> Self {
        Self {
            color: ColorStyle::DefaultForeground,
            horizontal_align: Default::default(),
            vertical_align: Default::default(),
        }
    }
}

pub struct Text<W: WidgetCtx> {
    content: MaybeReactive<String>,
    layout: Layout,
    style: Option<Box<dyn Fn(TextStyle<W::Color>) -> TextStyle<W::Color>>>,
}

impl<W: WidgetCtx> Text<W> {
    // TODO: 'static string optimization, can store &'static str directly without allocating String
    pub fn new(content: impl SignalMapRefMaybeReactive<str, String>) -> Self {
        let content =
            content.map_ref_maybe_reactive(|content| content.to_string());

        let layout = Layout::shrink(super::LayoutKind::Content(
            ContentLayout::text(content),
        ));

        Self { content, layout, style: None }
    }

    pub fn style(
        mut self,
        styler: impl Fn(TextStyle<W::Color>) -> TextStyle<W::Color> + 'static,
    ) -> Self {
        self.style = Some(Box::new(styler));
        self
    }

    // /// Sets fonts size by maybe reactive value.
    // /// Note that font size does nothing for fixed size fonts such as embedded_graphics MonoFont or U8G2 fonts.
    // pub fn font_size<S: Into<FontSize> + Clone + PartialEq + 'static>(
    //     mut self,
    //     font_size: impl IntoMaybeReactive<S>,
    // ) -> Self {
    //     // TODO: Warn about setting font size for fixed font (like EGMonoFont) that does not have any effect on font size.
    //     // Or try to make type state Text, like Text<W, IsReactive>/Text<W, IsInert>
    //     self.props.setter(font_size.maybe_reactive(), |props, font_size| {
    //         props.size = font_size.clone().into();
    //     });

    //     self
    // }

    // pub fn font_style(
    //     mut self,
    //     font_style: impl IntoMaybeReactive<FontStyle>,
    // ) -> Self {
    //     self.props.setter(font_style.maybe_reactive(), |props, &font_style| {
    //         props.style = font_style;
    //     });
    //     self
    // }

    // pub fn font<F: Into<Font> + PartialEq + Clone + 'static>(
    //     mut self,
    //     font: impl IntoMaybeReactive<F>,
    // ) -> Self {
    //     self.font.setter(font.maybe_reactive(), |font, new_font| {
    //         *font = new_font.clone().into();
    //     });
    //     self
    // }
}

impl<W: WidgetCtx> FontSettingWidget<W> for Text<W> {}

impl<W: WidgetCtx> Widget<W> for Text<W> {
    fn meta(&self, _: ElId) -> MetaTree {
        MetaTree::none()
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, ctx: &mut RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self("Text", |ctx| {
            #[cfg(feature = "embedded-graphics")]
            {
                let content = self.content;
                let style = ctx.get_style(|t| t.text, self.style.as_deref());
                let props = ctx.font_props;

                with!(move |content| {
                    let font = props.font();
                    let props = props.resolve(ctx.viewport.get());

                    todo!()
                    // ctx.render_font(
                    //     font,
                    //     content,
                    //     props,
                    //     ctx.layout.inner,
                    //     style.color.expect(),
                    // )
                })
            }
            #[cfg(not(feature = "embedded-graphics"))]
            Ok(())
        })
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
    }
}

impl<'a, W: WidgetCtx> Into<El<W>> for &'a str {
    fn into(self) -> El<W> {
        Text::new(self.to_string().inert()).el()
    }
}

impl<W: WidgetCtx> Into<El<W>> for String {
    fn into(self) -> El<W> {
        Text::new(self.inert()).el()
    }
}

impl<W: WidgetCtx> Into<El<W>> for Signal<String> {
    fn into(self) -> El<W> {
        Text::new(self).el()
    }
}
