# Text layout design — intrinsic width range + height-for-width

Date: 2026-06-28
Status: Approved (design); ready for implementation planning
Scope: `rsact-ui` (`layout/`, `font/`, `widget/label.rs`)

## Problem

A leaf's size in `rsact` is currently a single scalar `min_size(ctx) -> Size`, resolved
into a final size by `Limits::resolve_size(size, content_size, padding)` which treats
the width and height axes symmetrically (`layout/limits.rs`).

Text does not fit this model. Its **height depends on the width it is given** (wrapping),
so it has no single intrinsic size. It has a *range* of natural widths:

- **min-content width** — the widest unbreakable unit (longest word). Below this, text
  cannot lay out without overflow.
- **max-content width** — the longest hard-break (`'\n'`) line, with no soft wrapping.
- **height-for-width** — once a width between those is chosen, the line count (hence
  height) follows.

Today `ContentLayout::min_size` for the `Text` variant calls
`ctx.fonts.measure_text_size(...).min()`, and `FixedFont::measure_text_size` returns
`Limits::new(Size::zero(), max_size)` (`font/fixed.rs:46`). So **a text leaf measures as
`Size::zero()`**, which makes `Shrink` labels collapse and flex sizing dishonest.
Separately, the render path wraps text into its bounds via `embedded_text::TextBox`
(`font/fixed.rs:90`) while layout never accounts for the extra height wrapping produces —
a latent vertical-clipping bug.

This design adopts the Flutter/SwiftUI/Xilem model adapted to the existing
`Limits`-down / `min_size`-up pipeline: text reports an intrinsic **width range** bottom-up,
the layout resolves the **width first**, then computes **height from that width**. There is
no inline/inline-block flow; line-breaking lives inside the text leaf, and the widget tree
deals only in boxes (horizontal/vertical arrangement stays the job of `Flex`).

## Non-goals

- No CSS-style inline layout, no bidi/shaping, no mixed-run rich text.
- No proportional-font support in this change (only a clean extension point for it — see
  EVOLUTION item *fontdue*).
- Vertical/RTL text. The inline axis is assumed to be width (X), block axis height (Y).

## Core principle (the asymmetry)

For a text leaf:

1. Resolve the **width** axis from `Length` + `Limits` + the intrinsic width range.
2. Compute **height = `height_for_width(resolved_width)`**.
3. Resolve the **height** axis from that height.

Non-text leaves (`Icon`, `Fixed`) report `min_content == max_content` and a
width-independent height, so they flow through the identical code path unchanged.

## §1 — Leaf measurement protocol (`font/`)

Replace `FontHandler::measure_text_size(content, props) -> Option<Limits>` with two queries
plus an overflow mode:

```rust
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum TextOverflow {
    #[default]
    Wrap,       // soft-wrap into available width; height grows
    Clip,       // single visual line per hard '\n'; clip horizontally at draw
    Ellipsis,   // like Clip, last visible run truncated with '…'
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextIntrinsics {
    pub min_content_width: u32, // widest unbreakable word (Wrap); 0 for Clip/Ellipsis
    pub max_content_width: u32, // longest hard-'\n' line, no soft wrap
    pub line_height: u32,       // height of a single line
}

pub trait FontHandler {
    fn measure_text(&self, content: &str, props: ResolvedFontProps)
        -> Option<TextIntrinsics>;

    fn text_height_for_width(
        &self, content: &str, props: ResolvedFontProps,
        width: u32, overflow: TextOverflow,
    ) -> u32;

    fn draw<W: WidgetCtx>(
        &self, content: &str, props: ResolvedFontProps,
        bounds: Rect, color: W::Color, overflow: TextOverflow,   // NEW arg
        renderer: &mut W::Renderer,
    ) -> Option<RenderResult>;
}
```

