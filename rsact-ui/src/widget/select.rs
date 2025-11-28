use super::{
    container::Container,
    text::{Text, TextStyle},
};
use crate::{
    declare_widget_style,
    layout::LayoutKind,
    render::Renderable,
    style::{ColorStyle, WidgetStylist},
    widget::{Meta, MetaTree, prelude::*},
};
use alloc::string::ToString;
use core::{cell::RefCell, fmt::Display, marker::PhantomData};
use embedded_graphics::{
    prelude::{Point, Transform},
    primitives::Rectangle,
};
use itertools::Itertools as _;
use layout::{axis::Anchor, size::RectangleExt};
use rsact_reactive::{maybe::IntoMaybeReactive, memo_chain::IntoMemoChain};

#[derive(Clone, Copy)]
pub struct SelectState {
    pub pressed: bool,
    pub active: bool,
    pub selected: Option<usize>,
}

impl SelectState {
    pub fn initial(selected: Option<usize>) -> Self {
        Self { pressed: false, active: false, selected }
    }

    fn options_offset(
        &self,
        inner: Rectangle,
        children_layouts: &[LayoutModelNode<'_>],
    ) -> (Point, Option<usize>) {
        if let Some(selected) = self.selected {
            let selected_child_layout = children_layouts.get(selected).unwrap();

            let options_offset =
                inner.center_offset_of(selected_child_layout.inner);

            (options_offset, Some(selected))
        } else if let Some(first_option) = children_layouts.first() {
            (inner.center_offset_of(first_option.inner), None)
        } else {
            (Point::zero(), None)
        }
    }
}

declare_widget_style! {
    SelectStyle (SelectState) {
        container: container,
        selected: container {
            selected_background_color: background_color,
            selected_border_color: border_color,
            selected_border_radius: border_radius,
        },
        selected_text_color: color {
            transparent_selected_text_color: transparent,
        },
        text_color: color {
            transparent_text_color: transparent,
        },
    }
}

impl<C: Color> SelectStyle<C> {
    pub fn base() -> Self {
        Self {
            container: BlockStyle::base(),
            selected: BlockStyle::base().border(
                BorderStyle::base().radius(5).color(C::default_foreground()),
            ),
            selected_text_color: ColorStyle::DefaultForeground,
            text_color: ColorStyle::DefaultForeground,
        }
    }
}

pub struct SelectOption<W: WidgetCtx, K: PartialEq> {
    key: K,
    // TODO: This RefCell needed to do on_mount for memoized (i.e. readonly) options. But it if we won't require options to depend on global states, we can get rid of this RefCell. For example, fonts and styles can be given by Select widget states/styles.
    el: RefCell<El<W>>,
}

impl<W: WidgetCtx, K: PartialEq> SelectOption<W, K> {
    pub fn new(key: K) -> Self
    where
        W::Styler: WidgetStylist<TextStyle<W::Color>>,
        K: Display,
    {
        let string = key.to_string();
        SelectOption {
            key,
            el: RefCell::new(
                Container::new(Text::new(string.inert()).el())
                    .padding(5u32)
                    .el(),
            ),
        }
    }
}

impl<W: WidgetCtx, K: PartialEq> PartialEq for SelectOption<W, K> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

pub struct Select<W: WidgetCtx, K: PartialEq + 'static, Dir: Direction> {
    layout: Signal<Layout>,
    state: Signal<SelectState>,
    style: MemoChain<SelectStyle<W::Color>>,
    // TODO: Can we do fixed size?
    options: Memo<Vec<SelectOption<W, K>>>,
    dir: PhantomData<Dir>,
}

impl<W, K> Select<W, K, ColDir>
where
    W: WidgetCtx,
    W::Styler: WidgetStylist<TextStyle<W::Color>>,
    K: PartialEq + Clone + Display + 'static,
{
    pub fn vertical(
        selected: impl IntoMaybeSignal<K>,
        options: impl IntoMemo<Vec<K>>,
    ) -> Self {
        Self::new(selected, options)
    }
}

impl<W, K> Select<W, K, RowDir>
where
    W::Styler: WidgetStylist<TextStyle<W::Color>>,
    W: WidgetCtx,
    K: PartialEq + Clone + Display + 'static,
{
    pub fn horizontal(
        selected: impl IntoMaybeSignal<K>,
        options: impl IntoMemo<Vec<K>>,
    ) -> Self {
        Self::new(selected, options)
    }
}

