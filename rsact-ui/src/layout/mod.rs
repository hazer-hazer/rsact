use alloc::vec::Vec;
use box_model::BoxModel;
use core::{fmt::Display, u32};
use embedded_graphics::{
    prelude::{Point, Transform},
    primitives::Rectangle,
};
use flex::model_flex;
use padding::Padding;
use rsact_core::prelude::*;
use size::{DivFactors, Length, Size, SubTake};

pub mod axis;
pub mod box_model;
mod flex;
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
            (Align::Center, Axis::Y) => "—",
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
pub struct ContainerLayout {
    pub box_model: BoxModel,
    pub horizontal_align: Align,
    pub vertical_align: Align,
}

impl ContainerLayout {
    pub fn base() -> Self {
        Self {
            box_model: BoxModel::zero(),
            horizontal_align: Align::Start,
            vertical_align: Align::Start,
        }
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
}

impl FlexLayout {
    /// Default but with specific axis
    pub fn base(axis: Axis) -> Self {
        Self {
            wrap: false,
            box_model: BoxModel::zero(),
            axis,
            gap: Size::zero(),
            horizontal_align: Align::Start,
            vertical_align: Align::Start,
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DevLayoutKind {
    Zero,
    Edge,
    Container(ContainerLayout),
    Flex(FlexLayout),
    Scrollable,
}

impl DevLayoutKind {
    pub fn padding(&self) -> Option<Padding> {
        match self {
            DevLayoutKind::Zero
            | DevLayoutKind::Edge
            | DevLayoutKind::Scrollable => None,
            DevLayoutKind::Container(ContainerLayout { box_model, .. })
            | DevLayoutKind::Flex(FlexLayout { box_model, .. }) => {
                Some(box_model.padding)
            },
        }
    }
}

impl Display for DevLayoutKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DevLayoutKind::Zero => write!(f, "[Zero]"),
            DevLayoutKind::Edge => write!(f, "Edge"),
            DevLayoutKind::Container(ContainerLayout {
                horizontal_align,
                vertical_align,
                box_model: _,
            }) => write!(
                f,
                "Container align:h{},v{}",
                horizontal_align.display_code(Axis::X),
                vertical_align.display_code(Axis::Y)
            ),
            DevLayoutKind::Flex(FlexLayout {
                wrap,
                box_model: _,
                axis,
                gap,
                horizontal_align,
                vertical_align,
            }) => write!(
                f,
                "Flex {}{};gap:{};align:h{}v{}",
                axis.dir_name(),
                if *wrap { ";wrap" } else { "" },
                gap,
                horizontal_align.display_code(Axis::X),
                vertical_align.display_code(Axis::Y),
            ),
            DevLayoutKind::Scrollable => write!(f, "Scrollable"),
        }
    }
}

// Agenda: Full box model in dev tools

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DevHoveredLayout {
    pub kind: DevLayoutKind,
    pub area: Rectangle,
    pub children_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutKind {
    Edge,
    Container(ContainerLayout),
    Flex(FlexLayout),
    Scrollable,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Layout {
    pub(crate) kind: LayoutKind,
    pub(crate) size: Size<Length>,
    pub(crate) content_size: Memo<Limits>,
}

impl Layout {
    pub fn new(kind: LayoutKind, content_size: Memo<Limits>) -> Self {
        Self { kind, size: Size::shrink(), content_size }
    }

    pub fn set_size(&mut self, size: Size<Length>) {
        self.size = size;
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

    pub fn box_model(&self) -> BoxModel {
        match self.kind {
            LayoutKind::Edge | LayoutKind::Scrollable => BoxModel::zero(),
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
pub struct LayoutModelTree<'a> {
    pub area: Rectangle,
    model: &'a LayoutModel,
    dev_kind: DevLayoutKind,
}

impl<'a> LayoutModelTree<'a> {
    pub fn translate(&self, by: Point) -> Self {
        Self {
            area: self.area.translate(by),
            model: self.model,
            dev_kind: self.dev_kind,
        }
    }

    pub fn children(&self) -> impl Iterator<Item = LayoutModelTree> {
        self.model.children.iter().map(|child| LayoutModelTree {
            area: Rectangle::new(
                child.relative_area.top_left + self.area.top_left,
                child.relative_area.size,
            ),
            model: child,
            dev_kind: child.dev_kind,
        })
    }

    pub fn dev_hover(&self, point: Point) -> Option<DevHoveredLayout> {
        self.children().find_map(|child| child.dev_hover(point)).or_else(|| {
            if self.area.contains(point) {
                Some(DevHoveredLayout {
                    area: self.area,
                    kind: self.dev_kind,
                    children_count: self.model.children.len(),
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
    children: Vec<LayoutModel>,
    dev_kind: DevLayoutKind,
}

impl LayoutModel {
    pub fn new(
        size: Size,
        children: Vec<LayoutModel>,
        dev_kind: DevLayoutKind,
    ) -> Self {
        Self {
            relative_area: Rectangle::new(Point::zero(), size.into()),
            children,
            dev_kind,
        }
    }

    pub fn tree_root(&self) -> LayoutModelTree {
        LayoutModelTree {
            area: self.relative_area,
            model: self,
            dev_kind: self.dev_kind,
        }
    }

    fn zero() -> Self {
        Self {
            relative_area: Rectangle::zero(),
            children: vec![],
            dev_kind: DevLayoutKind::Zero,
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
    // TODO: is_growing and is_shrinking
) -> LayoutModel {
    let layout = tree.data.get();
    let size = layout.size;
    let content_size = layout.content_size.get();

    // TODO: Resolve size container against `content_size` (limits), not only
    // min

    match layout.kind {
        LayoutKind::Edge => {
            let limits = parent_limits.limit_by(size);

            LayoutModel::new(
                limits.resolve_size(size, content_size.min()),
                vec![],
                DevLayoutKind::Edge,
            )
        },
        LayoutKind::Container(container_layout) => {
            let ContainerLayout { box_model, horizontal_align, vertical_align } =
                container_layout;

            let full_padding =
                box_model.padding + Padding::new_equal(box_model.border_width);

            let limits = parent_limits.limit_by(size).shrink(full_padding);

            // TODO: Panic or warn in case when there're more than a single
            // child

            let content_layout = model_layout(
                tree.children.with(|children| children[0]),
                limits,
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
                DevLayoutKind::Container(container_layout),
            )
        },
        LayoutKind::Scrollable => {
            let limits = parent_limits.limit_by(size);

            let content_layout = model_layout(
                tree.children.with(|children| children[0]),
                limits,
            );

            // Note: For [`LayoutKind::Scrollable`], parent_limits are used as
            // content limits are unlimited on one axis
            let real_size =
                parent_limits.resolve_size(size, content_layout.size());

            LayoutModel::new(
                real_size,
                vec![content_layout],
                DevLayoutKind::Scrollable,
            )
        },
        LayoutKind::Flex(flex_layout) => {
            model_flex(tree, parent_limits, flex_layout, size)
        },
    }
}
