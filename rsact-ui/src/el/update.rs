use crate::el::*;
use log::debug;

#[derive(Debug, Clone, Copy)]
pub enum Update {
    HoverChange(bool),
    ChildHoverChange(bool),
    MouseEnter,
    ChildMouseEnter,
    MouseLeave,
    ChildMouseLeave,
}

impl Update {
    pub fn as_bubble(&self) -> Option<Self> {
        match self {
            Self::HoverChange(hovered) => {
                Some(Self::ChildHoverChange(*hovered))
            },
            Self::ChildHoverChange(_) => None,
            Self::MouseEnter => Some(Self::ChildMouseEnter),
            Self::ChildMouseEnter => None,
            Self::MouseLeave => Some(Self::ChildMouseLeave),
            Self::ChildMouseLeave => None,
        }
    }
}

pub struct UpdateCtx<'a, W: WidgetCtx> {
    pub id: ElId,
    pub update: Update,
    pub state: &'a mut ElState<W>,
    // pub page_state: &'a mut PageState<W>,
}

impl<'a, W: WidgetCtx> UpdateCtx<'a, W> {
    pub fn handle(&mut self) {
        debug!(
            "Handle update for {}[{:?}]: {:?}",
            self.state.debug_name, self.id, self.update
        );
        match self.update {
            Update::HoverChange(hovered) => {
                self.state.maybe_hover(hovered);
            },
            Update::ChildHoverChange(child_hovered) => {
                self.state.maybe_hover_from_child(child_hovered);
            },
            Update::MouseEnter => {},
            Update::ChildMouseEnter => {},
            Update::MouseLeave => {},
            Update::ChildMouseLeave => {},
        }
    }
}
