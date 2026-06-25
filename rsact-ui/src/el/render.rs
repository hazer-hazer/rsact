use crate::{
    el::{
        ClipPath, ElId, WidgetFlags, WithElId,
        arena::{ArenaChildren, ArenaEls, ElArena},
        ctx::{PageState, WidgetCtx},
    },
    font::{Font, FontCtx, FontProps, ResolvedFontProps},
    layout::model::LayoutModelNode,
    page::PageStyle,
    render::prelude::*,
    style::{
        Style, StylePseudoClass, StyleSelector, TreeStyle, stylist::Stylist,
    },
};
use core::{fmt::Display, hash::Hash, marker::PhantomData};
use itertools::Itertools as _;
use log::{debug, error};
use rsact_reactive::{prelude::*, signal::marker::ReadOnly};
use rsact_render::color::ACCENT_COUNT;

pub struct CtxReady;
pub struct CtxUnready;

// TODO: Make RenderCtx a delegate to renderer so u can do
// `Primitive::(...).render(ctx)`? Maybe later, and surely not .render(ctx), at
// least .render(ctx.renderer), otherwise it breaks encapsulation of the crates.

pub struct RenderShared<'a, W: WidgetCtx> {
    pub page_state: &'a PageState<W>,
    pub page_style: Signal<PageStyle<W::Color>, ReadOnly>,
    pub viewport: MaybeReactive<Size>,
    pub fonts: Signal<FontCtx, ReadOnly>,
    pub stylist: &'a W::Stylist,
    /// Page-level flag that triggers a full redraw (e.g. after layout change).
    pub force_redraw: Signal<bool>,
}

impl<'a, W: WidgetCtx> Clone for RenderShared<'a, W> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, W: WidgetCtx> Copy for RenderShared<'a, W> {}

pub struct RenderVisual<W: WidgetCtx> {
    pub tree_style: TreeStyle<W::Color>,
    pub font_props: FontProps,
}

impl<W: WidgetCtx> Clone for RenderVisual<W> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<W: WidgetCtx> Copy for RenderVisual<W> {}

#[derive(Clone, Copy)]
pub struct RenderFrame {
    parent_dirty: bool,
    nesting_level: usize,
    call: usize,
}

impl RenderFrame {
    pub fn root(call: usize) -> Self {
        Self { parent_dirty: false, nesting_level: 0, call }
    }
}

pub struct RenderCtx<'a, W: WidgetCtx, S = CtxUnready> {
    pub id: ElId,
    debug_name: &'a str,
    dirten: &'a mut bool,
    needs_redraw: bool,
    hovered: bool,

    pub renderer: &'a mut W::Renderer,
    pub layout: &'a LayoutModelNode<'a>,
    /// Inheritable visual properties (tree_style, font_props).
    pub visual: RenderVisual<W>,
    /// Per-element rendering state (dirty flags, nesting, call counter).
    frame: RenderFrame,
    /// Shared page-level context (signals, page state, stylist).
    pub shared: RenderShared<'a, W>,
    _marker: PhantomData<S>,
}

