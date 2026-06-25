# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`rsact` is an early-stage, reactive Rust GUI framework for embedded systems. It is `no_std` by default and built around a fine-grained reactivity system (signals/memos/effects), hence the name.

## Commands

The reactive runtime is stored in thread-locals and the `single-thread` feature turns it into a single global. Either way **tests must run serially** (`--test-threads=1`), and on the host the `std` feature is required (the `single-thread` feature needs a `critical-section` impl that isn't linkable in host test builds).

```sh
# Reactive crate tests (host). NOTE: ~5 tests currently fail on clean master (WIP repo) — not your regression.
cargo test -p rsact-reactive --features std -- --test-threads=1

# UI / render tests
cargo test -p rsact-ui   --features std -- --test-threads=1
cargo test -p rsact-render --features "std,embedded-graphics,tiny-skia" -- --test-threads=1

# Run a single test
cargo test -p rsact-reactive --features std <test_name> -- --test-threads=1

# Reactivity benchmark (criterion)
cargo bench -p rsact-reactive --features std

# Check the whole feature matrix compiles (mutually-exclusive std/single-thread)
cargo hack check --feature-powerset --no-dev-deps --all \
  --mutually-exclusive-features std,single-thread --at-least-one-of std,single-thread

# Formatting (config in rustfmt.toml: 80 cols, edition 2024, crate-granularity imports)
cargo fmt
```

### Running examples

Examples live in `rsact-ui/examples/` and run against an `embedded-graphics-simulator` desktop window. Each declares its own `required-features` in `rsact-ui/Cargo.toml`; pass them explicitly:

```sh
# Most examples (sandbox is the scratch/playground example):
cargo run -p rsact-ui --example sandbox --features "std,simulator,embedded-graphics,tiny-icons"
# tiny-skia renderer example:
cargo run -p rsact-ui --example tiny_skia --features "std,simulator,tiny-skia"
```

Build examples with the `example` profile (`opt-level = 3`) — `dev` is `opt-level = 0` and the simulator is unusably slow. `.cargo/config.toml` sets `RUST_LOG=trace`; logs go through `env_logger` (call `env_logger::init()` in example `main`).

The canonical "all features" dev set used by the maintainer (see `.vscode/settings.json`) is: `single-thread, debug-info, simulator, tiny-skia` for rust-analyzer checks, with default features off.

## Architecture

### Workspace crates (dependency order)

- **`rsact-reactive`** — standalone fine-grained reactivity engine. No UI dependencies. `Signal`/`Memo`/`Effect`/`MemoChain`/`Trigger`, plus the `MaybeReactive`/`MaybeSignal`/`Inert` abstraction for "value that may or may not be reactive." Everything implements the `ReactiveValue` trait. The runtime is a `slotmap`-based dependency graph held in a thread-local (`runtime.rs`, `storage.rs`).
- **`rsact-render`** — renderer abstraction + geometry/primitives. Defines the `Renderer` trait and backend impls under `eg/` (embedded-graphics) and `tiny_skia/`. Geometry types (`Size`, `Point`, `Padding`, `BlockModel`, etc.) live here, not in the UI crate.
- **`rsact-tiny-icons`** — build-script-generated, pre-rendered bitmap icons for tiny sizes. Icons are generated per-size (5–24px) and per-set (`system`, `common`) via feature flags in `build/main.rs` from SVGs; `src/rendered/` is generated output.
- **`rsact-macros`** — proc macros. Currently just `#[derive(IntoMaybeReactive)]`.
- **`rsact-ui`** — the UI framework: widgets, layout, events, styling, the `UI` driver. Depends on all of the above.
- **`rsact-encoder`**, **`rsact-widgets`** — planned/stub crates (commented out of the workspace members).
- **`src/` (root `rsact` crate)** — thin facade re-exporting `rsact-ui`.

### Reactivity model (`rsact-reactive`)

The mental model: `Signal`s are mutable sources; `Memo`s are cached derived values that recompute lazily and only fire downstream if their value actually changed (change-detection is at the *consumer* end, not the source — see `EVOLUTION.md`); `Effect`s run side effects when dependencies change. `MaybeReactive<T>` lets an API accept either a constant (`Inert`) or a reactive source uniformly — widget builder methods take `impl IntoMaybeReactive<T>` so a property can be set once or wired to a signal. Prefer `signal.with(|s| s.field)` over `signal.get().field` to avoid cloning the whole value (see `AGENTS.md`).

How the engine actually works (`runtime.rs`, `storage.rs`):

- **Everything is a `Copy` handle into a thread-local runtime.** A `Signal`/`Memo`/`Effect` is just a `ValueId` key; the data lives in `Storage`, a `SlotMap<ValueId, Value>` where each `Value` is `{ value: Rc<RefCell<dyn Any>>, kind, state, height }`. All access goes through `with_current_runtime`. This shared per-thread graph is *why tests must be serial* (`--test-threads=1`).
- **The dependency graph is built by tracking at read time.** The runtime holds bidirectional `subscribers` / `sources` maps and a single current-`observer` cell. While a memo/effect runs (`Runtime::with_observer`), any `.get()`/`.with()` calls `ReadSignal::track` → `Runtime::subscribe`, recording the edge; the `_untracked` variants skip it. Dependencies are **dynamic**: `Runtime::cleanup` clears a node's edges before each re-run, so conditional reads are tracked correctly.
- **Propagation is three-state push-pull**: `ValueState` is ordered `Clean < Check < Dirty`. *Push* (on write): `notify` → `Runtime::mark_dirty` sets the source `Dirty` and marks all transitive subscribers (`get_deep_deps`) `Check`. *Pull* (lazy, on read or effect flush): `Runtime::maybe_update` — a `Check` node first re-checks its sources, recomputing only if one is actually `Dirty`. Nothing recomputes until read.
- **Change detection at the consumer.** In `Runtime::update` a `Signal` always reports `changed = true`; a `Memo` recomputes and returns `changed` via `PartialEq` (`MemoCallback::run`), and **only then** marks its own subscribers `Dirty`. So `set(x); set(x)` fires repeatedly but a memo over it cuts propagation dead once the value stops changing.
- **Effects are the eager leaves, flushed in topological order.** Effects are never memoized (`EffectCallback::run` always reports changed); they queue into `pending_effects` and `Runtime::run_effects` drains them sorted by `height` (= max source height + 1, maintained by `update_height`) and loops until stable — glitch-free, no stale intermediate reads.
- **Scheduling controls**: `batch(f)` / `defer_effects()` defer effect flushing until the outermost guard drops; `untrack(f)` reads without subscribing; `observe(key, f)` is a keyed re-runnable observer that runs `f` only when its tracked deps changed — this is what gates `rsact-ui` redraws.
- **Reactive-on-write**: an inert `Stored` value can be upgraded to a `Signal` in place, keeping its `ValueId` (`Runtime::make_reactive`), so every existing handle becomes reactive at once — the basis of `MaybeSignal`/`MaybeReactive`.
- **Ownership & disposal**: values are owned by the innermost `ScopeHandle` (and by the observer that created them); dropping a scope or re-running an observer disposes them. `ReactiveValue::dispose` is `unsafe` (use-after-free if a live edge still points at the node) — let scopes manage lifetimes. `Computed` is a memo without the `PartialEq` gate (always propagates); `Trigger` is a `Signal<()>` for pure invalidation; `MemoChain` adds `first`/`last` callbacks for per-widget style inheritance.

### UI pipeline (`rsact-ui`)

The widget tree is stored in a **tree arena** (`el/arena.rs`), not as nested boxed structs — `slotmap` keyed by `ElId`, with a `SecondaryMap` for parent→children. This replaced an earlier flat-arena design (recent git history: "Going tree arena").

- **`Widget<W>` trait** (`widget/mod.rs`) — every widget implements `build`/`layout`/`on_event`/`render`/`update`. `W` is a `WidgetCtx` type parameter (see below). `El<W>` is the type-erased `Box<dyn Widget<W>>` node + `ElState`. Builder traits `SizedWidget`/`BlockModelWidget`/`FontSettingWidget` add chainable `.width()`/`.padding()`/`.font_size()` etc.
- **`WidgetCtx` / `Wtf` type family** (`el/ctx.rs`) — `WidgetCtx` bundles the associated types `Renderer`, `Color`, `PageId`, `Stylist`, `CustomEvent` so they don't have to be threaded as separate generics everywhere. `Wtf<R, I, S, E>` is the concrete implementor. New widgets are generic over `W: WidgetCtx`.
- **`UI<W, P>` driver** (`ui.rs`) — owns pages, renderer, fonts, stylist, message queue. Driven by `ui.tick(events)` (process input + UI messages) then `ui.render(target)` in the app loop. Pages are registered with `.with_page(id, el)` and built lazily; only the active page's arena is kept built (navigating frees the old tree).
- **`Page<W>`** (`page/mod.rs`) — holds one arena and a reactive `Memo<LayoutModel>`. Layout is recomputed reactively when fonts/viewport change (`model_layout`). Rendering is gated through reactive `observe`/`force_redraw` so only dirty regions redraw.
- **Layout** (`layout/`) — flex/grid/container model with `Length` (fill/shrink/fixed), `Limits`, `Align`. Layout properties on widgets are `MaybeReactive` via `layout.setter(...)`.
- **Events** (`event/`) — `Event<CustomEvent>` flows through the focused/hovered element. `PageState` tracks `focused` and `PointerState` (`pos`/`captured_by`/`hovered`). `event/simulator.rs` adapts simulator input; encoder/button input is a first-class control model.
- **Styling** (`style/`) — `Theme`, `Stylist`, and per-widget reactive style functions via the `declare_widget_style!` macro and `MemoChain` style inheritance.

## Project conventions

From `AGENTS.md` (read it before non-trivial changes):

- **Never delete `Note:` or `TODO:` comments** unless the referenced work is 100% done. The codebase has many large commented-out blocks and TODO/Note markers that are intentional design notes — leave them.
- **Factor out repetitive logic into a shared wrapper rather than copy-pasting it into every widget.** If a change would add the same few lines to every `Widget` impl, push it into the shared method (e.g. into `render_child`'s parameters) instead.
- Prefer `.with(|v| v.field)` over `.get().field` for non-trivial reactive values.

`no_std` discipline: the workspace is `no_std` by default (`#![cfg_attr(not(feature = "std"), no_std)]`); use `alloc` (`extern crate alloc`), not `std`. `std` is a feature, mutually exclusive with `single-thread`. `rsact-ui` sets `#![deny(unused_must_use)]`.

## LLM-driven evolution workflow

`EVOLUTION.md` is the maintainer's living design doc and contains a **TODO checklist with a protocol**: when completing a TODO, mark it `[x]`; never redo a checked item; on each pass review all items for conflicts with current changes and report (don't silently fix) anything that looks incomplete or needs investigation. Items marked **`WIP`** are not ready — skip them. The broad direction recorded there: decouple from `embedded-graphics` as a hard dependency (it should be one render *target* among several via `rsact-render`), eliminate `unwrap`s (the UI must log errors rather than panic), and optimize memo/signal change propagation.
