use crate::layout::length::LengthSize;
use crate::render::prelude::*;
use crate::{
    font::FontProps,
    layout::{
        Align, ContainerLayout, ContentLayout, DevHoveredLayout, LayoutCtx,
        LayoutKind, Limits, ScrollableLayout, flex::model_flex, node::Layout,
    },
};
use alloc::vec::Vec;
use core::fmt::Debug;
use core::fmt::Display;
use rsact_reactive::prelude::*;

#[cfg(feature = "debug-info")]
use crate::layout::{DevLayout, DevLayoutKind};

/// Layout tree representation with real position in viewport
pub struct LayoutModelNode<'a> {
    pub outer: Rect,
    pub inner: Rect,
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
    pub fn font_props(&self) -> Option<FontProps> {
        self.model.font_props
    }

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
    outer: Rect,
    inner: Rect,

    font_props: Option<FontProps>,

    children: Vec<LayoutModel>,

    // Note: `dev` goes before `children` which is intentional to make more
    // readable pretty-printed debug
    // TODO: Make debug_assertions-only
    #[cfg(feature = "debug-info")]
    dev: DevLayout,
    // TODO: Tinyvec
}

impl LayoutModel {
    pub fn new(
        inner_size: Size,
        children: Vec<LayoutModel>,
        #[cfg(feature = "debug-info")] dev: DevLayout,
    ) -> Self {
        Self {
            outer: Rect::new(Point::zero(), inner_size),
            inner: Rect::new(Point::zero(), inner_size),
            children,
            font_props: None,
            #[cfg(feature = "debug-info")]
            dev,
        }
    }

    pub fn with_font_props(mut self, fp: Option<FontProps>) -> Self {
        self.font_props = fp;
        self
    }

    /// Full padding includes padding + border size
    pub fn with_full_padding(mut self, full_padding: Padding) -> Self {
        let padding_size: Size = full_padding.into();

        self.inner = self.inner.translate(full_padding.top_left());

        let new_size = self.outer.size + padding_size;
        self.outer = Rect::new(self.outer.top_left, new_size);
        self
    }

    pub fn tree_root(&self) -> LayoutModelNode<'_> {
        LayoutModelNode { outer: self.outer, inner: self.inner, model: self }
    }

    fn node(&self, parent_inner: Rect) -> LayoutModelNode<'_> {
        LayoutModelNode {
            outer: self.outer.translate(parent_inner.top_left),
            inner: self.inner.translate(parent_inner.top_left),
            model: self,
        }
    }

    pub fn zero() -> Self {
        Self {
            outer: Rect::zero(),
            inner: Rect::zero(),
            children: vec![],
            font_props: None,
            #[cfg(feature = "debug-info")]
            dev: DevLayout::zero(),
        }
    }

    pub fn outer_size(&self) -> Size {
        self.outer.size
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

    pub fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.outer.top_left += by;
        self.inner.top_left += by;
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

pub struct PPLayoutModel<'a> {
    model: &'a LayoutModel,
    indent: usize,
}

impl<'a> PPLayoutModel<'a> {
    pub fn root(model: &'a LayoutModel) -> Self {
        Self { model, indent: 0 }
    }
}

impl<'a> Display for PPLayoutModel<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:indent$}{}>{}",
            "",
            self.model.outer,
            self.model.inner,
            indent = self.indent
        )?;

        #[cfg(feature = "debug-info")]
        write!(f, " {}", self.model.dev)?;

        self.model.children.iter().try_for_each(|child| {
            write!(
                f,
                "\n{}",
                PPLayoutModel { model: child, indent: self.indent + 1 }
            )
        })
    }
}

// TODO: Split layouts decorations and layout modeling logic into separate files

// TODO: Should viewport be unwrapped value as we depend modeling on viewport value?
pub fn model_layout(
    ctx: &LayoutCtx,
    layout: Layout,
    parent_limits: Limits,
    parent_size: LengthSize, // viewport: Memo<Size>,
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
            LayoutKind::Edge => LayoutModel::new(
                parent_limits.resolve_size(size, Size::zero(), None),
                vec![],
                #[cfg(feature = "debug-info")]
                DevLayout::new(size, DevLayoutKind::Edge),
            ),
            LayoutKind::Content(content_layout) => {
                let min_content = content_layout.min_size(ctx);
                let layout_font_props = match content_layout {
                    ContentLayout::Text { font_props: text_fp, .. }
                        if text_fp.has_any() =>
                    {
                        let resolved = text_fp.inherited(&ctx.font_props);
                        Some(resolved)
                    },
                    _ => None,
                };

                LayoutModel::new(
                    parent_limits.resolve_size(size, min_content, None),
                    vec![],
                    #[cfg(feature = "debug-info")]
                    DevLayout::new(
                        size,
                        DevLayoutKind::Content(content_layout.clone()),
                    ),
                )
                .with_font_props(layout_font_props)
            },
            LayoutKind::Container(container_layout) => {
                let ContainerLayout {
                    block_model,
                    horizontal_align,
                    vertical_align,
                    content,
                    font_props: container_fp,
                } = container_layout;

                // let min_content = content_size.get().min();

                let full_padding = block_model.full_padding();

                let child_fp = container_fp.inherited(&ctx.font_props);
                let child_ctx = LayoutCtx { font_props: child_fp, ..*ctx };

                let content_layout = model_layout(
                    &child_ctx,
                    *content,
                    parent_limits.child_limits(size).shrink(full_padding),
                    size,
                );

                let content_size = content_layout.outer_size();
                let real_size = parent_limits.resolve_size(
                    size,
                    content_size,
                    Some(full_padding),
                );
                let content_layout = content_layout.aligned(
                    *horizontal_align,
                    *vertical_align,
                    real_size - content_size,
                );

                LayoutModel::new(
                    // TODO: Generalize logic with real_size.expand/shrink and
                    // full_padding
                    real_size,
                    vec![content_layout],
                    #[cfg(feature = "debug-info")]
                    DevLayout::new(
                        size,
                        DevLayoutKind::Container(container_layout.clone()),
                    ),
                )
                .with_full_padding(full_padding)
                .with_font_props(container_fp.has_any().then_some(child_fp))
            },
            LayoutKind::Scrollable(scrollable_layout) => {
                let ScrollableLayout { content, font_props: scrollable_fp } =
                    scrollable_layout;

                let child_fp = scrollable_fp.inherited(&ctx.font_props);
                let child_ctx = LayoutCtx { font_props: child_fp, ..*ctx };

                let content_layout = model_layout(
                    &child_ctx,
                    *content,
                    parent_limits.child_limits(size),
                    size,
                );

                let real_size = parent_limits.resolve_size(
                    size,
                    content_layout.outer_size(),
                    None,
                );

                LayoutModel::new(
                    real_size,
                    vec![content_layout],
                    #[cfg(feature = "debug-info")]
                    DevLayout::new(
                        size,
                        DevLayoutKind::Scrollable(scrollable_layout.clone()),
                    ),
                )
                .with_font_props(scrollable_fp.has_any().then_some(child_fp))
            },
            LayoutKind::Flex(flex_layout) => {
                model_flex(ctx, parent_limits, flex_layout, size)
            },
        }
    })
}
