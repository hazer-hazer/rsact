//! WS21 — the **environment**: properties that cascade down the tree.
//!
//! Environment is the third property category, alongside the two WS5.5 settled:
//!
//! | Category | Where it lives | Cascades? | Affects the box? | Pseudo-classes? |
//! | --- | --- | --- | --- | --- |
//! | **Layout** — width, padding, gap | arena [`LayoutData`], per node | no | yes | no |
//! | **Environment** — font, font size, font style; later locale, direction | here | **yes** | yes (measurement) | no |
//! | **Style** — colors, border width, radius | stylist, per widget | no | no (pixels only) | yes |
//!
//! Cascading is adopted **deliberately and narrowly**, for properties that
//! genuinely inherit down a text/reading context. It is not a general styling
//! model: no selectors, no specificity, no "any style property may cascade".
//! The project's anti-CSS stance for styles is unchanged.
//!
//! # The two groups
//!
//! An environment property is split by the same box/pixel rule WS5.5 used for
//! `border_width` — *does it change the box, or only the pixels inside it?*
//!
//! - [`LayoutEnv`] — the **box** group. Resolved by the layout pass, cached into
//!   the [`LayoutModel`], replayed by render. A change to it changes
//!   measurement, so it must mark the arena dirty.
//! - [`VisualEnv`] — the **pixel** group. Resolved by the render walk. A change
//!   to it must **never** relayout.
//!
//! The split is two sibling types rather than one struct with a comment, and
//! that is load-bearing in two ways. It makes assigning across the groups a
//! compile error instead of a review catch. And because [`VisualEnv`] is generic
//! over the colour type while [`LayoutEnv`] is not, keeping them separate is
//! what keeps `LayoutCtx`, [`LayoutModel`] and the layout tree **free of a
//! colour parameter** — a single `Env<C>` would drag `W::Color` into the layout
//! tree, which is precisely the coupling the box/pixel rule exists to prevent.
//!
//! Every new environment property must be classified by that rule *at the point
//! it is added*. Adding a colour to [`LayoutEnv`] is how "a colour change
//! relayouts the page" arrives later.
//!
//! # The one mechanism, three tenses
//!
//! [`LayoutEnv`] is not a new mechanism — WS21 names one the codebase built
//! three times independently. The three sites differ only in *tense*:
//!
//! | Site | Tense |
//! | --- | --- |
//! | `LayoutCtx::env` | the cascade **in progress**, threaded down the layout walk and merged per node by [`LayoutEnv::inherited`] |
//! | `LayoutModel::env` | the cascade **resolved**, cached per node so render replays it instead of recomputing — `None` means "nothing new here, keep the parent's" |
//! | `Retained::input_env` | the cascade **as of entry**, the per-node resume anchor that lets WS5.2's `recompute_upward` re-run a subtree without walking from the root |
//!
//! [`LayoutData`]: crate::layout::LayoutData
//! [`LayoutModel`]: crate::layout::model::LayoutModel

use crate::{
    font::{Font, FontSize, FontStyle, ResolvedFontProps},
    render::prelude::*,
};
use core::fmt::Display;

// TODO: Get rid of LayoutEnv in every widget, Remove FontSettingWidget, create
// TextStyle widget that sets font properties and styles in the tree to be
// applied to all children. Not any node must contain LayoutEnv, only TextStyle
// and Content will, TextStyle will propagate LayoutEnv down the tree in layout
// modeling pass.
//
// Note: this is WS21's own charter, written before WS21 existed. 21.2 moves the
// override source out of the layout kinds into the arena's sparse env map and
// retargets `FontSettingWidget`; 21.3 adds the subtree-wrapping entry point.
// Kept until that work is 100% done.
/// The **box** group of the environment: properties that cascade *and* change
/// measurement. See the [module docs](self) for the box/pixel rule that puts
/// them here rather than in style.
///
/// `None` on a field means "no override at this node" — the cascade merge in
/// [`LayoutEnv::inherited`] is a field-wise [`Option::or`], child-first, so the
/// innermost setting wins.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct LayoutEnv {
    pub font: Option<Font>,
    pub font_size: Option<FontSize>,
    pub font_style: Option<FontStyle>,
}

impl LayoutEnv {
    pub fn has_any(&self) -> bool {
        self.font.is_some()
            || self.font_size.is_some()
            || self.font_style.is_some()
    }

    pub fn inherited(&self, parent: &LayoutEnv) -> Self {
        Self {
            font: self.font.or(parent.font),
            font_size: self.font_size.or(parent.font_size),
            font_style: self.font_style.or(parent.font_style),
        }
    }

    pub fn resolve(&self, viewport: Size) -> ResolvedFontProps {
        let font_size = self.font_size.unwrap_or_default().resolve(viewport);

        let font_style = self.font_style.unwrap_or_default();

        ResolvedFontProps { size: font_size, style: font_style }
    }

    pub fn font(&self) -> Font {
        // TODO: Is font required to be set at least by global default or we
        // should fallback here?
        self.font.unwrap()
    }
}

impl Display for LayoutEnv {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // TODO
        write!(f, "")
    }
}

/// The **pixel** group of the environment: properties that cascade but only
/// affect what is drawn inside an already-decided box. A change to one of these
/// must never trigger a relayout.
///
/// Note: this group is **vestigial today** and WS21 renamed it in place rather
/// than deleting it. It has no live producer — its only mutator,
/// `RenderCtx::with_tree_style`, is called from exactly one site and that call
/// is commented out (`widget/select.rs`) — and no live consumer either: `Label`
/// reads `text_color` from the *stylist*, not from here. So the value is built
/// at the page root, threaded through every `render_subtree` call, and read by
/// nobody. 21.2 gives it a producer; until then the cascade is real machinery
/// with nothing flowing through it.
#[derive(Debug, Clone, Copy)]
pub struct VisualEnv<C: Color> {
    pub text_color: ColorStyle<C>,
}

impl<C: Color> VisualEnv<C> {
    pub fn base() -> Self {
        Self { text_color: ColorStyle::DefaultForeground }
    }

    pub fn text_color(mut self, text_color: Option<C>) -> Self {
        self.text_color.set_high_priority(text_color);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::LayoutEnv;
    use crate::font::{FontSize, FontStyle};

    // b.3: `has_any` must mean "any field is set", not "all fields are set".
    // The layout model stores resolved text props for draw only when
    // `has_any()` is true (`layout/model.rs`), while measurement always merges
    // (`layout/mod.rs`). With the old has-ALL semantics a font-size-only
    // override (`label.font_size(20)`) was *measured* at 20 but *drawn* at the
    // inherited size, because `has_any()` returned false and the resolved props
    // were dropped from the model.
    #[test]
    fn has_any_reports_any_set_field_not_all() {
        assert!(!LayoutEnv::default().has_any(), "nothing set => no override");

        let size_only = LayoutEnv {
            font_size: Some(FontSize::Fixed(20)),
            ..Default::default()
        };
        assert!(
            size_only.has_any(),
            "font_size(20) alone must count as an override"
        );

        let style_only = LayoutEnv {
            font_style: Some(FontStyle::Bold),
            ..Default::default()
        };
        assert!(style_only.has_any(), "font_style alone must count");
    }
}