`measure_text` and `text_height_for_width` are added to `FontHandler` and implemented by
`FixedFont`, then delegated through `StoredFont` and `FixedFontCollection`. `FontCtx`
gains matching dispatch wrappers (mirroring today's `measure_text_size`).

### Monospace implementation (EGMonoFont / u8g2)

For fixed-advance fonts these are cheap arithmetic. With `char_w = character_size.width`,
`spacing = character_spacing`, advance `a = char_w + spacing`, `line_h = character_size.height`:

- `max_content_width` — as today (`font/fixed.rs:28-44`): per hard line, `n*char_w + (n-1)*spacing`, take the max.
- `min_content_width` — split each hard line on ASCII whitespace; widest token width by the same formula; take the max across lines. For `Clip`/`Ellipsis`, report `0` (can be squeezed arbitrarily).
- `line_height` — `character_size.height`.
- `text_height_for_width(width, overflow)`:
  - `Clip` / `Ellipsis`: `hard_line_count * line_h` (soft wrap ignored).
  - `Wrap`: `chars_per_line = ((width + spacing) / a).max(1)`; greedy word-fill (break at whitespace; a word longer than `chars_per_line` occupies its own line and overflows); total `lines * line_h`. Hard `'\n'` always starts a new line.

`text_height_for_width` lives on the trait so a future proportional font (fontdue) supplies
its own glyph-walking implementation without any layout-layer change.

## §2 — Content sizing (`layout/mod.rs`)

`ContentLayout::Text` gains a field:

```rust
ContentLayout::Text { font_props: FontProps, content: MaybeReactive<String>, overflow: TextOverflow }
```

`ContentLayout::text(content)` defaults `overflow: TextOverflow::default()` (`Wrap`).
Add `ContentLayout::text_with_overflow(content, overflow)` (or a builder setter).

Introduce:

```rust
pub struct ContentSizing { pub min_content_w: u32, pub max_content_w: u32, pub line_height: u32 }

impl ContentLayout {
    fn content_sizing(&self, ctx: &LayoutCtx) -> ContentSizing;        // bottom-up width range
    fn height_for_width(&self, ctx: &LayoutCtx, width: u32) -> u32;    // text wraps; icon/fixed ignore width
}
```

- `Text` → `content_sizing` calls `measure_text`; `height_for_width` calls `text_height_for_width`.
- `Icon`/`Fixed` → `min_content_w == max_content_w == size.width`, `line_height == size.height`;
  `height_for_width` returns the fixed height.

`min_size(ctx)` is **kept** but redefined for text to return `(min_content_w, line_height)`
— an honest lower bound used by flex (§4). It must never be `0` for `Wrap` text again.
`ContainerLayout`/`FlexLayout`/`ScrollableLayout::min_size` are unaffected (they aggregate
child `min_size`).

`measure_text` returns `Option`; on `None` (missing font / unmeasurable) the leaf falls back
to `ContentSizing` of zero and logs — **no new `unwrap`** (EVOLUTION item *unwraps*).

## §3 — Resolve + `model_layout` (`layout/limits.rs`, `layout/model.rs`)

New `Limits` method, used **only** by the `Content` arm:

```rust
pub fn resolve_content_size(
    &self,
    size: LengthSize,
    sizing: &ContentSizing,
    height_for_width: impl Fn(u32) -> u32,
) -> Size
```

Width axis resolution mirrors today's `resolve_axis` but uses the width range as the
content size:

- `Shrink` (and `InfiniteWindow(Shrink)`) → `clamp(sizing.max_content_w, limits.min.w, limits.max.w)`
- `Fixed(f)`          → `clamp(f, limits.min.w, limits.max.w)`
- `Pct(p)`            → `limits.max.w * p`
- `Div`/`Fill`/`InfiniteWindow(Div)` → `limits.max.w`

Then `let h = height_for_width(width);` and resolve the height axis exactly like today's
`resolve_axis(Axis::Y, size, h, limits)` (so `Fixed`/`Fill`/`Pct` heights still win; `Shrink`
height clamps `h` into limits).

The `Content` arm of `model_layout` (`layout/model.rs:267-289`) switches from
`resolve_size(size, min_content, None)` to `resolve_content_size(...)`. The existing
`resolve_size` is untouched and still serves `Edge`/`Container`/`Flex`/`Scrollable`.

### Why `Container` / `Scrollable` need no changes

They already recurse into their single child with derived `child_limits`, read back
`content_layout.outer_size()`, and resolve themselves. That child size now carries the
correct wrapped height automatically. Behavior follows Flutter's loose-constraint rule:

- `Shrink` container + `Fill` label → label fills the container's max width and wraps;
  container shrinks to the wrapped size.
- `Shrink` container + `Shrink` label → label takes `max_content_w` (unwrapped);
  container hugs it (no wrapping under loose constraints).

## §4 — Flex (`layout/flex.rs`) — two steps

`model_flex` reads `child.min_size(ctx)` in two places.

**Step 1 (ships with this change):**

- The lower-bound math at `flex.rs:110-114` / `192` now receives an honest
  `(min_content_w, line_height)` for text instead of `0`. Fixed/Shrink flex children and
  **all** children of a vertical flex (Column) are already correct, because their width is
  resolved before height: the second loop (`flex.rs:327-352`) re-runs `model_layout` with the
  assigned `Limits`, and the text leaf computes its real wrapped height there.
- Accepted limitation: a **fill-width** text child of a horizontal flex (Row) whose assigned
  width forces multiple wrapped lines may be vertically clipped by its line, because
  `model_line.cross` was pre-estimated single-line at `flex.rs:245-282`. This equals today's
  behavior for the fill case (which currently measures height as a single unwrapped line).

**Step 2 (defined follow-up):** after the second loop produces real child sizes, recompute
each line's cross extent as `max(real child outer cross)`, then re-derive `layout_size`
(`flex.rs:393`) and child cross positions/alignment. This closes the fill-text-in-a-Row
clipping. It is the one genuinely O(n)-extra-pass piece (the reason Flutter marks intrinsics
expensive), so it is isolated from step 1.

## §5 — `Label` API + render (`widget/label.rs`, `font/fixed.rs`)

- `Label` gains `pub fn overflow(self, TextOverflow) -> Self` and a `pub fn wrap(self) -> Self`
  convenience, threaded into `ContentLayout::Text { overflow, .. }`.
- The setter **must** mutate through `layout_mut()`, not a `layout()` copy, or the
  reactive-on-write upgrade is lost and reads panic (see EVOLUTION item *FontProps per-field*
  NOTE and `AGENTS.md`). It follows the same pattern as `.width()/.padding()/.font_size()`.
- `FixedFont::draw` takes the new `overflow` arg and honors it so render matches the measured
  height: `embedded_text::TextBox` already wraps for `Wrap`; `Clip`/`Ellipsis` configure its
  height/overflow mode. `Label::render` (`widget/label.rs:105-124`) passes the resolved
  overflow through.

## §6 — Tests

Run serially with `--test-threads=1`, `std` feature (per CLAUDE.md).

- `measure_text`: `min_content_width` = widest word; `max_content_width` = longest hard line;
  multi-line via `'\n'`.
- `text_height_for_width`: exact line counts for `Wrap` at several widths (incl. a word wider
  than the width → its own overflowing line); `Clip`/`Ellipsis` height == hard-line-count.
- `resolve_content_size`: `Shrink` label → unwrapped width + single-line height; `Fixed`-width
  label narrower than `max_content_w` → wrapped, taller height; `Fill` label in a narrow
  container wraps and grows.
- Flex: Column of labels stack with correct per-child wrapped heights (step 1); Row fill-text
  cross-size correct (step 2).

## §7 — EVOLUTION.md protocol pass

Reviewed the full TODO checklist. **No item conflicts with this design and none covers text
layout**, so nothing here redoes a checked item. Reporting (not changing) the interlocks —
no boxes are checked by this design:

- *"Fully get rid of embedded_graphics as a required dependency…"* and *"Move rendering to a
  separate crate…"*: this design routes all text measurement through the font-agnostic
  `FontHandler` trait rather than reaching into embedded-graphics from layout — consistent
  with the direction; does not complete it.
- *"Add support for `fontdue` crate"*: `text_height_for_width` on `FontHandler` is the exact
  extension point a proportional font needs. This design enables that item.
- *"Get rid of `unwrap`s, UI must never fail and should report errors via logs"*: the new
  `measure_text` returns `Option` and is handled with a logged zero-size fallback. **Flag for
  the user (not fixed here):** `FixedFont::measure_text_size`'s u8g2 arm currently
  `.unwrap()`s `get_rendered_dimensions` (`font/fixed.rs:59`) — a pre-existing violation that
  should be addressed when that arm is ported to `measure_text`.
- *"`FontProps` per-field reactivity… `LayoutWidget`/`layout_mut`"* (checked): the `Label`
  overflow setter reuses this established pattern.

## Resolved decisions

1. **Default overflow is `Wrap`** (approved). It is the "proper" behavior the work targets;
   `Clip`/`Ellipsis` are opt-in per label.
2. **Overflow is configurable per `Label`** (approved): `Wrap | Clip | Ellipsis`.
3. **`Shrink` resolves to `max_content_w`** clamped into limits (approved). When
   `limits.max.w < min_content_w` (screen narrower than the widest word), the width clamps to
   `limits.max.w` and the word overflows/clips per the overflow mode — matching Flutter; no
   mid-word break is forced.
4. **Flex multi-line cross-axis fix is a defined step-2 follow-up**, not in the first
   deliverable (approved).
