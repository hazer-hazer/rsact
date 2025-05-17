use super::{
    RenderResult, Widget,
    prelude::{Color, Size},
};
use crate::{
    el::{El, ElId, WithElId},
    event::{
        Capture, CaptureData, Event, EventResponse, FocusEvent, Propagate,
    },
    font::{AbsoluteFontProps, Font, FontCtx, FontProps},
    layout::{Layout, LayoutModelNode},
    page::{PageStyle, id::PageId},
    render::{Block, Border, Renderable as _, Renderer},
    style::{TreeStyle, WidgetStyle, WidgetStylist},
};
use alloc::vec::Vec;
use core::{fmt::Debug, hash::Hash, marker::PhantomData};
use embedded_graphics::{prelude::DrawTarget, primitives::Rectangle};
use itertools::Itertools as _;
use rsact_reactive::{
    prelude::*, runtime::get_observer, signal::marker::ReadOnly,
};

// TODO: Not an actual context, rename to something like `WidgetTypeFamily`
pub trait WidgetCtx: Sized + Clone + 'static {
    type Renderer: Renderer<Color = Self::Color>;
    type Styler: PartialEq + Copy;
    type Color: Color;
    type PageId: PageId;
    type CustomEvent: Debug;

    // Methods delegated from renderer //
    fn default_background() -> Self::Color {
        Self::Color::default_background()
    }

    fn default_foreground() -> Self::Color {
        Self::Color::default_foreground()
    }
}

/// WidgetTypeFamily
/// Type family of types used in Widgets
pub struct Wtf<R, S, I, E = ()>
where
    R: Renderer,
{
    _renderer: PhantomData<R>,
    _styler: PhantomData<S>,
    _page_id: PhantomData<I>,
    _event: PhantomData<E>,
}

impl<R, S, I, E> Clone for Wtf<R, S, I, E>
where
    R: Renderer,
{
    fn clone(&self) -> Self {
        Self {
            _renderer: self._renderer.clone(),
            _styler: self._styler.clone(),
            _page_id: self._page_id.clone(),
            _event: self._event.clone(),
        }
    }
}

impl<R, E, S, I> Wtf<R, E, S, I>
where
    R: Renderer,
{
    pub fn new() -> Self {
        Self {
            _renderer: PhantomData,
            _event: PhantomData,
            _styler: PhantomData,
            _page_id: PhantomData,
        }
    }
}

impl<R, S, I, E> WidgetCtx for Wtf<R, S, I, E>
where
    R: Renderer + DrawTarget<Color = <R as Renderer>::Color> + 'static,
    S: PartialEq + Copy + 'static,
    I: PageId + 'static,
    E: Debug + 'static,
{
    type Renderer = R;
    type Color = <R as DrawTarget>::Color;
    type Styler = S;
    type PageId = I;
    type CustomEvent = E;
}

pub struct PageState<W: WidgetCtx> {
    /// Element id + its absolute tree index among all focusable elements (see [`PageTree`])
    pub focused: Option<(ElId, usize)>,

    ctx: PhantomData<W>,
}

impl<W: WidgetCtx> PageState<W> {
    pub fn new() -> Self {
        Self { focused: None, ctx: PhantomData }
    }

    pub fn is_focused(&self, id: ElId) -> bool {
        self.focused.map(|focused| focused.0 == id).unwrap_or(false)
    }
}

pub struct CtxReady;
pub struct CtxUnready;

pub struct RenderSelf;

// TODO: Make RenderCtx a delegate to renderer so u can do `Primitive::(...).render(ctx)`
pub struct RenderCtx<'a, W: WidgetCtx, S = CtxUnready> {
    pub id: ElId,
    pub state: Signal<PageState<W>, ReadOnly>,
    renderer: &'a mut W::Renderer,
    pub layout: &'a LayoutModelNode<'a>,
    pub tree_style: TreeStyle<W::Color>,
    pub page_style: Signal<PageStyle<W::Color>, ReadOnly>,
    pub viewport: Memo<Size>,
    pub fonts: Signal<FontCtx, ReadOnly>,
    force_redraw: Trigger,
    parent_dirty: bool,

    ctx_state: PhantomData<S>,
}

