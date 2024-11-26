use crate::{
    declare_widget_style,
    font::{FontSize, FontStyle},
    render::Renderable,
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
use rsact_reactive::{maybe::MaybeReactive, memo_chain::IntoMemoChain};

pub const MIN_MONO_HEIGHT: u32 = 6;
pub const MAX_MONO_HEIGHT: u32 = 20;

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

    let height = size.resolve(viewport).clamp(MIN_MONO_HEIGHT, MAX_MONO_HEIGHT);

    match height {
        0..=6 => FONT_4X6,
        7 => FONT_5X7,
        8 => FONT_5X8,
        9 => FONT_6X9,
        10 => FONT_6X10,
        11 | 12 => FONT_6X12,
        // 13 => match style {
        //     FontStyle::Normal => FONT_6X13,
        //     FontStyle::Italic => FONT_6X13_ITALIC,
        //     FontStyle::Bold => FONT_6X13_ITALIC,
        // },
        // 13 => match style {
        //     FontStyle::Normal => FONT_7X13,
        //     FontStyle::Italic => FONT_7X13_ITALIC,
        //     FontStyle::Bold => FONT_7X13_BOLD,
        // },
        // Note: 8/13 is a better ratio, closer to 0.6
        13 | 14 => match style {
            FontStyle::Normal => FONT_8X13,
            FontStyle::Italic => FONT_8X13_ITALIC,
            FontStyle::Bold => FONT_8X13_BOLD,
        },
        15 => match style {
            FontStyle::Normal => FONT_9X15,
            FontStyle::Italic => FONT_9X15,
            FontStyle::Bold => FONT_9X15_BOLD,
        },
        16 | 17 | 18 => match style {
            FontStyle::Normal => FONT_9X18,
            FontStyle::Italic => FONT_9X18, // Note: No italic
            FontStyle::Bold => FONT_9X18_BOLD,
        },
        19.. => FONT_10X20,
    }
}

// TODO: Text wrap
fn measure_text_content_size(text: &str, font: &MonoFont) -> Limits {
    let char_size = font.character_size;

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
    content: MaybeReactive<String>,
    layout: Signal<Layout>,
    props: MaybeSignal<MonoFontProps>,
    font: Signal<MonoFont<'static>>,
    style: MemoChain<MonoTextStyle<W::Color>>,
}

impl<W: WidgetCtx + 'static> MonoText<W> {
    pub fn new_static<T: ToString + PartialEq + 'static>(content: T) -> Self {
        Self::new_inner(content.to_string().inert().into())
    }

    pub fn new<T: ToString + Clone + PartialEq + 'static>(
        content: impl Into<MaybeReactive<T>>,
    ) -> Self {
        Self::new_inner(content.into().map(|content| content.to_string()))
    }

    fn new_inner(content: MaybeReactive<String>) -> Self {
        let font = create_signal(FONT_6X10);

        let layout = Layout::shrink(LayoutKind::Content(ContentLayout {
            content_size: content.map(move |content| {
                measure_text_content_size(content, &font.get())
            }),
        }))
        .signal();

        Self {
            content,
            layout,
            props: MonoFontProps {
                size: FontSize::Unset,
                style: FontStyle::Normal,
            }
            .into(),
            font,
            style: MonoTextStyle::base().memo_chain(),
        }
    }

    pub fn style(
        self,
        style: impl Fn(MonoTextStyle<W::Color>) -> MonoTextStyle<W::Color> + 'static,
    ) -> Self {
        self.style.last(move |prev_style| style(*prev_style)).unwrap();
        self
    }

    // pub fn font_size<T: Into<FontSize> + Copy + 'static>(
    //     self,
    //     font_size: impl MaybeSignal<T> + 'static,
    // ) -> Self {
    //     self.props.setter(font_size.maybe_signal(), |&font_size, props| {
    //         props.size = font_size.into()
    //     });
    //     self
    // }

    // pub fn font_style(self, font_style: impl MaybeSignal<FontStyle>) -> Self {
    //     self.props.setter(font_style.maybe_signal(), |&font_style, props| {
    //         props.style = font_style
    //     });
    //     self
    // }

    pub fn font_size<T: Into<FontSize> + Copy + PartialEq + 'static>(
        mut self,
        font_size: impl Into<MaybeReactive<T>>,
    ) -> Self {
        self.props.setter(font_size.into(), |props, &font_size| {
            props.size = font_size.into();
        });
        self
    }

    pub fn font_style(
        mut self,
        font_style: impl Into<MaybeReactive<FontStyle>>,
    ) -> Self {
        self.props.setter(font_style.into(), |props, &font_style| {
            props.style = font_style;
        });
        self
    }
}

impl<W: WidgetCtx + 'static> Widget<W> for MonoText<W>
where
    W::Styler: Styler<MonoTextStyle<W::Color>, Class = ()>,
{
    fn meta(&self) -> crate::widget::MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, ());

        let viewport = ctx.viewport;
        let props = self.props.get();

        // self.font.set_from(mapped!(move |viewport, props| pick_font(
        //     props.size,
        //     props.style,
        //     *viewport
        // )));

        // TODO: Not reactive, font must be a computed
        self.font.update_untracked(|font| {
            *font = pick_font(props.size, props.style, viewport.get())
        });
    }

    fn layout(&self) -> Signal<crate::layout::Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<crate::layout::Layout> {
        MemoTree::childless(self.layout.memo())
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        let style = self.style.get().with_tree(ctx.tree_style);

        self.content.with(|content| {
            TextBox::with_textbox_style(
                content,
                ctx.layout.inner,
                MonoTextStyleBuilder::new()
                    .font(&self.font.get())
                    .text_color(style.text_color.expect())
                    .build(),
                TextBoxStyleBuilder::new()
                // TODO: Style clip/only_visible/visible
                    .height_mode(embedded_text::style::HeightMode::ShrinkToText(embedded_text::style::VerticalOverdraw::Visible))
                    .build(),
            ).render(ctx.renderer)
        })
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
        ctx.ignore()
    }
}

impl<'a, W: WidgetCtx + 'static> Into<El<W>> for &'a str
where
    W::Styler: Styler<MonoTextStyle<W::Color>, Class = ()>,
{
    fn into(self) -> El<W> {
        MonoText::new_static(self.to_string()).el()
    }
}

impl<W: WidgetCtx + 'static> Into<El<W>> for String
where
    W::Styler: Styler<MonoTextStyle<W::Color>, Class = ()>,
{
    fn into(self) -> El<W> {
        MonoText::new_static(self).el()
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
