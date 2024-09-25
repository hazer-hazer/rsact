use super::{
    size::{Length, Size},
    FlexLayout, Layout, LayoutModel, Limits,
};
use crate::layout::{
    axis::Axial as _,
    model_layout,
    padding::Padding,
    size::{DivFactors, SubTake as _},
};
use alloc::vec::Vec;
use embedded_graphics::prelude::Point;
use rsact_core::prelude::*;

pub fn model_flex(
    tree: MemoTree<Layout>,
    parent_limits: Limits,
    flex_layout: FlexLayout,
    size: Size<Length>,
) -> LayoutModel {
    let FlexLayout {
        wrap,
        box_model,
        axis,
        gap,
        horizontal_align,
        vertical_align,
    } = flex_layout;

    let full_padding =
        box_model.padding + Padding::new_equal(box_model.border_width);

    struct FlexItem {
        // Cross axis line number
        line: usize,
        last_in_line: bool,
    }

    // Single main axis line in flexbox
    #[derive(Clone, Copy)]
    struct FlexLine {
        div_factors: DivFactors,
        items_count: u32,
        free_main: u32,
        max_fixed_cross: u32,
    }

    let limits = parent_limits.limit_by(size).shrink(full_padding);

    let children_count = tree.children.with(Vec::len);
    let max_main = limits.max().main(axis);
    let max_cross = limits.max().cross(axis);

    let new_line = FlexLine {
        div_factors: DivFactors::zero(),
        items_count: 0,
        // fluid_space: axis.canon(max_main, 0),
        free_main: max_main,
        max_fixed_cross: 0,
    };

    let mut items: Vec<FlexItem> = Vec::with_capacity(children_count);
    let mut lines = vec![new_line];

    let children_content_sizes = tree.children.with(|children| {
        children
            .iter()
            .map(|child| child.data.with(|child| child.content_size.get()))
            .collect::<Vec<_>>()
    });

    let mut children_layouts = Vec::with_capacity(children_count);
    children_layouts.resize_with(children_count, || LayoutModel::zero());

    let mut container_free_cross = max_cross;
    tree.children.with(|children| {
        for ((i, child), child_content_size) in
            children.iter().enumerate().zip(children_content_sizes)
        {
            let child_size = child.data.with(|child| child.size);
            let min_item_size = child_size.max_fixed(child_content_size.min());

            let line = *lines.last().unwrap();
            let free_main = line.free_main;

            // Allow fluid item to wrap the container even if it is a
            // shrink length, so it fits its content.
            if wrap
                && (free_main < min_item_size.main(axis)
                    || i != 0 && free_main < gap.main(axis))
            {
                // On wrap, set main axis to the max limit (the width or
                // height of the container) and cross
                // container_free_cross = container_free_cross
                //     .saturating_sub(last_line.fluid_space.
                // cross(axis));
                container_free_cross =
                    container_free_cross - line.max_fixed_cross;

                lines.push(new_line);

                if let Some(last_item) = items.last_mut() {
                    last_item.last_in_line = true;
                }
            } else if line.items_count != 0 {
                lines.last_mut().unwrap().free_main = lines
                    .last()
                    .unwrap()
                    .free_main
                    .saturating_sub(gap.main(axis));
            }

            let line_number = lines.len() - 1;
            let line = lines.last_mut().unwrap();

            let child_div_factors = child_size.div_factors();

            if child_div_factors.main(axis) == 0 {
                let child_layout = model_layout(
                    *child,
                    Limits::only_max(
                        axis.canon(line.free_main, container_free_cross),
                    ),
                );

                // Min content size of child must have been less or
                // equal to resulting size.
                // FIXME: Remove, it MUST never happen because we set
                // free_main as max limit
                debug_assert!(child_layout.size().main(axis) <= line.free_main);

                let child_layout_size = child_layout.size();

                children_layouts[i] = child_layout;

                // Subtract known children main axis length from free
                // space to overflow. Free cross
                // axis is calculated on wrap
                line.free_main =
                    line.free_main.saturating_sub(child_layout_size.main(axis));

                // Subtract actual cross axis length from remaining
                // space
                line.max_fixed_cross =
                    line.max_fixed_cross.max(child_layout_size.cross(axis));
            }

            // Calculate total divisions for a line, even for fixed
            // items, where main axis div factor is
            // 0 but cross axis div can be non-zero
            line.div_factors += child_div_factors;
            line.items_count += 1;

            items.push(FlexItem { line: line_number, last_in_line: false });
        }
    });

    items.last_mut().map(|last| {
        last.last_in_line = true;
    });

    #[derive(Clone, Copy)]
    struct ModelLine {
        base_divs: Size,
        line_div_remainder: Size,
        line_div_remainder_rem: Size,
        base_div_remainder_part: Size,
        used_main: u32,
        cross: u32,
    }

    let lines_count = lines.len() as u32;

    let container_free_cross_div =
        container_free_cross.checked_div(lines_count).unwrap_or(0);
    let mut container_free_cross_rem =
        container_free_cross.checked_rem(lines_count).unwrap_or(0);
    let mut model_lines = lines
        .into_iter()
        .map(|line| {
            // let base_divs = if line.has_fluid {
            //     line.fluid_space / line.div_factors
            // } else {
            //     axis.canon(
            //         line.fluid_space.main(axis)
            //             / line.div_factors.main(axis) as u32,
            //         max_cross,
            //     )
            // };
            let mut div_factors = line.div_factors;
            let cross = if lines_count == 1 {
                *div_factors.cross_mut(axis) = 1;
                max_cross
            } else {
                line.max_fixed_cross
                    + container_free_cross_div
                    + if container_free_cross_rem > 0 {
                        container_free_cross_rem -= 1;
                        1
                    } else {
                        0
                    }
            };

            let fluid_space = axis.canon::<Size>(line.free_main, cross);

            // let main_base_div =
            //     line.free_main / line.div_factors.main(axis) as u32;
            // let line_div_remainder =
            //     line.fluid_space % line.div_factors;
            let base_divs = fluid_space / div_factors;
            let line_div_remainder = fluid_space % div_factors;

            let base_div_remainder_part = axis.canon(
                line_div_remainder
                    .main(axis)
                    .checked_div(line.items_count)
                    .unwrap_or(0),
                line_div_remainder
                    .cross(axis)
                    .checked_div(lines_count)
                    .unwrap_or(0),
            );
            let line_div_remainder_rem = axis.canon(
                line_div_remainder
                    .main(axis)
                    .checked_rem(line.items_count)
                    .unwrap_or(0),
                line_div_remainder
                    .cross(axis)
                    .checked_rem(lines_count)
                    .unwrap_or(0),
            );

            ModelLine {
                base_divs,
                line_div_remainder,
                line_div_remainder_rem,
                base_div_remainder_part,
                used_main: 0,
                cross,
            }
        })
        .collect::<Vec<_>>();

    let mut longest_line = 0;
    let mut used_cross = 0;
    let mut next_pos = Point::zero();

    tree.children.with(|children| {
        for ((i, child), item) in children.iter().enumerate().zip(items.iter())
        {
            let child_size = child.data.with(|child| child.size);
            let model_line = &mut model_lines[item.line];

            let child_div_factors = child_size.div_factors();
            if child_div_factors.main(axis) != 0 {
                let child_rem_part = model_line.base_div_remainder_part
                    + model_line.line_div_remainder_rem.sub_take(1);

                let child_max_size = child_size
                    .into_fixed(model_line.base_divs)
                    + child_rem_part;

                model_line.line_div_remainder -= child_rem_part;

                children_layouts[i] =
                    model_layout(*child, Limits::only_max(child_max_size));
            }

            children_layouts[i].move_mut(next_pos);

            let child_size = children_layouts[i].size();

            let child_length = child_size.main(axis);
            model_line.used_main += child_length;

            if item.last_in_line {
                *next_pos.main_mut(axis) = 0;
                *next_pos.cross_mut(axis) +=
                    (model_line.cross.saturating_add(gap.cross(axis))) as i32;

                longest_line = longest_line.max(model_line.used_main);
                used_cross += model_line.cross
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
    });

    let size = limits.resolve_size(size, axis.canon(longest_line, used_cross));

    for (child_layout, item) in children_layouts.iter_mut().zip(items) {
        let line = model_lines[item.line];

        let free_space = size - axis.canon::<Size>(line.used_main, used_cross);
        child_layout.align_mut(horizontal_align, vertical_align, free_space);
    }

    LayoutModel::new(
        size.expand(full_padding),
        children_layouts,
        crate::layout::DevLayoutKind::Flex(flex_layout),
    )
}
