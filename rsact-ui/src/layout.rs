use alloc::vec::Vec;
use embedded_graphics::{prelude::Point, primitives::Rectangle};

use crate::{
    axis::{Axial, Axis},
    block::BoxModel,
    el::El,
    size::{Length, Size, SizeExt},
    widget::{Ctx, Widget, WidgetCtx},
};

#[derive(Clone, Copy)]
pub struct Limits {
    min: Size<u32>,
    max: Size<u32>,
}

impl Limits {
    pub fn new(min: Size<u32>, max: Size<u32>) -> Self {
        Self { min, max }
    }

    pub fn only_max(max: Size<u32>) -> Self {
        Self { min: Size::zero(), max }
    }

    pub fn min(&self) -> Size<u32> {
        self.min
    }

    pub fn max(&self) -> Size<u32> {
        self.max
    }

    pub fn min_square(&self) -> u32 {
        self.min().width.min(self.min().height)
    }

    pub fn max_square(&self) -> u32 {
        self.max().width.min(self.max().height)
    }

    pub fn with_max(self, max: Size) -> Self {
        Self::new(self.min, max)
    }

    pub fn limit_by(self, size: impl Into<Size<Length>>) -> Self {
        let size = size.into();

        self.limit_axis(Axis::X, size.width).limit_axis(Axis::Y, size.height)
    }

    pub fn limit_width(self, width: impl Into<Length>) -> Self {
        match width.into() {
            Length::Shrink | Length::Div(_) | Length::Fill => self,
            Length::Fixed(fixed) => {
                let new_width = fixed.min(self.max.width).max(self.min.width);

                Self::new(
                    self.min.with_width(new_width),
                    self.max.with_width(new_width),
                )
            },
        }
    }

    pub fn limit_height(self, height: impl Into<Length>) -> Self {
        match height.into() {
            Length::Shrink | Length::Div(_) | Length::Fill => self,
            Length::Fixed(fixed) => {
                let new_height =
                    fixed.min(self.max.height).max(self.min.height);

                Self::new(
                    self.min.with_height(new_height),
                    self.max.with_height(new_height),
                )
            },
        }
    }

    pub fn limit_axis(self, axis: Axis, length: impl Into<Length>) -> Self {
        match axis {
            Axis::X => self.limit_width(length),
            Axis::Y => self.limit_height(length),
        }
    }

    pub fn shrink(self, by: impl Into<Size>) -> Self {
        let by = by.into();

        Limits::new(self.min() - by, self.max() - by)
    }

    pub fn resolve_size(
        &self,
        container_size: Size<Length>,
        content_size: Size<u32>,
    ) -> Size<u32> {
        let width = match container_size.width {
            Length::Fill | Length::Div(_) => self.max.width,
            Length::Fixed(fixed) => {
                fixed.min(self.max.width).max(self.min.width)
            },
            Length::Shrink => {
                content_size.width.min(self.max.width).max(self.min.width)
            },
        };

        let height = match container_size.height {
            Length::Fill | Length::Div(_) => self.max.height,
            Length::Fixed(fixed) => {
                fixed.min(self.max.height).max(self.min.height)
            },
            Length::Shrink => {
                content_size.height.min(self.max.height).max(self.min.height)
            },
        };

        Size::new(width, height)
    }

    pub fn resolve_square(&self, size: impl Into<Length>) -> u32 {
        let min_square = self.min_square();
        let max_square = self.max_square();

        match size.into() {
            Length::Fill | Length::Div(_) => max_square,
            Length::Fixed(fixed) => fixed.min(max_square).max(min_square),
            Length::Shrink => min_square,
        }
    }
}

impl From<Rectangle> for Limits {
    fn from(value: Rectangle) -> Self {
        Self::new(Size::zero(), value.size.into())
    }
}

#[derive(Clone, Copy)]
pub enum Align {
    Start,
    Center,
    End,
}

