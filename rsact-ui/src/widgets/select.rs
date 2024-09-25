use super::{flex::flex_content_size, mono_text::MonoText};
use crate::widget::prelude::*;
use core::{fmt::Display, marker::PhantomData};
use layout::size::RectangleExt;
use rsact_core::memo_chain::IntoMemoChain;

pub trait SelectEvent {
    fn as_select(&self, axis: Axis) -> Option<i32>;
}

#[derive(Clone, Copy)]
pub struct SelectState {
    pressed: bool,
    active: bool,
}

impl SelectState {
    pub fn none() -> Self {
        Self { pressed: false, active: false }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct SelectStyle<C: Color> {
    block: BoxStyle<C>,
}

impl<C: Color> SelectStyle<C> {
    pub fn base() -> Self {
        Self { block: BoxStyle::base() }
    }
}

pub struct SelectOption<W: WidgetCtx, K: PartialEq> {
    key: Memo<K>,
    el: MonoText<W>,
}

impl<W: WidgetCtx, K: PartialEq> SelectOption<W, K> {
    pub fn widget(&self) -> &impl Widget<W> {
        &self.el
    }
}

impl<W: WidgetCtx, K: PartialEq + Display + 'static> SelectOption<W, K> {
    pub fn new(key: impl IntoMemo<K>) -> Self {
        let key = key.into_memo();

        Self {
            key,
            el: MonoText::new(key.mapped(alloc::string::ToString::to_string)),
        }
    }
}

impl<W: WidgetCtx, K: PartialEq> PartialEq for SelectOption<W, K> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

pub struct Select<W: WidgetCtx, K: PartialEq, Dir: Direction> {
    id: ElId,
    layout: Signal<Layout>,
    state: Signal<SelectState>,
    style: MemoChain<SelectStyle<W::Color>>,
    // TODO: Should be Option?
    selected: Signal<usize>,
    options: Memo<Vec<SelectOption<W, K>>>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx, K: PartialEq + 'static, Dir: Direction> BoxModelWidget<W>
    for Select<W, K, Dir>
where
    W::Event: SelectEvent,
{
}

impl<W: WidgetCtx, K: PartialEq + 'static, Dir: Direction> SizedWidget<W>
    for Select<W, K, Dir>
where
    W::Event: SelectEvent,
{
}

impl<W: WidgetCtx, K: Clone + PartialEq + Display + 'static>
    Select<W, K, ColDir>
{
    pub fn vertical(options: impl IntoMemo<Vec<K>>) -> Self {
        Self::new(options)
    }
}

impl<W: WidgetCtx, K: Clone + PartialEq + Display + 'static>
    Select<W, K, RowDir>
{
    pub fn horizontal(options: impl IntoMemo<Vec<K>>) -> Self {
        Self::new(options)
    }
}

impl<
        W: WidgetCtx,
        K: Clone + PartialEq + Display + 'static,
        Dir: Direction,
    > Select<W, K, Dir>
{
    pub fn new(options: impl IntoMemo<Vec<K>>) -> Self {
        let options: Memo<Vec<SelectOption<W, K>>> =
            options.into_memo().mapped(|options| {
                options
                    .clone()
                    .into_iter()
                    .map(|opt| SelectOption::new(opt))
                    .collect()
            });

        Self {
            id: ElId::unique(),
            layout: Layout::new(
                crate::layout::LayoutKind::Flex(
                    FlexLayout::base(Dir::AXIS)
                        .gap(Dir::AXIS.canon(5, 0))
                        // .align_cross(Align::Center),
                ),
                options.mapped(|options| {
                    flex_content_size(
                        Dir::AXIS,
                        options.iter().map(SelectOption::widget),
                    )
                }),
            )
            .into_signal(),
            state: SelectState::none().into_signal(),
            style: SelectStyle::base().into_memo_chain(),
            selected: 0.into_signal(),
            options,
            dir: PhantomData,
        }
    }
}

impl<W: WidgetCtx, K: PartialEq + 'static, Dir: Direction> Widget<W>
    for Select<W, K, Dir>
where
    W::Event: SelectEvent,
{
    fn children_ids(&self) -> Memo<Vec<ElId>> {
        let id = self.id;
        use_memo(move |_| vec![id])
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> rsact_core::prelude::MemoTree<Layout> {
        MemoTree {
            data: self.layout.into_memo(),
            children: self.options.mapped(|options| {
                options.iter().map(|opt| opt.el.build_layout_tree()).collect()
            }),
        }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        ctx.draw_focus_outline(self.id)?;

        let children_layouts = ctx.layout.children().collect::<Vec<_>>();
        let selected_child_layout =
            children_layouts.get(self.selected.get()).unwrap();

        let options_offset =
            ctx.layout.area.center_offset_of(selected_child_layout.area);

        self.options.with(move |options| {
            ctx.renderer.clipped(ctx.layout.area, |renderer| {
                DrawCtx { state: ctx.state, renderer, layout: ctx.layout }
                    .draw_mapped_layouts(
                        options.iter().map(SelectOption::widget),
                        |layout| layout.translate(options_offset),
                    )
            })
        })
    }

    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, W>,
    ) -> EventResponse<<W as WidgetCtx>::Event> {
        let current_state = self.state.get();

        if current_state.active && ctx.is_focused(self.id) {
            if let Some(offset) = ctx.event.as_select(Dir::AXIS) {
                let current = self.selected.get();
                let new = (current as i32 + offset).clamp(
                    0,
                    self.options.with(|options| options.len().saturating_sub(1))
                        as i32,
                ) as usize;

                if current != new {
                    self.selected.set(new);
                }

                return Capture::Captured.into();
            }
        }

        ctx.handle_focusable(self.id, |pressed| {
            if current_state.pressed != pressed {
                let toggle_active = if !current_state.pressed && pressed {
                    true
                } else {
                    false
                };

                self.state.update(|state| {
                    state.pressed = pressed;
                    if toggle_active {
                        state.active = !state.active;
                    }
                });

                Capture::Captured.into()
            } else {
                Propagate::Ignored.into()
            }
        })
    }
}
