use crate::layout::length::LengthSize;
use crate::render::prelude::*;
use crate::{
    font::{FontCtx, FontProps, FontSize},
    layout::node::Layout,
};
use alloc::{string::String, vec::Vec};
use core::{
    fmt::{Debug, Display},
    u32,
};
use length::Length;
use num::traits::SaturatingAdd;
use rsact_reactive::prelude::*;

pub use limits::Limits;

pub mod flex;
pub mod grid;
pub mod length;
pub mod limits;
pub mod model;
pub mod node;

#[derive(Clone, Copy)]
pub struct LayoutCtx<'a> {
    pub fonts: &'a FontCtx,
    pub viewport: Size,
    pub font_props: FontProps,
}

#[derive(Clone, Copy, Debug, PartialEq, IntoMaybeReactive)]
pub enum Align {
    Start,
    Center,
    End,
}

impl Display for Align {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Align::Start => "Start",
            Align::Center => "Center",
            Align::End => "End",
        })
    }
}

impl Align {
    pub fn display_code(&self, axis: Axis) -> &str {
        match (self, axis) {
            (Align::Start, Axis::X) => "<",
            (Align::Start, Axis::Y) => "^",
            (Align::Center, Axis::X) => "|",
            (Align::Center, Axis::Y) => "-",
            (Align::End, Axis::X) => ">",
            (Align::End, Axis::Y) => "V",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContentLayout {
    Text { font_props: FontProps, content: MaybeReactive<String> },
    // TODO: MaybeReactive problem described in Icon widget
    Icon(Memo<FontSize>),
    Fixed(Size),
}

impl Display for ContentLayout {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ContentLayout::Text { font_props, content: _ } => {
                write!(f, "Text [{font_props}]")
            },
            ContentLayout::Icon(size) => {
                size.with(|size| write!(f, "Icon [{size}]"))
            },
            ContentLayout::Fixed(size) => write!(f, "Fixed [{size}]"),
        }
    }
}

impl ContentLayout {
    pub fn text(content: MaybeReactive<String>) -> Self {
        Self::Text { font_props: Default::default(), content }
    }

    pub fn icon(size: Memo<FontSize>) -> Self {
        Self::Icon(size)
    }

    pub fn fixed(size: Size) -> Self {
        Self::Fixed(size)
    }

    pub fn min_size(&self, ctx: &LayoutCtx) -> Size {
        match self {
            &ContentLayout::Text { font_props, content } => {
                let resolved = font_props.inherited(&ctx.font_props);
                with!(move |content| {
                    let props = resolved.resolve(ctx.viewport);
                    let font = resolved.font();
                    ctx.fonts.measure_text_size(font, content, props)
                })
                .min()
            },
            ContentLayout::Icon(memo) => {
                Size::new_equal(memo.with(|size| size.resolve(ctx.viewport)))
            },
            ContentLayout::Fixed(size) => *size,
        }
    }
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub struct ContainerLayout {
    pub block_model: BlockModel,
    pub horizontal_align: Align,
    pub vertical_align: Align,
    pub content: Layout,
    pub font_props: FontProps,
}

impl ContainerLayout {
    pub fn base(content: Layout) -> Self {
        Self {
            block_model: BlockModel::zero(),
            horizontal_align: Align::Start,
            vertical_align: Align::Start,
            content,
            font_props: Default::default(),
        }
    }

    pub fn block_model(mut self, block_model: BlockModel) -> Self {
        self.block_model = block_model;
        self
    }

