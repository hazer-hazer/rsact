use crate::{el::El, widget::WidgetCtx};

pub struct KeyedEl<W: WidgetCtx, K: PartialEq> {
    pub key: K,
    pub el: El<W>,
}

impl<W: WidgetCtx, K: PartialEq> PartialEq for KeyedEl<W, K> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
