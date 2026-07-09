# Architecture

rsact is a workspace of focused crates, in dependency order:

- **`rsact-reactive`** — the standalone fine-grained reactivity engine
  (`Signal` / `Memo` / `Effect` / `MemoChain` / `Trigger`), no UI dependencies.
- **`rsact-render`** — the `Renderer` trait, geometry primitives, and backend
  impls (embedded-graphics, tiny-skia). The one reactivity-free layer.
- **`rsact-tiny-icons`** — build-script-generated bitmap icons for tiny sizes.
- **`rsact-macros`** — proc macros (`#[derive(...)]` helpers).
- **`rsact-ui`** — widgets, layout, events, styling, and the `UI` driver.

## Reactivity model

Signals are mutable sources; memos are cached derived values that recompute
lazily and only propagate when their value actually changes (change detection
at the consumer); effects run side effects when dependencies change. The runtime
is a `slotmap`-based dependency graph in a thread-local — every `Signal`/`Memo`/
`Effect` is a `Copy` handle (a `ValueId`) into it.

Propagation is three-state push-pull (`Clean < Check < Dirty`): a write marks
subscribers `Check`; reads pull lazily, recomputing only when a source is truly
`Dirty`. Effects are the eager leaves, flushed in topological order.

`MaybeReactive<T>` lets an API accept either a constant or a reactive source
uniformly, so a widget property can be set once or wired to a signal.

## UI pipeline

The widget tree lives in a `slotmap` **tree arena** keyed by `ElId`. Every
widget implements `Widget<W>` (`build`/`layout`/`on_event`/`render`/`update`),
where `W: WidgetCtx` bundles the renderer, color, page-id, stylist and
custom-event types. The `UI` driver owns pages, renderer, fonts, stylist and a
message queue; you drive it with `ui.tick(events)` then `ui.render(target)`.
Layout is a flex/grid/container model recomputed reactively; rendering is gated
through reactive observers so only dirty regions redraw.

For the full, authoritative treatment see `CLAUDE.md` and `EVOLUTION.md` in the
repository.