impl<'a, W: WidgetCtx> RenderCtx<'a, W, RenderSelf> {
    pub fn renderer(&mut self) -> &mut W::Renderer {
        self.renderer
    }

    #[must_use]
    pub fn render_font(
        &mut self,
        font: Font,
        content: &str,
        props: AbsoluteFontProps,
        bounds: Rectangle,
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
}

impl<'a, W: WidgetCtx, S> RenderCtx<'a, W, S> {
    #[must_use]
    pub fn for_child<R>(
        &mut self,
        id: ElId,
        child_layout: &LayoutModelNode,
        f: impl FnOnce(&mut RenderCtx<'_, W, CtxUnready>) -> R,
    ) -> R {
        f(&mut (RenderCtx {
            id,
            state: self.state,
            renderer: self.renderer,
            layout: child_layout,
            tree_style: self.tree_style,
            page_style: self.page_style,
            viewport: self.viewport,
            fonts: self.fonts,
            force_redraw: self.force_redraw,
            parent_dirty: self.parent_dirty,
            ctx_state: PhantomData,
        }))
    }

    #[must_use]
    pub fn with_tree_style<R>(
        &mut self,
        tree_style: impl FnOnce(TreeStyle<W::Color>) -> TreeStyle<W::Color>,
        f: impl FnOnce(&mut RenderCtx<'_, W, S>) -> R,
    ) -> R {
        f(&mut (RenderCtx {
            id: self.id,
            state: self.state,
            renderer: self.renderer,
            layout: self.layout,
            tree_style: tree_style(self.tree_style),
            page_style: self.page_style,
            viewport: self.viewport,
            fonts: self.fonts,
            force_redraw: self.force_redraw,
            parent_dirty: self.parent_dirty,
            ctx_state: PhantomData,
        }))
    }

    pub fn clip_inner(
        &mut self,
        f: impl FnOnce(&mut RenderCtx<'_, W, S>) -> RenderResult,
    ) -> RenderResult {
        self.renderer.clipped(self.layout.inner, |renderer| {
            f(&mut (RenderCtx {
                id: self.id,
                state: self.state,
                renderer,
                layout: self.layout,
                tree_style: self.tree_style,
                page_style: self.page_style,
                viewport: self.viewport,
                fonts: self.fonts,
                force_redraw: self.force_redraw,
                parent_dirty: self.parent_dirty,
                ctx_state: PhantomData,
            }))
        })
    }
}

impl<'a, W: WidgetCtx + 'static> RenderCtx<'a, W, CtxUnready> {
    pub fn new(
        id: ElId,
        state: Signal<PageState<W>, ReadOnly>,
        renderer: &'a mut W::Renderer,
        layout: &'a LayoutModelNode<'a>,
        tree_style: TreeStyle<W::Color>,
        page_style: Signal<PageStyle<W::Color>, ReadOnly>,
        viewport: Memo<Size>,
        fonts: Signal<FontCtx, ReadOnly>,
        force_redraw: Trigger,
    ) -> Self {
        Self {
            id,
            state,
            renderer,
            layout,
            tree_style,
            page_style,
            viewport,
            fonts,
            force_redraw,
            parent_dirty: false,
            ctx_state: PhantomData,
        }
    }

    pub fn render_part<H: Hash + Copy>(
        &mut self,
        hash: H,
        f: impl FnOnce(&mut RenderCtx<'_, W, RenderSelf>) -> RenderResult,
    ) -> RenderResult {
        let render_id = WithElId::new(self.id, hash);

        if self.parent_dirty {
            get_observer(render_id).map(|observer| observer.dirten());
        }

        observe(render_id, || {
            if !self.parent_dirty {
                self.clear_outer()?;
            }

            self.parent_dirty = true;
            self.force_redraw.track();

            f(&mut (RenderCtx {
                id: self.id,
                state: self.state,
                renderer: self.renderer,
                layout: self.layout,
                tree_style: self.tree_style,
                page_style: self.page_style,
                viewport: self.viewport,
                fonts: self.fonts,
                force_redraw: self.force_redraw,
                parent_dirty: self.parent_dirty,
                ctx_state: PhantomData,
            }))
        })
        .unwrap_or(RenderResult::Ok(()))
    }

    #[must_use]
    pub fn render_self(
        &mut self,
        f: impl FnOnce(&mut RenderCtx<'_, W, RenderSelf>) -> RenderResult,
    ) -> RenderResult {
        self.render_part("render", f)
    }

    #[must_use]
    pub fn render_child(&mut self, child: &El<W>) -> RenderResult {
        self.render_children_inner(core::iter::once(child))
    }

    #[must_use]
    fn render_children_inner<'c, C: Iterator<Item = &'c El<W>> + 'c>(
        &mut self,
        children: C,
    ) -> RenderResult {
        let prev_dirty = self.parent_dirty;
        let result = children.zip_eq(self.layout.children()).try_for_each(
            |(child, child_layout)| {
                self.for_child(child.id(), &child_layout, |ctx| {
                    child.render(ctx)
                })
            },
        );
        self.parent_dirty = prev_dirty;
        result
    }

    #[must_use]
    pub fn render_children<'c>(
        &mut self,
        children: &MaybeSignal<Vec<El<W>>>,
    ) -> RenderResult {
        observe(WithElId::new(self.id, "render_children"), || {
            children.with(|children| {
                children.iter().zip_eq(self.layout.children()).try_for_each(
                    |(child, child_layout)| {
                        self.for_child(child.id(), &child_layout, |ctx| {
                            child.render(ctx)
                        })
                    },
                )
            })
        })
        .unwrap_or(RenderResult::Ok(()))
    }

    #[must_use]
    fn clear_outer(&mut self) -> RenderResult {
        self.page_style.with(|style| {
            if let Some(bg) = style.background_color {
                self.renderer.fill_solid(&self.layout.outer, bg).map_err(|_| ())
            } else {
                Ok(())
            }
        })
    }
}

// TODO: Move to event mod?
pub struct EventCtx<'a, W: WidgetCtx> {
    pub id: ElId,
    pub event: &'a Event<W::CustomEvent>,
    pub page_state: Signal<PageState<W>>,
    pub layout: &'a LayoutModelNode<'a>,
    // TODO: Instant now, already can get it from queue!
}

impl<'a, W: WidgetCtx> Copy for EventCtx<'a, W> {}

impl<'a, W: WidgetCtx> Clone for EventCtx<'a, W> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            event: self.event,
            page_state: self.page_state.clone(),
            layout: self.layout,
        }
    }
}

