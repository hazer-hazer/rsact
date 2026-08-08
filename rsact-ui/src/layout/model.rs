#[cfg(feature = "debug-info")]
use crate::layout::{DevLayout, DevLayoutKind};
use crate::{
    el::ElId,
    env::LayoutEnv,
    layout::{
        Align, ContainerLayout, ContentLayout, DevHoveredLayout, LayoutCtx,
        LayoutKind, Limits, ScrollableLayout,
        flex::model_flex,
        length::LengthSize,
        tree::{LayoutTree, effective_single_child},
    },
    render::prelude::*,
};
use alloc::vec::Vec;
use core::fmt::{Debug, Display};

/// Layout tree representation with real position in viewport
pub struct LayoutModelNode<'a> {
    pub outer: Rect,
    pub inner: Rect,
    model: &'a LayoutModel,
}

impl<'a> Debug for LayoutModelNode<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut f = f.debug_struct("LayoutModelNode");
        f.field("inner", &self.inner);
        f.field("outer", &self.outer);
        #[cfg(feature = "debug-info")]
        f.field("dev", &self.model.dev);
        // TODO: How can I avoid collecting to vector without `field_with`?
        f.field("children", &self.children().collect::<Vec<_>>());
        f.finish()
    }
}

impl<'a> LayoutModelNode<'a> {
    /// WS5.1: the `ElId` this layout node was computed for — the render/event
    /// passes dispatch to the widget by this identity (walking the layout tree)
    /// instead of positionally zipping arena children against layout children.
    pub fn id(&self) -> ElId {
        self.model.id
    }

    pub fn env(&self) -> Option<LayoutEnv> {
        self.model.env
    }

    pub fn translate(&self, by: Point) -> Self {
        Self {
            outer: self.outer.translate(by),
            inner: self.inner.translate(by),
            model: self.model,
        }
    }

    pub fn children(&'a self) -> impl Iterator<Item = LayoutModelNode<'a>> {
        self.model
            .children
            .iter()
            .map(|child| child.node(self.inner))
    }

    /// Number of layout children. WS5.1: the render/event passes no longer use
    /// this — they dispatch by `ElId` from the layout tree, not by positionally
    /// zipping arena children — but it stays as a small structural accessor
    /// (e.g. asserting child counts in tests).
    pub fn children_len(&self) -> usize {
        self.model.children.len()
    }

    // Note: May be slow and expensive
    pub fn dev_hover(&'a self, point: Point) -> Option<DevHoveredLayout> {
        self.children()
            .find_map(|child| child.dev_hover(point))
            .or_else(|| {
                if self.outer.contains(point) {
                    Some(DevHoveredLayout {
                        area: self.outer,
                        children_count: self.model.children.len(),
                        #[cfg(feature = "debug-info")]
                        layout: self.model.dev.clone(),
                    })
                } else {
                    None
                }
            })
    }
}

/// WS5.2: per-node state retained (only under `incremental-layout`) so a dirty
/// node can be recomputed in isolation — the exact inputs `model_layout` was
/// called with for this node — plus `min_size`, the second half of the
/// `(outer_size, min_size)` stop-rule key. ≤64 B; compiled out by default.
#[cfg(feature = "incremental-layout")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Retained {
    pub parent_limits: Limits,
    pub parent_size: LengthSize,
    pub input_env: LayoutEnv,
    pub min_size: Size,
}

/// Layout tree representation with relative positions
// WS5.2: `Clone` so the incremental path can splice a copy of the previous tree
// (`relayout_incremental`). Only exercised under `incremental-layout`.
#[derive(Debug, Clone)]
pub struct LayoutModel {
    // WS5.1: the `ElId` this layout node was computed for. Set by `model_layout`
    // (which is always called with the node's id) so the render/event passes can
    // dispatch to the widget by identity — walking the layout tree directly —
    // instead of positionally zipping arena children against layout children.
    // Transparent nodes (`Dynamic`) are already resolved by `effective_children`
    // at build, so every layout child carries the id of a real (rendered) widget.
    id: ElId,
    outer: Rect,
    inner: Rect,

    env: Option<LayoutEnv>,

    children: Vec<LayoutModel>,

    // WS5.2: retained recompute-inputs + min_size (see `Retained`). `None` until
    // stamped by `model_layout`; a `None` node forces a full recompute of its
    // subtree on the incremental path (conservative, always correct).
    #[cfg(feature = "incremental-layout")]
    retained: Option<Retained>,

    // Note: `dev` goes before `children` which is intentional to make more
    // readable pretty-printed debug
    // TODO: Make debug_assertions-only
    #[cfg(feature = "debug-info")]
    dev: DevLayout,
    // TODO: Tinyvec
}

// WS5.2: geometry-only equality. The memo's change-detection (and the
// differential fuzz test) compare what is VISIBLE — id + rects + env +
// children — never the retained recompute-metadata or the debug `dev`, both of
// which are derived and would otherwise spuriously invalidate the memo.
impl PartialEq for LayoutModel {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.outer == other.outer
            && self.inner == other.inner
            && self.env == other.env
            && self.children == other.children
    }
}

impl LayoutModel {
    pub fn new(
        inner_size: Size,
        children: Vec<LayoutModel>,
        #[cfg(feature = "debug-info")] dev: DevLayout,
    ) -> Self {
        Self {
            // Defaulted here; `model_layout` stamps the real id via `with_id`
            // (every model flows through it). The null default only lingers on a
            // model no `model_layout` call ever tags, which never reaches a pass.
            id: ElId::default(),
            outer: Rect::new(Point::zero(), inner_size),
            inner: Rect::new(Point::zero(), inner_size),
            children,
            env: None,
            #[cfg(feature = "incremental-layout")]
            retained: None,
            #[cfg(feature = "debug-info")]
            dev,
        }
    }

    /// WS5.1: stamp the `ElId` this layout node was computed for (see the field
    /// doc). Called by `model_layout` on every node it returns.
    pub fn with_id(mut self, id: ElId) -> Self {
        self.id = id;
        self
    }

