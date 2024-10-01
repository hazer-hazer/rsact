use alloc::vec::Vec;
use box_model::BoxModel;
use core::{
    fmt::{Debug, Display},
    u32,
};
use embedded_graphics::{
    prelude::{Point, Transform},
    primitives::Rectangle,
};
use flex::model_flex;
use padding::Padding;
use rsact_core::prelude::*;
use size::{Length, Size};

pub mod axis;
pub mod box_model;
pub mod flex;
pub mod limits;
pub mod padding;
pub mod size;

pub use axis::{Axial as _, Axis};
pub use limits::Limits;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Start,
    Center,
    End,
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

// #[derive(Clone, Copy, PartialEq)]
// pub struct EdgeLayout {}

// impl EdgeLayout {
//     pub fn base() -> Self {
//         Self {}
//     }
// }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentLayout {
    pub content_size: Memo<Limits>,
}

impl ContentLayout {
    pub fn new(content_size: impl IntoMemo<Limits>) -> Self {
        Self { content_size: content_size.into_memo() }
    }

    pub fn min_size(&self) -> Size {
        self.content_size.get().min()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContainerLayout {
    pub box_model: BoxModel,
    pub horizontal_align: Align,
    pub vertical_align: Align,
    pub content_size: Memo<Limits>,
}

impl ContainerLayout {
    pub fn base(content_size: impl IntoMemo<Limits>) -> Self {
        Self {
            box_model: BoxModel::zero(),
            horizontal_align: Align::Start,
            vertical_align: Align::Start,
            content_size: content_size.into_memo(),
        }
    }

    pub fn min_size(&self) -> Size {
        self.content_size.get().min() + self.box_model.padding
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexLayout {
    pub wrap: bool,
    pub box_model: BoxModel,
    // Readonly
    pub(self) axis: Axis,
    pub gap: Size,
    pub horizontal_align: Align,
    pub vertical_align: Align,
    pub content_size: Memo<Limits>,
}

impl FlexLayout {
    /// Default but with specific axis
    pub fn base(axis: Axis, content_size: impl IntoMemo<Limits>) -> Self {
        Self {
            wrap: false,
            box_model: BoxModel::zero(),
            axis,
            gap: Size::zero(),
            horizontal_align: Align::Start,
            vertical_align: Align::Start,
            content_size: content_size.into_memo(),
        }
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn box_model(mut self, box_model: BoxModel) -> Self {
        self.box_model = box_model;
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

    pub fn min_size(&self) -> Size {
        self.content_size.get().min() + self.box_model.padding
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DevFlexLayout {
    // lines: Vec<Rectangle>,
    real: FlexLayout,
}

#[derive(Clone, Copy, Debug, PartialEq)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DevLayoutKind {
    Zero,
    Edge,
    Content(ContentLayout),
    Container(ContainerLayout),
    Flex(DevFlexLayout),
    Scrollable(ContentLayout),
}

impl DevLayoutKind {
    pub fn padding(&self) -> Option<Padding> {
        match self {
            DevLayoutKind::Zero
            | DevLayoutKind::Edge
            | DevLayoutKind::Content(_)
            | DevLayoutKind::Scrollable(_) => None,
            DevLayoutKind::Container(ContainerLayout { box_model, .. })
            | DevLayoutKind::Flex(DevFlexLayout {
                real: FlexLayout { box_model, .. },
                ..
            }) => Some(box_model.padding),
        }
    }
}

impl Display for DevLayoutKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DevLayoutKind::Zero => write!(f, "[Zero]"),
            DevLayoutKind::Edge => write!(f, "Edge"),
            DevLayoutKind::Content(ContentLayout { content_size }) => {
                write!(f, "Content {}", content_size.get())
            },
            DevLayoutKind::Container(ContainerLayout {
                horizontal_align,
                vertical_align,
                box_model: _,
                content_size,
            }) => write!(
                f,
                "Container align:h{},v{} content:{}",
                horizontal_align.display_code(Axis::X),
                vertical_align.display_code(Axis::Y),
                content_size.get()
            ),
            DevLayoutKind::Flex(DevFlexLayout {
                real:
                    FlexLayout {
                        wrap,
                        box_model: _,
                        axis,
                        gap,
                        horizontal_align,
                        vertical_align,
                        content_size,
                    },
                // lines: _,
            }) => {
                write!(f, "Flex {} ", axis.dir_name())?;

                if *wrap {
                    f.write_str("wrap ")?;
                }

                if !gap.is_zero() {
                    write!(f, "gap{} ", gap)?;
                }

                write!(
                    f,
                    "align:h{}v{} content:{}",
                    horizontal_align.display_code(Axis::X),
                    vertical_align.display_code(Axis::Y),
                    content_size.get()
                )
            },
            DevLayoutKind::Scrollable(ContentLayout { content_size }) => {
                write!(f, "Scrollable content:{}", content_size.get())
            },
        }
    }
}

// Agenda: Full box model in dev tools

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DevHoveredLayout {
    pub layout: DevLayout,
    pub size: Size<Length>,
    pub area: Rectangle,
    pub children_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutKind {
    Edge,
    Content(ContentLayout),
    Container(ContainerLayout),
    Flex(FlexLayout),
    Scrollable(ContentLayout),
}

#[derive(Clone, Copy, PartialEq)]
pub struct Layout {
    pub(crate) kind: LayoutKind,
    pub(crate) size: Size<Length>,
}

impl Layout {
    pub fn shrink(kind: LayoutKind) -> Self {
        Self { kind, size: Size::shrink() }
    }

    // pub fn min_size(&self) -> Size {
    //     match self.kind {
    //         LayoutKind::Edge => Size::zero(),
    //         LayoutKind::Content(ContentLayout { content_size })
    //         | LayoutKind::Container(ContainerLayout { content_size, .. })
    //         | LayoutKind::Flex(FlexLayout { content_size, .. }) => {
    //             content_size.get().min()
    //         },
    //         LayoutKind::Scrollable(ContentLayout { content_size }) => {
    //             content_size.get().min()
    //         },
    //     }
    // }

    // pub fn set_size(&mut self, size: Size<Length>) {
    //     self.size = size;
    // }

    pub fn content_size(&self) -> Limits {
        match self.kind {
            LayoutKind::Edge => Limits::unlimited(),
            LayoutKind::Content(ContentLayout { content_size })
            | LayoutKind::Container(ContainerLayout { content_size, .. })
            | LayoutKind::Flex(FlexLayout { content_size, .. })
            | LayoutKind::Scrollable(ContentLayout { content_size }) => {
                content_size.get()
            },
        }
    }

    pub fn min_size(&self) -> Size {
        match self.kind {
            LayoutKind::Edge => Size::zero(),
            LayoutKind::Content(content_layout) => content_layout.min_size(),
            LayoutKind::Container(container_layout) => {
                container_layout.min_size()
            },
            LayoutKind::Flex(flex_layout) => flex_layout.min_size(),
            LayoutKind::Scrollable(content_layout) => content_layout.min_size(),
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

    // TODO: Panic on invalid layout kind usage?
    pub fn box_model(&self) -> BoxModel {
        match self.kind {
            LayoutKind::Edge
            | LayoutKind::Content(..)
            | LayoutKind::Scrollable(..) => BoxModel::zero(),
            LayoutKind::Container(ContainerLayout { box_model, .. })
            | LayoutKind::Flex(FlexLayout { box_model, .. }) => box_model,
        }
    }

    pub fn set_border_width(&mut self, border_width: u32) {
        match &mut self.kind {
            LayoutKind::Container(ContainerLayout { box_model, .. })
            | LayoutKind::Flex(FlexLayout { box_model, .. }) => {
                box_model.border_width = border_width
            },
            _ => {},
        }
    }

    pub fn set_padding(&mut self, padding: Padding) {
        match &mut self.kind {
            LayoutKind::Container(ContainerLayout { box_model, .. })
            | LayoutKind::Flex(FlexLayout { box_model, .. }) => {
                box_model.padding = padding
            },
            _ => {},
        }
    }
}

// pub struct LayoutTree {
//     layout: Signal<Layout>,
//     children: Vec<LayoutTree>,
// }

// impl LayoutTree {
//     pub fn build<C: WidgetCtx>(el: &El<C>) -> Self {
//         Self {
//             layout: el.layout(),
//             children: el.children().iter().map(Self::build).collect(),
//         }
//     }
// }

// pub struct Layout<K> {
//     pub size: Signal<Size<Length>>,
//     pub kind: Signal<K>,
// }

// impl<K: 'static> Layout<K> {
//     pub fn new(size: Size<Length>, kind: K) -> Self {
//         Self { size: use_signal(size), kind: use_signal(kind) }
//     }
// }

// impl Layout {
//     // /// Shrink limits by paddings to get free space for content/children
//     // fn content_limits(&self, limits: &Limits) -> Limits {
//     //     let full_padding = self.box_model.border + self.box_model.padding;
//     //     let self_limits = limits.limit_by(self.size);
//     //     self_limits.shrink(full_padding)
//     // }
// }

// pub struct Viewport {
//     size: Size,
// }

/// Layout tree representation with real position in viewport
#[derive(Clone, Copy)]
pub struct LayoutModelNode<'a> {
    pub area: Rectangle,
    model: &'a LayoutModel,
}

impl<'a> Debug for LayoutModelNode<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut f = f.debug_struct("LayoutModelNode");
        f.field("area", &self.area);
        f.field("dev", &self.model.dev);
        // TODO: How can I avoid collecting to vector without `field_with`?
        f.field("children", &self.children().collect::<Vec<_>>());
        f.finish()
    }
}

// impl<'a> Display for LayoutModelNode<'a> {
//     fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         f.debug_struct("LayoutModel").field_with("area", |f| {
//             write!(f, "[{};{}]", self.area.top_left, self.area.size)?;
//         });

//         self.children().try_for_each(|child| {})?;

//         f
//     }
// }

impl<'a> LayoutModelNode<'a> {
    pub fn translate(&self, by: Point) -> Self {
        Self { area: self.area.translate(by), model: self.model }
    }

    pub fn children(&'a self) -> impl Iterator<Item = LayoutModelNode<'a>> {
        self.model
            .children
            .iter()
            .map(|child| LayoutModel::node(child, self.area))
    }

    // Note: May be slow and expensive
    pub fn dev_hover(&'a self, point: Point) -> Option<DevHoveredLayout> {
        self.children().find_map(|child| child.dev_hover(point)).or_else(|| {
            if self.area.contains(point) {
                Some(DevHoveredLayout {
                    area: self.area,
                    children_count: self.model.children.len(),
                    layout: self.model.dev.clone(),
                    size: self.model.dev.size,
                })
            } else {
                None
            }
        })
    }
}

/// Layout tree representation with relative positions
#[derive(Debug, PartialEq)]
pub struct LayoutModel {
    relative_area: Rectangle,
    // Note: `dev` goes before `children` which is intentional to make more
    // readable pretty-printed debug
    dev: DevLayout,
    children: Vec<LayoutModel>,
}

impl LayoutModel {
    pub fn new(size: Size, children: Vec<LayoutModel>, dev: DevLayout) -> Self {
        Self {
            relative_area: Rectangle::new(Point::zero(), size.into()),
            children,
            dev,
        }
    }

    pub fn tree_root(&self) -> LayoutModelNode {
        LayoutModelNode { area: self.relative_area, model: self }
    }

    fn node(&self, parent_area: Rectangle) -> LayoutModelNode {
        LayoutModelNode {
            area: self.relative_area.translate(parent_area.top_left),
            model: self,
        }
    }

    fn zero() -> Self {
        Self {
            relative_area: Rectangle::zero(),
            children: vec![],
            dev: DevLayout::zero(),
        }
    }

    pub fn size(&self) -> Size {
        self.relative_area.size.into()
    }

    pub fn move_mut(&mut self, to: impl Into<Point>) -> &mut Self {
        self.relative_area.top_left = to.into();
        self
    }

    pub fn moved(mut self, to: impl Into<Point>) -> Self {
        self.move_mut(to);
        self
    }

    pub fn align_mut(
        &mut self,
        horizontal: Align,
        vertical: Align,
        free_space: Size,
    ) -> &mut Self {
        match horizontal {
            Align::Start => {},
            Align::Center => {
                self.relative_area.top_left.x += free_space.width as i32 / 2;
                // - self.relative_area.size.width as i32 / 2;
            },
            Align::End => {
                self.relative_area.top_left.x += free_space.width as i32;
                // - self.relative_area.size.width as i32 / 2;
            },
        }

        match vertical {
            Align::Start => {},
            Align::Center => {
                self.relative_area.top_left.y += free_space.height as i32 / 2;
                // - self.relative_area.size.height as i32 / 2;
            },
            Align::End => {
                self.relative_area.top_left.y += free_space.height as i32;
                // - self.relative_area.size.height as i32;
            },
        }

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

pub fn model_layout(
    tree: MemoTree<Layout>,
    parent_limits: Limits,
    parent_size: Size<Length>,
    // TODO: is_growing and is_shrinking
) -> LayoutModel {
    let layout = tree.data.get();
    let size = layout.size.in_parent(parent_size);
    // let content_size = layout.content_size.get();

    // TODO: Resolve size container against `content_size` (limits), not only
    // min

    match layout.kind {
        LayoutKind::Edge => {
            let limits = parent_limits.limit_by(size);

            LayoutModel::new(
                limits.resolve_size(size, Size::zero()),
                vec![],
                DevLayout::new(size, DevLayoutKind::Edge),
            )
        },
        LayoutKind::Content(content_layout) => {
            let ContentLayout { content_size } = content_layout;
            let min_content = content_size.get().min();

            LayoutModel::new(
                parent_limits.resolve_size(size, min_content),
                vec![],
                DevLayout::new(size, DevLayoutKind::Content(content_layout)),
            )
        },
        LayoutKind::Container(container_layout) => {
            let ContainerLayout {
                box_model,
                horizontal_align,
                vertical_align,
                // TODO: Useless?
                content_size,
            } = container_layout;

            let min_content = content_size.get().min();

            let full_padding =
                box_model.padding + Padding::new_equal(box_model.border_width);

            let limits = parent_limits.limit_by(size).shrink(full_padding);

            // TODO: Panic or warn in case when there're more than a single
            // child

            let content_layout = model_layout(
                tree.children.with(|children| children[0]),
                limits,
                size,
            );

            let content_size = content_layout.size();
            let real_size = limits.resolve_size(size, content_size);
            let content_layout =
                content_layout.moved(full_padding.top_left()).aligned(
                    horizontal_align,
                    vertical_align,
                    real_size - content_size,
                );

            LayoutModel::new(
                real_size.expand(full_padding),
                vec![content_layout],
                DevLayout::new(
                    size,
                    DevLayoutKind::Container(container_layout),
                ),
            )
        },
        LayoutKind::Scrollable(scrollable_layout) => {
            // TODO: Useless?
            let ContentLayout { content_size: _ } = scrollable_layout;

            let limits = parent_limits.limit_by(size);

            let content_layout = model_layout(
                tree.children.with(|children| children[0]),
                limits,
                size,
            );

            // Note: For [`LayoutKind::Scrollable`], parent_limits are used as
            // content limits are unlimited on one axis
            let real_size =
                parent_limits.resolve_size(size, content_layout.size());

            LayoutModel::new(
                real_size,
                vec![content_layout],
                DevLayout::new(
                    size,
                    DevLayoutKind::Scrollable(scrollable_layout),
                ),
            )
        },
        LayoutKind::Flex(flex_layout) => {
            model_flex(tree, parent_limits, flex_layout, size)
        },
    }
}
