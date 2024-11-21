pub mod bar;
pub mod button;
pub mod canvas;
pub mod checkbox;
pub mod combinators;
pub mod container;
pub mod edge;
pub mod flex;
pub mod icon;
pub mod image;
pub mod knob;
pub mod mono_text;
pub mod scrollable;
pub mod select;
pub mod show;
pub mod slider;
pub mod space;

use crate::{
    event::{BubbledData, EventPass, FocusedWidget},
    page::id::{PageId, SinglePage},
    render::Renderable,
    style::{NullStyler, Styler, TreeStyle, WidgetStyle},
};
use bitflags::bitflags;
use core::marker::PhantomData;
use prelude::*;

pub type DrawResult = Result<(), ()>;

bitflags! {
    #[derive(Clone, Copy, PartialEq)]
    pub struct Behavior: u8 {
        const NONE = 0;
        const FOCUSABLE = 1 << 0;
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Meta {
    pub behavior: Behavior,
    pub id: Option<ElId>,
}

impl Default for Meta {
    fn default() -> Self {
        Self::none()
    }
}

impl Meta {
    pub fn none() -> Self {
        Self { behavior: Behavior::NONE, id: None }
    }

    pub fn focusable(id: ElId) -> Self {
        Self { behavior: Behavior::FOCUSABLE, id: Some(id) }
    }
}

// TODO: Custom MemoTree with SmallVec<T, 1>
pub type MetaTree = MemoTree<Meta>;

// #[derive(PartialEq)]
// pub struct MetaTree {
//     data: MaybeReactive<Meta>,
//     children: MaybeReactive<Vec<MetaTree>>,
// }

// impl MetaTree {
//     pub fn flat_collect(&self) -> Vec<MaybeReactive<Meta>> {
//         self.children.with(|children| {
//             core::iter::once(self.data)
//                 .chain(children.iter().map(MetaTree::flat_collect).flatten())
//                 .collect()
//         })
//     }
// }

// TODO: Not an actual context, rename to something like `WidgetTypeFamily`
pub trait WidgetCtx: Sized + 'static {
    type Renderer: Renderer<Color = Self::Color>;
    type Styler: PartialEq + Copy;
    type Color: Color;
    type PageId: PageId;
    type Event: Event;

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
pub struct Wtf<R, E = NullEvent, S = NullStyler, I = SinglePage>
where
    R: Renderer,
    E: Event,
{
    _renderer: R,
    _event: E,
    _styler: S,
    _page_id: I,
}

impl<R, E, S, I> WidgetCtx for Wtf<R, E, S, I>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: PartialEq + Copy + 'static,
    I: PageId + 'static,
{
    type Renderer = R;
    type Color = R::Color;
    type Styler = S;
    type PageId = I;
    type Event = E;
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

// TODO: Make DrawCtx a delegate to renderer so u can do `Primitive::(...).render(ctx)`
pub struct DrawCtx<'a, W: WidgetCtx> {
    pub state: &'a PageState<W>,
    pub renderer: &'a mut W::Renderer,
    pub layout: &'a LayoutModelNode<'a>,
    pub tree_style: TreeStyle<W::Color>,
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
            Block {
                border: Border::zero()
                    // TODO: Theme focus color
                    .color(Some(<W::Color as Color>::default_foreground()))
                    .width(1),
                rect: self.layout.outer,
                background: None,
            }
            .render(self.renderer)
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
                    tree_style: self.tree_style,
                })
            },
        )
    }
}

pub struct EventCtx<'a, W: WidgetCtx> {
    pub event: &'a W::Event,
    pub page_state: Signal<PageState<W>>,
    pub layout: &'a LayoutModelNode<'a>,
    pub pass: &'a mut EventPass,
    // TODO: Instant now
}