    /// WS5.2: stamp the recompute-inputs + min_size (see [`Retained`]). Called by
    /// `model_layout` under `incremental-layout` so a dirty node can later be
    /// recomputed in isolation and its size compared against `min_size`.
    #[cfg(feature = "incremental-layout")]
    pub(crate) fn with_retained(mut self, retained: Retained) -> Self {
        self.retained = Some(retained);
        self
    }

    pub fn with_env(mut self, env: Option<LayoutEnv>) -> Self {
        self.env = env;
        self
    }

    /// Full padding includes padding + border size
    pub fn with_full_padding(mut self, full_padding: Padding) -> Self {
        let padding_size: Size = full_padding.into();

        self.inner = self.inner.translate(full_padding.top_left());

        let new_size = self.outer.size + padding_size;
        self.outer = Rect::new(self.outer.top_left, new_size);
        self
    }

    pub fn tree_root(&self) -> LayoutModelNode<'_> {
        LayoutModelNode { outer: self.outer, inner: self.inner, model: self }
    }

    fn node(&self, parent_inner: Rect) -> LayoutModelNode<'_> {
        LayoutModelNode {
            outer: self.outer.translate(parent_inner.top_left),
            inner: self.inner.translate(parent_inner.top_left),
            model: self,
        }
    }

    pub fn zero() -> Self {
        Self {
            id: ElId::default(),
            outer: Rect::zero(),
            inner: Rect::zero(),
            children: vec![],
            env: None,
            #[cfg(feature = "incremental-layout")]
            retained: None,
            #[cfg(feature = "debug-info")]
            dev: DevLayout::zero(),
        }
    }

    pub fn outer_size(&self) -> Size {
        self.outer.size
    }

    // pub fn move_mut(&mut self, to: impl Into<Point> + Copy) -> &mut Self {
    //     self.outer.top_left = to.into();
    //     self.inner.top_left = to.into();
    //     self
    // }

    // pub fn moved(mut self, to: impl Into<Point> + Copy) -> Self {
    //     self.move_mut(to);
    //     self
    // }

    pub fn translate_mut(&mut self, by: Point) -> &mut Self {
        self.outer.top_left += by;
        self.inner.top_left += by;
        self
    }

    pub fn align_mut(
        &mut self,
        horizontal: Align,
        vertical: Align,
        free_space: Size,
    ) -> &mut Self {
        let x = match horizontal {
            Align::Start => 0,
            Align::Center => (free_space.width as i32) / 2,
            Align::End => free_space.width as i32,
        };

        let y = match vertical {
            Align::Start => 0,
            Align::Center => {
                (free_space.height as i32) / 2
                // - self.relative_area.size.height as i32 / 2;
            },
            Align::End => {
                free_space.height as i32
                // - self.relative_area.size.height as i32;
            },
        };

        self.translate_mut(Point::new(x, y));

        self
    }

    pub fn aligned(
        mut self,
        horizontal: Align,
        vertical: Align,
        parent_size: Size,
    ) -> Self {
        self.align_mut(horizontal, vertical, parent_size);
        self
    }
}

pub struct PPLayoutModel<'a> {
    model: &'a LayoutModel,
    indent: usize,
}

impl<'a> PPLayoutModel<'a> {
    pub fn root(model: &'a LayoutModel) -> Self {
        Self { model, indent: 0 }
    }
}

impl<'a> Display for PPLayoutModel<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:indent$}{}>{}",
            "",
            self.model.outer,
            self.model.inner,
            indent = self.indent
        )?;

        #[cfg(feature = "debug-info")]
        write!(f, " {}", self.model.dev)?;

        self.model.children.iter().try_for_each(|child| {
            write!(
                f,
                "\n{}",
                PPLayoutModel { model: child, indent: self.indent + 1 }
            )
        })
    }
}

// TODO: Split layouts declarations and layout modeling logic into separate files

