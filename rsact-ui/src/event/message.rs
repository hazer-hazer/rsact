use crate::{
    anim::{Anim, AnimHandle},
    el::ctx::WidgetCtx,
};
use alloc::vec::Vec;
use rsact_reactive::prelude::*;

// TODO: Rename to SystemMessage?
#[derive(Clone, Debug)]
pub enum UiMessage<W: WidgetCtx> {
    GoTo(W::PageId),
    /// Go to previous page if some. Does nothing if there's no previous page.
    PreviousPage,
}

// TODO: Rename, this is not only about messages.
/// MessageQueue is indented to publish messages UI processes on `tick`
/// synchronously
pub struct UiQueue<W: WidgetCtx> {
    // TODO: Use ring-buffer/VecDeque for cheap pop-front.
    messages: Signal<Vec<UiMessage<W>>>,
    now_millis: Signal<u32>,
    /// Pre-stored Memo of `now_millis` to avoid creating Memo for each
    /// animation.
    anim_now_millis: Memo<u32>,
}

impl<W: WidgetCtx> Clone for UiQueue<W> {
    fn clone(&self) -> Self {
        Self {
            messages: self.messages.clone(),
            now_millis: self.now_millis,
            anim_now_millis: self.anim_now_millis,
        }
    }
}
impl<W: WidgetCtx> Copy for UiQueue<W> {}

impl<W: WidgetCtx> UiQueue<W> {
    pub fn new() -> Self {
        let now_millis = create_signal(0);
        Self {
            messages: create_signal(vec![]),
            now_millis,
            anim_now_millis: now_millis.map(|&now_millis| now_millis),
        }
    }

    /// Note: Animations don't run until [`UI::tick_time`] is called
    #[must_use = "Animations do nothing unless used (and don't run without UI::tick_time)"]
    pub fn anim(self, anim: Anim) -> AnimHandle {
        anim.handle(self.anim_now_millis)
    }

    pub fn goto(self, page_id: W::PageId) -> Self {
        self.publish(UiMessage::GoTo(page_id));
        self
    }

    pub fn previous_page(self) -> Self {
        self.publish(UiMessage::PreviousPage);
        self
    }

    pub fn publish(mut self, msg: UiMessage<W>) -> Self {
        self.messages.update(|messages| messages.push(msg));
        self
    }

    pub(crate) fn tick(&mut self, now_millis: u32) {
        self.now_millis.set(now_millis);
    }

    pub(crate) fn pop(mut self) -> Option<UiMessage<W>> {
        // Pop from the FRONT: `publish` appends, so removing from the end would
        // process messages LIFO (e.g. two goto()s in one tick would land on the
        // wrong page). Message counts per tick are tiny, so remove(0) is fine.
        self.messages.update_untracked(|messages| {
            if messages.is_empty() { None } else { Some(messages.remove(0)) }
        })
    }
}
