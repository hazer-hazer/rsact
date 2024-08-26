use crate::{
    el::{El, ElId},
    event::{Event, EventResponse},
    layout::{Layout, LayoutTree, Limits},
    render::{color::Color, Renderer},
    size::{Length, Size},
};
use alloc::boxed::Box;
use core::marker::PhantomData;

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

pub struct AppState<C: WidgetCtx> {
    focused: Option<ElId>,

    ctx: PhantomData<C>,
}

impl<C: WidgetCtx> AppState<C> {
    pub fn new() -> Self {
        Self { focused: None, ctx: PhantomData }
    }
}

pub struct LayoutCtx<'a, C: WidgetCtx> {
    pub state: &'a AppState<C>,
}

pub struct DrawCtx<'a, C: WidgetCtx> {
    pub state: &'a AppState<C>,
    pub renderer: &'a mut C::Renderer,
    pub layout: &'a LayoutTree<'a>,
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
    pub state: &'a AppState<C>,
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
    fn size(&self) -> Size<Length>;
    fn content_size(&self) -> Limits;
    fn layout(&self, ctx: &LayoutCtx<'_, C>) -> Layout;
    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> DrawResult;
    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, C>,
    ) -> EventResponse<C::Event> {
        let _ = ctx;
        crate::event::Propagate::Ignored.into()
    }
}