// TODO: Should viewport be unwrapped value as we depend modeling on viewport
// value?
// WS5.1: walks the arena (`tree` + `id`) instead of a self-contained `Layout`
// handle sub-tree. `tree.layout(id)` reads the node's live layout handle, so
// the walk sees the same reactive-current data as before; only the child
// *source* changed (arena `effective_children`, not `FlexLayout.children`).
pub fn model_layout<T: LayoutTree + ?Sized>(
    ctx: &LayoutCtx,
    tree: &T,
    id: ElId,
    parent_limits: Limits,
    parent_size: LengthSize, // viewport: Memo<Size>,
) -> LayoutModel {
    #[cfg(feature = "layout-counters")]
    crate::layout::counters::count_visit();

    // WS5.1: a transparent node (e.g. `Dynamic`) has no layout of its own — it
    // is flattened to its single effective child. As a *child* it is already
    // skipped by `effective_children`/`effective_single_child`, so this only
    // fires when a transparent node is the ROOT (e.g. a `Dynamic` page root),
    // which nothing else flattens. The returned model carries the effective
    // child's id, so the passes dispatch to the real widget, not the husk.
    if tree.is_transparent(id) {
        return effective_single_child(tree, id)
            .map(|child| {
                model_layout(ctx, tree, child, parent_limits, parent_size)
            })
            .unwrap_or_else(|| LayoutModel::zero().with_id(id));
    }

    // WS5.1: `tree.layout(id)` is the arena-owned `&LayoutData` (no handle,
    // no `.with`).
    let Some(layout) = tree.layout(id) else {
        return LayoutModel::zero().with_id(id);
    };
    if !layout.is_shown() {
        // A hidden element resolves to a zero layout here; `model_flex`
        // additionally filters hidden children out of its sizing/gap passes
        // (via `LayoutData::is_shown`) so they leave no phantom gap.
        return LayoutModel::zero().with_id(id);
    }

    let size = layout.size.in_parent(parent_size);

    // WS5.1: every arm's model is stamped with `id` (see `LayoutModel::id`) so
    // the render/event passes can dispatch to this widget by identity.
    let model = match &layout.kind {
        // TODO: Panic or not?
        LayoutKind::Zero => LayoutModel::zero(),
        LayoutKind::Edge => LayoutModel::new(
            parent_limits.resolve_size(size, Size::zero(), None),
            vec![],
            #[cfg(feature = "debug-info")]
            DevLayout::new(size, DevLayoutKind::Edge),
        ),
        LayoutKind::Content(content_layout) => {
            let sizing = content_layout.content_sizing(ctx);
            let layout_env = match content_layout {
                ContentLayout::Text { env: text_env, .. }
                    if text_env.has_any() =>
                {
                    let resolved = text_env.inherited(&ctx.env);
                    Some(resolved)
                },
                _ => None,
            };

            LayoutModel::new(
                parent_limits.resolve_content_size(size, &sizing, |width| {
                    content_layout.height_for_width(ctx, width)
                }),
                vec![],
                #[cfg(feature = "debug-info")]
                DevLayout::new(
                    size,
                    DevLayoutKind::Content(content_layout.clone()),
                ),
            )
            .with_env(layout_env)
        },
        LayoutKind::Container(container_layout) => {
            let ContainerLayout {
                block_model,
                horizontal_align,
                vertical_align,
                env: container_env,
            } = container_layout;

            // let min_content = content_size.get().min();

            let full_padding = block_model.padding;

            let child_env = container_env.inherited(&ctx.env);
            let child_ctx = LayoutCtx { env: child_env, ..*ctx };

            let content_limits =
                parent_limits.child_limits(size).shrink(full_padding);
            let content_layout = effective_single_child(tree, id)
                .map(|content_id| {
                    model_layout(
                        &child_ctx,
                        tree,
                        content_id,
                        content_limits,
                        size,
                    )
                })
                .unwrap_or_else(LayoutModel::zero);

            let content_size = content_layout.outer_size();
            let real_size = parent_limits.resolve_size(
                size,
                content_size,
                Some(full_padding),
            );
            let content_layout = content_layout.aligned(
                *horizontal_align,
                *vertical_align,
                real_size - content_size,
            );

            LayoutModel::new(
                // TODO: Generalize logic with real_size.expand/shrink and
                // full_padding
                real_size,
                vec![content_layout],
                #[cfg(feature = "debug-info")]
                DevLayout::new(
                    size,
                    DevLayoutKind::Container(container_layout.clone()),
                ),
            )
            .with_full_padding(full_padding)
            .with_env(container_env.has_any().then_some(child_env))
        },
        LayoutKind::Scrollable(scrollable_layout) => {
            let ScrollableLayout { env: scrollable_env } = scrollable_layout;

            let child_env = scrollable_env.inherited(&ctx.env);
            let child_ctx = LayoutCtx { env: child_env, ..*ctx };

            let content_limits = parent_limits.child_limits(size);
            let content_layout = effective_single_child(tree, id)
                .map(|content_id| {
                    model_layout(
                        &child_ctx,
                        tree,
                        content_id,
                        content_limits,
                        size,
                    )
                })
                .unwrap_or_else(LayoutModel::zero);

            let real_size = parent_limits.resolve_size(
                size,
                content_layout.outer_size(),
                None,
            );

            LayoutModel::new(
                real_size,
                vec![content_layout],
                #[cfg(feature = "debug-info")]
                DevLayout::new(
                    size,
                    DevLayoutKind::Scrollable(scrollable_layout.clone()),
                ),
            )
            .with_env(scrollable_env.has_any().then_some(child_env))
        },
        LayoutKind::Flex(flex_layout) => {
            model_flex(ctx, tree, id, parent_limits, flex_layout, size)
        },
    };

    let model = model.with_id(id);

    // WS5.2: retain this node's recompute-inputs + min_size so the incremental
    // path can recompute it in isolation and apply the (outer_size, min_size)
    // stop rule. `min_size` here is a full subtree descent (O(subtree)) — fine
    // for the rare full relayout that produces `prev`; a bottom-up single-pass
    // min_size is a follow-up optimisation. Compiled out by default.
    #[cfg(feature = "incremental-layout")]
    let model = model.with_retained(Retained {
        parent_limits,
        parent_size,
        input_env: ctx.env,
        // WS5.2: retain the RESOLVED min — exactly what a flex parent uses for
        // this child (`child_size.max_fixed(min, limits.max())`, see `model_flex`)
        // — not the raw content-min. For a fixed dimension this is the fixed
        // value, so a Fixed×Fixed node's stop-rule key is stable across content
        // changes (the "1 visit" acceptance); a shrink dimension still tracks the
        // content min, so it correctly propagates. No flex-behaviour change: the
        // flex already computes this resolved value itself.
        min_size: layout
            .size
            .max_fixed(layout.min_size(ctx, tree, id), parent_limits.max()),
    });

    model
}

// ─── WS5.2: incremental relayout ─────────────────────────────────────────────

/// Incremental relayout: splice the previous `LayoutModel` (`prev`), recomputing
/// only the `dirty` nodes and propagating upward until a node's
/// `(outer_size, min_size)` are both unchanged (the stop rule). The result is
/// equal, rect-for-rect, to a full `model_layout` over the mutated arena — the
/// differential fuzz test (`incremental_equals_full_recompute`) is the
/// guarantee. Feature-gated; the caller falls back to a full relayout for a
/// whole-tree (`full`) dirty set or a missing `prev`.
#[cfg(feature = "incremental-layout")]
pub fn relayout_incremental<T: LayoutTree + ?Sized>(
    prev: &LayoutModel,
    dirty: &[ElId],
    fonts: &crate::font::FontCtx,
    viewport: Size,
    tree: &T,
) -> LayoutModel {
    let mut result = prev.clone();
    for &id in dirty {
        recompute_upward(&mut result, id, fonts, viewport, tree);
    }
    result
}