    pub fn min_size(&self, ctx: &LayoutCtx) -> Size {
        let fp = self.font_props.inherited(&ctx.font_props);
        let child_ctx = LayoutCtx { font_props: fp, ..*ctx };
        self.content.with(|content| content.min_size(&child_ctx))
            + self.block_model.full_padding()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexLayout {
    pub wrap: bool,
    pub block_model: BlockModel,
    // Readonly
    pub(self) axis: Axis,
    pub gap: Size,
    pub horizontal_align: Align,
    pub vertical_align: Align,
    pub children: MaybeReactive<Vec<Layout>>,
    pub font_props: FontProps,
}

impl FlexLayout {
    /// Default but with specific axis
    pub fn base(axis: Axis, children: MaybeReactive<Vec<Layout>>) -> Self {
        Self {
            wrap: false,
            block_model: BlockModel::zero(),
            axis,
            gap: Size::zero(),
            horizontal_align: Align::Start,
            vertical_align: Align::Start,
            children,
            font_props: Default::default(),
        }
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn block_model(mut self, block_model: BlockModel) -> Self {
        self.block_model = block_model;
        self
    }

    pub fn gap(mut self, gap: Size) -> Self {
        self.gap = gap;
        self
    }

    pub fn horizontal_align(mut self, horizontal_align: Align) -> Self {
        self.horizontal_align = horizontal_align;
        self
    }

    pub fn vertical_align(mut self, vertical_align: Align) -> Self {
        self.vertical_align = vertical_align;
        self
    }

    pub fn align_main(self, align: Align) -> Self {
        match self.axis {
            Axis::X => self.horizontal_align(align),
            Axis::Y => self.vertical_align(align),
        }
    }

    pub fn align_cross(self, align: Align) -> Self {
        match self.axis {
            Axis::X => self.vertical_align(align),
            Axis::Y => self.horizontal_align(align),
        }
    }

    pub fn min_size(&self, ctx: &LayoutCtx) -> Size {
        let fp = self.font_props.inherited(&ctx.font_props);
        let child_ctx = LayoutCtx { font_props: fp, ..*ctx };
        self.children.with(|children| {
            children.iter().fold(Size::zero(), |min_size, child| {
                self.axis.infix(
                    min_size,
                    child.with(|child| child.min_size(&child_ctx)),
                    |lhs, rhs| SaturatingAdd::saturating_add(&lhs, &rhs),
                    core::cmp::max,
                )
            })
        })

        // children.fold(Limits::unlimited(), |limits, child| {
        //     let child_limits = child.layout().with(|child| child.content_size());
        //     Limits::new(
        //         axis.infix(
        //             limits.min(),
        //             child_limits.min(),
        //             |lhs: u32, rhs: u32| SaturatingAdd::saturating_add(&lhs, &rhs),
        //             core::cmp::max,
        //         ),
        //         axis.infix(
        //             limits.max(),
        //             child_limits.max(),
        //             |lhs: u32, rhs: u32| SaturatingAdd::saturating_add(&lhs, &rhs),
        //             core::cmp::max,
        //         ),
        //     )
        // })
    }
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub struct ScrollableLayout {
    pub content: Layout,
    pub font_props: FontProps,
}

impl ScrollableLayout {
    pub fn new(content: Layout) -> Self {
        Self { content, font_props: Default::default() }
    }

    pub fn min_size(&self, ctx: &LayoutCtx) -> Size {
        let fp = self.font_props.inherited(&ctx.font_props);
        let child_ctx = LayoutCtx { font_props: fp, ..*ctx };
        self.content.with(|content| content.min_size(&child_ctx))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DevFlexLayout {
    // lines: Vec<Rectangle>,
    real: FlexLayout,
}

/// DevLayout preserves some initial layout properties that are not required in LayoutModel.
#[derive(Debug, Clone, PartialEq)]
pub struct DevLayout {
    pub size: LengthSize,
    pub kind: DevLayoutKind,
}

impl DevLayout {
    pub fn new(size: LengthSize, kind: DevLayoutKind) -> Self {
        Self { size, kind }
    }

    pub fn zero() -> Self {
        Self { size: LengthSize::shrink(), kind: DevLayoutKind::Zero }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DevLayoutKind {
    Zero,
    Edge,
    Content(ContentLayout),
    Container(ContainerLayout),
    Flex(DevFlexLayout),
    Scrollable(ScrollableLayout),
}

impl DevLayoutKind {
    pub fn padding(&self) -> Option<Padding> {
        match self {
            DevLayoutKind::Zero
            | DevLayoutKind::Edge
            | DevLayoutKind::Content(_)
            | DevLayoutKind::Scrollable(_) => None,
            DevLayoutKind::Container(ContainerLayout {
                block_model, ..
            })
            | DevLayoutKind::Flex(DevFlexLayout {
                real: FlexLayout { block_model, .. },
                ..
            }) => Some(block_model.padding),
        }
    }
}

impl Display for DevLayoutKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DevLayoutKind::Zero => write!(f, "[Zero]"),
            DevLayoutKind::Edge => write!(f, "Edge"),
            DevLayoutKind::Content(content) => {
                write!(f, "Content {}", content)
            },
            DevLayoutKind::Container(ContainerLayout {
                horizontal_align,
                vertical_align,
                block_model: _,
                content: _,
                font_props: _,
            }) => write!(
                f,
                "Container align:h{},v{}",
                horizontal_align.display_code(Axis::X),
                vertical_align.display_code(Axis::Y)
            ),
            DevLayoutKind::Flex(DevFlexLayout {
                real:
                    FlexLayout {
                        wrap,
                        block_model: _,
                        axis,
                        gap,
                        horizontal_align,
                        vertical_align,
                        children: _,
                        font_props: _,
                    },
                // lines: _,
            }) => {
                write!(f, "Flex {} ", axis.flex_name())?;

                if *wrap {
                    f.write_str("wrap ")?;
                }

                if !gap.is_zero() {
                    write!(f, "gap{} ", gap)?;
                }

                write!(
                    f,
                    "align:h{}v{}",
                    horizontal_align.display_code(Axis::X),
                    vertical_align.display_code(Axis::Y)
                )
            },
            DevLayoutKind::Scrollable(ScrollableLayout {
                content: _,
                font_props: _,
            }) => {
                write!(f, "Scrollable content")
            },
        }
    }
}

// TODO: Full box model in dev tools

#[derive(Debug, PartialEq)]
pub struct DevHoveredLayout {
    #[cfg(feature = "debug-info")]
    pub layout: DevLayout,
    pub area: Rect,
    pub children_count: usize,
}

impl DevHoveredLayout {
    pub fn padding(&self) -> Option<Padding> {
        #[cfg(feature = "debug-info")]
        return self.layout.kind.padding();
        #[cfg(not(feature = "debug-info"))]
        return None;
    }
}

impl Display for DevHoveredLayout {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(feature = "debug-info")]
        write!(f, "{} ", self.layout.kind)?;

        write!(f, "{}x{}", self.area.size.width, self.area.size.height)?;

        #[cfg(feature = "debug-info")]
        write!(f, "({})", self.layout.size)?;

        if self.children_count > 0 {
            write!(f, " [{}]", self.children_count)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutKind {
    Zero,
    Edge,
    Content(ContentLayout),
    Container(ContainerLayout),
    Flex(FlexLayout),
    Scrollable(ScrollableLayout),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutData {
    kind: LayoutKind,
    pub size: LengthSize,
    show: Option<Memo<bool>>,
}

impl LayoutData {
    pub fn expect_container_mut(&mut self) -> &mut ContainerLayout {
        match &mut self.kind {
            LayoutKind::Container(container) => container,
            _ => unreachable!(),
        }
    }

    pub fn expect_flex_mut(&mut self) -> &mut FlexLayout {
        match &mut self.kind {
            LayoutKind::Flex(flex) => flex,
            _ => unreachable!(),
        }
    }

    pub fn min_size(&self, ctx: &LayoutCtx) -> Size {
        match &self.kind {
            LayoutKind::Zero => Size::zero(),
            // TODO: Wrong? Edge can be fixed
            LayoutKind::Edge => Size::zero(),
            LayoutKind::Content(content_layout) => content_layout.min_size(ctx),
            LayoutKind::Container(container_layout) => {
                container_layout.min_size(ctx)
            },
            LayoutKind::Flex(flex_layout) => flex_layout.min_size(ctx),
            LayoutKind::Scrollable(content_layout) => {
                content_layout.min_size(ctx)
            },
        }
    }

    // TODO: Panic on invalid layout kind usage in these methods?
    pub fn block_model(&self) -> BlockModel {
        match self.kind {
            LayoutKind::Zero
            | LayoutKind::Edge
            | LayoutKind::Content(..)
            | LayoutKind::Scrollable(..) => BlockModel::zero(),
            LayoutKind::Container(ContainerLayout { block_model, .. })
            | LayoutKind::Flex(FlexLayout { block_model, .. }) => block_model,
        }
    }

    pub fn font_props(&self) -> Option<FontProps> {
        match &self.kind {
            LayoutKind::Zero => None,
            LayoutKind::Edge => None,
            LayoutKind::Content(content_layout) => match content_layout {
                ContentLayout::Text { font_props, content: _ } => {
                    Some(*font_props)
                },
                ContentLayout::Icon(_) => None,
                ContentLayout::Fixed(_) => None,
            },
            LayoutKind::Container(container_layout) => {
                Some(container_layout.font_props)
            },
            LayoutKind::Flex(flex_layout) => Some(flex_layout.font_props),
            LayoutKind::Scrollable(scrollable_layout) => {
                Some(scrollable_layout.font_props)
            },
        }
    }

    pub fn font_props_mut(&mut self) -> Option<&mut FontProps> {
        match &mut self.kind {
            LayoutKind::Zero => None,
            LayoutKind::Edge => None,
            LayoutKind::Content(content_layout) => match content_layout {
                ContentLayout::Text { font_props, content: _ } => {
                    Some(font_props)
                },
                ContentLayout::Icon(_) => None,
                ContentLayout::Fixed(_) => None,
            },
            LayoutKind::Container(container_layout) => {
                Some(&mut container_layout.font_props)
            },
            LayoutKind::Flex(flex_layout) => Some(&mut flex_layout.font_props),
            LayoutKind::Scrollable(scrollable_layout) => {
                Some(&mut scrollable_layout.font_props)
            },
        }
    }

    pub fn set_border_width(&mut self, border_width: u32) {
        match &mut self.kind {
            LayoutKind::Container(ContainerLayout { block_model, .. })
            | LayoutKind::Flex(FlexLayout { block_model, .. }) => {
                block_model.border_width = border_width;
            },
            _ => {},
        }
    }

    pub fn set_padding(&mut self, padding: Padding) {
        match &mut self.kind {
            LayoutKind::Container(ContainerLayout { block_model, .. })
            | LayoutKind::Flex(FlexLayout { block_model, .. }) => {
                block_model.padding = padding;
            },
            _ => {},
        }
    }
}

impl Layout {
    pub fn new(kind: LayoutKind, size: LengthSize) -> Self {
        Self::inert(LayoutData { kind, size, show: None })
    }

    pub fn zero() -> Self {
        Self::new(LayoutKind::Zero, LengthSize::fixed_zero())
    }

    pub fn shrink(kind: LayoutKind) -> Self {
        Self::new(kind, LengthSize::shrink())
    }

    pub fn edge(size: LengthSize) -> Self {
        Self::new(LayoutKind::Edge, size)
    }

    /// Construct base scrollable layout where main axis will be shrinking and cross axis will fill. Also checks if content layout is with growing length on main axis which is disallowed.
    pub fn scrollable<Dir: Direction>(content: Layout) -> Self {
        let content_layout_length =
            content.with(|layout| layout.size.main(Dir::AXIS));

        if content_layout_length.is_grow() {
            panic!(
                "Don't use growing Length (Div/fill) for content {} inside {} Scrollable!",
                Dir::AXIS.length_name(),
                Dir::AXIS.dir_name()
            );
        }

        Self::inert(LayoutData {
            kind: LayoutKind::Scrollable(ScrollableLayout {
                content,
                font_props: Default::default(),
            }),
            size: Dir::AXIS.canon(
                Length::InfiniteWindow(Length::Shrink.try_into().unwrap()),
                Length::fill(),
            ),
            show: None,
        })
    }

    pub fn show(&mut self, show: Memo<bool>) {
        self.update_untracked(|l| l.show = Some(show));
    }

    pub fn size(mut self, size: LengthSize) -> Self {
        self.update_untracked(|l| l.size = size);
        self
    }
}

#[cfg(test)]
mod tests {
    // #[test]
    // fn flex_row() {
    //     let flex_layout = MemoTree {
    //         data: Layout {
    //             kind: super::LayoutKind::Flex(super::FlexLayout {
    //                 wrap: (),
    //                 block_model: (),
    //                 axis: super::Axis::X,
    //                 gap: Size::new_equal(5),
    //                 horizontal_align: super::Align::Center,
    //                 vertical_align: super::Align::Center,
    //                 content_size: ,
    //             }),
    //             size: Size::fill(),
    //         },
    //         children: vec![],
    //     };
    // }
}
