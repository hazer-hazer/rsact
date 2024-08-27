use crate::{
    el::{El, ElId},
    event::{Event, EventResponse, Propagate},
    layout::{
        padding::Padding,
        size::{Length, Size},
        Layout, LayoutKind, LayoutModelTree, Limits,
    },
    render::{color::Color, Renderer},
};
use alloc::boxed::Box;
use core::marker::PhantomData;
use rsact_core::{
    prelude::use_memo,
    signal::{EcoSignal, ReadSignal, Signal, SignalTree, WriteSignal},
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

impl<'a, C: WidgetCtx + 'static> DrawCtx<'a, C> {
    pub fn draw_child(&mut self, child: &El<C>) -> DrawResult {
        child.draw(&mut DrawCtx {
            state: &self.state,
            renderer: &mut self.renderer,
            layout: self.layout.children().next().as_ref().unwrap(),
        })
    }

    pub fn draw_children(&mut self, children: &[Signal<El<C>>]) -> DrawResult {
        children.iter().zip(self.layout.children()).try_for_each(
            |(child, child_layout)| {
                child.with(|child| {
                    child.draw(&mut DrawCtx {
                        state: self.state,
                        renderer: &mut self.renderer,
                        layout: &child_layout,
                    })
                })
            },
        )
    }
}

pub struct EventCtx<'a, C: WidgetCtx> {
    pub event: &'a C::Event,
    pub page_state: &'a mut PageState<C>,
}

impl<'a, C: WidgetCtx + 'static> EventCtx<'a, C> {
    pub fn pass_to_children(
        &mut self,
        children: &mut [Signal<El<C>>],
    ) -> EventResponse<C::Event> {
        for child in children.iter_mut() {
            child.maybe_update(|child| child.on_event(self))?;
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

    // fn children(&self) -> Signal<Vec<El<C>>>;
    // fn children_mut(&mut self) -> &mut [El<C>] {
    //     &mut []
    // }
    // fn size(&self) -> Size<Length>;
    // fn content_size(&self) -> Limits;
    // fn layout(&self, ctx: &LayoutCtx<'_, C>) -> LayoutKind;
    fn layout(&self) -> Signal<Layout>;
    fn build_layout_tree(&self) -> SignalTree<Layout>;

    fn width<L: Into<Length> + Copy + 'static>(
        self,
        width: impl EcoSignal<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        let layout = self.layout();
        let width = width.eco_signal();
        use_memo(move || {
            let width = width.get().into();
            layout.update_untracked(move |layout| layout.size.width = width);
            width
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
        use_memo(move || {
            let height = height.get().into();
            layout.update_untracked(move |layout| layout.size.height = height);
            height
        });
        self
    }

    fn border_width(self, border_width: impl EcoSignal<u32> + 'static) -> Self
    where
        Self: Sized + 'static,
    {
        let layout = self.layout();
        let border_width = border_width.eco_signal();
        use_memo(move || {
            let border_width = border_width.get();
            layout.update_untracked(move |layout| {
                layout.box_model.border_width = border_width
            });
            border_width
        });
        self
    }

    fn padding<P: Into<Padding> + Copy + 'static>(
        self,
        padding: impl EcoSignal<P> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        let layout = self.layout();
        let padding = padding.eco_signal();
        use_memo(move || {
            let padding = padding.get().into();
            layout.update_untracked(move |layout| {
                layout.box_model.padding = padding
            });
            padding
        });
        self
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> DrawResult;
    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, C>,
    ) -> EventResponse<C::Event>;
}
