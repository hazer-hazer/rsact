use crate::el::WidgetFlags;
use crate::{el::WidgetCtx, widget::Widget};
use core::fmt::Debug;
use core::marker::PhantomData;

#[derive(Debug, Clone, Copy)]
pub enum ClipPath {
    // Rect(Rect),
    InnerRect,
}

#[derive(Debug, PartialEq)]
pub enum RedrawReason {
    PseudoclassChange,
}

pub struct ElState<W: WidgetCtx> {
    _marker: PhantomData<W>,

    pub built: bool,

    pub debug_name: &'static str,

    pub flags: WidgetFlags,

    // TODO:Move ElState to a child module to hide implementation for hovers, etc. Because we should never set hover for non-hoverable widgets and need to encapsulate this logic.
    // Action state //
    pub hovered: bool,

    // // Styling //
    // pub pseudoclass: StylePseudoClass,

    // Rendering //
    needs_redraw: Option<RedrawReason>,
    pub clip_path: Option<ClipPath>,
}

impl<W: WidgetCtx> ElState<W> {
    pub fn for_widget(widget: &dyn Widget<W>) -> Self {
        let debug_name = Self::pretty_type_name(widget.debug_name());
        let flags = widget.flags();

        Self {
            _marker: PhantomData,
            debug_name,
            flags,
            built: false,

            hovered: false,

            needs_redraw: None,
            clip_path: None,
            // pseudoclass: StylePseudoClass::default(),
        }
    }

    fn pretty_type_name(debug_name: &'static str) -> &'static str {
        // TODO
        debug_name
    }

    pub fn set_needs_redraw(&mut self, reason: RedrawReason) {
        self.needs_redraw = Some(reason);
    }

    pub fn take_needs_redraw(&mut self) -> Option<RedrawReason> {
        self.needs_redraw.take()
    }
}

impl<W: WidgetCtx> Debug for ElState<W> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ElState")
            .field("_marker", &self._marker)
            .field("built", &self.built)
            .field("debug_name", &self.debug_name)
            .field("flags", &self.flags)
            .field("hovered", &self.hovered)
            .field("needs_redraw", &self.needs_redraw)
            .field("clip_path", &self.clip_path)
            .finish()
    }
}
