use super::{
    FlexLayout, LayoutCtx, LayoutModel, Limits,
    size::{Length, Size},
};
use crate::layout::{
    Align,
    axis::Axial as _,
    model_layout,
    size::{DivFactors, SubTake as _},
};
use alloc::vec::Vec;
use embedded_graphics::prelude::Point;
use rsact_reactive::prelude::*;

// TODO: Wrap and gap are not taken into account
// TODO: Move usage of this function into FlexLayout::base function accepting list of maybe reactive children
// pub fn flex_content_size<'a, W: WidgetCtx, E: Widget<W> + 'a>(
//     axis: Axis,
//     children: impl Iterator<Item = &'a E>,
// ) -> Limits {
//     children.fold(Limits::unlimited(), |limits, child| {
//         let child_limits = child.layout().with(|child| child.content_size());
//         Limits::new(
//             axis.infix(
//                 limits.min(),
//                 child_limits.min(),
//                 |lhs: u32, rhs: u32| SaturatingAdd::saturating_add(&lhs, &rhs),
//                 core::cmp::max,
//             ),
//             axis.infix(
//                 limits.max(),
//                 child_limits.max(),
//                 |lhs: u32, rhs: u32| SaturatingAdd::saturating_add(&lhs, &rhs),
//                 core::cmp::max,
//             ),
//         )
//     })
// }

struct FlexItem {
    // Line number
    line: usize,
    /// Marker for item which is the last in line
    last_in_line: bool,
}

// Single main axis line in flexbox
#[derive(Clone, Copy)]
struct FlexLine {
    div_factors: DivFactors,
    /// Number of items line contains
    items_count: u32,
    /// Available main axis left
    free_main: u32,
    /// The amount of main axis length left for fluid items determined by their minimum size.
    fluid_main_space: u32,
    /// Maximum item cross axis size in line
    max_cross: u32,
}

