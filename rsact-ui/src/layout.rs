use core::u32;

use alloc::vec::Vec;
use embedded_graphics::{prelude::Point, primitives::Rectangle};
use log::{error, info};

use crate::{
    axis::{Axial, Axis},
    block::BoxModel,
    el::El,
    size::{DivFactors, Length, Size, SizeExt},
    widget::{AppState, LayoutCtx, Widget, WidgetCtx},
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

    pub fn unknown() -> Self {
        Self { min: Size::zero(), max: Size::new(u32::MAX, u32::MAX) }
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

pub enum Layout {
    Edge,
    Container {
        box_model: BoxModel,
        horizontal_align: Align,
        vertical_align: Align,
    },
    // TODO: Flex wrap
    Flex {
        wrap: bool,
        axis: Axis,
        gap: Size,
        box_model: BoxModel,
        horizontal_align: Align,
        vertical_align: Align,
    },
}

// impl Layout {
//     // /// Shrink limits by paddings to get free space for content/children
//     // fn content_limits(&self, limits: &Limits) -> Limits {
//     //     let full_padding = self.box_model.border + self.box_model.padding;
//     //     let self_limits = limits.limit_by(self.size);
//     //     self_limits.shrink(full_padding)
//     // }
// }

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
#[derive(Debug)]
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

pub fn model_layout<C: WidgetCtx>(
    el: &El<C>,
    ctx: &LayoutCtx<C>,
    parent_limits: &Limits,
) -> LayoutModel {
    let layout = el.layout(ctx);
    let size = el.size();
    let children = el.children();

    // TODO: Resolve size container against `content_size` (limits)

    match layout {
        Layout::Edge => LayoutModel::new(
            parent_limits.limit_by(size).resolve_size(size, Size::zero()),
            vec![],
        ),
        Layout::Container { box_model, horizontal_align, vertical_align } => {
            let full_padding = box_model.border + box_model.padding;
            let limits = parent_limits.limit_by(size).shrink(full_padding);

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
        // LayoutKind::Flex {
        //     wrap,
        //     axis,
        //     gap,
        //     box_model,
        //     horizontal_align,
        //     vertical_align,
        // } => {
        //     let full_padding = box_model.border + box_model.padding;
        //     let limits = limits.limit_by(size).shrink(full_padding);

        //     let total_gap = gap * children.len().saturating_sub(1) as u32;
        //     let max_cross = limits.max().cross_for(axis);

        //     let mut children_layouts = Vec::with_capacity(children.len());
        //     children_layouts
        //         .resize_with(children.len(), || LayoutModel::zero());

        //     let mut free_main =
        //         limits.max().main_for(axis).saturating_sub(total_gap);
        //     let mut used_cross = match axis {
        //         Axis::X if size.width == Length::Shrink => 0,
        //         Axis::Y if size.height == Length::Shrink => 0,
        //         _ => max_cross,
        //     };

        //     let mut total_main_divs = 0;

        //     // Calculate fixed and shrink children
        //     for (i, child) in children.iter().enumerate() {
        //         let (fill_main_div, fill_cross_div) = {
        //             let size = child.size();
        //             axis.canon(
        //                 size.width.div_factor(),
        //                 size.height.div_factor(),
        //             )
        //         };

        //         if fill_main_div == 0 {
        //             let (max_width, max_height) = axis.canon(
        //                 free_main,
        //                 if fill_cross_div == 0 {
        //                     max_cross
        //                 } else {
        //                     used_cross
        //                 },
        //             );

        //             let child_limits = Limits::new(
        //                 Size::zero(),
        //                 Size::new(max_width, max_height),
        //             );

        //             let layout = model_layout(child, ctx, &child_limits);
        //             let size = layout.size();

        //             free_main -= size.main_for(axis);
        //             used_cross = used_cross.max(size.cross_for(axis));

        //             children_layouts[i] = layout;
        //         } else {
        //             total_main_divs += fill_main_div as u32;
        //         }
        //     }

        //     // Remaining main axis length after calculating sizes of
        //     // non-auto-sized children
        //     let remaining = match axis {
        //         Axis::X => match size.width {
        //             Length::Shrink => 0,
        //             _ => free_main.max(0),
        //         },
        //         Axis::Y => match size.height {
        //             Length::Shrink => 0,
        //             _ => free_main.max(0),
        //         },
        //     };
        //     let remaining_div =
        //         remaining.checked_div(total_main_divs).unwrap_or(0);
        //     let mut remaining_mod =
        //         remaining.checked_rem(total_main_divs).unwrap_or(0);

        //     // Calculate auto-sized children (Length::Fill, Length::Div(N))
        //     for (i, child) in children.iter().enumerate() {
        //         let (fill_main_div, fill_cross_div) = {
        //             let size = child.size();

        //             axis.canon(
        //                 size.width.div_factor(),
        //                 size.height.div_factor(),
        //             )
        //         };

        //         if fill_main_div != 0 {
        //             let max_main = if total_main_divs == 0 {
        //                 remaining
        //             } else {
        //                 remaining_div * fill_main_div as u32
        //                     + if remaining_mod > 0 { remaining_mod -= 1; 1
        //                     } else {
        //                         0
        //                     }
        //             };
        //             let min_main = 0;

        //             let (min_width, min_height) = axis.canon(min_main, 0);
        //             let (max_width, max_height) = axis.canon(
        //                 max_main,
        //                 if fill_cross_div == 0 {
        //                     max_cross
        //                 } else {
        //                     used_cross
        //                 },
        //             );

        //             let child_limits = Limits::new(
        //                 Size::new(min_width, min_height),
        //                 Size::new(max_width, max_height),
        //             );

        //             let layout = model_layout(child, ctx, &child_limits);
        //             used_cross =
        // used_cross.max(layout.size().cross_for(axis));
        // children_layouts[i] = layout;         }
        //     }

        //     let (main_padding, cross_padding) =
        //         axis.canon(full_padding.left, full_padding.right);
        //     let mut main_offset = main_padding;

        //     for (i, node) in children_layouts.iter_mut().enumerate() {
        //         if i > 0 {
        //             main_offset += gap;
        //         }

        //         let (x, y) =
        //             axis.canon(main_offset as i32, cross_padding as i32);

        //         node.move_mut(Point::new(x, y));
        //         node.align_mut(
        //             horizontal_align,
        //             vertical_align,
        //             axis.canon(free_main, used_cross),
        //         );

        //         let size = node.size();

        //         main_offset += size.main_for(axis);
        //     }

        //     let (content_width, content_height) =
        //         axis.canon(main_offset - main_padding, used_cross);
        //     let size = limits
        //         .resolve_size(size, Size::new(content_width,
        // content_height));

        //     LayoutModel::new(size, children_layouts)
        // },
        Layout::Flex {
            wrap,
            axis,
            gap,
            box_model,
            horizontal_align,
            vertical_align,
        } => {
            struct FlexItem {
                // Cross axis line number
                line: usize,
                last_in_line: bool,
            }

            // Single main axis line in flexbox
            #[derive(Clone, Copy)]
            struct FlexLine {
                div_factors: DivFactors,
                // free_main: u32,
                // max_cross: u32,
                items_count: u32,
                fluid_space: Size,
            }

            let full_padding = box_model.border + box_model.padding;
            let limits = parent_limits.limit_by(size).shrink(full_padding);

            let max_main = limits.max().main(axis);
            let max_cross = limits.max().cross(axis);

            let new_line = FlexLine {
                div_factors: DivFactors::zero(),
                items_count: 0,
                fluid_space: Size::new(max_main, 0),
            };

            let mut items: Vec<FlexItem> = Vec::with_capacity(children.len());
            let mut lines = vec![new_line];

            let children_content_sizes = children
                .iter()
                .map(|child| child.content_size())
                .collect::<Vec<_>>();

            let mut children_layouts = Vec::with_capacity(children.len());
            children_layouts
                .resize_with(children.len(), || LayoutModel::zero());

            let mut container_free_cross = max_cross;
            for ((i, child), child_content_size) in
                children.iter().enumerate().zip(children_content_sizes)
            {
                let min_item_size =
                    child.size().max_fixed(child_content_size.min());

                let last_line = *lines.last().unwrap();

                let free_main = last_line.fluid_space.main(axis);

                // Allow fluid item to wrap the container even if it is a shrink
                // length, so it fits its content.
                if wrap
                    && (free_main < min_item_size.main(axis)
                        || i != 0 && free_main < gap.main(axis))
                {
                    // On wrap, set main axis to the max limit (the width or
                    // height of the container) and cross
                    container_free_cross = container_free_cross
                        .saturating_sub(last_line.fluid_space.cross(axis));
                    lines.push(new_line);
                    if let Some(last_item) = items.last_mut() {
                        last_item.last_in_line = true;
                    }
                } else if i != 0 && gap.main(axis) > 0 {
                    *lines.last_mut().unwrap().fluid_space.main_mut(axis) =
                        lines
                            .last()
                            .unwrap()
                            .fluid_space
                            .main(axis)
                            .saturating_sub(gap.main(axis));
                }

                let line_number = lines.len() - 1;
                let line = lines.last_mut().unwrap();

                let child_size = child.size();

                let max_cross = if child_size.main(axis).div_factor() == 0 {
                    let child_layout = model_layout(
                        child,
                        ctx,
                        &Limits::only_max(axis.canon(
                            line.fluid_space.main(axis),
                            container_free_cross,
                        )),
                    );

                    // Min content size of child must have been less or equal to
                    // resulting size.
                    // FIXME: Remove, it MUST never happen because we set
                    // free_main as max limit
                    debug_assert!(
                        child_layout.size().main(axis)
                            <= line.fluid_space.main(axis)
                    );

                    let child_layout_size = child_layout.size();

                    children_layouts[i] = child_layout;

                    // Subtract known children main axis length from free space
                    // to overflow. Free cross axis is
                    // calculated on wrap
                    *line.fluid_space.main_mut(axis) = line
                        .fluid_space
                        .main_mut(axis)
                        .saturating_sub(child_layout_size.main(axis));

                    // Subtract actual cross axis length from remaining space
                    child_layout_size.cross(axis)
                } else {
                    // Allow fluid-sized items to take minimum content size on
                    // cross axis
                    min_item_size.cross(axis)
                };

                // Calculate total divisions for a line, even for fixed items,
                // where main axis div factor is 0 but cross axis div can be
                // non-zero
                line.div_factors += child_size.div_factors();
                *line.fluid_space.cross_mut(axis) =
                    line.fluid_space.cross(axis).max(max_cross);
                line.items_count += 1;

                items.push(FlexItem { line: line_number, last_in_line: false });
            }

            items.last_mut().map(|last| {
                last.last_in_line = true;
            });

            #[derive(Clone, Copy)]
            struct ModelLine {
                base_divs: Size,
                line_div_remainder: Size,
                line_div_remainder_rem: u32,
                base_div_remainder_part: u32,
                used_main: u32,
                max_cross: u32,
            }

            let mut model_lines = lines
                .into_iter()
                .map(|line| {
                    let base_divs = line.fluid_space / line.div_factors;
                    let line_div_remainder =
                        line.fluid_space % line.div_factors;
                    let base_div_remainder_part =
                        line_div_remainder.main(axis) / line.items_count;
                    let line_div_remainder_rem =
                        line_div_remainder.main(axis) % line.items_count;

                    ModelLine {
                        base_divs,
                        line_div_remainder,
                        line_div_remainder_rem,
                        base_div_remainder_part,
                        used_main: 0,
                        max_cross: line.fluid_space.cross(axis),
                    }
                })
                .collect::<Vec<_>>();

            let mut longest_line = 0;
            let mut used_cross = 0;
            let mut next_pos = Point::zero();
            for ((i, child), item) in
                children.iter().enumerate().zip(items.iter())
            {
                let child_size = child.size();
                let model_line = &mut model_lines[item.line];

                let child_div_factors = child_size.div_factors();
                if child_div_factors.main(axis) != 0 {
                    let child_rem_part = model_line.base_div_remainder_part
                        + if model_line.line_div_remainder_rem > 0 {
                            model_line.line_div_remainder_rem -= 1;
                            1
                        } else {
                            0
                        };

                    let child_max_size = child_size
                        .into_fixed(model_line.base_divs)
                        + axis.canon::<Size>(child_rem_part, 0);

                    model_line.line_div_remainder -= child_rem_part;

                    children_layouts[i] = model_layout(
                        child,
                        ctx,
                        &Limits::only_max(child_max_size),
                    );
                }

                children_layouts[i].move_mut(next_pos);

                let child_size = children_layouts[i].size();

                let child_length = child_size.main(axis);
                model_line.used_main += child_length;

                if item.last_in_line {
                    *next_pos.main_mut(axis) = 0;
                    *next_pos.cross_mut(axis) +=
                        (model_line.max_cross + gap.cross(axis)) as i32;

                    longest_line = longest_line.max(model_line.used_main);
                    used_cross += model_line.max_cross
                        + if item.line < model_lines.len() - 1 {
                            gap.cross(axis)
                        } else {
                            0
                        };
                } else {
                    model_line.used_main += gap.main(axis);

                    *next_pos.main_mut(axis) = model_line.used_main as i32;
                }
            }

            let size =
                limits.resolve_size(size, axis.canon(longest_line, used_cross));

            for (child_layout, item) in children_layouts.iter_mut().zip(items) {
                let line = model_lines[item.line];

                let free_space =
                    size - axis.canon::<Size>(line.used_main, used_cross);
                child_layout.align_mut(
                    horizontal_align,
                    vertical_align,
                    free_space,
                );
            }

            LayoutModel::new(size, children_layouts)
        },
    }
}
