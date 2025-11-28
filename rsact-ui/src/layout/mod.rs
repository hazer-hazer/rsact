use crate::font::{FontCtx, FontProps, FontSize};
use alloc::{string::String, vec::Vec};
use axis::Direction;
pub use axis::{Axial as _, Axis};
use block_model::BlockModel;
use core::{
    fmt::{Debug, Display},
    u32,
};
use embedded_graphics::{
    prelude::{Point, Transform},
    primitives::Rectangle,
};
use flex::model_flex;
pub use limits::Limits;
use num::traits::SaturatingAdd;
use padding::Padding;
use rsact_reactive::prelude::*;
use size::{Length, Size};

pub mod axis;
pub mod block_model;
pub mod flex;
pub mod grid;
pub mod limits;
pub mod padding;
pub mod size;

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

#[derive(Clone, Debug, PartialEq)]
pub enum ContentLayout {
    Text { font_props: FontProps, content: Memo<String> },
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
    pub fn text(content: Memo<String>) -> Self {
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
                with!(move |content| {
                    let props = font_props.resolve(ctx.viewport);
                    let font = font_props.font();
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

#[derive(Clone, Debug, PartialEq)]
pub struct ContainerLayout {
    pub block_model: BlockModel,
    pub horizontal_align: Align,
    pub vertical_align: Align,
    pub content: Memo<Layout>,
    pub font_props: FontProps,
}

impl ContainerLayout {
    pub fn base(content: impl IntoMemo<Layout>) -> Self {
        Self {
            block_model: BlockModel::zero(),
            horizontal_align: Align::Start,
            vertical_align: Align::Start,
            content: content.memo(),
            font_props: Default::default(),
        }
    }

    pub fn block_model(mut self, block_model: BlockModel) -> Self {
        self.block_model = block_model;
        self
    }

    pub fn min_size(&self, ctx: &LayoutCtx) -> Size {
        self.content.with(|content| content.min_size(ctx))
            + self.block_model.full_padding()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlexLayout {
    pub wrap: bool,
    pub block_model: BlockModel,
    // Readonly
    pub(self) axis: Axis,
    pub gap: Size,
    pub horizontal_align: Align,
    pub vertical_align: Align,
    pub children: Memo<Vec<Memo<Layout>>>,
    pub font_props: FontProps,
}

impl FlexLayout {
    /// Default but with specific axis
    pub fn base(axis: Axis, children: Memo<Vec<Memo<Layout>>>) -> Self {
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
        self.children.with(|children| {
            children.iter().fold(Size::zero(), |min_size, child| {
                self.axis.infix(
                    min_size,
                    child.with(|child| child.min_size(ctx)),
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

#[derive(Clone, Debug, PartialEq)]
pub struct ScrollableLayout {
    pub content: Memo<Layout>,
    pub font_props: FontProps,
}

impl ScrollableLayout {
    pub fn new(content: Memo<Layout>) -> Self {
        Self { content, font_props: Default::default() }
    }

    pub fn min_size(&self, ctx: &LayoutCtx) -> Size {
        self.content.with(|content| content.min_size(ctx))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DevFlexLayout {
    // lines: Vec<Rectangle>,
    real: FlexLayout,
}

/// DevLayout preserves some initial layout properties that are not required in LayoutModel.
#[derive(Clone, Debug, PartialEq)]
pub struct DevLayout {
    pub size: Size<Length>,
    pub kind: DevLayoutKind,
}

impl DevLayout {
    pub fn new(size: Size<Length>, kind: DevLayoutKind) -> Self {
        Self { size, kind }
    }

    pub fn zero() -> Self {
        Self { size: Size::shrink(), kind: DevLayoutKind::Zero }
    }
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct DevHoveredLayout {
    #[cfg(feature = "debug-info")]
    pub layout: DevLayout,
    pub area: Rectangle,
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

#[derive(Clone, Debug, PartialEq)]
pub enum LayoutKind {
    Zero,
    Edge,
    Content(ContentLayout),
    Container(ContainerLayout),
    Flex(FlexLayout),
    Scrollable(ScrollableLayout),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    kind: LayoutKind,
    pub(crate) size: Size<Length>,
    show: Option<Memo<bool>>,
}

impl Layout {
    pub fn new(kind: LayoutKind, size: Size<Length>) -> Self {
        Self { kind, size, show: None }
    }

    pub fn zero() -> Self {
        Self::new(LayoutKind::Zero, Size::zero().into())
    }

    pub fn shrink(kind: LayoutKind) -> Self {
        Self::new(kind, Size::shrink())
    }

    pub fn edge(size: Size<Length>) -> Self {
        Self::new(LayoutKind::Edge, size)
    }

    /// Construct base scrollable layout where main axis will be shrinking and cross axis will fill. Also checks if content layout is with growing length on main axis which is disallowed.
    pub fn scrollable<Dir: Direction>(content: Memo<Layout>) -> Self {
        let content_layout_length =
            content.with(|layout| layout.size.main(Dir::AXIS));

        if content_layout_length.is_grow() {
            panic!(
                "Don't use growing Length (Div/fill) for content {} inside {} Scrollable!",
                Dir::AXIS.length_name(),
                Dir::AXIS.dir_name()
            );
        }

        Self {
            kind: LayoutKind::Scrollable(ScrollableLayout {
                content,
                font_props: Default::default(),
            }),
            size: Dir::AXIS.canon(
                Length::InfiniteWindow(Length::Shrink.try_into().unwrap()),
                Length::fill(),
            ),
            show: None,
        }
    }

    pub fn set_show(&mut self, show: Memo<bool>) {
        self.show = Some(show);
    }

    pub fn show(mut self, show: Memo<bool>) -> Self {
        self.set_show(show);
        self
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

    pub fn size(mut self, size: Size<Length>) -> Self {
        self.size = size;
        self
    }

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
}

/// Layout tree representation with real position in viewport
#[derive(Clone, Copy)]
pub struct LayoutModelNode<'a> {
    pub outer: Rectangle,
    pub inner: Rectangle,
    model: &'a LayoutModel,
}

impl<'a> Debug for LayoutModelNode<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut f = f.debug_struct("LayoutModelNode");
        f.field("inner", &self.inner);
        f.field("outer", &self.outer);
        #[cfg(feature = "debug-info")]
        f.field("dev", &self.model.dev);
        // TODO: How can I avoid collecting to vector without `field_with`?
        f.field("children", &self.children().collect::<Vec<_>>());
        f.finish()
    }
}

impl<'a> LayoutModelNode<'a> {
    pub fn translate(&self, by: Point) -> Self {
        Self {
            outer: self.outer.translate(by),
            inner: self.inner.translate(by),
            model: self.model,
        }
    }

    pub fn children(&'a self) -> impl Iterator<Item = LayoutModelNode<'a>> {
        self.model.children.iter().map(|child| child.node(self.inner))
    }

    // Note: May be slow and expensive
    pub fn dev_hover(&'a self, point: Point) -> Option<DevHoveredLayout> {
        self.children().find_map(|child| child.dev_hover(point)).or_else(|| {
            if self.outer.contains(point) {
                Some(DevHoveredLayout {
                    area: self.outer,
                    children_count: self.model.children.len(),
                    #[cfg(feature = "debug-info")]
                    layout: self.model.dev.clone(),
                })
            } else {
                None
            }
        })
    }
}

/// Layout tree representation with relative positions
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutModel {
    outer: Rectangle,
    inner: Rectangle,
    // /// Full-padding: padding + border width
    // full_padding: Padding,
    // Note: `dev` goes before `children` which is intentional to make more
    // readable pretty-printed debug
    // TODO: Make debug_assertions-only
    #[cfg(feature = "debug-info")]
    dev: DevLayout,
    children: Vec<LayoutModel>,
}

impl LayoutModel {
    pub fn new(
        inner_size: Size,
        children: Vec<LayoutModel>,
        #[cfg(feature = "debug-info")] dev: DevLayout,
    ) -> Self {
        Self {
            outer: Rectangle::new(Point::zero(), inner_size.into()),
            inner: Rectangle::new(Point::zero(), inner_size.into()),
            children,
            #[cfg(feature = "debug-info")]
            dev,
        }
    }

    /// Full padding includes padding + border size
    fn with_full_padding(mut self, full_padding: Padding) -> Self {
        let padding_size: Size = full_padding.into();

        self.inner = self.inner.translate(full_padding.top_left());

        self.outer = self.outer.resized(
            self.outer.size + padding_size.into(),
            embedded_graphics::geometry::AnchorPoint::TopLeft,
        );
        self
    }

    pub fn tree_root(&self) -> LayoutModelNode {
        LayoutModelNode { outer: self.outer, inner: self.inner, model: self }
    }

    fn node(&self, parent_inner: Rectangle) -> LayoutModelNode {
        LayoutModelNode {
            outer: self.outer.translate(parent_inner.top_left),
            inner: self.inner.translate(parent_inner.top_left),
            model: self,
        }
    }

    fn zero() -> Self {
        Self {
            outer: Rectangle::zero(),
            inner: Rectangle::zero(),
            // full_padding: Padding::zero(),
            children: vec![],
            #[cfg(feature = "debug-info")]
            dev: DevLayout::zero(),
        }
    }

    pub fn outer_size(&self) -> Size {
        self.outer.size.into()
    }

    // pub fn move_mut(&mut self, to: impl Into<Point> + Copy) -> &mut Self {
    //     self.outer.top_left = to.into();
    //     self.inner.top_left = to.into();
    //     self
    // }

    // pub fn moved(mut self, to: impl Into<Point> + Copy) -> Self {
    //     self.move_mut(to);
    //     self
    // }

    fn translate_mut(&mut self, by: impl Into<Point> + Copy) -> &mut Self {
        self.outer.top_left += by.into();
        self.inner.top_left += by.into();
        self
    }

    pub fn align_mut(
        &mut self,
        horizontal: Align,
        vertical: Align,
        free_space: Size,
    ) -> &mut Self {
        let x = match horizontal {
            Align::Start => 0,
            Align::Center => (free_space.width as i32) / 2,
            Align::End => free_space.width as i32,
        };

        let y = match vertical {
            Align::Start => 0,
            Align::Center => {
                (free_space.height as i32) / 2
                // - self.relative_area.size.height as i32 / 2;
            },
            Align::End => {
                free_space.height as i32
                // - self.relative_area.size.height as i32;
            },
        };

        self.translate_mut(Point::new(x, y));

        self
    }

    pub fn aligned(
        mut self,
        horizontal: Align,
        vertical: Align,
        parent_size: Size,
    ) -> Self {
        self.align_mut(horizontal, vertical, parent_size);
        self
    }
}

#[derive(Clone, Copy)]
pub struct LayoutCtx<'a> {
    pub fonts: &'a FontCtx,
    pub viewport: Size,
}

// TODO: Should viewport be unwrapped value as we depend modeling on viewport value?
pub fn model_layout(
    ctx: &LayoutCtx,
    layout: Memo<Layout>,
    parent_limits: Limits,
    parent_size: Size<Length>, // viewport: Memo<Size>,
) -> LayoutModel {
    layout.with(|layout| {
        if !layout.show.map(|show| show.get()).unwrap_or(true) {
            // TODO: Should be zero or skipped? Doesn't zero layout take child place in flex?
            return LayoutModel::zero();
        }

        let size = layout.size.in_parent(parent_size);

        match &layout.kind {
            // TODO: Panic or not?
            LayoutKind::Zero => LayoutModel::zero(),
            LayoutKind::Edge => {
                LayoutModel::new(
                    parent_limits.resolve_size(size, Size::zero(),None),
                    vec![],
                    #[cfg(feature = "debug-info")] DevLayout::new(size, DevLayoutKind::Edge)
                )
            }
            LayoutKind::Content(content_layout) => {
                let min_content = content_layout.min_size(ctx);

                LayoutModel::new(
                    parent_limits.resolve_size(size, min_content, None),
                    vec![],
                    #[cfg(feature = "debug-info")] DevLayout::new(
                        size,
                        DevLayoutKind::Content(content_layout.clone())
                    )
                )
            }
            LayoutKind::Container(container_layout) => {
                let ContainerLayout {
                    block_model,
                    horizontal_align,
                    vertical_align,
                    content,
                    font_props: _,
                    // TODO: Useless?
                } = container_layout;

                // let min_content = content_size.get().min();

                let full_padding = block_model.full_padding();

                // TODO: Panic or warn in case when there're more than a single
                // child

                let content_layout = model_layout(
                    ctx,
                    *content,
                    parent_limits.child_limits(size).shrink(full_padding),
                    size
                    // viewport,
                );

                let content_size = content_layout.outer_size();
                let real_size = parent_limits.resolve_size(size, content_size, Some(full_padding));
                let content_layout = content_layout
                    // .moved(full_padding.top_left())
                    .aligned(*horizontal_align, *vertical_align, real_size - content_size);

                LayoutModel::new(
                    // TODO: Generalize logic with real_size.expand/shrink and
                    // full_padding
                    real_size,
                    vec![content_layout],
                    #[cfg(feature = "debug-info")] DevLayout::new(
                        size,
                        DevLayoutKind::Container(container_layout.clone())
                    )
                ).with_full_padding(full_padding)
            }
            LayoutKind::Scrollable(scrollable_layout) => {
                // TODO: Useless?
                let ScrollableLayout { content, font_props: _ } = scrollable_layout;

                let content_layout = model_layout(
                    ctx,
                    *content,
                    parent_limits.child_limits(size),
                    size
                    // viewport,
                );

                // Note: For [`LayoutKind::Scrollable`], parent_limits are used as
                // content limits are unlimited on one axis
                // TODO: This is wrong? I changed to use its limits
                let real_size = parent_limits.resolve_size(size, content_layout.outer_size(), None);

                // extern crate std;
                // println!("parent limits: {parent_limits}, self limits: {}, child_limits: {limits}, content_size: {}", parent_limits.self_limits(size), content_layout.outer.size);

                LayoutModel::new(
                    real_size,
                    vec![content_layout],
                    #[cfg(feature = "debug-info")] DevLayout::new(
                        size,
                        DevLayoutKind::Scrollable(scrollable_layout.clone())
                    )
                )
            }
            LayoutKind::Flex(flex_layout) => { model_flex(ctx, parent_limits, flex_layout, size) }
        }
    })
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
