use core::marker::PhantomData;
use prelude::*;

use crate::{
    event::BubbledData,
    style::{Styler, WidgetStyle},
};

pub type DrawResult = Result<(), ()>;

// Not an actual context, rename to something like `WidgetTypeFamily`
pub trait WidgetCtx: 'static {
    type Renderer: Renderer<Color = Self::Color>;
    type Event: Event;
    type Styler: PartialEq + Copy;

    type Color: Color;

    // Methods delegated from renderer //
    fn default_background() -> Self::Color {
        Self::Color::default_background()
    }

    fn default_foreground() -> Self::Color {
        Self::Color::default_foreground()
    }
}

/// Type family of types used in Widgets
pub struct PhantomWidgetCtx<R, E, S>
where
    R: Renderer,
    E: Event,
{
    _renderer: R,
    _event: E,
    _styler: S,
}

impl<R, E, S> WidgetCtx for PhantomWidgetCtx<R, E, S>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: PartialEq + Copy + 'static,
{
    type Renderer = R;
    type Event = E;
    type Color = R::Color;
    type Styler = S;
}

pub struct PageState<W: WidgetCtx> {
    pub focused: Option<ElId>,

    ctx: PhantomData<W>,
}

impl<W: WidgetCtx> PageState<W> {
    pub fn new() -> Self {
        Self { focused: None, ctx: PhantomData }
    }
}

pub struct LayoutCtx<'a, W: WidgetCtx> {
    pub page_state: &'a PageState<W>,
}

pub struct DrawCtx<'a, W: WidgetCtx> {
    pub state: &'a PageState<W>,
    pub renderer: &'a mut W::Renderer,
    pub layout: &'a LayoutModelNode<'a>,
    // TODO: For text and maybe something else
    // pub inherited_style
}