impl<'a, W: WidgetCtx + 'static> EventCtx<'a, W> {
    #[must_use]
    pub fn pass_to_children(
        &mut self,
        children: &mut [El<W>],
    ) -> EventResponse {
        for (child, child_layout) in
            children.iter_mut().zip_eq(self.layout.children())
        {
            child.on_event(EventCtx {
                id: child.id(),
                event: self.event,
                page_state: self.page_state,
                layout: &child_layout,
            })?;
        }
        self.ignore()
    }

    pub fn pass_to_child(&mut self, child: &mut El<W>) -> EventResponse {
        self.pass_to_children(core::slice::from_mut(child))
    }

    // TODO: Do not receive id, we already have it, same for `handle_focusable`
    pub fn is_focused(&self) -> bool {
        self.page_state.with(|page_state| page_state.is_focused(self.id))
    }

    #[must_use]
    pub fn handle_focusable(
        &mut self,
        press: impl FnOnce(&mut Self, bool) -> EventResponse,
    ) -> EventResponse {
        if let &Event::Focus(FocusEvent::Focus(new_focus)) = self.event {
            if new_focus == self.id {
                return self.capture();
            }
        }

        if self.is_focused() {
            match self.event {
                Event::Press(press_event) => {
                    let pressed = match press_event {
                        crate::event::PressEvent::Press => true,
                        crate::event::PressEvent::Release => false,
                    };

                    press(self, pressed)
                },
                _ => self.ignore(),
            }
        } else {
            self.ignore()
        }
    }

    #[inline]
    pub fn capture(&self) -> EventResponse {
        EventResponse::Break(Capture::Captured(CaptureData {
            absolute_position: self.layout.outer.top_left,
        }))
    }

    #[inline]
    pub fn ignore(&self) -> EventResponse {
        EventResponse::Continue(Propagate::Ignored)
    }
}