/// Recompute `target`'s subtree in isolation from its retained inputs; if its
/// `(outer_size, min_size)` are unchanged, splice it back at its previous offset
/// and stop, otherwise recompute its parent (which re-places its children,
/// `target` included) and repeat upward — stopping at the nearest size-stable
/// ancestor, or the root.
#[cfg(feature = "incremental-layout")]
fn recompute_upward<T: LayoutTree + ?Sized>(
    result: &mut LayoutModel,
    target: ElId,
    fonts: &crate::font::FontCtx,
    viewport: Size,
    tree: &T,
) {
    let mut target = target;
    loop {
        let Some(node) = find_node(result, target) else { return };
        // Copy the retained inputs + old size/offset out so `node`'s borrow of
        // `result` ends before the mutable splice below.
        let (Some(retained), old_size, old_offset) =
            (node.retained, node.outer.size, node.outer.top_left)
        else {
            // A node with no retained inputs (a hidden/zero slot that a `show`
            // toggle may now reveal) — conservatively rebuild the whole tree.
            rebuild_root(result, fonts, viewport, tree);
            return;
        };
        let old_min = retained.min_size;

        let ctx = LayoutCtx { fonts, viewport, env: retained.input_env };
        let mut new_sub = model_layout(
            &ctx,
            tree,
            target,
            retained.parent_limits,
            retained.parent_size,
        );
        let new_min = new_sub.retained.map_or(old_min, |r| r.min_size);

        if new_sub.outer.size == old_size && new_min == old_min {
            // Size stable ⇒ the parent places `target` at the same offset (or,
            // if `target` is the root, its recomputed subtree IS the new tree —
            // the root sits at the viewport origin, so no re-offset).
            match parent_id(result, target) {
                Some(_) => {
                    new_sub.translate_mut(old_offset);
                    splice_node(result, target, new_sub);
                },
                None => *result = new_sub,
            }
            return;
        }

        // Size changed ⇒ the parent must re-place its children.
        match parent_id(result, target) {
            Some(pid) => target = pid,
            None => {
                // `target` is the root: its recomputed subtree IS the new tree.
                *result = new_sub;
                return;
            },
        }
    }
}

/// Rebuild the whole tree from `result`'s root using the root's retained inputs
/// (the conservative fallback when a dirty node lost its retained state).
#[cfg(feature = "incremental-layout")]
fn rebuild_root<T: LayoutTree + ?Sized>(
    result: &mut LayoutModel,
    fonts: &crate::font::FontCtx,
    viewport: Size,
    tree: &T,
) {
    if let Some(r) = result.retained {
        let ctx = LayoutCtx { fonts, viewport, env: r.input_env };
        *result =
            model_layout(&ctx, tree, result.id, r.parent_limits, r.parent_size);
    }
}

#[cfg(feature = "incremental-layout")]
fn find_node(node: &LayoutModel, id: ElId) -> Option<&LayoutModel> {
    if node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_node(c, id))
}

#[cfg(feature = "incremental-layout")]
fn parent_id(node: &LayoutModel, id: ElId) -> Option<ElId> {
    if node.children.iter().any(|c| c.id == id) {
        return Some(node.id);
    }
    node.children.iter().find_map(|c| parent_id(c, id))
}

#[cfg(feature = "incremental-layout")]
fn find_node_mut(node: &mut LayoutModel, id: ElId) -> Option<&mut LayoutModel> {
    if node.id == id {
        return Some(node);
    }
    // Locate the child subtree containing `id` immutably, then recurse mutably
    // into just that one (sidesteps the borrow-checker's return-in-loop limit).
    let idx = node
        .children
        .iter()
        .position(|c| find_node(c, id).is_some())?;
    find_node_mut(&mut node.children[idx], id)
}

/// Replace the (non-root) node `id` with `new_node`, in place in its parent's
/// child list.
#[cfg(feature = "incremental-layout")]
fn splice_node(root: &mut LayoutModel, id: ElId, new_node: LayoutModel) {
    let Some(pid) = parent_id(root, id) else { return };
    if let Some(parent) = find_node_mut(root, pid) {
        for child in parent.children.iter_mut() {
            if child.id == id {
                *child = new_node;
                return;
            }
        }
    }
}

/// WS5.3: the layout **changed-set** — the geometry damage a relayout produced.
/// Diffs the previous and new `LayoutModel` trees and returns, for every node
/// whose **absolute** rect changed, the union of its old and new rect (erase
/// where it was, paint where it is). This is the *geometry* damage channel WS6
/// consumes.
///
/// Note the boundary: a same-size **content** change (e.g. a Fixed×Fixed label's
/// text) moves nothing, so it produces NO entry here — that widget repaints via
/// its render probe (the independent *paint-dirty* channel). WS6's total damage
/// is this set ∪ the paint-dirty widget rects.
///
/// `prev`/`new` share structure — incremental relayout never changes it (a
/// structure change falls back to a full relayout) — so nodes are matched by
/// `ElId`; the appeared/disappeared arms are defensive for a general diff.
#[cfg(feature = "incremental-layout")]
pub fn layout_changed_set(prev: &LayoutModel, new: &LayoutModel) -> Vec<Rect> {
    let mut damage = Vec::new();
    diff_changed(prev, Point::zero(), new, Point::zero(), &mut damage);
    damage
}

/// Recurse `prev`/`new` in lockstep. `*_off` is the parent's absolute inner
/// top-left — the origin its children are placed against (mirrors
/// [`LayoutModel::node`]) — so `node.outer.translate(off)` is the node's
/// absolute rect.
#[cfg(feature = "incremental-layout")]
fn diff_changed(
    prev: &LayoutModel,
    prev_off: Point,
    new: &LayoutModel,
    new_off: Point,
    damage: &mut Vec<Rect>,
) {
    let prev_abs = prev.outer.translate(prev_off);
    let new_abs = new.outer.translate(new_off);
    if prev_abs != new_abs {
        damage.push(prev_abs.union(&new_abs));
    }

    // Children are placed against each node's absolute inner origin.
    let prev_child_off = prev.inner.translate(prev_off).top_left;
    let new_child_off = new.inner.translate(new_off).top_left;

    for new_child in &new.children {
        match prev.children.iter().find(|c| c.id == new_child.id) {
            Some(prev_child) => diff_changed(
                prev_child,
                prev_child_off,
                new_child,
                new_child_off,
                damage,
            ),
            // Appeared (structure change): its outer rect covers the new subtree.
            None => damage.push(new_child.outer.translate(new_child_off)),
        }
    }
    // Disappeared: present in `prev`, gone from `new`.
    for prev_child in &prev.children {
        if !new.children.iter().any(|c| c.id == prev_child.id) {
            damage.push(prev_child.outer.translate(prev_child_off));
        }
    }
}

