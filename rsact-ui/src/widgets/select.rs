use super::{container::Container, keyed::KeyedEl, mono_text::MonoText};
use crate::{layout::LayoutKind, widget::prelude::*};
use core::{fmt::Display, marker::PhantomData};
use embedded_graphics::{
    prelude::{Point, Primitive, Transform},
    primitives::{Line, PrimitiveStyleBuilder},
};
use layout::{
    axis::{Anchor, AxisAnchorPoint},
    flex::flex_content_size,
    size::RectangleExt,
};
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
    selected: BoxStyle<C>,
}

impl<C: Color> SelectStyle<C> {
    pub fn base() -> Self {
        Self {
            block: BoxStyle::base(),
            selected: BoxStyle::base().border(
                BorderStyle::base()
                    .radius(5.into())
                    .color(C::default_foreground()),
            ),
        }
    }
}

pub struct SelectOption<W: WidgetCtx, K: PartialEq> {
    key: K,
    el: El<W>,
}

impl<W: WidgetCtx, K: PartialEq> SelectOption<W, K> {
    pub fn new(key: K) -> Self
    where
        K: Display,
    {
        let string = key.to_string();
        SelectOption {
            key,
            el: Container::new(MonoText::new(string).el()).padding(5).el(),
        }
    }

    pub fn widget(&self) -> &impl Widget<W> {
        &self.el
    }
}

impl<W: WidgetCtx, K: PartialEq> PartialEq for SelectOption<W, K> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

// pub trait IntoSelectOption<W: WidgetCtx>: PartialEq {
//     type Key: PartialEq + Clone;

//     fn into_select_option(self) -> SelectOption<W, Self::Key>;
// }

// impl<W: WidgetCtx, K: PartialEq + Clone> IntoSelectOption<W>
//     for SelectOption<W, K>
// {
//     type Key = K;

//     fn into_select_option(self) -> SelectOption<W, Self::Key> {
//         self
//     }
// }

// impl<W: WidgetCtx, K: PartialEq + Clone + Display> IntoSelectOption<W> for K
// {     type Key = K;

//     fn into_select_option(self) -> SelectOption<W, Self::Key> {
//         let string = self.to_string();
//         SelectOption { key: self, el: MonoText::new(string).el() }
//     }
// }

// impl<W: WidgetCtx, K: PartialEq + Clone> IntoSelectOption<W> for KeyedEl<W,
// K> {     type Key = K;

//     fn into_select_option(self) -> SelectOption<W, Self::Key> {
//         SelectOption { key: self.key, el: self.el }
//     }
// }

// pub trait IntoSelectOptions<W: WidgetCtx, K: PartialEq> {
//     fn into_select_options(self) -> Vec<SelectOption<W, K>>;
// }

// impl<T, W, K> IntoSelectOptions<W, K> for T
// where
//     T: IntoIterator<Item = SelectOption<W, K>> + 'static,
//     W: WidgetCtx,
//     K: PartialEq + Clone + 'static,
// {
//     fn into_select_options(self) -> Vec<SelectOption<W, K>> {
//         let options = self.into_iter().collect::<Vec<SelectOption<W, K>>>();
//         use_memo(move |_| options)
//     }
// }

// impl<W: WidgetCtx, K: PartialEq + Display + 'static> SelectOption<W, K> {
//     pub fn new(key: impl IntoMemo<K>) -> Self {
//         let key = key.into_memo();

//         Self {
//             key,
//             el: Container::new(
//                 MonoText::new(key.mapped(alloc::string::ToString::to_string))
//                     .el(),
//             )
//             .padding(5)
//             .el(),
//         }
//     }
// }

// pub trait SelectOption<W: WidgetCtx>: PartialEq {
//     fn eq(&self, other: &Self) -> bool;
//     fn el(&self) -> &El<W>;
// }

// impl<W: WidgetCtx, K: PartialEq> SelectOption<W> for (K, El<W>) {
//     fn eq(&self, other: &Self) -> bool {
//         self.0 == other.0
//     }

//     fn el(&self) -> &El<W> {
//         &self.1
//     }
// }

pub struct Select<W: WidgetCtx, K: PartialEq, Dir: Direction> {
    id: ElId,
    layout: Signal<Layout>,
    state: Signal<SelectState>,
    style: MemoChain<SelectStyle<W::Color>>,
    selected: Signal<Option<usize>>,
    options: Memo<Vec<SelectOption<W, K>>>,
    dir: PhantomData<Dir>,
}

impl<W, K, Dir> BoxModelWidget<W> for Select<W, K, Dir>
where
    W::Event: SelectEvent,
    W: WidgetCtx,
    K: PartialEq + Display + 'static,
    Dir: Direction,
{
}

