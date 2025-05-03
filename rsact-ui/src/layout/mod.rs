use crate::font::{FontCtx, FontProps, FontSize};
use alloc::{string::String, vec::Vec};
pub use axis::{Axial as _, Axis};
use axis::{Axial, Direction};
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Align2 {
    pub x: Align,
    pub y: Align,
}

impl Align2 {
    pub fn new_equal(align: Align) -> Self {
        Self { x: align, y: align }
    }

    pub fn center() -> Self {
        Self::new_equal(Align::Center)
    }

    pub fn is_start(&self) -> bool {
        self.x == Align::Start && self.x == self.y
    }
}

impl Display for Align2 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "h{},v{}",
            self.x.display_code(Axis::X),
            self.y.display_code(Axis::Y)
        )
    }
}

impl Default for Align2 {
    fn default() -> Self {
        Self { x: Align::Center, y: Align::Center }
    }
}

impl Axial for Align2 {
    type Data = Align;

    fn x(&self) -> Self::Data {
        self.x
    }

    fn y(&self) -> Self::Data {
        self.y
    }

    fn x_mut(&mut self) -> &mut Self::Data {
        &mut self.x
    }

    fn y_mut(&mut self) -> &mut Self::Data {
        &mut self.y
    }

    fn axial_new(x: Self::Data, y: Self::Data) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContentLayout {
    Text { font_props: Signal<FontProps>, content: Memo<String> },
    Icon(Memo<FontSize>),
    Fixed(Size),
}

impl Display for ContentLayout {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ContentLayout::Text { font_props, content: _ } => {
                font_props.with(|font_props| write!(f, "Text [{}]", font_props))
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
        Self::Text { font_props: FontProps::default().signal(), content }
    }

    pub fn icon(size: Memo<FontSize>) -> Self {
        Self::Icon(size)
    }

    pub fn fixed(size: Size) -> Self {
        Self::Fixed(size)
    }