impl<'a, W: WidgetCtx> RenderCtx<'a, W, CtxReady> {
    #[must_use]
    pub fn render_font(
        &mut self,
        font: Font,
        content: &str,
        props: ResolvedFontProps,
        bounds: Rect,
        color: W::Color,
    ) -> RenderResult {
        self.shared.fonts.with(|fonts| {
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
        if self.shared.page_state.is_focused(id) {
            Block::from_layout_style(
                self.layout.outer,
                BlockModel::zero(),
                BlockStyle::base().outline(
                    OutlineStyle::base()
                        .width(1)
                        .color(<W::Color as Color>::accents()[1]),
                ),
            )
            .render(self.renderer)
        } else {
            Ok(())
        }
    }

    /// Create a sub-context with a modified `tree_style`.
    #[must_use]
    pub fn with_tree_style<R>(
        &mut self,
        tree_style: impl FnOnce(TreeStyle<W::Color>) -> TreeStyle<W::Color>,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> R,
    ) -> R {
        f(RenderCtx {
            id: self.id,
            debug_name: self.debug_name,
            dirten: self.dirten,
            needs_redraw: self.needs_redraw,
            hovered: self.hovered,
            renderer: self.renderer,
            layout: self.layout,
            visual: RenderVisual {
                tree_style: tree_style(self.visual.tree_style),
                font_props: self.visual.font_props,
            },
            frame: self.frame,
            shared: self.shared,
            _marker: PhantomData,
        })
    }

    /// Returns the current style pseudo-class based on hover / focus state.
    ///
    /// `hovered` is pre-extracted into `frame.hovered` by [`RenderPass`]
    /// before the arena borrow was released, so this method needs no arena
    /// access.
    pub fn pseudoclass(&self) -> StylePseudoClass {
        debug!(
            "State for pseudoclass: hovered={} focused={}",
            self.hovered,
            self.shared.page_state.is_focused(self.id)
        );
        StylePseudoClass::default()
            .hovered(self.hovered)
            .focused(self.shared.page_state.is_focused(self.id))
    }

    pub fn get_style<S: Style>(
        &self,
        style: Option<&dyn Fn(&S, &StyleSelector) -> S>,
    ) -> S
    where
        W::Stylist: Stylist<S>,
    {
        let pseudoclass = self.pseudoclass();
        let selector = StyleSelector { pseudoclass };
        let base = self.shared.stylist.style(&S::base(), &selector);
        style.map(|f| f(&base, &selector)).unwrap_or(base)
    }

    /// Clip subsequent drawing operations to the layout's inner rect.
    ///
    /// Only `renderer` changes (to the clipped sub-renderer); every other
    /// field is `Copy` so construction is a single struct-update expression.
    #[must_use]
    pub fn clip_inner(
        &mut self,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> RenderResult,
    ) -> RenderResult {
        let inner = self.layout.inner;
        self.renderer.clipped(inner, |renderer| {
            f(RenderCtx {
                id: self.id,
                debug_name: self.debug_name,
                dirten: self.dirten,
                needs_redraw: self.needs_redraw,
                hovered: self.hovered,
                renderer,
                layout: self.layout,
                visual: self.visual,
                frame: self.frame,
                shared: self.shared,
                _marker: PhantomData,
            })
        })
    }
}

// CtxUnready //
impl<'a, W: WidgetCtx> RenderCtx<'a, W, CtxUnready> {
    pub fn render_part<H: Display + Hash + Copy>(
        &mut self,
        hash_source: H,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> RenderResult,
    ) -> RenderResult {
        // Imperative force-dirty flags that triggers redraw even if no reactive
        // dependencies changed in the `observe`
        let redraw = self.frame.parent_dirty || self.needs_redraw;

        let result = observe_with_force(
            WithElId::new(self.id, hash_source),
            redraw,
            || {
                debug!(
                    "{:indent$}Render {} [#{:?}]",
                    "",
                    hash_source,
                    self.id,
                    indent = self.frame.nesting_level
                );

                // Track force_redraw so this observer automatically re-runs
                // when the page-level force-redraw flag is set
                // (e.g. after layout change).
                self.shared.force_redraw.track();

                // Clear the element rect unless the parent already did so.
                //
                // Moved inside `observe` (vs old code where it was outside) so
                // the clear is always paired with an actual
                // redraw — never a clear-without-redraw or a
                // redraw-without-clear.
                if !self.frame.parent_dirty {
                    self.clear_outer()?;
                }

                f(RenderCtx {
                    id: self.id,
                    debug_name: self.debug_name,
                    dirten: self.dirten,
                    needs_redraw: self.needs_redraw,
                    hovered: self.hovered,
                    renderer: self.renderer,
                    layout: self.layout,
                    visual: self.visual,
                    shared: self.shared,
                    // Children inside this closure see parent_dirty=true
                    // because we just cleared/drew into
                    // this element's area above.
                    frame: RenderFrame {
                        parent_dirty: true,
                        nesting_level: self.frame.nesting_level + 1,
                        call: self.frame.call + 1,
                    },
                    _marker: PhantomData,
                })
            },
        );

        if result.is_some() {
            self.frame.parent_dirty = true;
            *self.dirten = true;
        }

        result.unwrap_or(RenderResult::Ok(()))
    }

    #[must_use]
    pub fn render_self(
        &mut self,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> RenderResult,
    ) -> RenderResult {
        // TODO: Maybe we can store preformatted string render_id for each
        // widget?
        let render_id = format!("{}[render_self]", self.debug_name);
        self.render_part(&render_id, f)
    }
}

impl<'a, W: WidgetCtx, S> RenderCtx<'a, W, S> {
    fn clear_outer(&mut self) -> RenderResult {
        // TODO: Feature-gated or debug-redraw flag
        // Debug redraws, works good only for colors with alpha. But we can use
        // some bright background too TODO: Actually, this should happen
        // after draw [ ] better when render_pass added, or do it right
        // now as a separate call.

        // self.renderer.rect(
        //     self.layout.outer,
        //     &DrawStyle::default().fill(
        //         W::Color::accents()[(self.frame.nesting_level
        //             + self.frame.call)
        //             % ACCENT_COUNT],
        //     ),
        //     // .stroke(W::Color::accents()[4])
        //     // .stroke_width(1),
        // )

        self.shared.page_style.with(|style| {
            if let Some(bg) = style.background_color {
                self.renderer.fill_solid(self.layout.outer, bg).map_err(|_| ())
            } else {
                Ok(())
            }
        })
    }
}

pub(crate) struct RenderPass<'a, W: WidgetCtx> {
    arena: &'a mut ElArena<W>,
    renderer: &'a mut W::Renderer,
    shared: RenderShared<'a, W>,
}

impl<'a, W: WidgetCtx> RenderPass<'a, W> {
    pub fn new(
        arena: &'a mut ElArena<W>,
        renderer: &'a mut W::Renderer,
        shared: RenderShared<'a, W>,
    ) -> Self {
        Self { arena, renderer, shared }
    }

