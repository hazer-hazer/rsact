use crate::{
    el::{El, ElId},
    event::{Event, EventResponse, Propagate},
    layout::{
        size::{Length, Size},
        Layout, LayoutKind, LayoutModelTree, Limits,
    },
    render::{color::Color, Renderer},
};
use alloc::boxed::Box;
use core::marker::PhantomData;
use rsact_core::{
    effect::use_effect,
    signal::{EcoSignal, ReadSignal, Signal, WriteSignal},
};

pub type DrawResult = Result<(), ()>;

pub trait WidgetCtx {
    type Renderer: Renderer<Color = Self::Color>;
    type Event: Event;

    type Color: Color;

    // Methods delegated from renderer //
    fn default_background() -> Self::Color {
        Self::Color::default_background()
    }

    fn default_foreground() -> Self::Color {
        Self::Color::default_foreground()
    }
}

pub struct PhantomWidgetCtx<R, E>
where
    R: Renderer,
    E: Event,
{
    renderer: PhantomData<R>,
    event: PhantomData<E>,
}

impl<R, E> WidgetCtx for PhantomWidgetCtx<R, E>
where
    R: Renderer,
    E: Event,
{
    type Renderer = R;
    type Event = E;
    type Color = R::Color;
}

pub struct PageState<C: WidgetCtx> {
    focused: Option<ElId>,

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
}

impl<'a, C: WidgetCtx> DrawCtx<'a, C> {
    pub fn draw_children(&mut self, children: &[El<C>]) -> DrawResult {
        children.iter().zip(self.layout.children()).try_for_each(
            |(child, child_layout)| {
                child.draw(&mut DrawCtx {
                    state: self.state,
                    renderer: &mut self.renderer,
                    layout: &child_layout,
                })
            },
        )
    }
}

pub struct EventCtx<'a, C: WidgetCtx> {
    pub event: &'a C::Event,
    pub page_state: &'a mut PageState<C>,
}

impl<'a, C: WidgetCtx> EventCtx<'a, C> {
    pub fn pass_to_children(
        &mut self,
        children: &mut [El<C>],
    ) -> EventResponse<C::Event> {
        for child in children.iter_mut() {
            child.on_event(self)?;
        }
        Propagate::Ignored.into()
    }
}

pub trait Widget<C>
where
    C: WidgetCtx,
{
    fn el(self) -> El<C>
    where
        Self: Sized + 'static,
    {
        El::new(Box::new(self))
    }

    // fn signal_el(self) -> Signal<El<C>>
    // where
    //     Self: Sized + 'static,
    //     C: 'static,
    // {
    //     Signal::new(self.el())
    // }

    fn children(&self) -> &[El<C>] {
        &[]
    }
    fn children_mut(&mut self) -> &mut [El<C>] {
        &mut []
    }
    // fn size(&self) -> Size<Length>;
    // fn content_size(&self) -> Limits;
    // fn layout(&self, ctx: &LayoutCtx<'_, C>) -> LayoutKind;
    fn layout(&self) -> Signal<Layout>;

    fn width<L: Into<Length> + Copy + 'static>(
        self,
        width: impl EcoSignal<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        let layout = self.layout();
        let width = width.eco_signal();
        use_effect(move |_| {
            let width = width.get().into();
            layout.update(move |layout| layout.size.width = width)
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
        let layout = self.layout();
        let height = height.eco_signal();
        use_effect(move |_| {
            let height = height.get().into();
            layout.update(move |layout| layout.size.height = height)
        });
        self
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> DrawResult;
    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, C>,
    ) -> EventResponse<C::Event> {
        ctx.pass_to_children(self.children_mut())
    }
}