impl<W, K, Dir> Select<W, K, Dir>
where
    K: PartialEq + Clone + Display + 'static,
    W: WidgetCtx,
    W::Styler: WidgetStylist<TextStyle<W::Color>>,
    Dir: Direction,
{
    // TODO: Static options?
    // TODO: MaybeReactive options
    pub fn new(
        selected: impl IntoMaybeSignal<K>,
        options: impl IntoMemo<Vec<K>>,
    ) -> Self {
        let options: Memo<Vec<_>> = options.memo().map(|options| {
            options
                .into_iter()
                .cloned()
                .map(|opt| SelectOption::new(opt))
                .collect()
        });

        let mut selected = selected.maybe_signal();

        let state = SelectState::initial(with!(move |selected, options| {
            options.iter().position(|opt| &opt.key == selected)
        }))
        .signal();

        selected.setter(
            state.map(|state| state.selected).maybe_reactive(),
            move |selected, position| {
                if let Some(option) = position.and_then(|pos| {
                    options.with(|options| {
                        options.get(pos).map(|opt| opt.key.clone())
                    })
                }) {
                    *selected = option;
                }
            },
        );

        Self {
            layout: Layout::new(
                LayoutKind::Flex(
                    FlexLayout::base(
                        Dir::AXIS,
                        options.map(|options| {
                            options
                                .iter()
                                .map(|opt| opt.el.borrow().layout().memo())
                                .collect()
                        }),
                    )
                    .block_model(BlockModel::zero().padding(1u32))
                    .gap(Dir::AXIS.canon(5, 0))
                    .align_main(Align::Center)
                    .align_cross(Align::Center),
                ),
                Dir::AXIS.canon(
                    Length::InfiniteWindow(Length::Shrink.try_into().unwrap()),
                    Length::Shrink,
                ),
            )
            .signal(),
            state,
            style: SelectStyle::base().memo_chain(),
            options,
            dir: PhantomData,
        }
    }

    // fn option_position(&self, key: &K) -> Option<usize> {
    //     self.options
    //         .with(|options| options.iter().position(|opt| &opt.key == key))
    // }

    // TODO: Use lenses
    // pub fn use_value(
    //     self,
    //     value: impl WriteSignal<K> + ReadSignal<K> + 'static,
    // ) -> Self {
    //     value.with(|initial| {
    //         self.selected.set(self.option_position(initial));
    //     });

    //     let options = self.options;
    //     value.setter(self.selected, move |pos, value| {
    //         if let &Some(pos) = pos {
    //             if let Some(opt) = options
    //                 .with(|options| options.get(pos).map(|opt| opt.key.clone()))
    //             {
    //                 *value = opt
    //             }
    //         }
    //     });

    //     self
    // }
}

impl<W, K, Dir> BlockModelWidget<W> for Select<W, K, Dir>
where
    W: WidgetCtx,
    K: PartialEq + Display + 'static,
    Dir: Direction,
    W::Styler: WidgetStylist<SelectStyle<W::Color>>,
{
}

impl<W, K, Dir> SizedWidget<W> for Select<W, K, Dir>
where
    W: WidgetCtx,
    K: PartialEq + Clone + Display + 'static,
    Dir: Direction,
    W::Styler: WidgetStylist<SelectStyle<W::Color>>,
{
}

impl<W, K, Dir: 'static> FontSettingWidget<W> for Select<W, K, Dir>
where
    W: WidgetCtx,
    K: PartialEq + Clone + Display + 'static,
    Dir: Direction,
    W::Styler: WidgetStylist<SelectStyle<W::Color>>,
{
}

impl<W: WidgetCtx, K: PartialEq + 'static, Dir: Direction> Widget<W>
    for Select<W, K, Dir>
where
    W::Styler: WidgetStylist<SelectStyle<W::Color>>,
{
    fn meta(&self, id: ElId) -> MetaTree {
        MetaTree::childless(Meta::focusable(id).inert().memo())
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);

        let layout = self.layout;
        self.options.with(|options| {
            options.iter().for_each(move |opt| {
                ctx.pass_to_child(layout, &mut *opt.el.borrow_mut());
            })
        });
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    #[track_caller]
    fn render(&self, ctx: &mut RenderCtx<'_, W>) -> RenderResult {
        let children_layouts = ctx.layout.children().collect::<Vec<_>>();

        ctx.render_self(|ctx| {
            let style = self.style.get();
            let state = self.state.get();

            if let (options_offset, Some(selected)) =
                state.options_offset(ctx.layout.inner, &children_layouts)
            {
                let selected_child_layout =
                    children_layouts.get(selected).unwrap();

                Block::from_layout_style(
                    selected_child_layout
                        .outer
                        .translate(options_offset)
                        .resized_axis(
                            Dir::AXIS.inverted(),
                            ctx.layout.inner.size.cross(Dir::AXIS),
                            Anchor::Center,
                        ),
                    BlockModel::zero().border_width(1),
                    style.selected,
                )
                .render(ctx.renderer())?;
            }

            // TODO: Review if focus outline visible
            ctx.render_focus_outline(ctx.id)
        })?;

        ctx.render_part("options", |ctx| {
            let state = self.state.get();
            let style = self.style.get();
            let (options_offset, _) =
                state.options_offset(ctx.layout.inner, &children_layouts);

            self.options.with(move |options| {
                ctx.clip_inner(|ctx| {
                    options
                        .iter()
                        .zip_eq(children_layouts.iter())
                        .enumerate()
                        .try_for_each(|(index, (option, option_layout))| {
                            ctx.with_tree_style(
                                |tree_style| {
                                    tree_style.text_color(
                                        (if Some(index) == state.selected {
                                            style.selected_text_color
                                        } else {
                                            style.text_color
                                        })
                                        .get(),
                                    )
                                },
                                |ctx| {
                                    let option = option.el.borrow();
                                    ctx.for_child(
                                        option.id(),
                                        &option_layout
                                            .translate(options_offset),
                                        |ctx| option.render(ctx),
                                    )
                                },
                            )
                        })
                })
            })
        })
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        let state = self.state.get();

        if state.active && ctx.is_focused() {
            // TODO: Right select interpretation
            if let Some(mut offset) = ctx.event.interpret_as_rotation() {
                let current = state.selected;

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
                        ((current as i32) + offset).clamp(
                            0,
                            self.options
                                .with(|options| options.len().saturating_sub(1))
                                as i32,
                        ) as usize
                    });

                if current != new {
                    self.state.update(|state| {
                        state.selected = new;
                    });
                }

                return ctx.capture();
            }
        }

        ctx.handle_focusable(|ctx, pressed| {
            // TODO: Generalize
            if state.pressed != pressed {
                let toggle_active = !state.pressed && pressed;

                self.state.update(|state| {
                    state.pressed = pressed;
                    if toggle_active {
                        state.active = !state.active;
                    }
                });

                ctx.capture()
            } else {
                ctx.ignore()
            }
        })
    }
}