    pub fn render(
        &mut self,
        root: ElId,
        layout: &LayoutModelNode<'_>,
        visual: RenderVisual<W>,
        frame: RenderFrame,
    ) -> RenderResult {
        render_subtree(
            &mut self.arena.els,
            &self.arena.children,
            self.renderer,
            self.shared,
            root,
            layout,
            visual,
            frame,
        )
    }
}

fn render_subtree<W: WidgetCtx>(
    els: &mut ArenaEls<W>,
    children: &ArenaChildren,
    renderer: &mut W::Renderer,
    shared: RenderShared<'_, W>,
    id: ElId,
    layout: &LayoutModelNode<'_>,
    visual: RenderVisual<W>,
    frame: RenderFrame,
) -> RenderResult {
    debug!("{:indent$}->", "", indent = frame.nesting_level);

    // TODO: Get rid of double element access
    let (clip_path,) = {
        let Some(data) = els.expect(id) else { return Ok(()) };
        (data.state.clip_path,)
    };

    // Build the per-element frame.
    let child_frame = RenderFrame {
        parent_dirty: frame.parent_dirty,
        nesting_level: frame.nesting_level,
        call: frame.call,
    };

    match clip_path {
        None => render_subtree_body(
            els,
            children,
            renderer,
            shared,
            id,
            layout,
            visual,
            child_frame,
        ),
        Some(ClipPath::InnerRect) => {
            renderer.clipped(layout.inner, |renderer| {
                render_subtree_body(
                    els,
                    children,
                    renderer,
                    shared,
                    id,
                    layout,
                    visual,
                    child_frame,
                )
            })
        },
    }
}

fn render_subtree_body<W: WidgetCtx>(
    els: &mut ArenaEls<W>,
    children: &ArenaChildren,
    renderer: &mut W::Renderer,
    shared: RenderShared<'_, W>,
    id: ElId,
    layout: &LayoutModelNode<'_>,
    visual: RenderVisual<W>,
    frame: RenderFrame,
) -> RenderResult {
    let needs_redraw = els
        .expect_mut(id)
        .map(|data| data.state.take_needs_redraw().is_some())
        .unwrap_or(false);

    let Some(data) = els.expect(id) else { return Ok(()) };

    debug!(
        "{:indent$}Render `{}` [{:?}]",
        "",
        data.state.debug_name,
        id,
        indent = frame.nesting_level
    );

    let mut dirten = false;
    let ctx = RenderCtx {
        id,
        debug_name: data.state.debug_name,
        dirten: &mut dirten,
        needs_redraw,
        hovered: data.state.hovered(),
        renderer,
        layout,
        visual,
        frame,
        shared,
        _marker: PhantomData::<CtxUnready>,
    };
    data.widget.render(ctx)?;

    let children_frame = RenderFrame {
        parent_dirty: dirten,
        nesting_level: frame.nesting_level + 1,
        ..frame
    };

    if data.state.flags.transparent_layout {
        // Transparent widget: child inherits the parent's layout rect.
        // Must have exactly one child.
        let children_ids = children.get(id).map(|c| c.to_vec());
        if let Some(children_ids) = children_ids {
            if children_ids.len() == 1 {
                let child_id = children_ids[0];
                render_subtree(
                    els,
                    children,
                    renderer,
                    shared,
                    child_id,
                    // transparent widget child reuses layout
                    layout,
                    visual,
                    children_frame,
                )?;
            } else {
                error!(
                    "Transparent widget with id {id:?} should have exactly one child"
                );
            }
        }
    } else {
        if let Some(children_ids) = children.get(id) {
            debug!(
                "{:indent$}Children [{}]:",
                "",
                children_ids.len(),
                indent = frame.nesting_level
            );
            for (child_id, child_layout) in
                children_ids.iter().zip_eq(layout.children())
            {
                let child_font_props =
                    child_layout.font_props().unwrap_or(visual.font_props);

                let child_visual = RenderVisual {
                    font_props: child_font_props,
                    tree_style: visual.tree_style,
                };

                render_subtree(
                    els,
                    children,
                    renderer,
                    shared,
                    *child_id,
                    &child_layout,
                    child_visual,
                    children_frame,
                )?;
            }
        }
    }

    debug!("{:indent$}<-", "", indent = frame.nesting_level);

    Ok(())
}
