use core::marker::PhantomData;
use prelude::*;

pub type DrawResult = Result<(), ()>;

pub trait WidgetCtx: 'static {
    type Renderer: Renderer<Color = Self::Color>;
    type Event: Event;
    type Styler;

    type Color: Color;

    // Methods delegated from renderer //
    fn default_background() -> Self::Color {
        Self::Color::default_background()
    }

    fn default_foreground() -> Self::Color {
        Self::Color::default_foreground()
    }
}

pub struct PhantomWidgetCtx<R, E, S>
where
    R: Renderer,
    E: Event,
{
    renderer: PhantomData<R>,
    event: PhantomData<E>,
    styler: S,
}

impl<R, E, S> WidgetCtx for PhantomWidgetCtx<R, E, S>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: 'static,
{
    type Renderer = R;
    type Event = E;
    type Color = R::Color;
    type Styler = S;
}

pub struct PageState<C: WidgetCtx> {
    pub focused: Option<ElId>,

    ctx: PhantomData<C>,
}

impl<C: WidgetCtx> PageState<C> {
    pub fn new() -> Self {
        Self { focused: None, ctx: PhantomData }
    }
}

pub struct LayoutCtx<'a, C: WidgetCtx> {
    pub page_state: &'a PageState<C>,
}

pub struct DrawCtx<'a, C: WidgetCtx> {
    pub state: &'a PageState<C>,
    pub renderer: &'a mut C::Renderer,
    pub layout: &'a LayoutModelTree<'a>,
    pub styler: &'a C::Styler,
}

impl<'a, C: WidgetCtx + 'static> DrawCtx<'a, C> {
    #[must_use]
    pub fn draw_child(&mut self, child: &El<C>) -> DrawResult {
        child.draw(&mut DrawCtx {
            state: &self.state,
            renderer: &mut self.renderer,
            layout: self.layout.children().next().as_ref().unwrap(),
            styler: &self.styler,
        })
    }

    #[must_use]
    pub fn draw_children(&mut self, children: &[El<C>]) -> DrawResult {
        children.iter().zip(self.layout.children()).try_for_each(
            |(child, child_layout)| {
                child.draw(&mut DrawCtx {
                    state: self.state,
                    renderer: &mut self.renderer,
                    layout: &child_layout,
                    styler: &self.styler,
                })
            },
        )
    }

    #[must_use]
    pub fn draw_focus_outline(&mut self, id: ElId) -> DrawResult {
        if self.state.focused == Some(id) {
            self.renderer.block(Block {
                border: Border::zero()
                    .color(Some(<C::Color as Color>::default_foreground()))
                    .width(2),
                rect: self.layout.area,
                background: None,
            })
        } else {
            Ok(())
        }
    }
}

pub struct EventCtx<'a, C: WidgetCtx> {
    pub event: &'a C::Event,
    pub page_state: &'a mut PageState<C>,
    pub layout: &'a LayoutModelTree<'a>,
    // TODO: Instant now
}

impl<'a, C: WidgetCtx + 'static> EventCtx<'a, C> {
    #[must_use]
    pub fn pass_to_children(
        &mut self,
        children: &mut [El<C>],
    ) -> EventResponse<C::Event> {
        for (child, child_layout) in
            children.iter_mut().zip(self.layout.children())
        {
            child.on_event(&mut EventCtx {
                event: &self.event,
                page_state: &mut self.page_state,
                layout: &child_layout,
            })?;
        }
        Propagate::Ignored.into()
    }

    pub fn is_focused(&self, id: ElId) -> bool {
        self.page_state.focused == Some(id)
    }

    #[must_use]
    pub fn handle_focusable(
        &self,
        id: ElId,
        press: impl FnOnce(bool) -> EventResponse<C::Event>,
    ) -> EventResponse<C::Event> {
        if self.is_focused(id) {
            if let Some(_) = self.event.as_focus_move() {
                return Capture::Bubbled(id, self.event.clone()).into();
            }

            let focus_click = if self.event.as_focus_press() {
                Some(true)
            } else if self.event.as_focus_release() {
                Some(false)
            } else {
                None
            };

            if let Some(activate) = focus_click {
                press(activate)
            } else {
                Propagate::Ignored.into()
            }
        } else {
            Propagate::Ignored.into()
        }
    }
}

pub struct IdTree {
    pub id: ElId,
    pub children: Signal<Vec<IdTree>>,
}

pub trait Widget<C>
where
    C: WidgetCtx,
{
    fn el(self) -> El<C>
    where
        Self: Sized + 'static,
    {
        El::new(self)
    }

    fn children_ids(&self) -> Memo<Vec<ElId>> {
        Vec::new().into_memo()
    }
    fn layout(&self) -> Signal<Layout>;
    fn build_layout_tree(&self) -> MemoTree<Layout>;

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> DrawResult;
    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, C>,
    ) -> EventResponse<C::Event>;
}

/// Not implementing [`SizedWidget`] and [`BoxModelWidget`] does not mean that
/// Widget has layout without size or box model, it can be intentional to
/// disallow user to set size or box model properties.
pub trait SizedWidget<C: WidgetCtx>: Widget<C> {
    fn fill(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.width(Length::fill()).height(Length::fill())
    }

    fn shrink(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.width(Length::Shrink).height(Length::Shrink)
    }

    fn width<L: Into<Length> + Copy + 'static>(
        self,
        width: impl EcoSignal<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(width.eco_signal(), |&width, layout| {
            layout.size.width = width.into();
        });
        self
    }

    fn height<L: Into<Length> + Copy + 'static>(
        self,
        height: impl EcoSignal<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(height.eco_signal(), |&height, layout| {
            layout.size.height = height.into();
        });
        self
    }
}

pub trait BoxModelWidget<C: WidgetCtx>: Widget<C> {
    fn border_width(self, border_width: impl EcoSignal<u32> + 'static) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(
            border_width.eco_signal(),
            |&border_width, layout| {
                layout.box_model.border_width = border_width;
            },
        );
        self
    }

    fn padding<P: Into<Padding> + Copy + 'static>(
        self,
        padding: impl EcoSignal<P> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(padding.eco_signal(), |&padding, layout| {
            layout.box_model.padding = padding.into();
        });
        self
    }
}

pub mod prelude {
    pub use crate::{
        el::{El, ElId},
        event::{
            Capture, Event, EventResponse, ExitEvent, FocusEvent, NullEvent,
            Propagate,
        },
        layout::{
            self,
            axis::{Axial as _, Axis, ColDir, Direction, RowDir},
            box_model::BoxModel,
            padding::Padding,
            size::{Length, Size},
            Align, ContainerLayout, FlexLayout, Layout, LayoutKind,
            LayoutModelTree, Limits,
        },
        render::{color::Color, Block, Border, Renderer},
        style::{block::*, text::*},
        widget::{
            BoxModelWidget, DrawCtx, DrawResult, EventCtx, LayoutCtx,
            SizedWidget, Widget, WidgetCtx,
        },
    };
    pub use alloc::{boxed::Box, string::String, vec::Vec};
    pub use rsact_core::prelude::*;
}