pub fn model_flex(
    ctx: LayoutCtx,
    // TODO: Replace with parent max size as parent_limits.min is not used at all.
    flex_layout: &FlexLayout,
    parent_limits: Memo<Limits>,
    size: Memo<Size<Length>>,
    // viewport: Memo<Size>,
) -> Memo<LayoutModel> {
    let &FlexLayout {
        wrap,
        block_model,
        axis,
        gap,
        align,
        children,
        font_props: _,
    } = flex_layout;

    let rsize = size;

    let content_limits = map!(move |block_model| {
        let full_padding = block_model.full_padding();

        with!(move |size, parent_limits| {
            parent_limits.limit_by(size).shrink(full_padding)
        })
    });

    let children_layouts = map!(move |content_limits,
                                      wrap,
                                      gap,
                                      children,
                                      size| {
        let wrap = *wrap;
        let gap = *gap;

        let (max_possible_main, max_possible_cross) =
            content_limits.max().destruct(axis);

        let children_count = children.len();

        let const_new_line = FlexLine {
            div_factors: DivFactors::zero(),
            items_count: 0,
            free_main: max_possible_main,
            fluid_main_space: 0,
            max_cross: 0,
        };

        let mut items: Vec<FlexItem> = Vec::with_capacity(children_count);
        let mut lines = vec![const_new_line];

        let mut children_layouts = Vec::with_capacity(children_count);
        children_layouts
            .resize_with(children_count, || LayoutModel::zero().inert().memo());

        let (gap_main, gap_cross) = gap.destruct(axis);

        let mut container_free_cross = max_possible_cross;

        for (item_index, child) in children.iter().copied().enumerate() {
            let (child_size, child_min_size) =
                child.with(|child| (child.size.get(), child.min_size(&ctx)));

            let min_item_size = child_size.max_fixed(child_min_size);
            let needed_item_space = min_item_size
                + if item_index != 0 { gap } else { Size::zero() };

            {
                // Wrapping //
                let last_line = lines.last().unwrap();
                let free_main = last_line.free_main;

                // Allow fluid item to wrap the container even if it is a
                // shrink length, so it fits its content.
                if wrap
                    && free_main.saturating_sub(last_line.fluid_main_space)
                        < needed_item_space.main(axis)
                {
                    // On wrap, set main axis to the max limit (the width or
                    // height of the container) and cross

                    // container_free_cross = container_free_cross
                    //     .saturating_sub(last_line.fluid_space.
                    // cross(axis));
                    container_free_cross = container_free_cross
                        .saturating_sub(last_line.max_cross);

                    if let Some(last_item) = items.last_mut() {
                        last_item.last_in_line = true;
                    }

                    lines.push(const_new_line);
                } else if last_line.items_count != 0 {
                    lines.last_mut().unwrap().free_main =
                        free_main.saturating_sub(gap_main);
                }
            }

            let line_number = lines.len() - 1;
            let line = lines.last_mut().unwrap();

            let child_div_factors = child_size.div_factors();

            // Calculate Fixed/Shrink items layouts
            if child_div_factors.main(axis) == 0 {
                let child_layout = child.with(|child| {
                    model_layout(
                        ctx,
                        child,
                        Limits::only_max(
                            // child_min_size,
                            axis.canon(line.free_main, container_free_cross),
                        )
                        .inert()
                        .memo(),
                        rsize,
                        // viewport,
                    )
                });

                // TODO: Not working properly
                // // Min content size of child must have been less or
                // // equal to resulting size.
                // debug_assert!(
                //     child_layout.outer_size().main(axis) <= line.free_main
                // );

                let child_layout_size =
                    child_layout.with(LayoutModel::outer_size);

                children_layouts[item_index] = child_layout;

                // Subtract known children main axis length from free space to overflow. Free cross axis is calculated on wrap
                line.free_main =
                    line.free_main.saturating_sub(child_layout_size.main(axis));

                line.max_cross =
                    line.max_cross.max(child_layout_size.cross(axis));
            } else {
                // TODO: Is it right to use min size of a child to determine max_cross? It won't let fill sized elements to grow.
                // - But otherwise how do we determine the amount the element should grow? We only exactly know fixed sizes of elements before computing fluid sizes. So fluid elements grow to maximum of fixed size used.
                line.max_cross = line.max_cross.max(child_min_size.cross(axis));
                line.fluid_main_space += needed_item_space.main(axis);
            }

            // Calculate total divisions for a line, even for fixed
            // items, where main axis div factor is
            // 0 but cross axis div can be non-zero
            line.div_factors += child_div_factors;
            line.items_count += 1;

            items.push(FlexItem {
                line: line_number,
                last_in_line: item_index == children_count - 1,
            });
        }

        lines.last().map(|line| {
            container_free_cross =
                container_free_cross.saturating_sub(line.max_cross);
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
        // TODO: Should find GCD or use some other technique to avoid remainders greater than children count.
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
                    debug_assert_eq!(
                        container_free_cross_div,
                        container_free_cross
                    );
                    debug_assert_eq!(container_free_cross_rem, 0);

                    *div_factors.cross_mut(axis) = 1;

                    match size.cross(axis) {
                        // TODO: InfiniteWindow inner
                        Length::InfiniteWindow(_) | Length::Shrink => {
                            // line.max_fixed_cross
                            line.max_cross
                        },
                        // Note: We can use max_possible_cross as we have one line,
                        // so it fills the parent and no wrap logic applied
                        Length::Div(_) => max_possible_cross,
                        Length::Fixed(fixed) => {
                            // line.max_fixed_cross
                            fixed
                        },
                    }
                } else {
                    line.max_cross
                        + if size.cross(axis).is_grow() {
                            container_free_cross_div
                                + if container_free_cross_rem > 0 {
                                    container_free_cross_rem -= 1;
                                    1
                                } else {
                                    0
                                }
                        } else {
                            0
                        }
                };

                let fluid_space = axis.canon::<Size>(line.free_main, cross);

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

        // TODO: Should flex item be at least of min content size?

        for ((i, child), item) in children.iter().enumerate().zip(items.iter())
        {
            let child_min_size = child.with(|child| child.min_size(&ctx));
            let child_size = child.with(|child| child.size.get());
            let model_line = &mut model_lines[item.line];

            let child_div_factors = child_size.div_factors();
            if child_div_factors.main(axis) != 0 {
                let child_rem_part = model_line.base_div_remainder_part
                    + model_line.line_div_remainder_rem.sub_take(1);

                let child_max_size = child_size
                    .into_fixed(model_line.base_divs)
                    + child_rem_part;

                model_line.line_div_remainder -= child_rem_part;

                children_layouts[i] = child.with(|child| {
                    model_layout(
                        ctx,
                        child,
                        Limits::new(child_min_size, child_max_size)
                            .inert()
                            .memo(),
                        rsize,
                        // viewport,
                    )
                });
            }

            children_layouts[i] = children_layouts[i]
                .map(|child_layout| child_layout.translated(next_pos));

            let child_size = children_layouts[i].with(LayoutModel::outer_size);

            let child_length = child_size.main(axis);
            model_line.used_main += child_length;

            if item.last_in_line {
                // TODO: Rewrite to this
                // longest_line = longest_line.max(model_line.used_main);
                // used_cross += model_line.cross
                //     + if item.line < model_lines.len() - 1 { gap.cross(axis)
                //     } else {
                //         0
                //     };

                // *next_pos.main_mut(axis) = 0;
                // *next_pos.cross_mut(axis) += used_cross as i32;

                longest_line = longest_line.max(model_line.used_main);

                used_cross += model_line.cross
                    + if item.line < model_lines.len() - 1 {
                        gap_cross
                    } else {
                        0
                    };

                *next_pos.main_mut(axis) = 0;
                *next_pos.cross_mut(axis) = used_cross as i32;
                // (model_line.cross.saturating_add(gap.cross(axis))) as i32;
            } else {
                model_line.used_main += gap_main;

                *next_pos.main_mut(axis) = model_line.used_main as i32;
            }
        }
    });

    map!(move |block_model, content_limits, align, children_layouts| {
        let layout_size = with!(|size, content_limits| content_limits
            .resolve_size(size, axis.canon(longest_line, used_cross)));

        // TODO: Review alignments
        if !matches!(
            (horizontal_align, vertical_align),
            (Align::Start, Align::Start)
        ) {
            for (child_layout, item) in children_layouts.iter_mut().zip(items) {
                let line = model_lines[item.line];

                let free_space = axis
                    .canon::<Size>(layout_size.main(axis), line.cross)
                    - axis.canon::<Size>(
                        line.used_main,
                        child_layout.outer_size().cross(axis),
                    );
                child_layout.align_mut(
                    horizontal_align,
                    vertical_align,
                    free_space,
                );
            }
        }

        LayoutModel::new(
            layout_size,
            children_layouts,
            #[cfg(feature = "debug-info")]
            DevLayout::new(
                size,
                crate::layout::DevLayoutKind::Flex(DevFlexLayout {
                    // TODO: Implement dev representation of lines such as browsers does.
                    // lines: model_lines.iter().fold((Vec::new(),
                    // Point::zero()),|line| {
                    //     Rectangle::new(line.)
                    // }),
                    real: flex_layout.clone(),
                }),
            ),
        )
        .with_full_padding(full_padding)
    })
}
