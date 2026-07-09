use super::{FontSettingWidget, prelude::*};
use crate::font::{TextHorizontalAlign, TextOverflow, TextVerticalAlign};
use alloc::string::{String, ToString};
use layout::ContentLayout;
use rsact_reactive::signal::Signal;

declare_widget_style! {
    LabelStyle () {
        text_color: color {
            transparent: transparent
        },
        horizontal_align: TextHorizontalAlign = TextHorizontalAlign::Left,
        vertical_align: TextVerticalAlign = TextVerticalAlign::Top,
    }
}

#[derive(View)]
pub struct Label<W: WidgetCtx> {
    content: MaybeReactive<String>,
    layout: Layout,
    style: WidgetStyleFn<LabelStyle<W::Color>>,
}

impl<W: WidgetCtx> Label<W> {
    // TODO: 'static string optimization, can store &'static str directly
    // without allocating String
    pub fn new(content: impl SignalMapRefMaybeReactive<str, String>) -> Self {
        let content =
            content.map_ref_maybe_reactive(|content| content.to_string());

        // Shrink on both axes: the label hugs its text, but because the
        // resolved width is clamped to the available space, text that exceeds
        // it wraps and the height grows (see `Limits::resolve_content_size`).
        // WS4.1: `content` (MaybeReactive<String>) is no longer `Copy`, and it
        // is needed both in the layout (for text measurement) and in the field
        // (for render). Clone the handle: for a reactive label this copies a
        // Memo handle (cheap); for a static label it clones the String once, at
        // build time. No runtime node is created either way (the double-*node*
        // is gone — `Inert::map` no longer allocates).
        let layout = Layout::shrink(super::LayoutKind::Content(
            ContentLayout::text(content.clone()),
        ));

        Self { content, layout, style: None }
    }

    pub fn style(mut self, class: impl StyleFn<LabelStyle<W::Color>>) -> Self {
        self.style = Some(Box::new(class));
        self
    }

    // TODO: MaybeReactive
    /// Set how the text behaves when its width is constrained: [`TextOverflow::Wrap`]
    /// (default), [`TextOverflow::Clip`], or [`TextOverflow::Ellipsis`].
    pub fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.layout
            .update_untracked(|data| data.set_text_overflow(overflow));
        self
    }

    /// Wrap text into the available width (the default).
    pub fn wrap(self) -> Self {
        self.overflow(TextOverflow::Wrap)
    }

    // /// Sets fonts size by maybe reactive value.
    // /// Note that font size does nothing for fixed size fonts such as
    // embedded_graphics MonoFont or U8G2 fonts. pub fn font_size<S:
    // Into<FontSize> + Clone + PartialEq + 'static>(     mut self,
    //     font_size: impl IntoMaybeReactive<S>,
    // ) -> Self {
    //     // TODO: Warn about setting font size for fixed font (like
    // EGMonoFont) that does not have any effect on font size.     // Or try
    // to make type state Text, like Text<W, IsReactive>/Text<W, IsInert>
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

impl<W: WidgetCtx> LayoutWidget<W> for Label<W> {
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}

impl<W: WidgetCtx> FontSettingWidget<W> for Label<W> {}

impl<W: WidgetCtx> Widget<W> for Label<W> {
    fn debug_name(&self) -> &'static str {
        "Label"
    }

    fn build(&mut self, _ctx: build::BuildCtx<W>) {}

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self(|mut ctx| {
            // Borrow, don't move/clone: keeps render allocation-free for both
            // reactive and (now inline) static content.
            let content = &self.content;
            let style = ctx.get_style(self.style.as_deref());
            let props = ctx.visual.font_props;

            with!(move |content| {
                let font = props.font();
                let props = props.resolve(ctx.shared.viewport.get());

                ctx.render_font(
                    font,
                    content,
                    props,
                    ctx.layout.inner,
                    // An unset text color must not panic (ColorStyle::expect is
                    // "Dangerous" per its own note): fall back to the theme's
                    // default foreground so text stays visible.
                    style
                        .text_color
                        .get()
                        .unwrap_or_else(W::default_foreground),
                )
            })
        })
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
    }
}

// I am not sure if it is a good idea to treat any text type as a Label because if we add a new text widget (for example rich text), then it will be ambiguous, better use LabelView::label like "text".label() to convert to Label explicitly.

impl<'a, W: WidgetCtx> View<W> for &'a str {
    fn into_el(self) -> El<W> {
        Label::new(self.to_string().inert()).el()
    }
}

impl<W: WidgetCtx> View<W> for String {
    fn into_el(self) -> El<W> {
        Label::new(self.inert()).el()
    }
}

impl<W: WidgetCtx> View<W> for Signal<String> {
    fn into_el(self) -> El<W> {
        Label::new(self).el()
    }
}

pub trait LabelView<W: WidgetCtx> {
    fn label(self) -> Label<W>;

    fn font_size<S: Into<FontSize> + Clone + PartialEq + 'static>(
        self,
        font_size: impl IntoMaybeReactive<S>,
    ) -> Label<W>
    where
        Self: Sized,
    {
        self.label().font_size(font_size)
    }
}

impl<'a, W: WidgetCtx> LabelView<W> for &'a str {
    fn label(self) -> Label<W> {
        Label::new(self.to_string().inert())
    }
}

impl<W: WidgetCtx> LabelView<W> for String {
    fn label(self) -> Label<W> {
        Label::new(self.inert())
    }
}

impl<W: WidgetCtx> LabelView<W> for Signal<String> {
    fn label(self) -> Label<W> {
        Label::new(self)
    }
}