pub enum LayoutKind {
    Edge,
    Block {
        box_model: BoxModel,
        horizontal_align: Align,
        vertical_align: Align,
    },
    // TODO: Flex wrap
    Flex {
        axis: Axis,
        gap: u32,
        box_model: BoxModel,
        horizontal_align: Align,
        vertical_align: Align,
    },
}

impl LayoutKind {
    pub fn into_layout(self, size: Size<Length>) -> Layout {
        Layout { kind: self, size }
    }
}

pub struct Layout {
    kind: LayoutKind,
    size: Size<Length>,
}

impl Layout {
    // /// Shrink limits by paddings to get free space for content/children
    // fn content_limits(&self, limits: &Limits) -> Limits {
    //     let full_padding = self.box_model.border + self.box_model.padding;
    //     let self_limits = limits.limit_by(self.size);
    //     self_limits.shrink(full_padding)
    // }
}

pub struct Viewport {
    size: Size,
}

/// Layout tree representation with real position in viewport
pub struct LayoutTree<'a> {
    pub area: Rectangle,
    model: &'a LayoutModel,
}

impl<'a> LayoutTree<'a> {
    pub fn children(&self) -> impl Iterator<Item = LayoutTree> {
        self.model.children.iter().map(|child| LayoutTree {
            area: Rectangle::new(
                child.relative_area.top_left + self.area.top_left,
                child.relative_area.size,
            ),
            model: child,
        })
    }
}

/// Layout tree representation with relative positions
pub struct LayoutModel {
    relative_area: Rectangle,
    children: Vec<LayoutModel>,
}

impl LayoutModel {
    pub fn new(size: Size, children: Vec<LayoutModel>) -> Self {
        Self {
            relative_area: Rectangle::new(Point::zero(), size.into()),
            children,
        }
    }

    pub fn tree_root(&self) -> LayoutTree {
        LayoutTree { area: self.relative_area, model: self }
    }