    pub fn min_size(&self, ctx: LayoutCtx) -> Size {
        match self {
            &ContentLayout::Text { font_props, content } => {
                let viewport = ctx.viewport;
                let fonts = ctx.fonts;
                with!(move |content, viewport, fonts, font_props| {
                    let props = font_props.resolve(*viewport);
                    let font = font_props.font();
                    fonts.measure_text_size(font, content, props)
                })
                .min()
            },
            ContentLayout::Icon(memo) => ctx.viewport.with(|viewport| {
                Size::new_equal(memo.with(|size| size.resolve(*viewport)))
            }),
            ContentLayout::Fixed(size) => *size,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContainerLayout {
    pub block_model: Signal<BlockModel>,
    pub align: Signal<Align2>,
    // TODO: To have Layout be MaybeReactive we need MaybeReactiveBox, as layout is a recursive data type, so need to store `Inert<Box<T>> | Memo<T>`
    pub content: Memo<Layout>,
    pub font_props: Signal<FontProps>,
}

impl ContainerLayout {
    pub fn base(content: impl IntoMemo<Layout>) -> Self {
        Self {
            block_model: BlockModel::zero().signal(),
            align: Align2::default().signal(),
            content: content.memo(),
            font_props: FontProps::default().signal(),
        }
    }

    pub fn align(mut self, align: Align2) -> Self {
        self.align.set(align);
        self
    }

    pub fn block_model(mut self, block_model: BlockModel) -> Self {
        self.block_model.set(block_model);
        self
    }

    pub fn min_size(&self, ctx: LayoutCtx) -> Size {
        self.content.with(|content| content.min_size(ctx))
            + self.block_model.with(BlockModel::full_padding)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexLayout {
    pub wrap: Signal<bool>,
    pub block_model: Signal<BlockModel>,
    // Readonly
    pub(self) axis: Axis,
    pub gap: Signal<Size>,
    pub align: Signal<Align2>,
    pub children: Memo<Vec<Layout>>,
    pub font_props: Signal<FontProps>,
}

impl FlexLayout {
    /// Default but with specific axis
    pub fn base(axis: Axis, children: Memo<Vec<Layout>>) -> Self {
        Self {
            wrap: false.signal(),
            block_model: BlockModel::zero().signal(),
            axis,
            gap: Size::zero().signal(),
            align: Align2::default().signal(),
            children,
            font_props: FontProps::default().signal(),
        }
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap.set(wrap);
        self
    }

    pub fn block_model(mut self, block_model: BlockModel) -> Self {
        self.block_model.set(block_model);
        self
    }

    pub fn gap(mut self, gap: Size) -> Self {
        self.gap.set(gap);
        self
    }

    pub fn horizontal_align(mut self, horizontal_align: Align) -> Self {
        self.align.update(|align| align.x = horizontal_align);
        self
    }

    pub fn vertical_align(mut self, vertical_align: Align) -> Self {
        self.align.update(|align| align.y = vertical_align);
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

    pub fn min_size(&self, ctx: LayoutCtx) -> Size {
        self.children.with(|children| {
            children.iter().fold(Size::zero(), |min_size, child| {
                self.axis.infix(
                    min_size,
                    child.min_size(ctx),
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollableLayout {
    pub content: Memo<Layout>,
    pub font_props: Signal<FontProps>,
}

impl ScrollableLayout {
    pub fn new(content: Memo<Layout>) -> Self {
        Self { content, font_props: FontProps::default().signal() }
    }

    pub fn min_size(&self, ctx: LayoutCtx) -> Size {
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
            }) => Some(block_model.with(|block_model| block_model.padding)),
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
                align,
                block_model: _,
                content: _,
                font_props: _,
            }) => write!(f, "Container align:{}", align.get()),
            DevLayoutKind::Flex(DevFlexLayout {
                real:
                    FlexLayout {
                        wrap,
                        block_model: _,
                        axis,
                        gap,
                        align,
                        children: _,
                        font_props: _,
                    },
                // lines: _,
            }) => {
                write!(f, "Flex {} ", axis.dir_name())?;

                if wrap.get() {
                    f.write_str("wrap ")?;
                }

                if !gap.get().is_zero() {
                    write!(f, "gap{} ", gap)?;
                }

                write!(f, "align:{}", align.get())
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutKind {
    Zero,
    Edge,
    Content(ContentLayout),
    Container(ContainerLayout),
    Flex(FlexLayout),
    Scrollable(ScrollableLayout),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    kind: LayoutKind,
    pub(crate) size: Signal<Size<Length>>,
    show: Option<Memo<bool>>,
}

impl Layout {
    pub fn zero() -> Self {
        Self {
            kind: LayoutKind::Zero,
            size: Size::zero_length().signal(),
            show: None,
        }
    }

    pub fn shrink(kind: LayoutKind) -> Self {
        Self { kind, size: Size::shrink().signal(), show: None }
    }

    pub fn edge(size: Size<Length>) -> Self {
        Self { kind: LayoutKind::Edge, size: size.signal(), show: None }
    }

    /// Construct base scrollable layout where main axis will be shrinking and cross axis will fill. Also checks if content layout is with growing length on main axis which is disallowed.
    pub fn scrollable<Dir: Direction>(content: Memo<Layout>) -> Self {
        let content_layout_length = content
            .with(|layout| layout.size.with(|size| size.main(Dir::AXIS)));

        if content_layout_length.is_grow() {
            panic!(
                "Don't use growing Length (Div/fill) for content {} inside Scrollable!",
                Dir::AXIS.length_name()
            );
        }

        Self {
            kind: LayoutKind::Scrollable(ScrollableLayout {
                content,
                font_props: FontProps::default().signal(),
            }),
            size: Dir::AXIS
                .canon::<Size<Length>>(
                    Length::InfiniteWindow(Length::Shrink.try_into().unwrap()),
                    Length::fill(),
                )
                .signal(),
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

    pub fn min_size(&self, ctx: LayoutCtx) -> Size {
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
        self.size.set(size);
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
        self.block_model_signal()
            .map(|block_model| block_model.get())
            .unwrap_or(BlockModel::zero())
    }

    pub fn block_model_signal(&self) -> Option<Signal<BlockModel>> {
        match self.kind {
            LayoutKind::Zero
            | LayoutKind::Edge
            | LayoutKind::Content(..)
            | LayoutKind::Scrollable(..) => None,
            LayoutKind::Container(ContainerLayout { block_model, .. })
            | LayoutKind::Flex(FlexLayout { block_model, .. }) => {
                Some(block_model)
            },
        }
    }

    pub fn font_props(&self) -> Option<Signal<FontProps>> {
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
}

/// Layout tree representation with real (global) position
#[derive(Clone, PartialEq)]
pub struct LayoutModelNode {
    pub outer: Rectangle,
    pub inner: Rectangle,
    // Original children from LayoutModel, not accessed directly, but mapped into LayoutModelNode with parent origin position translation.
    children: Memo<Vec<Memo<LayoutModel>>>,
}

impl Debug for LayoutModelNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut f = f.debug_struct("LayoutModelNode");
        f.field("inner", &self.inner);
        f.field("outer", &self.outer);
        #[cfg(feature = "debug-info")]
        f.field("dev", &self.model.dev);
        // TODO: How can I avoid collecting to vector without `field_with`?
        f.field("children", &self.children);
        f.finish()
    }
}

impl LayoutModelNode {
    pub fn translate(&self, by: Point) -> Self {
        Self {
            outer: self.outer.translate(by),
            inner: self.inner.translate(by),
            children: self.children,
        }
    }

    // // TODO: AAAAA iteratorssssss and referencesssss
    // pub fn children(&self) -> impl Iterator<Item = Self> {
    //     self.children(|children| {
    //         children
    //             .iter()
    //             .map(|child| child.with(|child| child.node(self.inner)))
    //             .collect::<Vec<_>>()
    //             .into_iter()
    //     })
    // }

    pub fn children(&self) -> Vec<LayoutModelNode> {
        self.children.with(|children| {
            children
                .iter()
                .map(|child| {
                    child.with(|child| child.node(self.inner.top_left))
                })
                .collect()
        })
    }

    // Note: May be slow and expensive
    pub fn dev_hover(&self, point: Point) -> Option<DevHoveredLayout> {
        let children = self.children();
        children.iter().find_map(|child| child.dev_hover(point)).or_else(|| {
            if self.outer.contains(point) {
                Some(DevHoveredLayout {
                    area: self.outer,
                    children_count: children.len(),
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
    // TODO: SmallVec optimization or make LayoutModel an enum of childless and parent
    children: Memo<Vec<Memo<LayoutModel>>>,
}

impl LayoutModel {
    pub fn childless(
        inner_size: Size,
        #[cfg(feature = "debug-info")] dev: DevLayout,
    ) -> Self {
        Self::new(inner_size, vec![].inert().memo())
    }

    pub fn new(
        inner_size: Size,
        children: Memo<Vec<Memo<LayoutModel>>>,
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
        self.node(Point::zero())
    }

    fn node(&self, origin: Point) -> LayoutModelNode {
        LayoutModelNode {
            outer: self.outer.translate(origin),
            inner: self.inner.translate(origin),
            children: self.children,
        }
    }

    fn zero() -> Self {
        Self::childless(Size::zero())
    }

    pub fn outer_size(&self) -> Size {
        self.outer.size.into()
    }

    fn translate_mut(&mut self, by: impl Into<Point> + Copy) -> &mut Self {
        self.outer.top_left += by.into();
        self.inner.top_left += by.into();
        self
    }

    fn translated(mut self, by: impl Into<Point> + Copy) -> Self {
        self.translate_mut(by);
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
            Align::Center => free_space.width as i32 / 2,
            Align::End => free_space.width as i32,
        };

        let y = match vertical {
            Align::Start => 0,
            Align::Center => {
                free_space.height as i32 / 2
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

    pub fn aligned(mut self, align: Align2, parent_size: Size) -> Self {
        self.align_mut(align.x, align.y, parent_size);
        self
    }
}

#[derive(Clone, Copy)]
pub struct LayoutCtx {
    pub fonts: Signal<FontCtx>,
    pub viewport: Memo<Size>,
}

// TODO: Should viewport be unwrapped value as we depend modeling on viewport value?
pub fn model_layout(
    ctx: LayoutCtx,
    layout: Layout,
    parent_limits: Memo<Limits>,
    parent_size: Memo<Size<Length>>,
    // viewport: Memo<Size>,
) -> Memo<LayoutModel> {
    // TODO: Is reactive?
    if !layout.show.map(|show| show.get()).unwrap_or(true) {
        // TODO: Should be zero or skipped? Doesn't zero layout take child place in flex?
        return LayoutModel::zero().inert().memo();
    }

    let size = layout.size;
    let size = map!(move |size, parent_size| size.in_parent(parent_size));

    match layout.kind {
        // TODO: Panic or not?
        LayoutKind::Zero => LayoutModel::zero().inert().memo(),
        LayoutKind::Edge => {
            map!(move |size, parent_limits| {
                LayoutModel::childless(
                    parent_limits.resolve_size(size, Size::zero()),
                    #[cfg(feature = "debug-info")]
                    DevLayout::new(size, DevLayoutKind::Edge),
                )
            })
        },
        LayoutKind::Content(content_layout) => {
            map!(move |size, parent_limits| {
                LayoutModel::childless(
                    parent_limits
                        .resolve_size(size, content_layout.min_size(ctx)),
                    #[cfg(feature = "debug-info")]
                    DevLayout::new(
                        size,
                        DevLayoutKind::Content(content_layout.clone()),
                    ),
                )
            })
        },
        LayoutKind::Container(container_layout) => {
            let ContainerLayout {
                block_model,
                align,
                content,
                font_props: _,
                // TODO: Useless?
            } = container_layout;

            let child_limits = map!(move |block_model| {
                let full_padding = block_model.full_padding();

                let child_limits = with!(move |size, parent_limits| {
                    parent_limits.limit_by(*size).shrink(full_padding)
                });

                child_limits
            });

            // let full_padding = block_model.full_padding();

            // let limits = parent_limits.limit_by(size).shrink(full_padding);

            // TODO: Panic or warn in case when there're more than a single child

            let content_layout = content.map(move |&content| {
                model_layout(ctx, content, child_limits, size)
            });

            // let content_size = content_layout.map(|content_layout| {
            //     content_layout
            //         .with(|content_layout| content_layout.outer_size())
            // });

            // let inner_size = map!(|child_limits, size, content_size| {
            //     child_limits.resolve_size(size, *content_size)
            // });

            map!(move |block_model, child_limits, align, content_layout| {
                let align = *align;

                let (real_size, content_size) =
                    with!(|size, content_layout| {
                        let content_size = content_layout.outer_size();
                        let real_size =
                            child_limits.resolve_size(size, content_size);

                        (real_size, content_size)
                    });

                // TODO: This is hard to get rid of this alignment mapping but it would be nice to.
                let content_layout =
                    content_layout.map(move |content_layout| {
                        content_layout.aligned(align, real_size - content_size)
                    });

                LayoutModel::new(
                    // TODO: Generalize logic with real_size.expand/shrink and
                    // full_padding
                    real_size,
                    // TODO: MaybeReactive, SmallVec optimization single child
                    vec![content_layout].inert().memo(),
                    #[cfg(feature = "debug-info")]
                    DevLayout::new(
                        size,
                        DevLayoutKind::Container(container_layout.clone()),
                    ),
                )
                .with_full_padding(block_model.full_padding())
            })

            // let content_size = content_layout.outer_size();
            // let real_size = limits.resolve_size(size, content_size);
            // let content_layout = content_layout
            //     // .moved(full_padding.top_left())
            //     .aligned(
            //         *horizontal_align,
            //         *vertical_align,
            //         real_size - content_size,
            //     );

            // LayoutModel::new(
            //     // TODO: Generalize logic with real_size.expand/shrink and
            //     // full_padding
            //     real_size,
            //     vec![content_layout],
            //     #[cfg(feature = "debug-info")]
            //     DevLayout::new(
            //         size,
            //         DevLayoutKind::Container(container_layout.clone()),
            //     ),
            // )
            // .with_full_padding(full_padding)
        },
        LayoutKind::Scrollable(scrollable_layout) => {
            let ScrollableLayout { content, font_props } = scrollable_layout;

            let child_limits = map!(move |size, parent_limits| {
                parent_limits.limit_by(*size)
            });

            let content_layout = content.map(move |&content| {
                model_layout(ctx, content, child_limits, size)
            });

            map!(move |content_layout, child_limits| {
                // Note: For [`LayoutKind::Scrollable`], parent_limits are used as
                // content limits are unlimited on one axis
                let (real_size, content_size) =
                    with!(move |size, content_layout| {
                        let content_size = content_layout.outer_size();
                        let real_size =
                            child_limits.resolve_size(size, content_size);

                        (real_size, content_size)
                    });

                LayoutModel::new(
                    real_size,
                    vec![*content_layout].inert().memo(),
                    #[cfg(feature = "debug-info")]
                    DevLayout::new(
                        size,
                        DevLayoutKind::Scrollable(scrollable_layout.clone()),
                    ),
                )
            })
        },
        LayoutKind::Flex(flex_layout) => {
            model_flex(ctx, flex_layout, parent_limits, size)
        },
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
