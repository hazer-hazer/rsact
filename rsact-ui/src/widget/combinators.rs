use super::prelude::*;

// pub struct Maybe<W: WidgetCtx> {
//     widget: Memo<bool>,

// }

// impl<W: WidgetCtx> Widget<W> for Maybe<W> {
//     fn meta(&self) -> MetaTree {
//         // TODO: Looks ugly...
//         let meta = self
//             .widget
//             .mapped(|widget| widget.as_ref().map(|widget| widget.meta()));

//         MetaTree {
//             data: meta.mapped(|meta| {
//                 meta.map(|meta| meta.data.get()).unwrap_or(Meta::none())
//             }),
//             children: meta.mapped(|meta| {
//                 meta.map(|meta| meta.children.get_cloned())
//                     .unwrap_or(Vec::new())
//             }),
//         }
//     }

//     fn on_mount(&mut self, ctx: super::MountCtx<W>) {
//         self.widget.
//     }

//     fn layout(&self) -> rsact_reactive::prelude::Signal<super::Layout> {
//         todo!()
//     }

//     fn draw(&self, ctx: &mut super::DrawCtx<'_, W>) -> super::DrawResult {
//         todo!()
//     }

//     fn on_event(
//         &mut self,
//         ctx: &mut super::EventCtx<'_, W>,
//     ) -> super::EventResponse {
//         todo!()
//     }
// }

// impl<W: WidgetCtx> Widget<W> for Memo<Keyed<(), Option<El<W>>>> {
//     fn meta(&self) -> MetaTree {
//         self.with(|widget| widget.meta())
//     }

//     fn on_mount(&mut self, ctx: super::MountCtx<W>) {
//         self
//     }

//     fn layout(&self) -> rsact_reactive::prelude::Signal<Layout> {
//         todo!()
//     }

//     fn draw(&self, ctx: &mut super::DrawCtx<'_, W>) -> super::DrawResult {
//         todo!()
//     }

//     fn on_event(
//         &mut self,
//         ctx: &mut super::EventCtx<'_, W>,
//     ) -> super::EventResponse {
//         todo!()
//     }
// }

// pub struct Conditional<W: WidgetCtx> {
//     cond: Memo<bool>,
//     then: El<W>,
//     otherwise: El<W>,
// }

// impl<W: WidgetCtx> Conditional<W> {
//     pub fn new(cond: Memo<bool>, then: El<W>) -> Self {
//         Self { cond, then, otherwise: Unit.el() }
//     }

//     pub fn otherwise(mut self, otherwise: impl Into<El<W>>) -> Self {
//         self.otherwise = otherwise.into();
//         self
//     }

//     fn el(&self) -> &El<W> {
//         if self.cond.get() {
//             &self.then
//         } else {
//             &self.otherwise
//         }
//     }

//     fn el_mut(&mut self) -> &mut El<W> {
//         if self.cond.get() {
//             &mut self.then
//         } else {
//             &mut self.otherwise
//         }
//     }
// }

// impl<W: WidgetCtx> Widget<W> for Conditional<W> {
//     fn meta(&self) -> MetaTree {
//         self.el().meta()
//     }

//     fn on_mount(&mut self, ctx: super::MountCtx<W>) {
//         self.el_mut().on_mount(ctx);
//     }

//     fn layout(&self) -> rsact_reactive::prelude::Signal<Layout> {
//         self.el
//     }

//     fn draw(&self, ctx: &mut super::DrawCtx<'_, W>) -> super::DrawResult {
//         todo!()
//     }

//     fn on_event(
//         &mut self,
//         ctx: &mut super::EventCtx<'_, W>,
//     ) -> super::EventResponse {
//         todo!()
//     }
// }

/*
 * let state = use_signal(100);
 * let child = move || if state % 2 == 0 {
 *     "asd"
 * } else {
 *     "kek"
 * };
 */

// impl<W: WidgetCtx, F, T> From<F> for El<W>
// where
//     F: Fn() -> T,
//     T: Widget<W> + 'static,
// {
//     fn from(value: F) -> Self {
//         value().el()
//     }
// }

#[derive(Clone, Copy, PartialEq)]
pub struct Unit;

impl<W: WidgetCtx> Widget<W> for Unit {
    fn meta(&self) -> MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn on_mount(&mut self, ctx: super::MountCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> rsact_reactive::prelude::Signal<Layout> {
        Layout::zero().signal()
    }

    fn draw(&self, ctx: &mut super::DrawCtx<'_, W>) -> super::DrawResult {
        let _ = ctx;
        Ok(())
    }

    fn on_event(
        &mut self,
        ctx: &mut super::EventCtx<'_, W>,
    ) -> super::EventResponse {
        let _ = ctx;
        ctx.ignore()
    }
}

impl<W: WidgetCtx, T: Widget<W> + 'static> Into<El<W>> for Option<T> {
    fn into(self) -> El<W> {
        self.map(El::new).unwrap_or(Unit.el())
    }
}

impl<W: WidgetCtx> Widget<W> for Option<El<W>> {
    fn meta(&self) -> MetaTree {
        self.as_ref()
            .map(|widget| widget.meta())
            .unwrap_or(MetaTree::childless(Meta::none))
    }

    fn on_mount(&mut self, ctx: super::MountCtx<W>) {
        self.as_mut().map(|widget| widget.on_mount(ctx));
    }

    fn layout(&self) -> rsact_reactive::prelude::Signal<super::Layout> {
        self.as_ref()
            .map(|widget| widget.layout())
            .unwrap_or(Layout::zero().signal())
    }

    fn draw(&self, ctx: &mut super::DrawCtx<'_, W>) -> super::DrawResult {
        self.as_ref().map(|widget| widget.draw(ctx)).unwrap_or(Ok(()))
    }

    fn on_event(
        &mut self,
        ctx: &mut super::EventCtx<'_, W>,
    ) -> super::EventResponse {
        self.as_mut().map(|widget| widget.on_event(ctx)).unwrap_or(ctx.ignore())
    }
}