/// WS6.1: the **repaint roots** a targeted relayout must redraw to realize `new`
/// from `prev` without ghosting — the render/invalidation counterpart of
/// [`layout_changed_set`]'s flush rects.
///
/// When a node *moves* (its absolute rect changes), repainting it at its new
/// position leaves a stale copy at the old one. The fix is to repaint the
/// **nearest size-stable ancestor** of each moved node: its own rect is
/// unchanged and its inner area contains *both* the old and new child positions,
/// so re-clearing + redrawing it erases the ghost and paints the new layout in
/// one bounded op. That ancestor is the parent of the *top* of each changed
/// subtree (the shallowest node whose absolute rect changed — its parent's did
/// not). The whole changed subtree below it is covered by the ancestor's
/// repaint, so we do not descend into it.
///
/// `full` is set when the ROOT itself changed: no stable ancestor exists, so the
/// caller must fall back to a whole-viewport repaint.
#[cfg(feature = "incremental-layout")]
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RepaintRoots {
    pub roots: Vec<ElId>,
    pub full: bool,
}

#[cfg(feature = "incremental-layout")]
impl RepaintRoots {
    fn mark(&mut self, id: ElId) {
        if !self.roots.contains(&id) {
            self.roots.push(id);
        }
    }
}

/// See [`RepaintRoots`]. Pure function of `(prev, new)`; matches nodes by `ElId`
/// (incremental relayout never changes structure).
#[cfg(feature = "incremental-layout")]
pub fn layout_repaint_roots(
    prev: &LayoutModel,
    new: &LayoutModel,
) -> RepaintRoots {
    let mut out = RepaintRoots::default();
    diff_repaint_roots(prev, Point::zero(), new, Point::zero(), None, &mut out);
    out
}

/// Recurse `prev`/`new` in lockstep (mirrors [`diff_changed`]), carrying the
/// current node's *parent* id. A node whose absolute rect changed is the top of
/// a changed subtree (its parent recursed here only because the parent was
/// stable): mark the parent as the repaint root and stop — the parent's repaint
/// covers the entire subtree. A stable node recurses into its children.
#[cfg(feature = "incremental-layout")]
fn diff_repaint_roots(
    prev: &LayoutModel,
    prev_off: Point,
    new: &LayoutModel,
    new_off: Point,
    parent_id: Option<ElId>,
    out: &mut RepaintRoots,
) {
    let prev_abs = prev.outer.translate(prev_off);
    let new_abs = new.outer.translate(new_off);
    if prev_abs != new_abs {
        match parent_id {
            // The parent is stable and contains both old and new positions.
            Some(pid) => out.mark(pid),
            // The root moved/resized — no stable ancestor, repaint everything.
            None => out.full = true,
        }
        return;
    }

    // Stable node: its children are placed against its absolute inner origin.
    let prev_child_off = prev.inner.translate(prev_off).top_left;
    let new_child_off = new.inner.translate(new_off).top_left;
    for new_child in &new.children {
        match prev.children.iter().find(|c| c.id == new_child.id) {
            Some(prev_child) => diff_repaint_roots(
                prev_child,
                prev_child_off,
                new_child,
                new_child_off,
                Some(new.id),
                out,
            ),
            // Appeared child (structure change — defensive; incremental relayout
            // falls back to full for those): repaint this stable parent so the
            // new subtree is drawn.
            None => out.mark(new.id),
        }
    }
    // Disappeared child: repaint this parent to clear where it was.
    for prev_child in &prev.children {
        if !new.children.iter().any(|c| c.id == prev_child.id) {
            out.mark(new.id);
        }
    }
}

#[cfg(all(test, feature = "incremental-layout"))]
mod incremental_fuzz {
    use super::{LayoutModel, model_layout, relayout_incremental};
    use crate::{
        el::ElId,
        env::LayoutEnv,
        font::FontCtx,
        layout::{
            FlexLayout, LayoutCtx, LayoutData, LayoutKind, Limits,
            length::LengthSize, tree::LayoutTree,
        },
        render::prelude::*,
    };
    use alloc::vec::Vec;
    use slotmap::{KeyData, SecondaryMap};

    fn el_id(n: u64) -> ElId {
        ElId::from(KeyData::from_ffi(n))
    }

    /// A minimal mutable arena for the kernel, including transparent
    /// (`Dynamic`-like) single-child pass-through nodes.
    struct TestTree {
        layouts: SecondaryMap<ElId, LayoutData>,
        children: SecondaryMap<ElId, Vec<ElId>>,
        transparent: SecondaryMap<ElId, ()>,
    }
    impl LayoutTree for TestTree {
        fn layout(&self, id: ElId) -> Option<&LayoutData> {
            self.layouts.get(id)
        }
        fn children(&self, id: ElId) -> &[ElId] {
            self.children.get(id).map(|v| v.as_slice()).unwrap_or(&[])
        }
        fn is_transparent(&self, id: ElId) -> bool {
            self.transparent.contains_key(id)
        }
    }
    impl TestTree {
        fn new() -> Self {
            Self {
                layouts: SecondaryMap::new(),
                children: SecondaryMap::new(),
                transparent: SecondaryMap::new(),
            }
        }
    }

    /// Deterministic SplitMix64 so any failure reproduces from its seed.
    struct Rng(u64);
    impl Rng {
        fn u(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^ (z >> 31)
        }
        fn range(&mut self, n: u32) -> u32 {
            (self.u() % n as u64) as u32
        }
    }

    fn rand_edge(rng: &mut Rng) -> LayoutData {
        LayoutData::edge(LengthSize::fixed_length(
            5 + rng.range(40),
            5 + rng.range(40),
        ))
    }

