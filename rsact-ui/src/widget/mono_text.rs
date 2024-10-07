use crate::{
    declare_widget_style,
    font::{FontSize, FontStyle},
    style::{ColorStyle, Styler, TreeStyled},
    widget::{prelude::*, Meta, MetaTree},
};
use alloc::string::{String, ToString};
use embedded_graphics::mono_font::{
    ascii::FONT_6X10, MonoFont, MonoTextStyleBuilder,
};
use embedded_text::{
    alignment::{HorizontalAlignment, VerticalAlignment},
    style::TextBoxStyleBuilder,
    TextBox,
};
use layout::ContentLayout;
use rsact_core::memo_chain::IntoMemoChain;

pub const MIN_MONO_WIDTH: u32 = 4;
pub const MAX_MONO_WIDTH: u32 = 10;

#[derive(Clone, Copy)]
pub struct MonoFontProps {
    size: FontSize,
    style: FontStyle,
}

fn pick_font(
    size: FontSize,
    style: FontStyle,
    viewport: Size,
) -> MonoFont<'static> {
    use embedded_graphics::mono_font::ascii::*;

    let width = size.resolve(viewport).clamp(MIN_MONO_WIDTH, MAX_MONO_WIDTH);

    match width {
        0..=4 => FONT_4X6,
        5 => FONT_5X8,
        6 => match style {
            FontStyle::Normal => FONT_6X13,
            FontStyle::Italic => FONT_6X13_ITALIC,
            FontStyle::Bold => FONT_6X13_ITALIC,
        },
        7 => match style {
            FontStyle::Normal => FONT_7X13,
            FontStyle::Italic => FONT_7X13_ITALIC,
            FontStyle::Bold => FONT_7X13_BOLD,
        },
        8 => match style {
            FontStyle::Normal => FONT_8X13,
            FontStyle::Italic => FONT_8X13_ITALIC,
            FontStyle::Bold => FONT_8X13_BOLD,
        },
        9 => match style {
            FontStyle::Normal => FONT_9X15,
            FontStyle::Italic => FONT_9X15,
            FontStyle::Bold => FONT_9X15_BOLD,
        },
        10.. => FONT_10X20,
    }
}

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

declare_widget_style! {
    MonoTextStyle () {
        text_color: color {
            transparent: transparent,
        },
        align: HorizontalAlignment,
        vertical_align: VerticalAlignment,
    }
}

impl<C: Color> TreeStyled<C> for MonoTextStyle<C> {
    fn with_tree(mut self, tree: crate::style::TreeStyle<C>) -> Self {
        self.text_color.set_low_priority(tree.text_color.get());
        self
    }
}

impl<C: Color> MonoTextStyle<C> {
    pub fn base() -> Self {
        Self {
            text_color: ColorStyle::DefaultForeground,
            align: HorizontalAlignment::Left,
            vertical_align: VerticalAlignment::Top,
        }
    }
}

pub struct MonoText<W: WidgetCtx> {
    content: Memo<String>,
    layout: Signal<Layout>,
    props: Signal<MonoFontProps>,
    font: Signal<MonoFont<'static>>,
    style: MemoChain<MonoTextStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> MonoText<W> {
    pub fn new<T: ToString + PartialEq + 'static>(
        content: impl IntoMemo<T> + 'static,
    ) -> Self {
        let font = use_signal(FONT_6X10);
        let content = content.into_memo().mapped(|content| content.to_string());

        let layout = Layout::shrink(LayoutKind::Content(ContentLayout {
            content_size: content.mapped(move |content| {
                measure_text_content_size(content, &font.get())
            }),
        }))
        .into_signal();

        Self {
            content,
            layout,
            props: use_signal(MonoFontProps {
                size: FontSize::Unset,
                style: FontStyle::Normal,
            }),
            font,
            style: MonoTextStyle::base().into_memo_chain(),
        }
    }

    pub fn style(
        self,
        style: impl Fn(MonoTextStyle<W::Color>) -> MonoTextStyle<W::Color> + 'static,
    ) -> Self {
        self.style.last(move |prev_style| style(*prev_style));
        self
    }

    pub fn font_size<T: Into<FontSize> + Copy + 'static>(
        self,
        font_size: impl MaybeSignal<T> + 'static,
    ) -> Self {
        self.props.setter(font_size.maybe_signal(), |&font_size, props| {
            props.size = font_size.into()
        });
        self
    }

    pub fn font_style(self, font_style: impl MaybeSignal<FontStyle>) -> Self {
        self.props.setter(font_style.maybe_signal(), |&font_style, props| {
            props.style = font_style
        });
        self
    }

    // pub fn style(
    //     mut self,
    //     style: impl IntoSignal<MonoTextStyle<C::Color>>,
    // ) -> Self {
    //     self.style = style.signal();
    //     self
    // }
}

impl<W: WidgetCtx + 'static> Widget<W> for MonoText<W>
where
    W::Styler: Styler<MonoTextStyle<W::Color>, Class = ()>,
{
    fn meta(&self) -> crate::widget::MetaTree {
        MetaTree::childless(Meta::none())
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, ());

        let viewport = ctx.viewport;
        let props = self.props;

        self.font.set_from(mapped!(move |viewport, props| pick_font(
            props.size,
            props.style,
            *viewport
        )));
    }

    fn layout(&self) -> Signal<crate::layout::Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<crate::layout::Layout> {
        MemoTree::childless(self.layout.into_memo())
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        let style = self.style;

        self.content.with(|content| {
            let style = style.get().with_tree(ctx.tree_style);

            ctx.renderer.mono_text(TextBox::with_textbox_style(
                content,
                ctx.layout.area,
                MonoTextStyleBuilder::new()
                    .font(&self.font.get())
                    .text_color(style.text_color.expect())
                    .build(),
                TextBoxStyleBuilder::new()
                // TODO: Style clip/only_visible/visible
                    .height_mode(embedded_text::style::HeightMode::ShrinkToText(embedded_text::style::VerticalOverdraw::Visible))
                    // .height_mode(embedded_text::style::HeightMode::Exact(
                    //     embedded_text::style::VerticalOverdraw::Visible,
                    // ))
                    .build(),
            ))
        })
    }

    fn on_event(
        &mut self,
        _ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
        W::ignore()
    }
}

impl<'a, W: WidgetCtx + 'static> IntoSignal<El<W>> for &'a str
where
    W::Styler: Styler<MonoTextStyle<W::Color>, Class = ()>,
{
    fn into_signal(self) -> Signal<El<W>> {
        MonoText::new(self.to_string()).el().into_signal()
    }
}

impl<W> From<MonoText<W>> for El<W>
where
    W::Styler: Styler<MonoTextStyle<W::Color>, Class = ()>,
    W: WidgetCtx + 'static,
{
    fn from(value: MonoText<W>) -> Self {
        El::new(value)
    }
}
