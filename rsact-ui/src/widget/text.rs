use super::{FontSettingWidget, prelude::*};
use crate::font::{TextHorizontalAlign, TextVerticalAlign};
use alloc::string::{String, ToString};
use layout::ContentLayout;
use rsact_reactive::{
    memo::{IntoMemo, Memo},
    prelude::MemoChain,
    signal::Signal,
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
    content: Memo<String>,
    layout: Layout,
    style: MemoChain<TextStyle<W::Color>>,
}

impl<W: WidgetCtx> Text<W> {
    pub fn new_inert(content: impl ToString) -> Self {
        // TODO: Is it possible to make real MaybeReactive text content?
        // The problem is that ContentLayout needs to know the content. One possible solution is to create
        Self::new(content.to_string().inert())
    }

    pub fn new(content: impl IntoMemo<String>) -> Self {
        let content = content.memo();
        let style = TextStyle::base().memo_chain();

        let layout = Layout::shrink(super::LayoutKind::Content(
            ContentLayout::text(content),
        ));

        Self { content, layout, style }
    }

    pub fn style(
        self,
        styler: impl Fn(TextStyle<W::Color>, ()) -> TextStyle<W::Color> + 'static,
    ) -> Self {
        self.style.last(move |base| styler(*base, ())).unwrap();
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

impl<W: WidgetCtx> FontSettingWidget<W> for Text<W> where
    W::Styler: WidgetStylist<TextStyle<W::Color>>
{
}

impl<W: WidgetCtx> Widget<W> for Text<W>
where
    W::Styler: WidgetStylist<TextStyle<W::Color>>,
{
    fn meta(&self) -> MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn on_mount(&mut self, ctx: MountCtx<W>) {
        ctx.accept_styles(self.style, ());
        ctx.inherit_font_props(&mut self.layout);
    }

    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }

    fn render(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let content = self.content;
        let style = self.style.get();
        let props = self.font_props();

        with!(move |content| {
            let font = props.font();
            let props = props.resolve(ctx.viewport.get());

            ctx.fonts.draw::<W>(
                font,
                content,
                props,
                ctx.layout.inner,
                style.color.expect(),
                ctx.renderer,
            )
        })
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
    }
}

impl<'a, W: WidgetCtx> Into<El<W>> for &'a str
where
    W::Styler: WidgetStylist<TextStyle<W::Color>>,
{
    fn into(self) -> El<W> {
        Text::new_inert(self).el()
    }
}

impl<W: WidgetCtx> Into<El<W>> for Text<W>
where
    W::Styler: WidgetStylist<TextStyle<W::Color>>,
{
    fn into(self) -> El<W> {
        self.el()
    }
}

impl<W: WidgetCtx> Into<El<W>> for String
where
    W::Styler: WidgetStylist<TextStyle<W::Color>>,
{
    fn into(self) -> El<W> {
        Text::new_inert(self).el()
    }
}

impl<W: WidgetCtx> Into<El<W>> for Signal<String>
where
    W::Styler: WidgetStylist<TextStyle<W::Color>>,
{
    fn into(self) -> El<W> {
        Text::new(self).el()
    }
}