    /// Build a random tree of flex containers + fixed-size edge leaves. Collects
    /// every leaf id (the mutation targets).
    fn build(
        rng: &mut Rng,
        tree: &mut TestTree,
        next: &mut u64,
        depth: u32,
        leaves: &mut Vec<ElId>,
    ) -> ElId {
        let id = el_id(*next);
        *next += 1;
        if depth == 0 || rng.range(3) == 0 {
            tree.layouts.insert(id, rand_edge(rng));
            leaves.push(id);
        } else if rng.range(4) == 0 {
            // Transparent single-child pass-through (like `Dynamic`): flattened
            // by `effective_children`, so absent from the layout tree — the
            // incremental walk must skip it via layout-tree `parent_id`.
            tree.transparent.insert(id, ());
            tree.layouts.insert(id, LayoutData::zero());
            let child = build(rng, tree, next, depth - 1, leaves);
            tree.children.insert(id, alloc::vec![child]);
        } else {
            let n = 1 + rng.range(3);
            let kids: Vec<ElId> = (0..n)
                .map(|_| build(rng, tree, next, depth - 1, leaves))
                .collect();
            let axis = if rng.range(2) == 0 { Axis::X } else { Axis::Y };
            // Mix shrink (size flows up on any child change) with fixed
            // (size-stable ⇒ the stop rule halts at this flex) containers so both
            // stop-rule branches are exercised.
            let size = if rng.range(2) == 0 {
                LengthSize::shrink()
            } else {
                LengthSize::fixed_length(80 + rng.range(80), 80 + rng.range(80))
            };
            tree.layouts.insert(
                id,
                LayoutData::new(
                    LayoutKind::Flex(
                        FlexLayout::base(axis)
                            .gap(Size::new_equal(rng.range(6))),
                    ),
                    size,
                ),
            );
            tree.children.insert(id, kids);
        }
        id
    }

    /// WS5.2 differential fuzz: for many random trees + a random single leaf-size
    /// mutation, incremental relayout must equal a full recompute rect-for-rect
    /// (geometry-only `LayoutModel` equality). Covers the stop-at-leaf path (same
    /// size), upward propagation (shrink chains), and halting at a fixed ancestor.
    #[test]
    fn incremental_equals_full_recompute() {
        let fonts = FontCtx::new();
        let viewport = Size::new(200, 200);
        let ctx =
            LayoutCtx { fonts: &fonts, viewport, env: LayoutEnv::default() };

        for seed in 0u64..500 {
            let mut rng =
                Rng(seed.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(1));
            let mut tree = TestTree::new();
            let mut next = 1u64;
            let mut leaves = Vec::new();
            let root = build(&mut rng, &mut tree, &mut next, 4, &mut leaves);
            if leaves.is_empty() {
                continue;
            }

            let prev = model_layout(
                &ctx,
                &tree,
                root,
                Limits::only_max(viewport),
                viewport.into(),
            );

            // Mutate one random leaf's size — may be the same (stop at the leaf)
            // or different (propagate upward).
            let target = leaves[rng.range(leaves.len() as u32) as usize];
            tree.layouts[target] = rand_edge(&mut rng);

            let incremental =
                relayout_incremental(&prev, &[target], &fonts, viewport, &tree);
            let full = model_layout(
                &ctx,
                &tree,
                root,
                Limits::only_max(viewport),
                viewport.into(),
            );

            assert_eq!(
                incremental, full,
                "seed {seed}: incremental != full recompute (target {target:?})"
            );
        }
    }

    /// WS5.2 acceptance (D3): a content change to a Fixed×Fixed leaf visits
    /// exactly ONE node. The leaf's `(outer_size, resolved min_size)` are both
    /// the fixed box (the resolved min clamps content-min to the fixed dim), so
    /// the stop rule halts at the leaf — the whole point of the retained
    /// `min_size` resolution. Needs `layout-counters` for the visit count.
    #[cfg(feature = "layout-counters")]
    #[test]
    fn fixed_content_change_is_one_visit() {
        use crate::font::Font;
        use crate::layout::{ContentLayout, counters};
        use alloc::{string::ToString, vec};
        use rsact_reactive::prelude::MaybeReactive;

        let fonts = FontCtx::new();
        let viewport = Size::new(300, 300);
        let ctx = LayoutCtx {
            fonts: &fonts,
            viewport,
            // Match the page memo: an inheritable auto font, so text nodes have a
            // font to measure with (`LayoutEnv::default()` has `font: None`).
            env: LayoutEnv {
                font: Some(Font::Auto),
                font_size: None,
                font_style: None,
            },
        };

        // A Fixed×Fixed text box (short text fits, so the resolved min is the
        // fixed size regardless of the exact content).
        fn text_leaf(s: &str) -> LayoutData {
            LayoutData::new(
                LayoutKind::Content(ContentLayout::text(
                    MaybeReactive::new_inert(s.to_string()),
                )),
                LengthSize::fixed_length(200, 40),
            )
        }

        let (root, leaf) = (el_id(1), el_id(2));
        let mut tree = TestTree::new();
        tree.layouts.insert(
            root,
            LayoutData::new(
                LayoutKind::Flex(FlexLayout::base(Axis::Y)),
                LengthSize::shrink(),
            ),
        );
        tree.layouts.insert(leaf, text_leaf("hello"));
        tree.children.insert(root, vec![leaf]);

        let prev = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );

        // Update the text, keeping the fixed box — only the leaf is dirty.
        tree.layouts[leaf] = text_leaf("world");

        counters::reset();
        let incremental =
            relayout_incremental(&prev, &[leaf], &fonts, viewport, &tree);
        let (visits, _measures) = counters::snapshot();

        assert_eq!(
            visits, 1,
            "a Fixed×Fixed content change must visit exactly one node, got {visits}"
        );

