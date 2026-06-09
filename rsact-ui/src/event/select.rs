use crate::el::ElId;
use slotmap::SlotMap;

// TODO

pub struct SelectState {
    select_chain: SlotMap<ElId, SelectNode>,

    selected: Option<ElId>,
    select_captured_by: Option<ElId>,
}

struct SelectNode {
    prev: Option<ElId>,
    next: Option<ElId>,
}
