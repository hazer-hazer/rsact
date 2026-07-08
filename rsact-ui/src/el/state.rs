use crate::{
    el::{UpdateResult, WidgetCtx, WidgetFlags},
    widget::Widget,
};
use core::{
    fmt::{Debug, Display},
    marker::PhantomData,
};
use rsact_reactive::probe::Probe;
use tinyvec::TinyVec;

#[derive(Debug, Clone, Copy)]
pub enum ClipPath {
    // Rect(Rect),
    InnerRect,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RedrawReason {
    PseudoclassChange,
    ChildDirty,
}

impl Display for RedrawReason {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RedrawReason::PseudoclassChange => write!(f, "PseudoclassChange"),
            RedrawReason::ChildDirty => write!(f, "ChildDirty"),
        }
    }
}

pub struct ElState<W: WidgetCtx> {
    _marker: PhantomData<W>,

    pub built: bool,

    pub debug_name: &'static str,

    pub flags: WidgetFlags,

    // TODO: Can these states be stored only globally to avoid excessive memory usage?
    // Action state //
    hovered: bool,
    pressed: bool,

    // // Styling //
    // pub pseudoclass: StylePseudoClass,

    // Rendering //
    needs_redraw: Option<RedrawReason>,
    pub clip_path: Option<ClipPath>,

    /// Render probes owned by this element, one per widget "part" it draws
    /// (`"self"`, `"thumb"`, `"options"`, …). WS2 moved render identity out of
    /// the reactive core's global registry to its owner: the handle *is* the
    /// identity, so cross-page aliasing is impossible by construction and the
    /// probes die with the element (disposed in `remove_subtree`).
    ///
    /// Looked up by a **linear scan with content comparison** — a widget's
    /// part-name set is tiny (≤4) so this beats any hash, and `&'static str`
    /// pointer identity is NOT guaranteed equal across codegen units, so keys
    /// must be compared by content, never by pointer.
    // TODO: `PartId(u16)` compaction — ~12 B/entry vs 16, integer compare, no
    // string bytes in flash.
    pub(crate) part_probes: TinyVec<[(&'static str, Probe); 2]>,
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
            pressed: false,

            needs_redraw: None,
            clip_path: None,
            part_probes: TinyVec::new(),
            // pseudoclass: StylePseudoClass::default(),
        }
    }

    fn pretty_type_name(debug_name: &'static str) -> &'static str {
        // TODO
        debug_name
    }

    pub fn maybe_hover(&mut self, hover: bool) -> UpdateResult {
        if self.flags.is_hoverable() {
            self.hovered = hover;
            self.set_needs_redraw(RedrawReason::PseudoclassChange);
            UpdateResult::request_redraw()
        } else {
            UpdateResult::none()
        }
    }

    pub fn maybe_hover_from_child(
        &mut self,
        child_hover: bool,
    ) -> UpdateResult {
        if self.flags.is_hoverable() && self.flags.is_hoverable_from_children() {
            // Child hovered only affects true values because we could already
            // hover this element directly
            self.hovered = self.hovered || child_hover;

            self.set_needs_redraw(RedrawReason::ChildDirty);

            UpdateResult::request_redraw()
        } else {
            UpdateResult::none()
        }
    }

    #[inline(always)]
    pub fn hovered(&self) -> bool {
        self.hovered
    }

    /// Update the cached "pressed" flag used for rendering (mirrors
    /// [`maybe_hover`]). The single source of truth lives in
    /// [`PageState`]/[`PointerState`]; this cache is kept in sync by
    /// [`Update::PressChange`] so `render` needs no page-state access. A widget
    /// is pressable if it is `clickable` (mouse) or `focusable` (encoder).
    pub fn maybe_press(&mut self, pressed: bool) -> UpdateResult {
        if self.flags.is_clickable() || self.flags.is_focusable() {
            self.pressed = pressed;
            self.set_needs_redraw(RedrawReason::PseudoclassChange);
            UpdateResult::request_redraw()
        } else {
            UpdateResult::none()
        }
    }

    #[inline(always)]
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    pub fn set_needs_redraw(&mut self, reason: RedrawReason) {
        log::debug!(
            "Set {} needs redraw because of {}",
            self.debug_name,
            reason
        );
        self.needs_redraw = Some(reason);
    }

    pub fn take_needs_redraw(&mut self) -> Option<RedrawReason> {
        self.needs_redraw.take()
    }

    /// Dispose every render probe this element owns and empty the set (WS2.3).
    /// Called when the element leaves the tree (`remove_subtree`) or its page
    /// is dropped, so a probe never outlives its element.
    pub(crate) fn dispose_probes(&mut self) {
        for (_key, probe) in core::mem::take(&mut self.part_probes) {
            // SAFETY: the element is leaving the tree, so nothing will render
            // it again and no live edge points at the probe. Probes are created
            // untracked (owned by no observer/scope), so this is the only path
            // that disposes them — no cascade can double-dispose.
            unsafe { probe.dispose() };
        }
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