impl<'a, W: WidgetCtx + 'static> DrawCtx<'a, W> {
    #[must_use]
    pub fn draw_child(&mut self, child: &impl Widget<W>) -> DrawResult {
        self.draw_children(core::iter::once(child))
    }

    #[must_use]
    pub fn draw_children<
        'c,
        C: Iterator<Item = &'c (impl Widget<W> + 'c)> + 'c,
    >(
        &mut self,
        children: C,
    ) -> DrawResult {
        self.draw_mapped_layouts(children, |layout| layout)
    }

    #[must_use]
    pub fn draw_focus_outline(&mut self, id: ElId) -> DrawResult {
        if self.state.focused == Some(id) {
            self.renderer.block(Block {
                border: Border::zero()
                    .color(Some(<W::Color as Color>::default_foreground()))
                    .width(2),
                rect: self.layout.area,
                background: None,
            })
        } else {
            Ok(())
        }
    }

    #[must_use]
    pub fn draw_mapped_layouts<
        'c,
        C: Iterator<Item = &'c (impl Widget<W> + 'c)> + 'c,
    >(
        &mut self,
        children: C,
        map_layout: impl Fn(LayoutModelNode<'a>) -> LayoutModelNode<'a>,
    ) -> DrawResult {
        children.zip(self.layout.children().map(map_layout)).try_for_each(
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

pub struct EventCtx<'a, W: WidgetCtx> {
    pub event: &'a W::Event,
    pub page_state: &'a mut PageState<W>,
    pub layout: &'a LayoutModelNode<'a>,
    // TODO: Instant now
}

impl<'a, W: WidgetCtx + 'static> EventCtx<'a, W> {
    #[must_use]
    pub fn pass_to_children(
        &mut self,
        children: &mut [impl Widget<W>],
    ) -> EventResponse<W::Event> {
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

    pub fn pass_to_child(
        &mut self,
        child: &mut impl Widget<W>,
    ) -> EventResponse<W::Event> {
        self.pass_to_children(core::slice::from_mut(child))
    }

    pub fn is_focused(&self, id: ElId) -> bool {
        self.page_state.focused == Some(id)
    }

    #[must_use]
    pub fn handle_focusable(
        &self,
        id: ElId,
        press: impl FnOnce(bool) -> EventResponse<W::Event>,
    ) -> EventResponse<W::Event> {
        if self.is_focused(id) {
            if let Some(_) = self.event.as_focus_move() {
                return Capture::Bubble(BubbledData::Focused(
                    id,
                    self.layout.area.top_left,
                ))
                .into();
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

pub struct MountCtx<W: WidgetCtx> {
    pub viewport: Memo<Size>,
    pub styler: Memo<W::Styler>,
}

impl<W: WidgetCtx> MountCtx<W> {
    pub fn accept_styles<I: Clone, S: WidgetStyle<Inputs = I> + 'static>(
        &self,
        style: MemoChain<S>,
        inputs: impl MaybeSignal<I> + 'static,
    ) where
        W::Styler: Styler<S, Class = ()>,
    {
        let inputs = inputs.maybe_signal();
        let styler = self.styler;
        style.then(move |base| {
            styler.get().style(())(base.clone(), inputs.get_cloned())
        });
    }

    // TODO: Use watch?

    pub fn pass_to_children(
        self,
        children: impl RwSignal<Vec<El<W>>> + 'static,
    ) {
        use_effect(move |_| {
            children.track();
            children.update_untracked(|children| {
                for child in children {
                    child.on_mount(self);
                }
            });
        });
    }

    pub fn pass_to_child(self, child: impl RwSignal<El<W>> + 'static) {
        use_effect(move |_| {
            child.track();
            child.update_untracked(|child| {
                child.on_mount(self);
            });
        });
    }
}

impl<W: WidgetCtx> Clone for MountCtx<W> {
    fn clone(&self) -> Self {
        Self { viewport: self.viewport.clone(), styler: self.styler.clone() }
    }
}
impl<W: WidgetCtx> Copy for MountCtx<W> {}

pub trait Widget<W>
where
    W: WidgetCtx,
{
    fn el(self) -> El<W>
    where
        Self: Sized + 'static,
    {
        El::new(self)
    }

    // These functions MUST be called only ones per widget //
    fn on_mount(&mut self, ctx: MountCtx<W>);
    fn children_ids(&self) -> Memo<Vec<ElId>> {
        Vec::new().into_memo()
    }
    fn layout(&self) -> Signal<Layout>;
    fn build_layout_tree(&self) -> MemoTree<Layout>;

    // Hot-loop called functions //
    // TODO: Reactive draw?
    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult;
    // TODO: Reactive event context? Is it possible?
    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, W>,
    ) -> EventResponse<W::Event>;
}

// impl<W: WidgetCtx, T> Widget<W> for T
// where
//     T: ReadSignal<El<W>> + WriteSignal<El<W>>,
// {
//     fn on_mount(&mut self, ctx: MountCtx<W>) {
//         self.update_untracked(|this| this.on_mount(ctx))
//     }

//     fn layout(&self) -> Signal<Layout> {
//         self.with(|this| this.layout())
//     }

//     fn build_layout_tree(&self) -> MemoTree<Layout> {
//         todo!()
//     }

//     fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
//         todo!()
//     }

//     fn on_event(
//         &mut self,
//         ctx: &mut EventCtx<'_, W>,
//     ) -> EventResponse<<W as WidgetCtx>::Event> {
//         todo!()
//     }
// }

/// Not implementing [`SizedWidget`] and [`BoxModelWidget`] does not mean that
/// Widget has layout without size or box model, it can be intentional to
/// disallow user to set size or box model properties.
pub trait SizedWidget<W: WidgetCtx>: Widget<W> {
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
        width: impl MaybeSignal<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(width.maybe_signal(), |&width, layout| {
            layout.size.width = width.into();
        });
        self
    }

    fn height<L: Into<Length> + Copy + 'static>(
        self,
        height: impl MaybeSignal<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(height.maybe_signal(), |&height, layout| {
            layout.size.height = height.into();
        });
        self
    }
}

pub trait BoxModelWidget<W: WidgetCtx>: Widget<W> {
    fn border_width(self, border_width: impl MaybeSignal<u32> + 'static) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(
            border_width.maybe_signal(),
            |&border_width, layout| {
                layout.set_border_width(border_width);
            },
        );
        self
    }

    fn padding<P: Into<Padding> + Copy + 'static>(
        self,
        padding: impl MaybeSignal<P> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(padding.maybe_signal(), |&padding, layout| {
            layout.set_padding(padding.into());
        });
        self
    }
}

pub trait IntoWidget<W: WidgetCtx> {
    type Widget: Widget<W>;

    fn into_widget(self) -> Self::Widget;
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
            LayoutModelNode, Limits,
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