        // …and still equal a full recompute.
        let full = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );
        assert_eq!(incremental, full);
    }

    /// WS5.3: two identical relayouts produce no geometry damage.
    #[test]
    fn changed_set_empty_when_layout_unchanged() {
        use super::layout_changed_set;
        let fonts = FontCtx::new();
        let viewport = Size::new(200, 200);
        let ctx =
            LayoutCtx { fonts: &fonts, viewport, env: LayoutEnv::default() };
        let mut rng = Rng(12345);
        let mut tree = TestTree::new();
        let mut next = 1u64;
        let mut leaves = Vec::new();
        let root = build(&mut rng, &mut tree, &mut next, 4, &mut leaves);
        let a = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );
        let b = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );
        assert!(
            layout_changed_set(&a, &b).is_empty(),
            "no mutation ⇒ no geometry damage"
        );
    }

    /// WS5.3: a leaf that grows damages itself AND the sibling it pushes down
    /// (and their shrink-sized ancestor).
    #[test]
    fn changed_set_nonempty_when_a_leaf_resizes() {
        use super::layout_changed_set;
        use alloc::vec;
        let fonts = FontCtx::new();
        let viewport = Size::new(200, 200);
        let ctx =
            LayoutCtx { fonts: &fonts, viewport, env: LayoutEnv::default() };
        let (root, a, b) = (el_id(1), el_id(2), el_id(3));
        let mut tree = TestTree::new();
        tree.layouts
            .insert(a, LayoutData::edge(LengthSize::fixed_length(10, 10)));
        tree.layouts
            .insert(b, LayoutData::edge(LengthSize::fixed_length(10, 10)));
        tree.layouts.insert(
            root,
            LayoutData::new(
                LayoutKind::Flex(FlexLayout::base(Axis::Y)),
                LengthSize::shrink(),
            ),
        );
        tree.children.insert(root, vec![a, b]);

        let prev = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );
        // Grow leaf `a` taller: it resizes and pushes `b` down.
        tree.layouts[a] = LayoutData::edge(LengthSize::fixed_length(10, 30));
        let new = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );

        let damage = layout_changed_set(&prev, &new);
        assert!(
            damage.len() >= 2,
            "resized leaf + shifted sibling must both be damaged, got {damage:?}"
        );
    }

    /// WS5.3: `layout_changed_set` equals a brute-force diff of every node's
    /// ABSOLUTE rect computed via the independent `LayoutModelNode` walker (the
    /// one render/event trust) — across the same 500 random trees + a single
    /// mutation as the WS5.2 fuzz. This cross-checks the changed-set's own
    /// offset accumulation against production's.
    #[test]
    fn changed_set_matches_brute_force_diff() {
        use super::{LayoutModelNode, layout_changed_set};

        // Every node's absolute outer rect, via the trusted walker.
        fn flatten(node: &LayoutModelNode, out: &mut Vec<(ElId, Rect)>) {
            out.push((node.id(), node.outer));
            for c in node.children() {
                flatten(&c, out);
            }
        }
        fn key(r: &Rect) -> (i32, i32, u32, u32) {
            (r.top_left.x, r.top_left.y, r.size.width, r.size.height)
        }

        let fonts = FontCtx::new();
        let viewport = Size::new(200, 200);
        let ctx =
            LayoutCtx { fonts: &fonts, viewport, env: LayoutEnv::default() };

        for seed in 0u64..500 {
            let mut rng =
                Rng(seed.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(1));
            let mut tree = TestTree::new();
            let mut next = 1u64;
            let mut leaves = Vec::new();
            let root = build(&mut rng, &mut tree, &mut next, 4, &mut leaves);
            if leaves.is_empty() {
                continue;
            }

            let prev = model_layout(
                &ctx,
                &tree,
                root,
                Limits::only_max(viewport),
                viewport.into(),
            );
            let target = leaves[rng.range(leaves.len() as u32) as usize];
            tree.layouts[target] = rand_edge(&mut rng);
            let new = model_layout(
                &ctx,
                &tree,
                root,
                Limits::only_max(viewport),
                viewport.into(),
            );

            // Brute-force expected damage: diff every node's absolute rect. The
            // node set is identical (leaf-size mutation preserves structure).
            let mut pv = Vec::new();
            flatten(&prev.tree_root(), &mut pv);
            let mut nv = Vec::new();
            flatten(&new.tree_root(), &mut nv);
            let mut expected = Vec::new();
            for (id, prect) in &pv {
                let nrect = nv
                    .iter()
                    .find(|(nid, _)| nid == id)
                    .map(|(_, r)| *r)
                    .expect("same node set");
                if *prect != nrect {
                    expected.push(prect.union(&nrect));
                }
            }

            let mut got = layout_changed_set(&prev, &new);
            got.sort_by_key(key);
            expected.sort_by_key(key);
            assert_eq!(
                got, expected,
                "seed {seed}: changed-set != brute-force diff (target {target:?})"
            );
        }
    }

    /// WS6.1: identical relayouts have no repaint roots and are not full.
    #[test]
    fn repaint_roots_empty_when_unchanged() {
        use super::layout_repaint_roots;
        let fonts = FontCtx::new();
        let viewport = Size::new(200, 200);
        let ctx =
            LayoutCtx { fonts: &fonts, viewport, env: LayoutEnv::default() };
        let mut rng = Rng(999);
        let mut tree = TestTree::new();
        let mut next = 1u64;
        let mut leaves = Vec::new();
        let root = build(&mut rng, &mut tree, &mut next, 4, &mut leaves);
        let a = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );
        let b = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );
        assert_eq!(layout_repaint_roots(&a, &b), Default::default());
    }

    /// WS6.1: a leaf moving inside a FIXED-size parent marks that parent (its
    /// rect is stable, so it is the repaint root), NOT the moved leaves — one
    /// clear of the parent erases the old positions and redraws the new ones.
    #[test]
    fn repaint_roots_marks_stable_parent_when_leaf_moves() {
        use super::layout_repaint_roots;
        use alloc::vec;
        let fonts = FontCtx::new();
        let viewport = Size::new(200, 200);
        let ctx =
            LayoutCtx { fonts: &fonts, viewport, env: LayoutEnv::default() };
        let (root, a, b) = (el_id(1), el_id(2), el_id(3));
        let mut tree = TestTree::new();
        tree.layouts
            .insert(a, LayoutData::edge(LengthSize::fixed_length(10, 10)));
        tree.layouts
            .insert(b, LayoutData::edge(LengthSize::fixed_length(10, 10)));
        // FIXED root: it does NOT resize when a child grows, so it stays the
        // stable ancestor (unlike the shrink root in the next test).
        tree.layouts.insert(
            root,
            LayoutData::new(
                LayoutKind::Flex(FlexLayout::base(Axis::Y)),
                LengthSize::fixed_length(100, 100),
            ),
        );
        tree.children.insert(root, vec![a, b]);

        let prev = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );
        // Grow `a`: it resizes and pushes `b` down, both inside the fixed root.
        tree.layouts[a] = LayoutData::edge(LengthSize::fixed_length(10, 30));
        let new = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );

        let roots = layout_repaint_roots(&prev, &new);
        assert!(!roots.full, "a fixed root does not resize ⇒ not full");
        assert_eq!(
            roots.roots,
            alloc::vec![root],
            "the stable parent is the single repaint root"
        );
    }

    /// WS6.1: when the change resizes the ROOT (a shrink-sized root grows with
    /// its content), there is no stable ancestor ⇒ `full` (whole-viewport
    /// repaint fallback).
    #[test]
    fn repaint_roots_full_when_shrink_root_grows() {
        use super::layout_repaint_roots;
        use alloc::vec;
        let fonts = FontCtx::new();
        let viewport = Size::new(200, 200);
        let ctx =
            LayoutCtx { fonts: &fonts, viewport, env: LayoutEnv::default() };
        let (root, a) = (el_id(1), el_id(2));
        let mut tree = TestTree::new();
        tree.layouts
            .insert(a, LayoutData::edge(LengthSize::fixed_length(10, 10)));
        tree.layouts.insert(
            root,
            LayoutData::new(
                LayoutKind::Flex(FlexLayout::base(Axis::Y)),
                LengthSize::shrink(),
            ),
        );
        tree.children.insert(root, vec![a]);

        let prev = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );
        tree.layouts[a] = LayoutData::edge(LengthSize::fixed_length(10, 40));
        let new = model_layout(
            &ctx,
            &tree,
            root,
            Limits::only_max(viewport),
            viewport.into(),
        );

        let roots = layout_repaint_roots(&prev, &new);
        assert!(roots.full, "shrink root grew ⇒ full-viewport repaint");
    }

    /// WS6.1 correctness (no ghosting): the repaint roots' absolute outer rects
    /// must COVER every changed node's OLD and NEW absolute rect — otherwise a
    /// moved widget's stale pixels are never cleared. Cross-checked against the
    /// brute-force per-node absolute-rect diff over the same 500 random trees +
    /// single mutation as the changed-set fuzz.
    #[test]
    fn repaint_roots_cover_all_damage() {
        use super::{LayoutModelNode, layout_repaint_roots};

        fn flatten(node: &LayoutModelNode, out: &mut Vec<(ElId, Rect)>) {
            out.push((node.id(), node.outer));
            for c in node.children() {
                flatten(&c, out);
            }
        }
        // `inner ⊆ outer`, treating rects as half-open [tl, tl+size).
        fn covers(outer: &Rect, inner: &Rect) -> bool {
            inner.is_zero_sized()
                || (inner.top_left.x >= outer.top_left.x
                    && inner.top_left.y >= outer.top_left.y
                    && inner.top_left.x + inner.size.width as i32
                        <= outer.top_left.x + outer.size.width as i32
                    && inner.top_left.y + inner.size.height as i32
                        <= outer.top_left.y + outer.size.height as i32)
        }

        let fonts = FontCtx::new();
        let viewport = Size::new(200, 200);
        let ctx =
            LayoutCtx { fonts: &fonts, viewport, env: LayoutEnv::default() };

        for seed in 0u64..500 {
            let mut rng =
                Rng(seed.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(1));
            let mut tree = TestTree::new();
            let mut next = 1u64;
            let mut leaves = Vec::new();
            let root = build(&mut rng, &mut tree, &mut next, 4, &mut leaves);
            if leaves.is_empty() {
                continue;
            }

            let prev = model_layout(
                &ctx,
                &tree,
                root,
                Limits::only_max(viewport),
                viewport.into(),
            );
            let target = leaves[rng.range(leaves.len() as u32) as usize];
            tree.layouts[target] = rand_edge(&mut rng);
            let new = model_layout(
                &ctx,
                &tree,
                root,
                Limits::only_max(viewport),
                viewport.into(),
            );

            let roots = layout_repaint_roots(&prev, &new);
            // `full` trivially repaints everything.
            if roots.full {
                continue;
            }

            // Absolute rects (new frame) keyed by id, for the root lookup.
            let mut nv = Vec::new();
            flatten(&new.tree_root(), &mut nv);
            let root_outers: Vec<Rect> = roots
                .roots
                .iter()
                .map(|id| {
                    nv.iter()
                        .find(|(nid, _)| nid == id)
                        .map(|(_, r)| *r)
                        .expect("repaint root is a live node in `new`")
                })
                .collect();

            // Every node whose absolute rect changed, old AND new, must be
            // covered by some repaint root's outer.
            let mut pv = Vec::new();
            flatten(&prev.tree_root(), &mut pv);
            for (id, prect) in &pv {
                let nrect = nv
                    .iter()
                    .find(|(nid, _)| nid == id)
                    .map(|(_, r)| *r)
                    .expect("same node set");
                if *prect == nrect {
                    continue;
                }
                for rect in [prect, &nrect] {
                    assert!(
                        root_outers.iter().any(|o| covers(o, rect)),
                        "seed {seed}: changed rect {rect:?} of {id:?} not \
                         covered by any repaint root {root_outers:?} \
                         (roots {:?}, target {target:?})",
                        roots.roots
                    );
                }
            }
        }
    }
}
