use crate::widget::WidgetCtx;

// TODO: Rename to SystemMessage?
#[derive(Clone, Debug)]
pub enum Message<W: WidgetCtx> {
    GoTo(W::PageId),
}
