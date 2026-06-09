use crate::{
    el::{
        ElData, ElId, WithElId,
        arena::{ArenaChildren, ArenaEls, ElArena},
        ctx::{PageState, WidgetCtx},
    },
    font::{Font, FontCtx, FontProps, ResolvedFontProps},
    layout::model::LayoutModelNode,
    page::PageStyle,
    render::prelude::*,
    style::{TreeStyle, theme::Theme},
};
use core::{fmt::Display, hash::Hash, marker::PhantomData};
use itertools::Itertools as _;
use log::{debug, error};
use rsact_reactive::{
    prelude::*, runtime::get_observer, signal::marker::ReadOnly,
};
use rsact_render::color::ACCENT_COUNT;

pub struct CtxReady;
pub struct CtxUnready;

// TODO: Make RenderCtx a delegate to renderer so u can do `Primitive::(...).render(ctx)`
pub struct RenderCtx<'a, W: WidgetCtx, S = CtxUnready> {
    pub id: ElId,
    pub state: Signal<PageState<W>, ReadOnly>,
    pub renderer: &'a mut W::Renderer,
    pub layout: &'a LayoutModelNode<'a>,
    pub tree_style: TreeStyle<W::Color>,
    pub page_style: Signal<PageStyle<W::Color>, ReadOnly>,
    pub viewport: MaybeReactive<Size>,
    pub fonts: Signal<FontCtx, ReadOnly>,
    pub theme: Inert<Theme<W::Color>>,
    pub font_props: FontProps,
    pub force_redraw: Signal<bool>,
    pub parent_dirty: bool,

    // Nesting level only used for redraw debugging, making different colors on each call to distinguish between elements.
    pub nesting_level: usize,

    pub call: usize,

    pub _marker: PhantomData<S>,
}

impl<'a, W: WidgetCtx> RenderCtx<'a, W, CtxReady> {
    pub fn renderer(&mut self) -> &mut W::Renderer {
        self.renderer
    }

    #[must_use]
    pub fn render_font(
        &mut self,
        font: Font,
        content: &str,
        props: ResolvedFontProps,
        bounds: Rect,
        color: W::Color,
    ) -> RenderResult {
        self.fonts.with(|fonts| {
            fonts.render::<W>(
                font,
                content,
                props,
                bounds,
                color,
                self.renderer,
            )
        })
    }

    #[must_use]
    pub fn render_focus_outline(&mut self, id: ElId) -> RenderResult {
        if self.state.with(|state| state.is_focused(id)) {
            (Block {
                border: Border::zero()
                    // TODO: Theme focus color
                    .color(Some(<W::Color as Color>::accents()[1]))
                    .width(1),
                rect: self.layout.outer,
                background: None,
            })
            .render(self.renderer)
        } else {
            Ok(())
        }
    }

    #[must_use]
    pub fn with_tree_style<R>(
        &mut self,
        tree_style: impl FnOnce(TreeStyle<W::Color>) -> TreeStyle<W::Color>,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> R,
    ) -> R {
        f(RenderCtx {
            id: self.id,
            state: self.state,
            renderer: self.renderer,
            layout: self.layout,
            tree_style: tree_style(self.tree_style),
            page_style: self.page_style,
            viewport: self.viewport,
            fonts: self.fonts,
            theme: self.theme,
            font_props: self.font_props,
            force_redraw: self.force_redraw,
            parent_dirty: self.parent_dirty,
            nesting_level: self.nesting_level,
            call: self.call,
            _marker: PhantomData,
        })
    }

    pub fn clip_inner(
        &mut self,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> RenderResult,
    ) -> RenderResult {
        self.renderer.clipped(self.layout.inner, |renderer| {
            f(RenderCtx {
                id: self.id,
                state: self.state,
                renderer,
                layout: self.layout,
                tree_style: self.tree_style,
                page_style: self.page_style,
                viewport: self.viewport,
                fonts: self.fonts,
                theme: self.theme,
                font_props: self.font_props,
                force_redraw: self.force_redraw,
                parent_dirty: self.parent_dirty,
                nesting_level: self.nesting_level + 1,
                call: self.call,
                _marker: PhantomData,
            })
        })
    }

    pub fn get_style<Style: Copy>(
        &self,
        base: impl FnOnce(&Theme<W::Color>) -> Style,
        style: Option<&dyn Fn(Style) -> Style>,
    ) -> Style {
        let base = self.theme.with(base);
        style.map(|f| f(base)).unwrap_or(base)
    }
}