    fn zero() -> Self {
        Self { relative_area: Rectangle::zero(), children: vec![] }
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
            },
            Align::End => {
                self.relative_area.top_left.x += free_space.width as i32;
            },
        }

        match vertical {
            Align::Start => {},
            Align::Center => {
                self.relative_area.top_left.y += free_space.height as i32 / 2
                    - self.relative_area.size.height as i32 / 2;
            },
            Align::End => {
                self.relative_area.top_left.y += free_space.height as i32
                    - self.relative_area.size.height as i32;
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

pub fn model_layout<C: WidgetCtx>(
    el: &El<C>,
    ctx: &Ctx<C>,
    limits: &Limits,
) -> LayoutModel {
    let layout = el.layout(ctx);
    let size = layout.size;
    let children = el.children();

    match layout.kind {
        LayoutKind::Edge => LayoutModel::new(
            limits.limit_by(size).resolve_size(size, Size::zero()),
            vec![],
        ),
        LayoutKind::Block { box_model, horizontal_align, vertical_align } => {
            let full_padding = box_model.border + box_model.padding;
            let limits = limits.limit_by(size).shrink(full_padding);

            // TODO: Panic or warn in case when there're more than a single
            // child

            let content_limits = limits.shrink(full_padding);
            let content_layout =
                model_layout(&children[0], ctx, &content_limits);

            let real_size = limits.resolve_size(size, content_layout.size());
            let content_layout = content_layout
                .moved(full_padding.top_left())
                .aligned(horizontal_align, vertical_align, real_size);

            LayoutModel::new(real_size, vec![content_layout])
        },
        LayoutKind::Flex {
            axis,
            gap,
            box_model,
            horizontal_align,
            vertical_align,
        } => {
            let full_padding = box_model.border + box_model.padding;
            let limits = limits.limit_by(size).shrink(full_padding);

            let total_gap = gap * children.len().saturating_sub(1) as u32;
            let max_cross = limits.max().cross_for(axis);

            let mut children_layouts = Vec::with_capacity(children.len());
            children_layouts
                .resize_with(children.len(), || LayoutModel::zero());

            let mut free_main =
                limits.max().main_for(axis).saturating_sub(total_gap);
            let mut used_cross = match axis {
                Axis::X if size.width == Length::Shrink => 0,
                Axis::Y if size.height == Length::Shrink => 0,
                _ => max_cross,
            };

            let mut total_main_divs = 0;

            // Calculate fixed and shrink children
            for (i, child) in children.iter().enumerate() {
                let (fill_main_div, fill_cross_div) = {
                    let size = child.size();
                    axis.canon(
                        size.width.div_factor(),
                        size.height.div_factor(),
                    )
                };

                if fill_main_div == 0 {
                    let (max_width, max_height) = axis.canon(
                        free_main,
                        if fill_cross_div == 0 {
                            max_cross
                        } else {
                            used_cross
                        },
                    );

                    let child_limits = Limits::new(
                        Size::zero(),
                        Size::new(max_width, max_height),
                    );

                    let layout = model_layout(child, ctx, &child_limits);
                    let size = layout.size();

                    free_main -= size.main_for(axis);
                    used_cross = used_cross.max(size.cross_for(axis));

                    children_layouts[i] = layout;
                } else {
                    total_main_divs += fill_main_div as u32;
                }
            }

            // Remaining main axis length after calculating sizes of
            // non-auto-sized children
            let remaining = match axis {
                Axis::X => match size.width {
                    Length::Shrink => 0,
                    _ => free_main.max(0),
                },
                Axis::Y => match size.height {
                    Length::Shrink => 0,
                    _ => free_main.max(0),
                },
            };
            let remaining_div =
                remaining.checked_div(total_main_divs).unwrap_or(0);
            let mut remaining_mod =
                remaining.checked_rem(total_main_divs).unwrap_or(0);

            // Calculate auto-sized children (Length::Fill, Length::Div(N))
            for (i, child) in children.iter().enumerate() {
                let (fill_main_div, fill_cross_div) = {
                    let size = child.size();

                    axis.canon(
                        size.width.div_factor(),
                        size.height.div_factor(),
                    )
                };

                if fill_main_div != 0 {
                    let max_main = if total_main_divs == 0 {
                        remaining
                    } else {
                        remaining_div * fill_main_div as u32
                            + if remaining_mod > 0 {
                                remaining_mod -= 1;
                                1
                            } else {
                                0
                            }
                    };
                    let min_main = 0;

                    let (min_width, min_height) = axis.canon(min_main, 0);
                    let (max_width, max_height) = axis.canon(
                        max_main,
                        if fill_cross_div == 0 {
                            max_cross
                        } else {
                            used_cross
                        },
                    );

                    let child_limits = Limits::new(
                        Size::new(min_width, min_height),
                        Size::new(max_width, max_height),
                    );

                    let layout = model_layout(child, ctx, &child_limits);
                    used_cross = used_cross.max(layout.size().cross_for(axis));
                    children_layouts[i] = layout;
                }
            }

            let (main_padding, cross_padding) =
                axis.canon(full_padding.left, full_padding.right);
            let mut main_offset = main_padding;

            for (i, node) in children_layouts.iter_mut().enumerate() {
                if i > 0 {
                    main_offset += gap;
                }

                let (x, y) =
                    axis.canon(main_offset as i32, cross_padding as i32);

                node.move_mut(Point::new(x, y));
                node.align_mut(
                    horizontal_align,
                    vertical_align,
                    axis.canon(free_main, used_cross),
                );

                let size = node.size();

                main_offset += size.main_for(axis);
            }

            let (content_width, content_height) =
                axis.canon(main_offset - main_padding, used_cross);
            let size = limits
                .resolve_size(size, Size::new(content_width, content_height));

            LayoutModel::new(size, children_layouts)
        },
    }
}
