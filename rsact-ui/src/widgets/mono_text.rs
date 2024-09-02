use crate::widget::prelude::*;
use embedded_graphics::mono_font::{
    ascii::FONT_6X10, MonoFont, MonoTextStyleBuilder,
};
use embedded_text::TextBox;

// TODO: Text wrap
fn measure_text_content_size(text: &str, font: &MonoFont) -> Limits {
    let char_size = font.character_size;
    // let max_size = text.chars().fold((0u32, Size::zero()), |(max_line, size),
    // char| {

    // });

    let max_size =
        text.split(|char| char == '\n').fold(Size::zero(), |size, a| {
            if a == "\r" {
                size
            } else {
                let chars_count = a.chars().count() as u32;
                let line_len = chars_count * char_size.width
                    + chars_count.saturating_sub(1) * font.character_spacing;
                Size::new(
                    size.width.max(line_len),
                    size.height + char_size.height,
                )
            }
        });

    Limits::new(max_size, max_size)
}

pub struct MonoText<C: WidgetCtx> {
    content: Signal<alloc::string::String>,
    layout: Signal<Layout>,
    font: Signal<MonoFont<'static>>,
    style: Signal<MonoTextStyle<C::Color>>,
}

impl<C: WidgetCtx + 'static> MonoText<C> {
    pub fn new(content: impl IntoSignal<alloc::string::String>) -> Self {
        let style = MonoTextStyle::base().into_signal();
        let font = use_signal(FONT_6X10);
        let content = content.into_signal();

        let layout = Layout {
            kind: crate::layout::LayoutKind::Edge,
            size: Size::shrink(),
            box_model: BoxModel::zero(),
            content_size: content.mapped(move |content| {
                measure_text_content_size(content, &font.get())
            }),
        }
        .into_signal();

        Self { content, layout, font, style }
    }

    // pub fn style(
    //     mut self,
    //     style: impl IntoSignal<MonoTextStyle<C::Color>>,
    // ) -> Self {
    //     self.style = style.signal();
    //     self
    // }
}

impl<C: WidgetCtx + 'static> Widget<C> for MonoText<C> {
    fn layout(&self) -> Signal<crate::layout::Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<crate::layout::Layout> {
        MemoTree::childless(self.layout.into_memo())
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, C>,
    ) -> crate::widget::DrawResult {
        let style = self.style;

        self.content.with(|content| {
            let style = style.get();

            ctx.renderer.mono_text(TextBox::new(
                content,
                ctx.layout.area,
                MonoTextStyleBuilder::new()
                    .font(&self.font.get())
                    .text_color(style.text_color)
                    .build(),
            ))
        })
    }

    fn on_event(
        &mut self,
        _ctx: &mut crate::widget::EventCtx<'_, C>,
    ) -> crate::event::EventResponse<<C as WidgetCtx>::Event> {
        Propagate::Ignored.into()
    }
}

impl<'a, C: WidgetCtx + 'static> IntoSignal<El<C>> for &'a str {
    fn into_signal(self) -> Signal<El<C>> {
        MonoText::new(String::from(self)).el().into_signal()
    }
}

impl<C> From<MonoText<C>> for El<C>
where
    C: WidgetCtx + 'static,
{
    fn from(value: MonoText<C>) -> Self {
        El::new(value)
    }
}