impl<'a, W: WidgetCtx> RenderCtx<'a, W, CtxUnready> {
    /// Render part of the widget that is dependent on some reactive state.
    // Note: Display is required for logs, but as for now, all render_part calls are used with a string to be hashed, so we either require it to always be a string or keep it so, idk.
    pub fn render_part<H: Display + Hash + Copy>(
        &mut self,
        hash_source: H,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> RenderResult,
    ) -> RenderResult {
        let render_id = WithElId::new(self.id, hash_source);

        // If the parent already cleared the area, force this child observer
        // to re-run so it redraws into the now-cleared region.
        if self.parent_dirty {
            get_observer(render_id).map(|observer| observer.dirten());
        }

        let result = observe(render_id, || {
            debug!(
                "{:indent$}Render {} [#{:?}]",
                "",
                hash_source,
                self.id,
                indent = self.nesting_level
            );

            // Clear our own rect only when the parent hasn't already cleared
            // the containing area (avoids redundant smaller fills inside a
            // larger background that was already repainted).
            if self.force_redraw.get() {
                // TODO: Full screen clear when force redraw so no need to clear for each widget?
                self.clear_outer()?;
            }

            // Pass parent_dirty=true into the closure so children of this
            // render_part know the area is already cleared.
            f(RenderCtx {
                id: self.id,
                state: self.state,
                renderer: self.renderer,
                layout: self.layout,
                tree_style: self.tree_style,
                page_style: self.page_style,
                viewport: self.viewport,
                fonts: self.fonts,
                theme: self.theme,
                font_props: self.font_props,
                force_redraw: self.force_redraw,
                parent_dirty: true,
                nesting_level: self.nesting_level + 1,
                call: self.call + 1,
                _marker: PhantomData,
            })
        });

        // Propagate dirty state back to self so sibling render_part / render_child
        // calls on the same ctx see that the area has been painted.
        if result.is_some() {
            self.parent_dirty = true;
        }

        result.unwrap_or(RenderResult::Ok(()))
    }

    #[must_use]
    pub fn render_self(
        &mut self,
        widget_name: &str,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> RenderResult,
    ) -> RenderResult {
        // TODO: Get rid of this, we have debug_name
        let render_id = format!("{widget_name}[render_self]");
        self.render_part(&render_id, f)
    }
}

impl<'a, W: WidgetCtx, S> RenderCtx<'a, W, S> {
    #[must_use]
    fn for_child<R>(
        &mut self,
        id: ElId,
        child_layout: &LayoutModelNode,
        f: impl FnOnce(RenderCtx<'_, W, CtxUnready>) -> R,
    ) -> R {
        let font_props = child_layout.font_props().unwrap_or(self.font_props);
        f(RenderCtx {
            id,
            state: self.state,
            renderer: self.renderer,
            layout: child_layout,
            tree_style: self.tree_style,
            page_style: self.page_style,
            viewport: self.viewport,
            fonts: self.fonts,
            theme: self.theme,
            font_props,
            force_redraw: self.force_redraw,
            parent_dirty: self.parent_dirty,
            nesting_level: self.nesting_level + 1,
            call: self.call,
            _marker: PhantomData,
        })
    }

    pub fn render<'arena>(mut self, arena: &'arena ElArena<W>) -> RenderResult {
        self.render_(self.id, &arena.els, &arena.children)
    }

