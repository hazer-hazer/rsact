use alloc::vec::Vec;
use rsact_reactive::prelude::*;

use crate::widget::WidgetCtx;

// TODO: Rename to SystemMessage?
#[derive(Clone, Debug)]
pub enum Message<W: WidgetCtx> {
    GoTo(W::PageId),
    /// Go to previous page if some. Does nothing if there's no previous page.
    PreviousPage,
}

/// MessageQueue is indented to reactively publish messages UI processes on `tick` synchronously
pub struct MessageQueue<W: WidgetCtx> {
    messages: Signal<Vec<Message<W>>>,
}

impl<W: WidgetCtx> Copy for MessageQueue<W> {}

impl<W: WidgetCtx> Clone for MessageQueue<W> {
    fn clone(&self) -> Self {
        Self { messages: self.messages.clone() }
    }
}

impl<W: WidgetCtx> MessageQueue<W> {
    pub fn new() -> Self {
        Self { messages: create_signal(vec![]) }
    }

    // TODO: Per-message-kind helpers

    pub fn publish(mut self, msg: Message<W>) -> Self {
        self.messages.update(|messages| messages.push(msg));
        self
    }

    pub(crate) fn pop(mut self) -> Option<Message<W>> {
        self.messages.update_untracked(|messages| messages.pop())
    }
}
