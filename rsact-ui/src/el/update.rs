use log::debug;

use crate::el::*;

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
        match self.update {
            Update::HoverChange(hovered) => {
                if self.state.flags.hoverable {
                    debug!(
                        "Hover change {}[{:?}]",
                        self.state.debug_name, self.id
                    );
                    self.state.hovered = hovered;
                    self.state
                        .set_needs_redraw(RedrawReason::PseudoclassChange);
                }
            },
            Update::ChildHoverChange(child_hovered) => {
                if self.state.flags.hoverable_from_children {
                    // Child hovered only affects true values because we could already hover this element directly
                    self.state.hovered = self.state.hovered || child_hovered;
                    self.state
                        .set_needs_redraw(RedrawReason::PseudoclassChange);
                }
            },
            Update::MouseEnter => {},
            Update::ChildMouseEnter => {},
            Update::MouseLeave => {},
            Update::ChildMouseLeave => {},
        }
    }
}