impl<'a, W: WidgetCtx + 'static> EventCtx<'a, W> {
    #[must_use]
    pub fn pass_to_children(
        &mut self,
        children: &mut [impl Widget<W>],
    ) -> EventResponse<W> {
        for (child, child_layout) in
            children.iter_mut().zip(self.layout.children())
        {
            child.on_event(&mut EventCtx {
                event: self.event,
                page_state: self.page_state,
                layout: &child_layout,
                pass: &mut self.pass,
            })?;
        }
        self.ignore()
    }

    pub fn pass_to_child(
        &mut self,
        child: &mut impl Widget<W>,
    ) -> EventResponse<W> {
        self.pass_to_children(core::slice::from_mut(child))
    }

    pub fn is_focused(&self, id: ElId) -> bool {
        self.page_state.with(|state| state.focused == Some(id))
    }

    #[must_use]
    pub fn handle_focusable(
        &mut self,
        id: ElId,
        press: impl FnOnce(&mut Self, bool) -> EventResponse<W>,
    ) -> EventResponse<W> {
        if let Some(_) = self.event.as_focus_move() {
            if self.pass.focus_search == Some(0) {
                // return Capture::Bubble(BubbledData::Focused(
                //     id,
                //     self.layout.area.top_left,
                // ))
                // .into();
                self.pass.set_focused(FocusedWidget {
                    id,
                    absolute_position: self.layout.outer.top_left,
                });
            } else {
                self.pass
                    .focus_search
                    .as_mut()
                    .map(|focus_target| *focus_target -= 1);
            }
        } else if self.is_focused(id) {
            let focus_click = if self.event.as_focus_press() {
                Some(true)
            } else if self.event.as_focus_release() {
                Some(false)
            } else {
                None
            };

            return if let Some(activate) = focus_click {
                press(self, activate)
            } else {
                self.ignore()
            };
        }

        self.ignore()
    }

    #[inline]
    pub fn capture(&self) -> EventResponse<W> {
        EventResponse::Break(Capture::Captured)
    }

    #[inline]
    pub fn bubble(&self, bubbled_data: BubbledData<W>) -> EventResponse<W> {
        EventResponse::Break(Capture::Bubble(bubbled_data))
    }

    #[inline]
    pub fn ignore(&self) -> EventResponse<W> {
        EventResponse::Continue(Propagate::Ignored)
    }
}

pub struct MountCtx<W: WidgetCtx> {
    pub viewport: Memo<Size>,
    pub styler: Memo<W::Styler>,
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
        W::Styler: Styler<S, Class = ()>,
    {
        let styler = self.styler;
        let inputs = inputs.into();
        style.then(move |base| {
            styler.get().style(())(base.clone(), inputs.get_cloned())
        });
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

    // TODO: Use MaybeReactive tree
    // TODO: Can rewrite so that meta is called once?
    fn meta(&self) -> MetaTree;

    // These functions MUST be called only ones per widget //
    fn on_mount(&mut self, ctx: MountCtx<W>);
    fn layout(&self) -> Signal<Layout>;
    fn build_layout_tree(&self) -> MemoTree<Layout>;

    // Hot-loop called functions //
    // TODO: Reactive draw?
    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult;
    // TODO: Reactive event context? Is it possible?
    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W>;
}

/// Not implementing [`SizedWidget`] and [`BlockModelWidget`] does not mean that
/// Widget has layout without size or box model, it can be intentional to
/// disallow user to set size or box model properties.
pub trait SizedWidget<W: WidgetCtx>: Widget<W> {
    fn fill(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.width(Length::fill()).height(Length::fill())
    }

    fn fill_width(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.width(Length::fill())
    }

    fn shrink(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.width(Length::Shrink).height(Length::Shrink)
    }

    fn width<L: Into<Length> + PartialEq + Copy + 'static>(
        self,
        width: impl Into<MaybeReactive<L>>,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(width.into(), |layout, &width| {
            layout.size.width = width.into();
        });
        self
    }

    fn height<L: Into<Length> + PartialEq + Copy + 'static>(
        self,
        height: impl Into<MaybeReactive<L>> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(height.into(), |layout, &height| {
            layout.size.height = height.into();
        });
        self
    }

    fn fill_height(self) -> Self
    where
        Self: Sized + 'static,
    {
        self.height(Length::fill())
    }
}

pub trait BlockModelWidget<W: WidgetCtx>: Widget<W> {
    fn border_width(
        self,
        border_width: impl Into<MaybeReactive<u32>> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(border_width.into(), |layout, &border_width| {
            layout.set_border_width(border_width);
        });
        self
    }

    fn padding<P: Into<Padding> + PartialEq + Copy + 'static>(
        self,
        padding: impl Into<MaybeReactive<P>> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(padding.into(), |layout, &padding| {
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
            message::Message, BubbledData, Capture, Event, EventResponse,
            ExitEvent, FocusEvent, NullEvent, Propagate,
        },
        font::{FontSize, FontStyle},
        layout::{
            self,
            axis::{
                Anchor, Axial as _, Axis, AxisAnchorPoint, ColDir, Direction,
                RowDir,
            },
            block_model::BlockModel,
            padding::Padding,
            size::{Length, Size},
            Align, ContainerLayout, FlexLayout, Layout, LayoutKind,
            LayoutModelNode, Limits,
        },
        render::{color::Color, Block, Border, Renderer},
        style::{block::*, declare_widget_style, ColorStyle, Styler},
        widget::{
            BlockModelWidget, DrawCtx, DrawResult, EventCtx, LayoutCtx, Meta,
            MetaTree, MountCtx, SizedWidget, Widget, WidgetCtx,
        },
    };
    pub use alloc::{boxed::Box, string::String, vec::Vec};
    pub use rsact_reactive::prelude::*;
}
