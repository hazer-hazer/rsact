use crate::{
    font::FontCtx,
    layout::{
        Align, ContainerLayout, DevHoveredLayout, LayoutCtx, LayoutKind,
        Limits, ScrollableLayout,
        flex::model_flex,
        node::Layout,
        padding::Padding,
        size::{Length, Size},
    },
};
use alloc::vec::Vec;
use core::fmt::Debug;
use embedded_graphics::{
    geometry::Point, primitives::Rectangle, transform::Transform as _,
};
use rsact_reactive::prelude::*;

#[cfg(feature = "debug-info")]
use crate::layout::{DevLayout, DevLayoutKind};

/// Layout tree representation with real position in viewport
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
#[derive(Debug, PartialEq)]
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
    pub fn with_full_padding(mut self, full_padding: Padding) -> Self {
        let padding_size: Size = full_padding.into();

        self.inner = self.inner.translate(full_padding.top_left());

        self.outer = self.outer.resized(
            self.outer.size + padding_size.into(),
            embedded_graphics::geometry::AnchorPoint::TopLeft,
        );
        self
    }

    pub fn tree_root(&self) -> LayoutModelNode<'_> {
        LayoutModelNode { outer: self.outer, inner: self.inner, model: self }
    }

    fn node(&self, parent_inner: Rectangle) -> LayoutModelNode<'_> {
        LayoutModelNode {
            outer: self.outer.translate(parent_inner.top_left),
            inner: self.inner.translate(parent_inner.top_left),
            model: self,
        }
    }

    pub fn zero() -> Self {
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

    pub fn translate_mut(&mut self, by: impl Into<Point> + Copy) -> &mut Self {
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

// TODO: Split layouts decorations and layout modeling logic into separate files

// TODO: Should viewport be unwrapped value as we depend modeling on viewport value?
pub fn model_layout(
    ctx: &LayoutCtx,
    layout: Layout,
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