    fn render_<'arena>(
        &mut self,
        id: ElId,
        arena: &'arena ArenaEls<W>,
        children: &'arena ArenaChildren,
    ) -> RenderResult {
        debug!("{:indent$}->", "", indent = self.nesting_level);

        let Some(data) = Self::expect_el(id, arena) else { return Ok(()) };

        self.render_el(id, data)?;

        if data.flags.transparent_layout {
            if let Some(children_ids) = children.get(id) {
                if children_ids.len() != 1 {
                    error!(
                        "Transparent widget with id {id:?} should have exactly one child, but has {}",
                        children_ids.len()
                    );

                    self.for_child(id, self.layout, |mut this| {
                        this.render_(id, arena, children)
                    })?;
                }
            } else {
                error!(
                    "Transparent widget with id {id:?} does not have child widget"
                );
            }
        } else if let Some(children_ids) = children.get(id) {
            debug!(
                "{:indent$}Children [{}]:",
                "",
                children_ids.len(),
                indent = self.nesting_level
            );

            for (child_id, child_layout) in
                children_ids.iter().zip_eq(self.layout.children())
            {
                self.for_child(*child_id, &child_layout, |mut this| {
                    this.render_(*child_id, arena, children)
                })?;
            }
        }

        debug!("{:indent$}<-", "", indent = self.nesting_level);

        Ok(())
    }

    fn render_el<'arena>(
        &mut self,
        id: ElId,
        data: &ElData<W>,
    ) -> RenderResult {
        // TODO: Observable rendering!!!

        debug!(
            "{:indent$}Render `{}` [{:?}]",
            "",
            data.debug_name,
            id,
            indent = self.nesting_level
        );

        data.widget.render(RenderCtx {
            id,
            state: self.state,
            renderer: self.renderer,
            layout: self.layout,
            tree_style: self.tree_style,
            page_style: self.page_style,
            viewport: self.viewport,
            fonts: self.fonts,
            theme: self.theme,
            font_props: self.font_props,
            force_redraw: self.force_redraw,
            parent_dirty: self.parent_dirty,
            nesting_level: self.nesting_level,
            call: self.call,
            _marker: PhantomData,
        })
    }

    fn expect_el<'arena>(
        id: ElId,
        arena: &'arena ArenaEls<W>,
    ) -> Option<&'arena ElData<W>> {
        if let Some(el) = arena.get(id).as_mut() {
            if let Some(data) = el.data.as_ref() {
                Some(data)
            } else {
                error!(
                    "Trying to run event on element with id {:?} that has no data",
                    id
                );
                None
            }
        } else {
            error!(
                "Trying to run event on non-existent element with id {:?}",
                id
            );
            None
        }
    }

    // #[must_use]
    // pub fn render_child(&mut self, child: &El<W>) -> RenderResult {
    //     self.render_children_inner(core::iter::once(child))
    // }

    // #[must_use]
    // fn render_children_inner<'c, C: Iterator<Item = &'c El<W>> + 'c>(
    //     &mut self,
    //     children: C,
    // ) -> RenderResult {
    //     children.zip_eq(self.layout.children()).try_for_each(
    //         |(child, child_layout)| {
    //             self.for_child(child.id(), &child_layout, |ctx| {
    //                 child.render(ctx)
    //             })
    //         },
    //     )
    // }

    // #[must_use]
    // pub fn render_children<'c>(
    //     &mut self,
    //     children: &MaybeSignal<Vec<El<W>>>,
    // ) -> RenderResult {
    //     let render_id = WithElId::new(self.id, "render_children");

    //     // If the parent already cleared the area, force the render_children
    //     // observer to re-run so children repaint into the cleared region.
    //     if self.parent_dirty {
    //         get_observer(render_id).map(|observer| observer.dirten());
    //     }

    //     // TODO: Create observe_with_force that forces execution based on boolean (for this case -- parent_dirty)
    //     let result = observe(render_id, || {
    //         debug!(
    //             "{:indent$}Render children [#{:?}]",
    //             "",
    //             self.id,
    //             indent = self.nesting_level
    //         );

    //         // Rendering children does not require to clear the rect unless force redraw is called. Because rendering children is done in such widgets that may not have render_self, meaning that nothing is drawn before the children, this allows safely redrawing only the children that are changed.
    //         if !self.parent_dirty && self.force_redraw.get() {
    //             self.clear_outer()?;
    //         }

    //         children.with(|children| {
    //             children.iter().zip_eq(self.layout.children()).try_for_each(
    //                 |(child, child_layout)| {
    //                     // Pass parent_dirty through so children know whether
    //                     // the containing area has already been cleared.
    //                     self.for_child(child.id(), &child_layout, |ctx| {
    //                         child.render(ctx)
    //                     })
    //                 },
    //             )
    //         })
    //     });

    //     result.unwrap_or(RenderResult::Ok(()))
    // }

    #[must_use]
    fn clear_outer(&mut self) -> RenderResult {
        // TODO: Feature-gated or debug-redraw flag
        // Debug redraws, works good only for colors with alpha. But we can use some bright background too
        self.renderer.rect(
            self.layout.outer,
            &DrawStyle::default().fill(
                W::Color::accents()
                    [(self.nesting_level + self.call) % ACCENT_COUNT],
            ),
            // .stroke(W::Color::accents()[4])
            // .stroke_width(1),
        )

        // self.page_style.with(|style| {
        //     if let Some(bg) = style.background_color {
        //         self.renderer.fill_solid(self.layout.outer, bg).map_err(|_| ())
        //     } else {
        //         Ok(())
        //     }
        // })
    }
}