pub struct MountCtx<W: WidgetCtx> {
    pub viewport: Memo<Size>,
    pub styler: Memo<W::Styler>,
    pub inherit_font_props: FontProps,
}

impl<W: WidgetCtx> MountCtx<W> {
    pub fn accept_styles<
        I: Clone + 'static,
        S: WidgetStyle<Inputs = I> + 'static,
    >(
        &self,
        style: MemoChain<S>,
        inputs: impl Into<MaybeSignal<I>>,
    ) where
        W::Styler: WidgetStylist<S>,
    {
        let styler = self.styler;
        let inputs = inputs.into();
        style
            .first(move |base| { styler.get().style()(base.clone(), inputs.get_cloned()) })
            // TODO: Don't panic, better overwrite `first` but emit a warning
            .unwrap();
    }

    // Note: Setting inherited font is not a reactive process. If user didn't set the font, the inherited is set. But user cannot unset font, thus font never fallbacks to inherited.
    pub fn inherit_font_props(self, mut layout: Signal<Layout>) -> Self {
        // Set inherited font props in layout
        layout.update_untracked(|layout| {
            if let Some(font_props) = layout.font_props_mut() {
                font_props.inherit(&self.inherit_font_props);
            }
        });

        // Set new inherited font for use with children
        if let Some(font_props) = layout.with(|layout| layout.font_props()) {
            Self { inherit_font_props: font_props, ..self }
        } else {
            self
        }
    }

    pub fn pass_to_child(
        self,
        this_layout: Signal<Layout>,
        child: &mut impl Widget<W>,
    ) {
        child.on_mount(self.inherit_font_props(this_layout));
    }

    pub fn pass_to_children(
        mut self,
        this_layout: Signal<Layout>,
        children: &mut MaybeSignal<Vec<El<W>>>,
    ) {
        self = self.inherit_font_props(this_layout);

        if let Some(inert) = children.as_inert_mut() {
            inert.iter_mut().for_each(|child| child.on_mount(self));
        } else if let Some(mut children) = children.as_signal() {
            create_effect(move |_| {
                children.track();
                children.update(|children| {
                    children.iter_mut().for_each(|child| child.on_mount(self));
                });
            });
        }
    }

    // TODO: Use watch?
    // FIXME: Wtf
    // TODO: Use computed
    // pub fn pass_to_children(
    //     self,
    //     children: impl RwSignal<Vec<El<W>>> + 'static,
    // ) {
    //     use_effect(move |_| {
    //         children.track();
    //         children.update_untracked(|children| {
    //             for child in children {
    //                 child.on_mount(self);
    //             }
    //         });
    //     });
    // }

    // pub fn pass_to_child(self, child: impl RwSignal<El<W>> + 'static) {
    //     use_effect(move |_| {
    //         child.track();
    //         child.update_untracked(|child| {
    //             child.on_mount(self);
    //         });
    //     });
    // }

    // TODO: Rewrite with lenses
    // pub fn pass_to_children(self, children: &mut [El<W>]) {
    //     for child in children {
    //         child.on_mount(self);
    //     }
    // }

    // pub fn pass_to_child(self, child: &mut El<W>) {
    //     child.on_mount(self);
    // }
}

impl<W: WidgetCtx> Clone for MountCtx<W> {
    fn clone(&self) -> Self {
        Self {
            viewport: self.viewport.clone(),
            styler: self.styler.clone(),
            inherit_font_props: self.inherit_font_props.clone(),
        }
    }
}

impl<W: WidgetCtx> Copy for MountCtx<W> {}
