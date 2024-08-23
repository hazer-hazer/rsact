use core::marker::PhantomData;

use alloc::boxed::Box;

use crate::{
    el::{El, ElId},
    event::{Event, EventResponse},
    layout::{Layout, LayoutTree},
    render::{color::Color, Renderer},
    size::{Length, Size},
};

pub type DrawResult = Result<(), ()>;

pub trait WidgetCtx {
    type Renderer: Renderer;
    type Event: Event;

    // Methods delegated from renderer //
    fn default_background() -> <Self::Renderer as Renderer>::Color {
        <<Self::Renderer as Renderer>::Color as Color>::default_background()
    }

    fn default_foreground() -> <Self::Renderer as Renderer>::Color {
        <<Self::Renderer as Renderer>::Color as Color>::default_foreground()
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
}

pub struct Ctx<C: WidgetCtx> {
    focused: Option<ElId>,

    ctx: PhantomData<C>,
}

impl<C: WidgetCtx> Ctx<C> {
    pub fn new() -> Self {
        Self { focused: None, ctx: PhantomData }
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

    fn children(&self) -> &[El<C>];
    fn size(&self) -> Size<Length>;
    fn layout(&self, ctx: &Ctx<C>) -> Layout;
    fn draw(
        &self,
        ctx: &Ctx<C>,
        renderer: &mut C::Renderer,
        layout: &LayoutTree,
    ) -> DrawResult;
    fn on_event(
        &mut self,
        ctx: &mut Ctx<C>,
        event: C::Event,
    ) -> EventResponse<C::Event>;
}
