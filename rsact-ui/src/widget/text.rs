use super::prelude::*;
use crate::font::{Font, TextHorizontalAlign, TextVerticalAlign};
use alloc::string::{String, ToString};
use layout::ContentLayout;
use rsact_reactive::{
    maybe::{IntoMaybeReactive, MaybeSignal},
    memo::{IntoMemo, Memo},
    prelude::MemoChain,
    read::SignalMap,
    signal::Signal,
};

// TODO: Actually not used and does not give any effect
/// Properties of the font that affect what font is picked.
pub struct TextProps {
    size: FontSize,
    style: FontStyle,
}

impl Default for TextProps {
    fn default() -> Self {
        todo!()
    }
}

declare_widget_style! {
    TextStyle () {
        color: color {
            transparent: transparent
        },
    }
}

impl<C: Color> TextStyle<C> {
    pub fn base() -> Self {
        Self { color: ColorStyle::DefaultForeground }
    }
}

pub struct Text<W: WidgetCtx> {
    content: Memo<String>,
    props: MaybeSignal<TextProps>,
    font: Signal<Font>,
    layout: Signal<Layout>,
    horizontal_align: TextHorizontalAlign,
    vertical_align: TextVerticalAlign,
    style: MemoChain<TextStyle<W::Color>>,
}

impl<W: WidgetCtx> Text<W> {
    pub fn fixed(content: impl ToString) -> Self {
        // TODO: Is it possible to make real MaybeReactive text content?
        Self::new(content.to_string().inert())
    }

    pub fn new(content: impl IntoMemo<String>) -> Self {
        let content = content.memo();
        let props = (TextProps::default()).maybe_signal();
        let style = TextStyle::base().memo_chain();

        let font: Signal<Font> = Font::Auto.signal();

        let layout = Layout::shrink(super::LayoutKind::Content(
            ContentLayout::text(font.memo(), content),
        ))
        .signal();

        Self {
            content,
            props,
            font,
            layout,
            style,
            horizontal_align: Default::default(),
            vertical_align: Default::default(),
        }
    }

    pub fn style(
        self,
        styler: impl Fn(TextStyle<W::Color>, ()) -> TextStyle<W::Color> + 'static,
    ) -> Self {
        self.style.last(move |base| styler(*base, ())).unwrap();
        self
    }

    pub fn font_size<S: Into<FontSize> + Clone + PartialEq + 'static>(
        mut self,
        font_size: impl IntoMaybeReactive<S>,
    ) -> Self {
        // TODO: Warn about setting font size for fixed font (like EGMonoFont) that does not have any effect on font size.
        self.props.setter(font_size.maybe_reactive(), |props, font_size| {
            props.size = font_size.clone().into();
        });

        self
    }

    pub fn font_style(
        mut self,
        font_style: impl IntoMaybeReactive<FontStyle>,
    ) -> Self {
        self.props.setter(font_style.maybe_reactive(), |props, &font_style| {
            props.style = font_style;
        });
        self
    }

    pub fn font<F: Into<Font> + PartialEq + Clone + 'static>(
        mut self,
        font: impl IntoMaybeReactive<F>,
    ) -> Self {
        self.font.setter(font.maybe_reactive(), |font, new_font| {
            *font = new_font.clone().into();
        });
        self
    }
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

        // Note: Setting inherited font is not a reactive process. If user didn't set the font, the inherited is set. But user cannot unset font, thus font never fallbacks to inherited.
        if self.font.with(|font| font.is_auto()) {
            self.font.set(Font::Inherited(ctx.inherited_font));
        }
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let content = self.content;
        let font = self.font;
        let style = self.style.get();

        with!(move |font, content| {
            ctx.fonts.draw::<W>(
                font,
                content,
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
        Text::fixed(self).el()
    }
}
