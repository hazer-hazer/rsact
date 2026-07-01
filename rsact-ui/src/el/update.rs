use crate::el::*;
use log::debug;

#[derive(Debug, Clone, Copy)]
pub enum Update {
    HoverChange(bool),
    ChildHoverChange(bool),
    PressChange(bool),
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
            Self::ChildHoverChange(_) => Some(*self),
            // Press does not bubble to parents: a pressed child does not make
            // its container "pressed" (unlike hover).
            Self::PressChange(_) => None,
            Self::MouseEnter => Some(Self::ChildMouseEnter),
            Self::ChildMouseEnter => Some(*self),
            Self::MouseLeave => Some(Self::ChildMouseLeave),
            Self::ChildMouseLeave => Some(*self),
        }
    }
}

pub struct UpdateResult {
    request_redraw: bool,
}

impl UpdateResult {
    pub fn none() -> Self {
        Self { request_redraw: false }
    }

    pub fn request_redraw() -> Self {
        Self { request_redraw: true }
    }

    pub fn is_redraw_requested(&self) -> bool {
        self.request_redraw
    }

    pub fn should_bubble(&self) -> bool {
        self.request_redraw
    }

    pub fn merge(mut self, other: Self) -> Self {
        self.request_redraw = self.request_redraw || other.request_redraw;
        self
    }
}

pub struct UpdateCtx<'a, W: WidgetCtx> {
    pub id: ElId,
    pub update: Update,
    pub state: &'a mut ElState<W>,
    // pub page_state: &'a mut PageState<W>,
}

impl<'a, W: WidgetCtx> UpdateCtx<'a, W> {
    pub fn handle(&mut self) -> UpdateResult {
        debug!(
            "Handle update {:?} for {}[{:?}]",
            self.update, self.state.debug_name, self.id
        );
        match self.update {
            Update::HoverChange(hovered) => {
                return self.state.maybe_hover(hovered);
            },
            Update::ChildHoverChange(child_hovered) => {
                return self.state.maybe_hover_from_child(child_hovered);
            },
            Update::PressChange(pressed) => {
                return self.state.maybe_press(pressed);
            },
            Update::MouseEnter => {},
            Update::ChildMouseEnter => {},
            Update::MouseLeave => {},
            Update::ChildMouseLeave => {},
        }

        UpdateResult::none()
    }
}