impl<W, K, Dir> SizedWidget<W> for Select<W, K, Dir>
where
    W::Event: SelectEvent,
    W: WidgetCtx,
    K: PartialEq + Clone + Display + 'static,
    Dir: Direction,
{
}

impl<W, K> Select<W, K, ColDir>
where
    W: WidgetCtx,
    K: PartialEq + Clone + Display + 'static,
{
    pub fn vertical(options: impl IntoMemo<Vec<K>>) -> Self {
        Self::new(options)
    }
}

impl<W, K> Select<W, K, RowDir>
where
    W: WidgetCtx,
    K: PartialEq + Clone + Display + 'static,
{
    pub fn horizontal(options: impl IntoMemo<Vec<K>>) -> Self {
        Self::new(options)
    }
}

impl<W, K, Dir> Select<W, K, Dir>
where
    K: PartialEq + Clone + Display + 'static,
    W: WidgetCtx,
    Dir: Direction,
{
    pub fn new(options: impl IntoMemo<Vec<K>>) -> Self {
        let options: Memo<Vec<SelectOption<W, K>>> =
            options.into_memo().mapped(|options| {
                options
                    .into_iter()
                    .cloned()
                    .map(|opt| SelectOption::new(opt))
                    .collect()
            });

        Self {
            id: ElId::unique(),
            layout: Layout::shrink(LayoutKind::Flex(
                FlexLayout::base(
                    Dir::AXIS,
                    options.mapped(|options| {
                        flex_content_size(
                            Dir::AXIS,
                            options.iter().map(SelectOption::widget),
                        )
                    }),
                )
                .gap(Dir::AXIS.canon(5, 0))
                .align_main(Align::Center)
                .align_cross(Align::Center),
            ))
            .into_signal(),
            state: SelectState::none().into_signal(),
            style: SelectStyle::base().into_memo_chain(),
            selected: None.into_signal(),
            options,
            dir: PhantomData,
        }
    }

    fn option_position(&self, key: &K) -> Option<usize> {
        self.options
            .with(|options| options.iter().position(|opt| &opt.key == key))
    }

    pub fn use_value(
        self,
        value: impl WriteSignal<K> + ReadSignal<K> + 'static,
    ) -> Self {
        value.with(|initial| {
            self.selected.set(self.option_position(initial));
        });

        let options = self.options;
        value.setter(self.selected, move |pos, value| {
            if let &Some(pos) = pos {
                if let Some(opt) = options
                    .with(|options| options.get(pos).map(|opt| opt.key.clone()))
                {
                    *value = opt
                }
            }
        });

        self
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
        // TODO: Styles
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
        let style = self.style.get();

        ctx.draw_focus_outline(self.id)?;

        // ctx.renderer.line(
        //     Line::new(
        //         ctx.layout.area.anchor_point(
        //             Dir::AXIS
        //                 .canon::<AxisAnchorPoint>(Anchor::Center,
        // Anchor::Start)                 .into(),
        //         ),
        //         ctx.layout.area.anchor_point(
        //             Dir::AXIS
        //                 .canon::<AxisAnchorPoint>(Anchor::Center,
        // Anchor::End)                 .into(),
        //         ),
        //     )
        //     .into_styled(
        //         PrimitiveStyleBuilder::new()
        //             .stroke_width(2)
        //             .stroke_color(W::Color::default_foreground())
        //             .build(),
        //     ),
        // )?;

        let children_layouts = ctx.layout.children().collect::<Vec<_>>();

        let options_offset = if let Some(selected) = self.selected.get() {
            let selected_child_layout = children_layouts.get(selected).unwrap();

            let options_offset =
                ctx.layout.area.center_offset_of(selected_child_layout.area);

            ctx.renderer.block(Block::from_layout_style(
                selected_child_layout
                    .area
                    .translate(options_offset)
                    .resized_axis(
                        Dir::AXIS.inverted(),
                        ctx.layout.area.size.cross(Dir::AXIS),
                        Anchor::Center,
                    ),
                BoxModel::zero().border_width(1),
                style.selected,
            ))?;

            options_offset
        } else if let Some(first_option) = children_layouts.first() {
            ctx.layout.area.center_offset_of(first_option.area)
        } else {
            Point::zero()
        };

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
            if let Some(mut offset) = ctx.event.as_select(Dir::AXIS) {
                let current = self.selected.get();

                let new = current
                    .or_else(|| {
                        if offset > 0 {
                            offset -= 1;
                            Some(0)
                        } else {
                            None
                        }
                    })
                    .map(|current| {
                        (current as i32 + offset).clamp(
                            0,
                            self.options
                                .with(|options| options.len().saturating_sub(1))
                                as i32,
                        ) as usize
                    });

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
