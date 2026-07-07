# rsact Evolution Roadmap

**Target class (decided 2026-07-06):** the floor is the **Blue Pill** (STM32F103, Cortex-M3/thumbv7m: 64–128 K flash, 20 K RAM); the comfortable tier is the **Black Pill** (STM32F401CE, Cortex-M4F/thumbv7em+FPU: 512 K flash, 96 K RAM). Cortex-M0/thumbv6m is a dream tier, kept **compile-only** in CI (portable-atomic makes that ~free) with no size budgets asserted. Note the relaxation changes little in substance: Blue Pill's 64 K flash is still below today's 70–90 K framework estimate (WS4/WS9 stay load-bearing), and Black Pill's 96 K RAM still cannot hold a full 240×240 RGB565 framebuffer (WS6 strip mode stays existential for color).

> **For agentic workers:** each workstream (WS) below is designed to run as its **own Claude Code session** (some as 2–3 sessions — noted per WS). At the start of a WS session: (1) read this file's WS section + the "Cross-cutting invariants" and "Baselines" sections; (2) **verify the current state first** — earlier sessions may have shifted the ground (re-run the baseline test/bench commands, re-read the cited code); (3) use the superpowers:writing-plans skill to expand the WS charter into a bite-sized TDD plan before touching code; (4) when a work item lands, mark its checkbox here `[x]` and record the commit hash; never redo a checked item; (5) follow the EVOLUTION.md TODO protocol (report conflicts, don't silently fix); (6) `> comment:` blocks are the maintainer's live review notes — **never delete or resolve them yourself**; an item carrying an unresolved comment is still under discussion and is **not ready to execute** — skip it and note that in your report. Items conflicting with reality get reported back, not forced.

**Goal:** evolve rsact into the lightest credible reactive GUI framework for embedded — beating LVGL on RAM/Flash for its target class, without feature-flag sprawl and without a v2.0 that eats 10× more memory.

**Philosophy (maintainer's):** polish from the deep first, moving up the abstraction stack. Core changes may force full API reimplementation, so API-surface work is _decided early on paper_ but _executed late_, batched into at most two breaking releases. Exterior polish (docs, examples, naming) is scheduled last.

**Method:** seven parallel adversarial deep-analyses (2026-07-05) over the working tree — reactive core (D1), UI core design (D2), incremental layout (D3), embedded-dev ergonomics (D4), reactive rendering (D5), RAM/Flash footprint (D6), minimal-mode architecture (D7) — then cross-direction reconciliation. Several findings were **empirically confirmed** with scratch harnesses and real thumbv6m builds; measured numbers are recorded in "Baselines" below. Prior context: the 86-finding audit (report artifact `ed1e601a`; 27 done / 6 partial as of this date), phases 1–4 + storage-soundness already landed.

---

## The one-page picture

### What is already strong (measured, defend it)

- **The reactive core is genuinely small on target:** 16.8 KiB `.text` on a real thumbv6m build, ~56 B of statics, **zero steady-state allocations** after warm-up.
- **Idle frames are already free:** 16 ns / 0 allocs per no-change frame (host); the page-level observe gate works. "Reactivity overhead" is _not_ an idle-CPU problem.
- **The architecture is right for embedded:** consumer-side change detection + per-part render observers + `render() -> bool` is what people hand-build on LVGL. Monochrome-first theming, encoder-first events, packed 1-bit framebuffer, MIT license, mermaid graph export — these are real differentiators.

### The six structural problems (what the whole plan is organized around)

| #   | Problem                                                                                                                                                                                                                                                                                                                                                         | Evidence owner |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| P1  | **The framework can't build for its own target.** std-only f32 math in eg primitives + an ARMv6-M-illegal `fetch_add` block every thumb build; all size numbers are host proxies.                                                                                                                                                                               | D4, D6         |
| P2  | **Nothing is ever disposed.** No scopes anywhere in rsact-ui; page navigation leaks every page-owned node and leaves live effects aimed at a disposed arena (delayed panic); observers never run cleanup (stale deps re-trigger redraws forever, owned values leak per re-run); nested scope drop corrupts `current_scope` in the core.                         | D1, D2, D5     |
| P3  | **Statics pay reactive freight.** Every `Inert` prop, every static `Layout`, every builder literal = a full runtime node (~150–250 B each on 32-bit); 31 of 46 nodes in a measured 10-widget page are constants. ~1.7 KB heap per static label.                                                                                                                 | D7, D6         |
| P4  | **Change-frames burn bookkeeping, and identity is fragile.** Per executed frame each widget part pays `format!` + string-hash + BTreeMap + an O(N²) subscribe scan (~4–8 ms per change frame @100 parts on M0); observer identity is a bare u64 hash — and cross-page aliasing is _deterministic_ (fresh arenas mint identical ElId sequences).                 | D5             |
| P5  | **One change relayouts and repaints everything.** Whole-page `Memo<LayoutModel>`; `min_size` recursion makes it O(N·D); no text-measure caching; `force_redraw.set(true)` _inside_ the memo forces every widget to repaint even when the tree is identical; and the flush then streams the **entire viewport per-pixel** to the display.                        | D3, D4         |
| P6  | **Flash diet needed, in known places.** BTree machinery + quicksort ≈ 5–7 KiB; widget-local type params (`Dir`, `V`, …) ≈ 2.4 KiB per instantiation; per-`T` reactive shims; icons feature retains 46 KiB for one icon; forced `defmt`. The viral `W: WidgetCtx` is an API/coupling problem, **not** a measured flash problem (exactly one `Wtf` per firmware). | D6, D2         |

### Workstream map (deep → surface)

```
            ┌─────────────────────────────────────────────────────────────┐
 LAYER 0    │ WS0 buildable+measurable   WS1 core correctness quick wins  │  ← + WS1b UI quick fixes:
            │                            WS1b UI-side quick wins (sweep)  │     start all three, parallel
            └───────────────┬──────────────────────┬──────────────────────┘
                            │                      │
 LAYER 1    ┌───────────────▼──────────┐  ┌────────▼─────────────────────┐
 (engine)   │ WS2 probe/render         │  │ WS9a engine diet: collections │
            │     identity redesign    │  │      (independent, low risk)  │
            └───────┬───────────┬──────┘  └───────────────────────────────┘
                    │           │
 LAYER 2    ┌───────▼─────┐ ┌───▼──────────────────┐
 (lifecycle │ WS3 scopes/ │ │ WS4 zero-cost statics │   [gate G1: Copy-ness]
  & memory) │ page dispose│ │     + singleton degreed│
            └───────┬─────┘ └───┬──────────────────┘
                    │           │
 LAYER 3    ┌───────▼───────────▼──────┐   ┌──────────────────────────────┐
 (pipeline) │ WS5 layout off-graph +   │──▶│ WS6 damage-driven rendering  │
            │     incremental          │   │     + partial flush + fb modes│
            └──────────────────────────┘   └──────────────────────────────┘
                    │
 LAYER 4    ┌───────▼──────────────────────────────┐  ┌───────────────────┐
 (surface)  │ WSi internals polish →               │  │ WS8 primitives     │
            │ WS7 API collapse (events, WidgetCtx, │  │ WS10 platform layer│
            │     Stylist, Widget trait) — BREAKING│  └───────────────────┘
            └──────────────────────────────────────┘
                    │
 LAYER 5    ┌───────▼──────────────────────────────┐
 (exterior) │ WS11 polish: examples, docs, naming  │   ← last of the core plan
            └───────┬──────────────────────────────┘
                    │
 LAYER 6    ┌───────▼───────────────────────────────────────────────────────┐
 (expansion)│ WS12 release eng. → WS13 view-builders ∥ WS14 devtools v2     │
            │ ∥ WS15 fonts ∥ WS16 desktop tier ∥ WS17 hardware validation   │
            │ ∥ WS18 no-alloc storage      (WS17 can start after WS6)       │
            └───────────────────────────────────────────────────────────────┘
```

**Suggested execution order:** WS0 ∥ WS1 ∥ WS1b → WS2 → (WS3 ∥ WS4 ∥ WS9a) → WS5 → WS6 → WSi → WS7 → (WS8 ∥ WS10 ∥ WS9b) → WS11 → WS12 → (WS13 ∥ WS14 ∥ WS15 ∥ WS16 ∥ WS18). WS17 may start any time after WS6 — ideally before WS11.7 needs its README numbers.
WS7's _decisions_ are locked at Gate time (now); only its _execution_ is late. 7.2's cheap `Dir`/`V` de-genericization may ride along with WS2/WS4 if convenient — it's zero-user-impact. (7.1 no longer qualifies: G5 keeps `Event::Custom`, and its remaining scope carries a breaking rename plus a G4-dependent default.)

---

## Decision gates (answer these; each gates the marked WS)

Recommendations reflect the agents' converged analysis; where two directions disagreed, the resolution is noted.

- [x] **G1 (gates WS4) — DECIDED IN DIRECTION (2026-07-06): yes, drop blanket `Copy` on `MaybeReactive<T>`/`Inert<T>`** (`Copy` only for `T: Copy`, `Clone` otherwise — the `MaybeSignal::Inert` precedent). Execution is gated on the **WS4.0 analysis** (maintainer-authored): `Layout` cannot be inlined freely (needs shared identity; `Widget::layout` may need to return `&Layout`; `Copy` removal from `Layout` for safety) — all `MaybeReactive` usages rethought with Copy-loss in mind before 4.1 lands.
- [x] **G2 (gates WS2) — DECIDED (2026-07-06): no** — children re-run iff their own deps changed or the caller passes `force` (what today's working render path does via `parent_dirty`). The failing test `observe_recreates_disposed_child_observer` encodes the _old_ cleanup-dispose-revive contract and gets **rewritten**, not satisfied. (D1's "parent forces children" rejected: it's exactly what the explicit `force` param already expresses.)
- [x] **G3 (gates WS6) — DECIDED (2026-07-07):** measurement stays **display-agnostic**; the reference displays for the floor pair are **128×64 mono OLED (SSD1306-class) on Blue Pill** and **240×240 16-bit ST7789 on Black Pill**. Consequence: 240×240 RGB565 = 112.5 KiB > Black Pill's 96 K RAM ⇒ **WS6.4 strip/partial modes are existential** for the color reference; and ST7789 has its own GRAM with window addressing, so the WS6.3 regions API maps directly onto partial window writes.
- [ ] **G4 (gates WS7) — elaboration delivered 2026-07-07, awaiting maintainer sign-off.** With G5 keeping custom events, the collapse target becomes **5 → 2 real degrees of freedom** (`Renderer` + `CustomEvent`): `WidgetCtx` keeps exactly two associated types (+ `Color` kept as an assoc type set from `Renderer::Color` so `W::Color` keeps compiling); a blanket `impl<R: Renderer> WidgetCtx for R { type Event = (); … }` makes `type W = MyRenderer` work with zero ceremony for the no-custom-events case; apps with custom events write one 3-line ctx impl. `PageId` demotes to the driver; `Stylist` dyn-erases behind `get_style`. Existing `impl<W: WidgetCtx> Widget<W>` blocks compile **unmodified**. (Full before/after in the review chat; fold into WS7.5 on approval.)
- [x] **G5 (gates WS7) — DECIDED (2026-07-07): KEEP `Event::Custom` + `CustomEvent`** — reverses the audit recommendation. Rationale (maintainer): unused today, but required flexibility for users implementing their own widgets that handle app-defined events — user widgets are concrete over their own ctx, so they _can_ match `Event::Custom(MyEvent::…)`. Consequences: WS7.1 rewritten (no deletion; renames + `Key` reservation + zero-ceremony `E = ()` default), and G4's collapse keeps `CustomEvent` as a real degree of freedom.
- [x] **G6 (gates WS2) — DECIDED (2026-07-06): delete the registry from the render path**; the polled primitive becomes the owned **`Probe`** handle (see WS2). Public keyed `observe()` is deprecated (kept at most as a thin compat wrapper during migration; final removal call at WS2 execution). `ahash` leaves the no_std build with it.
- [ ] **G7 (gates WS10) — REFRAMED (2026-07-07) as an investigation; mailbox REJECTED** (maintainer: reactive ops must be small and concise; no extra primitives). Direction to investigate, all platform-agnostic: **(a) CS-narrowing** — user closures (probe polls, memo/effect callbacks — the long-running parts) run _outside_ the critical section; only individual storage operations are guarded (this un-parks the register item); **(b) deferred-effect writes** for ISR contexts — a write variant with existing `defer_effects` semantics (value write + `mark_dirty` under a short CS, effects queued, flushed at next `tick`) so an ISR never runs the effect cascade. Known hazards the investigation must test: mid-pull dirty marks consumed by `mark_clean` (lost update), re-entrant `mark_dirty` during an unguarded closure, effect flush never in ISR context. Full plan in WS10.1.
- [x] **G8 (gates WS5) — DECIDED (2026-07-07): yes** — whole-page relayout is the current semantic, and **incremental layout modeling is confirmed as the destination** (WS5 stages 2–3 are wanted, not optional).
- [x] **G9 (gates WS7) — DECIDED (2026-07-07): styler registry with a measurement tripwire.** TypeId-keyed registry (sorted `Vec<(TypeId, Box<dyn Any>)>`, binary search — no hashing) with `S::base()` fallback; third-party widgets define their style type + `base()`, themes register per-style-type styler closures, per-instance `.style()` closures stay on top. Costs quantified and accepted: ~30–60-cycle lookup + the dyn call `get_style` already pays today, only inside actually-redrawing parts; ~24 B/entry ≈ ~300 B/theme. Guardrail: a `style_resolution` micro-bench joins the 0.3 metrics snapshot from day one; **pre-approved response** if it ever misbehaves = hoist resolution to build time (cache the resolved styler per widget instance, 8 B each, re-resolve on theme swap). Hybrid (static built-ins + registry only for third-party) considered and rejected: two mechanisms. Kills `InternalStylist`'s closed set and the per-style trait-impl coherence pain.
- [ ] **G10 (gates WS10) — POSTPONED (2026-07-07)** by the maintainer; revisit before WS10.2 starts. Candidates on record: tree-order traversal (audit recommendation) vs the legacy absolute-index model vs the `event/select.rs` chain stub.
- [x] **G11 (gates WS3) — DECIDED (2026-07-07): yes** — page-created = page-owned; the per-page scope disposes everything the `PageInitFn` created; signals meant to outlive a page are created outside it; `persist()` escape hatch only if a real case appears.
- [x] **G12 (informs WS0/WS6) — DECIDED (2026-07-07): two metric layers.** **Layer 1 — platform-agnostic framework metrics** (node counts by kind, allocs/op and /frame, bytes/value, layout counters — host-measured, stable across platforms; the primary regression surface). **Layer 2 — "do we still fit" target tracking** (`.text/.rodata/.bss` budgets on thumbv7m Blue Pill + thumbv7em-hf Black Pill; thumbv6m compile-only). Both layers come from the same 0.3 snapshot tool. _Small remainder still open: release logging policy (`log max_level_off` vs defmt)._

---

## Workstreams

### WS0 — Make it buildable and measurable

**Sessions:** 1–2 · **Risk:** low · **Directions:** D4, D6, D7 · **Depends on:** nothing. **Do first — every other WS's acceptance criteria depend on it.**

Why: rsact-ui cannot link for any thumb target (P1), so nothing about the embedded goal is currently falsifiable. The measurement harnesses built during the audit (scratchpad probe crates, greed-audit) exist and are ready to be adapted in-repo.

Work items:

- [x] **0.1 no_std f32 math via `FloatExt` re-export (decided 2026-07-06)** — landed `795d6ba`: rsact-render gets two mutually-exclusive math features — **`libm` (default)** → `pub use num_traits::Float as FloatExt;` (forward `num-traits/libm`) and **`micromath` (opt-in)** → `pub use micromath::F32Ext as FloatExt;` (faster approximations at accuracy cost — the _user_ decides; micromath 2.1's `F32Ext`, incl. `sin_cos`, is drop-in std-compatible — maintainer-verified). `compile_error!` when both/neither (same pattern as the storage backends). Primitives (`eg/primitives/sector.rs`, `arc.rs`, `line.rs`, siblings) just `use crate::FloatExt as _;` — on std builds inherent `f32` methods shadow the trait automatically, so the simulator uses std math with zero cfg. Acceptance: `cargo build -p rsact-render --no-default-features --features embedded-graphics,libm --target thumbv7m-none-eabi` succeeds (+ same with `micromath`; + thumbv6m compile-check).
- [x] **0.2 portable-atomic, plain (decided 2026-07-06)** — landed `a30f4f5`: replace `FONT_UNIQUE_ID`'s `core::sync::atomic` use (`rsact-ui/src/font/mod.rs:289`) with `portable-atomic` types, **no rsact feature wiring**: on thumbv7m+ it compiles to native instructions; on thumbv6m the _end product_ selects the fallback itself — via feature unification on its own `portable-atomic` dep (`features = ["critical-section"]`) or `--cfg portable_atomic_unsafe_assume_single_core` in RUSTFLAGS. Document both in the README's thumbv6m note. Acceptance: rsact-ui builds for thumbv7m with `unsafe-single-thread,embedded-graphics`; thumbv6m compile-check documented.
- [x] **0.3 metrics contract, local-first (decided 2026-07-06)** — landed `81dc475` (Profile now counts Observer/Probe nodes) + `257f587` (metrics-probe snapshot/diff/viewer tool): one command (`cargo run -p metrics-probe -- record` / `-- diff <rev|file>`) emitting a per-commit JSON snapshot — node counts by kind, live heap bytes, allocs/frame (idle + change), layout counters (0.5), `.text/.rodata/.bss` per target — stored locally, keyed by `git rev-parse HEAD`, plus a static HTML viewer. **Layer 1 (host framework metrics) is fully landed** — node counts by kind, live+peak heap, build allocs, idle/change-frame allocs (idle = 0 for all scenarios), layout counters; JSON keyed by rev (`-dirty` suffix for dirty trees), self-contained inlined HTML viewer; snapshots/index.html git-ignored (metrics/README.md documents the store). **Layer-2 target section sizes also landed** (follow-up commit): the excluded `size-probe` crate (`cortex-m-rt` + `embedded-alloc` + a generic `memory.x`; linked-but-never-run) with `reactive` + `ui` binaries builds at opt-z/fat-LTO for the floor targets; `metrics-probe -- record --sizes` builds them and reads `.text/.rodata/.bss` via the `object` crate into the snapshot (measured: reactive .text ≈ 27.4 KiB thumbv7m / 27.5 KiB thumbv6m; ui .text ≈ 79/84 KiB). `.bss` is dominated by the probe's fixed heap buffer + cortex-m-rt statics — `.text/.rodata` are the flash signal. thumbv6m links soundly via the portable-atomic critical-section fallback (no unsafe cfg). **Remaining:** (a) thumbv7em-hf (Black Pill) target row + real budget thresholds (WS10-adjacent); (b) the **CI half** — CI runs this same binary, archives snapshots, posts PR delta comments.
- [x] **0.4 node-count regression test** — landed `5f95e8d`: canonical static page asserts node counts + idle-frame allocs through the 0.3 snapshot (42 nodes / 10 labels, 0 idle allocs — locked).
- [x] **0.5 layout counters** — landed `1f27a83`: `#[cfg(feature = "layout-counters")]` visit/measure counters (a `layout::counters` module; `count_visit` at each `model_layout` entry — captures flex's multi-pass child re-visits since `model_flex` re-enters `model_layout`; `count_measure` in `ContentLayout::content_sizing`/`height_for_width`). metrics-probe grows a `layout-counters` feature and attributes the change-frame layout work into the snapshot. Baseline-locking test `layout_counter_baseline` (gated) + criterion benches `layout_full`/`layout_leaf_change`. NOTE: baseline locked on the canonical 5/10-label scenarios (ui_labels_10 = 11 visits / 40 measures — visits == node count ⇒ whole-tree relayout per single change), not the roadmap's cited 30-node page; the pathology (visits ≈ node count) is the same signal.
- [x] **0.6 workspace feature audit (expanded 2026-07-06)** — landed `b956c57` (core leaks + matrix + hack commands; see the three final-sweep additions below, still `[ ]`): a systematic pass, not just the found leaks — every optional dep behind `dep?/feature` syntax; per-crate defaults minimal; features propagate top-down through the crate tree (root → ui → render → reactive) with nothing extra enabled by default; feature-matrix doc table; `cargo hack --feature-powerset` green; `cargo tree` of the minimal profile shows zero unexpected entries. Known targets found by the audit: `std → tiny-skia/png-format` leak (missing `?`), forced workspace `embedded-graphics/defmt`, unused workspace `smallvec` (`micromath` is no longer unused — 0.1 consumes it as the opt-in math backend). **Final-sweep additions (2026-07-07):** [x] remove the unused out-of-repo `paw` dev-dependency — landed `1a2de5f` (`rsact-ui/Cargo.toml:41`; nothing referenced it; gone from Cargo.lock). [ ] gate `anim` behind a real feature (the invariants list it as a sanctioned axis but it doesn't exist — `pub mod anim` is unconditional and pulls the non-optional `num` dep, a second float-math path parallel to 0.1's `FloatExt`; unify) — **deferred**: needs the `num`→`FloatExt` unification + auditing widgets that use `anim` unconditionally, more than a flag flip. [ ] root `rsact` facade feature passthrough (it forwards no render-backend/tiny-icons/u8g2/debug-info features — a facade user cannot build a working app; cross-ref 12.5) — **deferred** to WS12.5.

- [x] **0.7 Review fixes — code-review of the WS0 branch (2026-07-07, range `f298b98..fb64525`, all findings verified/reproduced).** All 11 sub-items landed (`94a65c5..612ef1d`); baselines preserved (reactive 54/2, ui-lib 44/0, render 6/0, metrics-probe 3/0 now parallel-safe, thumbv7m green). Each sub-item: file:line · failure · fix shape. Fix = repro/failing test first where applicable.
  - [x] **0.7a micromath unreachable through rsact-ui** — done `94a65c5` (`rsact-ui/Cargo.toml:28`): rsact-ui pulls rsact-render with default features (libm), so enabling `rsact-render/micromath` anywhere in a graph containing rsact-ui trips the mutual-exclusion `compile_error!` (reproduced). Fix: `rsact-render = { workspace = true, default-features = false }` in rsact-ui + passthrough features `libm = ["rsact-render/libm"]` (in rsact-ui defaults) and `micromath = ["rsact-render/micromath"]`; forward both from the root facade. Acceptance: `cargo check -p rsact-ui --no-default-features --features "std,embedded-graphics,micromath"` builds.
  - [x] **0.7b defmt forwarding stops before rsact-render** — done `6c74998` (`rsact-ui/Cargo.toml:59`): add `"rsact-render/defmt"` to rsact-ui's `defmt` feature so the `Format` derives on `Size`/`Axis`/geometry activate. One token.
  - [x] **0.7c std builds shouldn't require a math backend** — done `0ae920f` (`rsact-render/src/lib.rs:12`): `--no-default-features --features std` fails the backend `compile_error!` although std's inherent f32 methods shadow the trait and the backend is never called (reproduced). Exempt std: `#[cfg(not(any(feature = "std", feature = "libm", feature = "micromath")))]`.
  - [x] **0.7d metrics-probe tests are parallel-flaky by construction** — done `d508c9b` (`metrics-probe/src/scenarios.rs:250,296`): both tests share the process-global tracking allocator + layout counters; plain `cargo test -p metrics-probe --features layout-counters` (libtest default = parallel, and what `cargo test --workspace` runs) → flaky asserts. Fix: shared `static TEST_LOCK: Mutex<()>` or merge into one `#[test]` — the doc comment alone enforces nothing.
  - [x] **0.7e panicked frames record fake layout counts** — done `bf61365` (`metrics-probe/src/scenarios.rs:191-210`): `read_layout()` runs unconditionally after `reset_layout()` even when the guarded paint/change frame panicked → `Some {visits: 0, measures: 0}` instead of `None`; a later `diff` shows a phantom −100% "improvement". Gate the layout read on the frame completing, like the alloc metrics already are.
  - [x] **0.7f snapshot schema erases history on additive change** — done `2223a6b` (`metrics-probe/src/snapshot.rs:13`, `html.rs:14-18`): no `#[serde(default)]` on additive fields + silent `if let Ok` skip in html regeneration — the next added field (exactly what 0.3a's `observers` did) makes older snapshots undeserializable and silently dropped; `diff <old-rev>` aborts. Add `#[serde(default)]` to additive fields; log skipped files. Acceptance: re-record post-fix and `diff` against a pre-fix snapshot works.
  - [x] **0.7g `diff <rev>` resolves only literal full-hash filenames** — done `e7bbf86` (`metrics-probe/src/main.rs:86-103`): `diff HEAD~1` / short revs / branches fail although the snapshot exists. Add a `git rev-parse --verify` fallback.
  - [x] **0.7h `layout_full` bench window includes runtime+build+first paint** — done `220639a` (`rsact-ui/benches/layout.rs:40-52`): dilutes the WS5 speedup this bench exists to demonstrate; contradicts the sibling bench's documented iter_custom discipline. Restructure like `layout_leaf_change` (setup outside the timed window) or rename to `build_and_layout_full` and add a true `layout_only` bench.
  - [x] **0.7i stale NOTE contradicts landed 0.1** — done `9b3b6c5` (`rsact-ui/Cargo.toml:71-74`): still claims "no_std rsact-ui blocked by eg's std-only f32 math" — this branch removed that blocker (verified green). Reword to the current contract (math backend via features).
  - [x] **0.7j de-duplicate the measurement primitives** — done `40e4c51` (they WILL drift and make bench vs snapshot numbers incomparable): churn-counting allocator (`metrics-probe/src/alloc.rs:29-56` ≈ `rsact-reactive/benches/allocations.rs:44-64`, same policy comments) and the canonical n-labels scenario (`rsact-ui/benches/layout.rs:25-39` ≈ `metrics-probe/src/scenarios.rs:135-160`) each live twice. Share from one home (allocator: metrics-probe grows a lib target the bench dev-deps on; scenario: `#[doc(hidden)]` test-support module in rsact-ui both consume). Also add `Profile::total()` in rsact-reactive so the node-sum formula isn't duplicated (`scenarios.rs:56-71` vs `Display for Profile`).
  - [x] **0.7k tool nits (one commit):** — done `612ef1d` tracking allocator commits counters before the underlying allocation can fail (`alloc.rs:29-51` — a failed grow-realloc bakes drift into LIVE forever; count after non-null return); unchecked `usize` heap-delta subtraction (`scenarios.rs:107-108,174-175` — dev-profile overflow panic aborts recording; saturate like `alloc.rs:42` already does); document in `docs/features.md` that metrics-probe/size-probe in the workspace break `cargo check --workspace --target thumb*` (std-only tools) — record the `--exclude` escape.

- [ ] **0.8 Commit-time metrics automation (maintainer-proposed 2026-07-07; design agreed):** a **post-commit** hook — NOT pre-commit: at pre-commit time HEAD is still the parent and the tree is dirty, so the snapshot would be keyed to the wrong hash (`-dirty`); post-commit sees the new hash and a clean tree, matching the tool's own keying. Behavior: runs `metrics-probe record` **Layer-1 only** (no `--sizes` — opt-z thumb builds are too slow for commit cadence; sizes stay on-demand + CI), **in the background** (commit returns instantly; output → `metrics/hook.log`), **never blocks or fails the commit** (metrics observe, they don't gate — the 0.4 regression test is the hard gate; CI PR deltas are the review surface), **skips** during rebase/cherry-pick and when HEAD's snapshot already exists. Distribution: committed `.githooks/post-commit` + one-time `git config core.hooksPath .githooks` documented in README + `metrics/README.md` (optionally a `metrics-probe hook-install` subcommand that sets the config). Caveat on record: snapshots are git-ignored, so the hook completes the *local* timeline only — the durable shared record remains 0.3's CI half; the pair together is the full answer. Acceptance: two consecutive commits → two keyed snapshots, zero perceptible commit latency; a broken-build commit still commits (hook logs and exits 0).

**Session prompt (0.7 + 0.8 — review fixes & hook):**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS0 items 0.7 (each sub-item
carries file:line, the failure, and the fix shape) and 0.8 (post-commit hook; design is
fully specified in the item). Verify each 0.7 finding still reproduces first: 0.7a/0.7c
have exact repro commands; 0.7d = run metrics-probe tests WITHOUT --test-threads=1 and
watch them interfere. Fix 0.7a–0.7k, one commit per letter (0.7k may be one commit),
then implement 0.8 (its own commit; verify: two consecutive commits produce two keyed
snapshots, and a deliberately broken-build commit still commits with a logged warning).
Baselines to preserve: reactive 54/2, ui-lib 44/0 (--lib --features
"std,embedded-graphics"), render 6/0, metrics-probe 2/0 serial; thumbv7m rsact-ui build
stays green. After 0.7f, re-record a snapshot and confirm `diff` against a pre-fix
snapshot works. Mark sub-items done here with commit hashes. Do not start WS1/WS1b work
in this session.
```

**Session prompt (original WS0 — 0.1–0.6 landed, kept for reference):**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — section WS0 (all decisions are
recorded inline), plus Baselines and Cross-cutting invariants. Verify current state first
(the cited breakages may have moved). Execute WS0: make rsact-ui build for thumbv7m
(Blue Pill floor; thumbv6m compile-only, no budgets) via the FloatExt re-export (0.1) and
portable-atomic (0.2), then land the local-first metrics contract (0.3 snapshot/diff tool
with Probe-counting profile, 0.4 node-count regression, 0.5 layout counters) and the
workspace feature audit (0.6). Use TDD where a test can exist; mark items done in the
roadmap file with commit hashes. Do not fix unrelated findings — note them.
```

---

### WS1 — Reactive-core correctness quick wins

**Sessions:** 1 · **Risk:** low · **Directions:** D1, D5 · **Depends on:** nothing (parallel with WS0).

Why: five empirically-confirmed bugs with small, contained fixes; they also de-risk everything later (WS2/WS3 build on scopes and observer state).

Work items (each = failing test → fix → pass; all decisions folded 2026-07-06):

- [ ] **1.1 Scope parent chain** (`runtime.rs:373,732-746`, `scope.rs`): `new_scope` overwrites `current_scope`, `drop_scope` never restores → values created after an inner scope drops are owned by nothing and leak forever. **Decided:** `parent: Option<ScopeId>` in `ScopeData` — the parent pointer _is_ the stack (intrusive, no `Vec`); `drop_scope` restores `current_scope` to the dropped scope's parent **only if** the dropped scope is current; out-of-order drops (page scopes are held across frames and dropped non-lexically) leave it untouched. A store-nothing RAII guard was rejected: guards assume LIFO order, page scopes don't obey it.
- [ ] **1.2 Multi-runtime: hide + fix, postpone the rest (decided):** move `create_runtime`/`with_new_runtime` behind a **`test-utils` feature** — dependents enable it via **dev-dependencies only** (`[dev-dependencies] rsact-reactive = { features = ["test-utils"] }`), so the API doesn't exist in any production build graph. Fix the restore bug _inside_ it in the same commit (RAII guard restoring `prev` — `runtime.rs:98-111,49-64` currently discards it, bricking the runtime; our own tests call it ~15×). Single global runtime is the only public reality. No compound `ValueId`+`RuntimeId` — postponed indefinitely.
- [ ] **1.3a Pin the push-queues-effects invariant with a test first:** `update()`'s commit path (`runtime.rs:938-950`) marks subscribers Dirty with bare `storage.mark`, which only flips the state byte — it does **not** enqueue effect-subscribers into `pending_effects` (only `mark_node` does). It works today solely because the write-time `mark_dirty` push already queued every transitively-reachable effect — an undocumented invariant. Test: an effect whose memo source recomputes during a pull must already be queued (push suppressed variants). **1.3b** then two one-line hardenings: commit path uses `mark_node` (pull becomes self-sufficient — insurance for WS5's lazier marking), and the cycle-degradation skip path (`try_borrow_mut` fail at `runtime.rs:924-930`) leaves the node's state untouched instead of marking the never-recomputed node Clean (`:952-954` — stale-but-Clean bug).
- [ ] **1.4 Delete `Debug`/`Display` impls on reactive handles entirely (decided):** `read.rs:195-291` currently implements them via tracked `with()` — a debug print inside any observer subscribes it permanently. Formatting a signal becomes a **compile error**; users write `signal.with(|v| ...)` so the read is visible and deliberate. `PartialEq`/arith ops **stay tracked** — they are dataflow (a memo computing `a == b` must re-run when either changes). If `Debug` ever returns, only the id-only form (never reads the value) is acceptable — not now.
- [ ] **1.5a Check-residue correctness suite first (decided: prove before implementing):** memo-cut diamond (after an equal-value recompute, downstream is Clean and the next idle read does zero source re-walks — uses 0.3/0.5 counters), the checkbox nested-observer scenario re-run, cut-then-real-change (a genuine change after a cut still propagates), dynamic-deps (source set changes across runs, then cut, then change through the new source), and a property test: random small graphs + random writes vs a recompute-everything oracle. **1.5b** then the fix in `maybe_update` (`runtime.rs:834-869`): after a **completed** source walk finds nothing dirty, downgrade `Check → Clean`. Safety argument: the walk recursively freshens every source; a changed source would have marked this node Dirty; walked-and-still-not-Dirty ⇒ genuinely unchanged. Invariant: only `Check → Clean`, **never** `Dirty → Clean` — the past consumed-dirtiness bug class stays impossible.
- [ ] **1.6 Remove `SignalMapReactive` entirely (decided — it's the anti-pattern, its own TODO at `maybe/mod.rs:11` agrees):** slider `step` becomes an explicit match at the call site (inert → compute once; signal → `.map()`); flex `layout_children` keeps a local, honestly-named helper until WS5 dissolves the need for an always-memo. Kills both the inert-arm live-memo-cloning-per-read and the `MaybeReactive` impl's double node (`.map(map).memo()`). Fix the stale `IntoMemo` doc alongside.
- [ ] **1.7 Kill render-path `format!`** (`el/render.rs:290-298`): `render_self` key becomes `&'static str` (`"self"`); tighten `render_part` keys to `&'static str`. This stops the per-frame heap churn now; the identity/ownership redesign is WS2 (the maintainer's encapsulation constraint is recorded there).
- [ ] **1.8 `try_*` APIs + contextful errors (decided: expose Option to the user where possible):** public `try_with`/`try_get`/`try_update` on the read/write traits returning `Option` (dead handle → `None`, logged); the panicking APIs become thin wrappers over them with contextful messages (id/type/creation-site under `debug-info`) replacing the bare unwraps at `storage.rs:56,118`; rsact-ui render/event paths migrate to `try_*` + `log::error!`. Full `Result<_, ReactiveError>` plumbing through widget APIs stays rejected (flash cost, signature infection).

Acceptance: rsact-reactive ≥ 54 pass + new regression tests (incl. the 1.3a invariant test and the 1.5a suite); rsact-ui 44/0 unchanged; `benches/allocations.rs` shows no regressions and change-frame allocs drop (no `format!`); a compile-fail check covers `format!("{:?}", signal)`.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — section WS1 (all decisions are
recorded inline in the items) + Baselines + Cross-cutting invariants. Verify each cited
bug still reproduces (write the failing test first). Fix 1.1–1.8 with TDD, one commit
each; note 1.3 and 1.5 are split test-first (1.3a/1.5a land their test suites BEFORE the
1.3b/1.5b behavior changes). Run: cargo test -p rsact-reactive --features std --
--test-threads=1 (baseline 54/2) and the rsact-ui suite (44/0). The 2 known-fails are
static_wrapper (WS4's acceptance test) and observe_recreates_disposed_child_observer
(WS2 rewrites it) — do NOT chase them.
```

---

### WS1b — UI-side correctness quick wins (final sweep, 2026-07-07)

**Sessions:** 1 · **Risk:** low · **Directions:** D2, D4 · **Depends on:** nothing (parallel with WS0/WS1). Live bugs found by the final gap sweep — test-first fixes, rsact-ui/rsact-render side.

- [ ] **b.1 Canvas blanks after any forced redraw.** `DrawQueue` drains on render (`widget/canvas.rs:201-205`, consumed at `:249`) — commands are gone after one execution, and `force_redraw` fires on every relayout, so any relayout/navigation/devtools toggle repaints the background and the Canvas renders nothing. Decide the model — immediate-mode (redraw callback re-issues per frame) vs retained replay (keep last command list, re-play on overdraw) vs `Memo<Vec<DrawCommand>>`-only — record the rationale, fix accordingly. Related: `Image` `PartialEq` returns false for `Owned == Owned` (`rsact-render/src/image/mod.rs:40`), defeating memo-diffing of command lists. The decision is design input for WS16.1.
- [ ] **b.2 Animation correctness trio** (`rsact-ui/src/anim/mod.rs`): (a) restart silently no-ops every other `start()` — completion is checked against a stale `last_tick` (`:298`) that neither `AnimHandle::start` (`:161-163`) nor the `StartRequested→Running` transition (`:274-279`) resets; (b) u32 clock wrap breaks running animations (`:331-332`; `ui.rs:318`'s `% u32::MAX` is an off-by-one modulus); (c) `AnimCycles::N(0)` plays one full cycle (`:24`); (d) `Easing::EaseOutSine` is inverted (`easing.rs:92` runs 1→0; easings.net defines `sin(x·π/2)`).
- [ ] **b.3 `FontProps::has_any()` implements has-ALL** (`font/mod.rs:77-87`): layout stores resolved text props only when all three fields are `Some` (`layout/model.rs:270-278`) while measurement always merges (`layout/mod.rs:117-121`) — so `label.font_size(20)` alone is *measured* at 20 but *drawn* at the inherited size. Align measure and draw; note the fix in WS15.1's measure-parity scope.
- [ ] **b.4 `declare_widget_style!` broken macro arm** (`style/mod.rs:151-155`): `$crate::stable::` path typo + unbound `$field` in the no-opts `text_color: color` arm — any widget declaring it without an explicit opts block gets an incomprehensible error. Two-line fix now; WS7.4 keeps the macro.

Acceptance: each bug lands with a failing test first; UI suite stays green (44/0 baseline); the b.1 canvas decision recorded in this file.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS1b. Verify each bug still
reproduces (write the failing test first). b.1 needs a design decision — record it in
the roadmap before implementing. Do not touch rsact-reactive (WS1 owns it) or rendering
internals (WS6 owns those). Baseline: rsact-ui suite 44/0.
```

---

### WS2 — Render-identity redesign: the `Probe` primitive

**Sessions:** 2 · **Risk:** high (structural) · **Directions:** D1+D5 merged · **Depends on:** WS1 · **Gated by:** G2 ✓, G6 ✓ (both decided).

Why: the render-gating mechanism is the fragile heart. Identity = bare u64 hash in a global map → deterministic cross-page aliasing (fresh arenas mint identical ElId sequences; page B _reuses_ page A's observers), silent collision under-render, unbounded observer leak for dynamic children (`remove_subtree` disposes nothing), O(N²) subscribe scans, dead "revive" branch, disabled cleanup (append-only sources + owned lists). All are consequences of _hash-keyed global identity_; D5's cost model shows idle is already fine — this is a correctness+change-frame play.

Design (decided 2026-07-06 after maintainer review):

- **Encapsulation constraint (maintainer, binding):** rsact-reactive stays UI-vocabulary-free — no `ElId`/"part" knowledge in the core, ever. The observer map moves _out of the framework_ to its owner (rsact-ui). Tracking correctness survives this because the global map never participated in tracking: its only job was call-site-hash → `ValueId` resolution; the tracking itself is `with_observer` + `subscribe`, which stay byte-for-byte identical.
- **Naming (decided): the polled primitive is `Probe`** — `create_probe()`, `Probe::poll(force, f) -> Option<R>`, `ValueKind::Observer` → `ValueKind::Probe`. The word _observer_ remains for the internal "currently-running dependent" concept (`with_observer`, the current-observer cell) — that usage is correct; it was the primitive squatting on the word that confused. Docs taxonomy: **memo** = lazy cached value · **effect** = self-scheduling · **probe** = externally polled reaction.
- **`Probe` is a first-class Copy handle** (a `ValueId` newtype, same species as `Signal`): identity _is_ the handle — no registry, no keys, no hashing. `use_observe`'s body becomes the identity-free `run_probe`, preserving today's exact step order: `is_alive` check (disposed ⇒ honest `None`) → `subscribe` → `maybe_update` → if changed‖force: `clear_sources` → `with_observer(f)` → `mark_clean` (the checkbox-fix placement, unchanged). Sketch:

  ```rust
  // rsact-reactive/src/probe.rs — zero UI knowledge
  #[derive(Clone, Copy, PartialEq)]
  pub struct Probe(ValueId);

  #[track_caller]
  pub fn create_probe() -> Probe { /* add_value(ValueKind::Probe, Dirty) */ }

  impl Probe {
      /// Runs `f` (tracked) iff any dependency changed since the last poll, or `force`.
      #[track_caller]
      pub fn poll<R>(&self, force: bool, f: impl FnOnce() -> R) -> Option<R> {
          with_current_runtime(|rt| rt.run_probe(self.0, force, f))
      }
  }
  ```

- **Per-poll cleanup = `clear_sources`, not a diff.** Memos/effects _already_ clear + re-track their edges on every recompute (`rt.cleanup(id)` before the callback, `runtime.rs:932`) — O(fan-in) over TinyVec-inline lists, fan-in typically 1–8. Probes are the only node kind skipping it, because `cleanup` does _two_ jobs and the second was the bug: it also disposes **owned children**, which nuked nested probes on parent re-run (why the call is commented out at `runtime.rs:654-656`). Split the jobs: `clear_sources(id)` (edge clear only — the cost memos already pay) runs per executed poll, fixing stale-dep accumulation; ownership disposal leaves the re-run path entirely (nested render probes are owned by `ElState`; user-created values get the scope/`on_cleanup` story). No prev-vs-new set comparison, no allocation, no new data structure. A true diff is a _later optimization_ only if 0.3 metrics show subscribe churn.
- **rsact-ui owns the map:** `ElState` holds `part_probes: TinyVec<[(&'static str, Probe); 2]>` — lookup is a **linear scan with content comparison** (≤4 entries beats any hash; pointer identity of `&'static str` is NOT guaranteed across codegen units, so compare content — put that in a code comment). Keys are stable part names (`"self"`, `"thumb"`, `"options"`), so **rearranged render order is irrelevant**, new keys create probes lazily, and a conditionally-skipped part's probe stays dormant — bounded by construction, since a widget's part-name set is finite in its source. Carry a `TODO:` in code pointing at the `PartId(u16)` compaction (≈12 B/entry vs 16, integer compare, no string bytes in flash). Same key twice in one frame = widget-author bug → `debug_assert`. `Page` owns `render_probe`. Interior mutability: prototype the `render_subtree_body` borrow choreography first (pre-extraction like `needs_redraw`, or `Cell`-based).
- **What replaces `static_observers`: ownership.** With handles stored where they're used (`Page.render_probe`, `ElState.part_probes`) there is no lookup left to perform, so nothing replaces the registry — its job ceases to exist. Deleting it removes: per-frame key hashing, the BTreeMap walk, the reverse-index insert on every call even when idle (`runtime.rs:644`), the dead revive branch, the deterministic cross-page aliasing (two pages cannot hold the same handle), and — per G6 — `ahash` from the no_std build. Cost: ~32 B in `ElState` per part-rendering element + the disposal discipline WS3 makes systematic.
- **Rewrite** `observe_recreates_disposed_child_observer` per G2 semantics; add the two replacement tests from D5's criteria 5 (child not disposed by parent re-run, re-runs iff own deps changed or forced; child disposed with its element, recreated on next render, runs exactly once).

Work items:

- [ ] 2.1 Core `Probe` handle API (`create_probe`/`poll`) + `run_probe` refactor + the `clear_sources` split (+ tests: conditional-dep unsubscribe, nested-probe survival across parent re-runs, owned-value story).
- [ ] 2.2 rsact-ui arena-owned probes (`ElState.part_probes` keyed by `&'static str` with the `PartId(u16)` TODO; `Page.render_probe`); borrow-choreography prototype first.
- [ ] 2.3 Lifecycle: dispose on `remove_subtree` + `Page::drop`; leak regression test (100 goto round-trips + 100 `set_children` reconciliations → node counts return to baseline).
- [ ] 2.4 Delete `static_observers`/`observer_hashes`/`hasher` (+ `ahash` from the no_std path per G6; keyed `observe()` deprecated, thin compat wrapper at most); rewrite the semantics test; delete the dead revive branch (`runtime.rs:626-638`).
- [ ] 2.5 **Probe documentation & third-party pattern** (A5, post 2.1–2.4): module-level taxonomy doc (memo = lazy cached value · effect = self-scheduling · probe = externally polled), rustdoc on the handle API, and a worked example of driving probes from an external render engine — the observer map now lives with its owner (`ElState`), so document that ownership pattern as the public contract for out-of-tree render paths.

**Design sketch:**

```rust
// rsact-reactive/src/probe.rs — no UI vocabulary in the core
#[derive(Clone, Copy, PartialEq)]
pub struct Probe(ValueId);
pub fn create_probe() -> Probe;         // node: ValueKind::Probe, born Dirty
impl Probe {
    /// Runs `f` (tracked) iff any dependency changed since the last poll, or `force`.
    #[track_caller]
    pub fn poll<R>(&self, force: bool, f: impl FnOnce() -> R) -> Option<R>;
    // poll = subscribe(parent edge) → maybe_update → if dirty || force:
    //        clear_sources(id)  ← edges only; ownership NOT touched (the old cleanup bug)
    //        → with_observer(id, f) → mark_clean
}
```

```text
identity & data flow, before → after
BEFORE  render_part(key) → hash(ElId|part) → static_observers: BTreeMap<u64, ValueId> → run
        global map · deterministic cross-page aliasing · leaks · per-frame hash+lookup
AFTER   ElState { part_probes: TinyVec<[(&'static str, Probe); 2]> } ──owns──▶ Probe(ValueId)
        render_part("thumb") → linear scan ≤4 entries (content compare; TODO: PartId(u16))
        → probe.poll(force, draw)
        remove_subtree / Page::drop ──▶ probe dispose        no map · no hash · no leak
```

Acceptance (D5's criteria): idle frame 100 parts = 0 allocs, 0 hashes, ≤5 state probes; change frame = 0 bookkeeping allocs, `observe_redraw_1_of_n/64` ≥3× better; aliasing impossible by construction; profile counts flat across navigation; suites green with the rewritten test.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — section WS2 (the design is fully
decided and recorded there, incl. the Probe naming and the binding encapsulation
constraint: rsact-reactive stays UI-vocabulary-free), Gates G2/G6 (decided), and
Cross-cutting invariants I1–I7. Verify WS1 landed (scope fix, Check-residue suite).
Two-session structural change: session 1 = rsact-reactive Probe handle
(create_probe/poll) + clear_sources split + tests; session 2 = rsact-ui arena-owned
probes + registry deletion. Prototype the ElState interior-mutability/borrow
choreography before committing to it. Baselines in the roadmap file.
```

---

### WS3 — Ownership & lifecycle: scopes, page disposal, one-shot

**Sessions:** 1 · **Risk:** medium · **Directions:** D2, D7, D1 · **Depends on:** WS1 (1.1), WS2 (arena-owned probes) · **Gated by:** G11.

Why: P2's UI half. No scopes exist in rsact-ui; `Page::drop` disposes only the arena signal; build-time effects (`Dynamic`, `Flex` children) survive navigation holding a disposed arena signal → **delayed panic** when an app signal fires (D2-F1); PageState keeps stale ElIds (`focused` can point at freed nodes indefinitely).

Work items (3.0 added per maintainer review, folded 2026-07-07):

- [ ] **3.0a Leak-attribution diagnostics** (`debug-info`): a `leak_report(snapshot)` API — snapshot the live node-set before a page/subtree build, diff after its disposal; survivors reported **with their creation site** (the `Location` breadcrumb already exists in `ValueDebugInfoState` — this is plumbing, not new tracking). The 0.3 metrics _detect_ leaks (counts moved); 3.0a _attributes_ them (which `file:line` created the survivor).
- [ ] **3.0b Full disposal audit of rsact-ui**: inventory every `create_signal`/`create_memo`/`create_effect`/`Layout` creation site (widgets, page, ui, event — **explicitly including the easy-to-miss ones**: `Anim::handle` mints a signal + memo per animation (`anim/mod.rs:248,255`), `DrawQueue::new` two signals (`canvas.rs:74-79`), `UiQueue::new` two signals + a memo (`event/message.rs:40-47`), all unowned today); record who _should_ own each; one lifecycle test per path (page drop, `set_children` subtree replace, `Dynamic` rebuild, navigation round-trip) asserting counts return to baseline. **Answer on record to the maintainer's question ("do widget-stored signals get disposed?"): no — nothing disposes them today.** Widget structs hold `Copy` handles; dropping `ElData` drops 8-byte keys while the runtime nodes live forever. The scope model is the fix: widgets are _built_ inside a scope (3.1 per-page, 3.2 per-subtree), so build-time `create_*` calls are scope-owned and die with it; the widget's now-dangling handles are safe post-disposal (the element is gone; any straggler read becomes a logged no-op via WS1.8 `try_*`). This audit's inventory is the direct design input for 3.2.
- [ ] 3.1 Per-page `ScopeHandle` created in `UI::load_page` (`ui.rs:212-230`); `Page::drop` disposes the scope (everything the `PageInitFn` created). Regression test: the disposed-arena effect panic repro; navigation leak test counting runtime nodes.
- [ ] 3.2 Subtree disposal: `remove_subtree` (`el/arena.rs:170-179`) disposes widget-owned reactive nodes (scope-per-subtree or a `Widget`-level dispose hook — design with WS2's ownership model).
- [ ] 3.3 PageState pruning on element removal (`ctx.rs:97-99` TODO): invalidate `focused`/`captured_by`/`hovered` referencing freed ids (D2-F5).
- [ ] 3.4 `rsact::render_once` one-shot sugar (D7): build → layout → render → drop; heap returns to ~baseline. Acceptance: e-paper sketch from the D7 report compiles as a doc-test/example.
- [ ] 3.5 `zip_eq` → checked zip + `error!` in both hot passes (`el/event.rs:89,126`, `el/render.rs:499`) — arena/layout divergence must degrade, not abort (D2-F3).

**Design sketch:**

```rust
// ui.rs::load_page — G11: page-created = page-owned
let scope = new_scope();                         // ScopeData { parent, values } (WS1.1 chain)
let root  = with_scope(&scope, || init_fn());    // every create_* inside lands in scope.values
pages.insert(id, Page { scope, .. });
// Page::drop ─▶ drop_scope(self.scope) ─▶ dispose all owned values
//            ─▶ current_scope restored via the parent pointer
// remove_subtree(el) ─▶ dispose the subtree's scope (3.2) + its part_probes (WS2)
// signals meant to OUTLIVE the page: create them outside the PageInitFn — that's the contract
```

Acceptance: navigation and reconciliation leak tests flat; the delayed-panic repro fixed; PageState never routes to freed ids; UI suite green.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS3 + gate G11. Verify WS1.1 and
WS2 landed (their ownership machinery is what pages/subtrees hook into). Run 3.0a + 3.0b
first — the disposal-audit inventory is the design input for 3.2. Then write the
disposed-arena delayed-panic repro test (page A holds dynamic(|| app_signal.get()),
navigate, fire app_signal) before fixing it, and 3.1–3.5 with TDD.
```

---

### WS4 — Zero-cost statics (the RAM workstream)

**Sessions:** 2 (+ the 4.0 analysis) · **Risk:** medium · **Directions:** D1(c), D7, D6 · **Depends on:** WS2 (probe shape), WS0 (regression harness) · **Gated by:** G1 ✓ in direction; 4.0 delivers the execution go/no-go.

Why: P3. Statics dominate real UIs (31/46 nodes measured); `Inert` node ≈ 150–250 B each; label = double node; builder literals leak one node per call; 9 unconditional singletons per app/page; probes are minted even for parts with zero reactive deps. Target: **a fully static page allocates ~0 runtime nodes** — same API, no "static mode", it just falls out.

Work items (maintainer review folded 2026-07-07; execution order: 4.0 → 4.1 → rest):

- [ ] **4.0 Analysis first (maintainer-authored): can `Inert` be stored inline, and what breaks?** The two user classes of `MaybeReactive`/`Inert`: (1) **user-facing props** (reactive-or-inert) — the inline target; (2) **`Layout`**, which uses `Inert` for static layouts and **cannot be inlined freely**: it needs shared identity (the widget's copy and the parent's children-vec copy must observe the same data), and mutating-through-`Copy` already produced the `LayoutMut` bug class — so analyze `Widget::layout` returning `&Layout` and removing `Clone`/`Copy` from `Layout` for safety; there may be deeper problems buried. Inlining also removes blanket `Copy` from `MaybeReactive`, so **all usages are rethought with Copy-loss in mind** (G1; the compiler finds every site, `MaybeSignal::Inert` is the in-tree precedent). **Agreed scope cut:** `Layout` is out of WS4's execution scope — it is resolved by WS5.1's off-graph shared handle; 4.0's Layout analysis is the design input for WS5.1, not a blocker for 4.1. Deliverable: written analysis + go/no-go for 4.1.
- [ ] **4.1 Inline `Inert(T)`** (gated on 4.0): in rsact-reactive (`inert.rs`, `maybe/maybe_reactive.rs`, `memo.rs::Memo::Inert`) mirroring `MaybeSignal::Inert`; `Copy` for `T: Copy` only. Flip `maybe::tests::static_wrapper` to **passing** — it is the acceptance test. Ripple through rsact-ui (`label.rs` double-node fix included, setter traits); `Layout` untouched (4.0 scope cut).
- [ ] **4.2 Builder-literal leak — becomes a verification item after 4.1.** The problem today (yes — exactly as the review guessed): every `.gap(2u32)`-style call converts the literal into an `Inert` **runtime node**; the setter reads it once, and because view construction runs with no scope active the node is owned by nothing — not merely undisposed but _undisposable_: one permanent node per builder call, forever. 4.0/4.1 dissolve the mechanism (the value arrives inline, by move — no node ever exists). 4.2 = sweep the setter paths (`layout.setter`, the `widget/mod.rs` builder traits) confirming no node-creating conversion path survived, asserted via the 0.4 node-count test.
- [ ] **4.3 Singleton de-greed.** The issue: every app/page allocates ~9 reactive nodes unconditionally for things that never change reactively — pure graph freight. Each gets demoted to the cheapest primitive its real use supports: renderer signal → plain field (its own TODO at `ui.rs:61-64` already says it shouldn't be reactive); dev_tools signal → exists only under `simulator`; fonts signal → plain data + an explicit "fonts changed → relayout" call (fonts change at startup, not reactively); page style signal → `MaybeSignal`; `force_redraw` signal → imperative flag through the existing `force` path (fully dies in WS5/WS6); viewport stays inert; the eager per-page `LayoutModel` memo → dissolved by WS5. Payoff beyond node count: every render probe currently tracks `force_redraw`, so that one demotion removes a page-wide fan-out edge per part.
- [ ] **4.4 Zero-source render probes — demote, not dispose (investigation; maintainer correction folded).** A part with no reactive deps still needs _force_-rerendering (parent overdraw / damage) — but force needs no graph node: `probe.poll(force=true, f)` runs `f` unconditionally, and the node's only job ("did my deps change?") is eternally "no" for zero deps. So: pull the node out of the runtime and keep the closure as a plain render function — `ElState`'s part entry becomes `enum PartGate { Static, Probe(Probe) }` — saving runtime slots and dispatch. **Soundness hazard to resolve first: conditional first runs** — `if state.expanded { signal.get() }` with `expanded == false` on run #1 would demote the part, and the later reactive read would never re-register → stale UI. Options on record: (a) **opt-in** — the widget declares a part static (safe, zero magic — the default posture); (b) demote-after-N-clean-runs (heuristic, unsound — rejected); (c) **lazy re-promotion** — demoted parts run under a sentinel observer that mints a real `Probe` on the first `track()` call and re-registers (sound and automatic; costs one branch on the track path). Separate investigation; not blocking WS4's main line.
- [ ] **4.5 Interactive-widget signal audit.** The issue: widget _constructors_ create real `Signal`s unconditionally, whether or not that instance is ever interactive — checkbox value (`checkbox.rs:36`), slider state+value (`slider.rs:81-82`), knob state, scrollable state (`scrollable.rs:106`), select (`select.rs:143,166`), and **icon allocates a signal AND a memo just for its size** (`icon.rs:74-78`). Measured: 10 static checkboxes → 16 signals. Per constructor, ask "does interactivity _require_ this node, or is it reactive-by-habit?" — keep the former (checkbox's value signal is its job), demote the latter to plain fields / `MaybeReactive` props (icon size is the poster child). Acceptance: 10-checkbox probe 16 → ~10 signals; a static icon → 0 nodes. (Whether interactive state should later move to `ElState`-style flags the way hover/press did: separate topic, parked.)

- [ ] **4.6 Storage capacity management (A6).** Fact on record (maintainer question answered): `SlotMap::remove` **does** drop the stored value immediately — the heap payload (`Rc` + boxed data) is freed on dispose. What never shrinks is the backing **capacity**: removed slots become vacant entries reused by future inserts, but the slot array and our dense `SecondaryMap`s (sources/subscribers/owned/mark_seen) keep their high-water size forever — peak node count = permanent RAM. On embedded that peak-sizing is often _desirable_ (deterministic memory). Work: expose high-water marks + vacant counts in `Profile` (extends 0.4), then decide policy — document "capacity = peak" as the contract, and/or an explicit `shrink`-style compaction call for host/long-running use. Coordinate with 9b.1 (the storage rework changes the layout this measures).

**Design sketch:**

```rust
// BEFORE (today): even constants are runtime nodes
pub struct Inert<T>(ValueId, PhantomData<T>);              // slot + Rc<RefCell<dyn Any>> each
pub enum MaybeReactive<T> { Inert(Inert<T>), Memo(Memo<T>) }   // Copy for ALL T (it's just ids)

// AFTER (4.1, mirrors MaybeSignal::Inert): constants are plain values
pub struct Inert<T>(T);                                     // zero runtime presence
pub enum MaybeReactive<T> { Inert(T), Memo(Memo<T>) }       // Copy iff T: Copy (G1)
// .gap(2u32) ⇒ MaybeReactive::Inert(2) ⇒ setter consumes by move ⇒ NO node, ever (4.2)

// 4.4 (investigation) — demote zero-dep render parts out of the graph:
enum PartGate {
    Static,          // plain render fn — force-rerender still works: poll(force=true) needs no node
    Probe(Probe),    // parts with real reactive deps keep their graph node
}
```

Acceptance: canonical static page node count ≈ 0 (WS0's regression test re-baselined); live heap for the 10-widget probe −30–40%; static label ≤ ~0.2 KB; reactive suite green (`static_wrapper` now passing); UI suite green.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS4 + gate G1. Verify WS0's
node-count test and WS2's Probe handles exist. Execute in order: 4.0 analysis FIRST
(written deliverable; confirms the Layout scope-cut to WS5.1 and the go/no-go for 4.1).
Then session 1 = rsact-reactive inline Inert (make static_wrapper pass); session 2 =
rsact-ui de-greed (4.2 verification sweep, 4.3 singletons, 4.5 widget audit, 4.6
capacity metrics + policy). 4.4 is an investigation — do NOT land a demotion mechanism
without resolving the conditional-first-run hazard (options recorded in the item).
Re-baseline the WS0 regression numbers and record old→new in the roadmap.
```

---

### WS5 — Layout: off the graph, then incremental

**Sessions:** 2–3 · **Risk:** medium-high · **Directions:** D3 + D7(P2) merged · **Depends on:** WS4 (MaybeReactive/Layout field ripples), G8 · **Feeds:** WS6.

Why: P5's layout half. Whole-page memo, O(N·D) min*size recursion, no measure caching, side-effecting memo, O(N) tree PartialEq. The reconciled design (D3×D7): layout data lives **outside the reactive graph** (`Rc<RefCell<LayoutData>>`, identity preserved — retiring the `layout_mut` reactive-on-write trap class entirely); each \_reactive binding* creates exactly one effect that writes the data **and marks the node id in a page-level dirty set** — D7 gets pay-per-binding, D3 gets write→node invalidation, from the same mechanism.

Stages:

- [ ] **5.0 Quick wins (independent, can run any time after WS0.5):** per-pass `min_size`/`ContentSizing` reuse in `model_flex`/`model_layout` (kills O(N·D) → O(N), zero retained RAM); move `force_redraw.set(true)` out of the memo (`page/mod.rs:134`), fire only when the model actually changed (e.g. `Keyed` generation instead of O(N) `PartialEq`) — if WS4.3 has already landed, `force_redraw` is an imperative flag and this targets that path instead. Expected: 3–10× on deep pages, repaint-on-no-change gone.
- [ ] **5.1 Layout off-graph:** `Layout` → shared `LayoutData` handle outside the graph + binding-effects + page dirty set; node identity = `ElId` recorded at build (also makes the arena↔layout `zip_eq` invariant explicit); `transparent_layout` maps to parent. Consumes WS4.0's Layout analysis (`Widget::layout` returning `&Layout`, `Clone`/`Copy` removal from `Layout` for mutation safety).
- [ ] **5.2 Retained tree + boundary stop rule:** per-node `(last_inputs, outer_size, min_size, flags)` ≤ 64 B/node (feature-gated `incremental-layout`); skip-and-splice clean subtrees; recompute dirty via the unchanged `model_layout` kernel; stop upward when `(outer_size, min_size)` unchanged (tight limits / Fixed×Fixed / `InfiniteWindow` scrollables are natural boundaries). **Differential fuzz test**: random trees + random single mutations, incremental result `==` full recompute.
- [ ] **5.3 Changed-set output:** relayout returns the list of nodes whose absolute rect changed (old∪new rects) — the damage channel WS6 consumes.
- [ ] **5.4 Persistent text-measure cache (A7, after 5.0's per-pass reuse):** small feature-gated cache _across_ passes, strict RAM budget (e.g. 16 entries, fixed-size, no text storage). Key design decided in-session with a bench: 64-bit hash of (font id, text, width constraint) — collision ⇒ silently wrong size, astronomically unlikely but deterministic-per-build, so either document it or exact-compare texts ≤ N bytes inline. Invalidated by 4.3's explicit "fonts changed" call. Guard: hash cost must stay well below measure cost (add to the 0.3 snapshot).

**Design sketch:**

```rust
// BEFORE: Layout::Static(ValueId) | Reactive(Signal<LayoutData>); the page-wide
//         Memo<LayoutModel> tracks every reactive layout signal and recomputes the world.
// AFTER (5.1): layout data lives OUTSIDE the reactive graph; a binding = exactly one effect.
pub struct Layout(Rc<RefCell<LayoutData>>);   // shared identity; NOT Copy (4.0's analysis:
                                              // Widget::layout may return &Layout)
// .width(sig)  ⇒  create_effect(move || {
//     layout.borrow_mut().size.width = sig.get();   // write the data
//     page_dirty.mark(el_id);                       // D3 write→node ∧ D7 pay-per-binding
// });
```

```text
write flow   sig.set(w) ─▶ binding effect ─▶ LayoutData updated + dirty_set ∪= {ElId}
flush (lazy, same pull points as today: event hit-test / render)
  relayout_incremental(prev_tree, dirty_set) → (LayoutModel, changed_set)
    ├─ clean subtree + unchanged inputs         → skip & splice retained result
    ├─ dirty subtree                            → recompute via UNCHANGED model_layout kernel
    ├─ stop upward iff outer_size AND min_size both unchanged   ← the stop rule (invariants)
    └─ changed_set = [(ElId, old∪new rect)] ────▶ WS6 damage channel (5.3)
```

Rejected on the record: per-node layout memos (350–500 B/node graph freight — disqualified on M0 RAM, D3 candidate b).

Acceptance (D3's criteria): text update in a Fixed×Fixed label = 1 node visit, ≤2 measures, only that label's probe re-runs; show-toggle re-solves only the parent flex; root change ≤ +10% of today; per-node retained state ≤ 64 B, compiled out under default features; existing layout/UI tests green.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS5 + gate G8 + the D3 stop-rule
paragraph (upward propagation may stop ONLY if outer_size AND min_size are both unchanged
— min_size feeds the parent's wrap decision and min-clamps over max). Verify WS0.5
counters and WS4's Layout/MaybeReactive state. 5.4 (text-measure cache) comes only
after 5.0's per-pass reuse has landed. Run 5.0 first as its own commit series
(it is independent and pays immediately). 5.1–5.3 are the structural stages; keep the
full-relayout path as the default compiled path, incremental behind a feature until the
fuzz test has soaked.
```

---

### WS6 — Damage-driven rendering & the flush pipeline

**Sessions:** 2 · **Risk:** medium · **Directions:** D5(C), D3(P4), D4, D6(P1a) · **Depends on:** WS2, WS5 (changed-set) · **Gated by:** G3, G12.

Why: P5's render half — the biggest _practical_ gap vs LVGL/Slint. Fine-grained observers already know what re-rendered, then `finish_frame` streams **every pixel of the viewport** through a per-pixel iterator (`eg/framebuf.rs:147-161`, `eg/output.rs:7-25`); no dirty rects; e-paper region refresh impossible; full-screen SPI transfer per change. Also the blanket `force_redraw` defeats per-part gating (three directions demanded its death).

Work items:

- [ ] 6.1 **Replace blanket `force_redraw`** with targeted invalidation: WS5's changed-set → `needs_redraw` on affected `ElId`s (mechanism exists: `ElState::take_needs_redraw`, `observe_with_force`); clear only old∪new rect unions. Keep an explicit full-invalidate escape hatch (page enter, dev tools).
- [ ] 6.2 **Dirty-region accumulation**: render pass records executed parts' clip rects into a small LVGL-style joined-areas list (4–8 rects) on `RenderShared`.
- [ ] 6.3 **`FinishRender` regions API** + row-contiguous output: `finish_frame_regions(target, &[Rect])` using `fill_contiguous`/scanline runs instead of the per-pixel iterator (likely 5–10× SPI win even before partial flush — D4 measured path). **Final-sweep addition — the render side too:** `PackedFramebuf` implements only `draw_iter`, so every background/clear/fill is per-pixel bit-twiddling (`eg/renderer.rs:300-310`, `eg/framebuf.rs:201-213`); implement `fill_solid`/`fill_contiguous` on it (whole-byte writes for mono, `slice::fill` runs for RGB) — an 8–32× render-side win, distinct from the flush-side fix.
- [ ] 6.4 **Framebuffer modes** in rsact-render: full (current), N-line strip rendering, direct-to-target; user-provided buffer. Acceptance: 240×240 RGB565 heap from 112.5 KiB → <20 KiB in strip mode (D6 P1 target). **Final-sweep design constraints:** (a) `PackedFramebuf`'s area-based packing asserts `area % pps == 0` — a real 122×250 mono e-paper **panics in the constructor**, and rows are never byte-aligned (`eg/framebuf.rs:230-243,163-177`) — strip/partial modes need per-row pitch/stride (also what SSD1306 page addressing and 6.3's scanline runs require; G3's 128×64 reference accidentally hides this); (b) `EGRenderer::pixel_alpha` blends against untranslated coordinates (`eg/renderer.rs:166-173`) — dead today (`ViewportKind::Cropped` is never constructed) but a live AA bug the moment strips/windows translate; (c) decide the fate of the **layer dimension** first: two parallel `Layering` implementations exist (`layer.rs:15-55` + a private copy in `EGRenderer`, TODO at `renderer.rs:109`) maintaining a per-draw BTreeMap lookup, yet no code path can create layer > 0 — delete the dimension or deliberately revive it before building modes on top.
- [ ] 6.5 **e-paper story**: partial window refresh driven by the regions API; `render() -> bool` + regions documented as the e-paper contract.
- [ ] 6.6 (Optional, after 6.1–6.3) **dirty-list walk** (D5 Phase C): `mark_dirty` enqueues `(ElId, part)` into a page-owned map; render walks dirty paths + forced subtrees only — eliminates the O(N) tree walk on change frames. Preserve parent-clears-before-children ordering.
- [ ] **6.7 Non-blocking flush investigation (A8 — sans-IO, NO new deps).** Design `FinishRender` so region flushing is _resumable/chunkable_ rather than monolithic: `flush_regions()` yields an iterator/state machine of (window, scanline-run) chunks that the **app** drives to completion — a blocking driver loops it; a DMA/async user feeds chunks into their own machinery between polls. This gives generic async rendering support with **zero async dependency in rsact**; an optional `embedded-hal-async` adapter can live in a separate opt-in crate later, only if demand appears. Design together with 6.3 so the regions output is chunk-friendly from day one.
- [ ] **6.8 Display rotation/orientation (A9):** 0/90/180/270 at the framebuf/regions layer; design inside 6.4's mode work (rotation interacts with strip windows and region coordinates); per-backend transform behind a target-agnostic API.
- [ ] **6.9 Golden-image render tests (A10) — land FIRST in this WS:** tiny-skia PNG snapshots (host) + NullRenderer/RecordingRenderer draw-call goldens, with a blessed-image update workflow (`UPDATE_GOLDENS=1`). Every subsequent WS6 item (and WS6.10, and WS17) is then reviewable by golden diff.
- [ ] **6.10 Renderer parity audit (A11 — the EVOLUTION TODO):** EG vs tiny-skia primitive behavior — arc start/sweep points, stroke alignment, corner radii — executed as golden-test pairs on 6.9's harness; divergences fixed or explicitly documented. **Pre-found divergence (final sweep):** tiny-skia rounded corners use `KAPPA = 0.5` instead of `0.5523` (`tiny_skia/path.rs:8` — the comment cites `(4/3)·tan(π/8)` but the constant is wrong; 9.5% short → visibly squarish corners vs EG).
- [ ] **6.11 tiny-skia clipping is a no-op (final sweep — also the old audit's `tinyskia-clipped-noop`):** `clipped()` pushes `ViewportKind::Clipped` onto the layering stack (`tiny_skia/mod.rs:154-163`) but every draw path uses `Transform::identity()` and mask `None`, never consulting `current_viewport()` — Scrollable content overflows its bounds on this backend, and WS16.3 plans to build the desktop tier on it. Implement mask-based clipping (or transform+clip rect); precondition for WS16.3, verified by 6.9 goldens.

**Design sketch:**

```text
damage pipeline (6.1–6.5):
probe ran / WS5 changed_set ─▶ needs_redraw(ElId) ─▶ render pass records executed clip rects
  ─▶ joined-areas list (≤8 rects, LVGL-style) on RenderShared
  ─▶ clear + redraw only old∪new regions ─▶ flush only those regions to the display
     (ST7789: GRAM window writes · e-paper: partial refresh · full-invalidate hatch stays)
```

```rust
// 6.7 sans-IO chunkable flush — the APP drives it; zero async deps inside rsact
let mut flush = renderer.flush_regions(&regions);        // a state machine, not a loop
while let Some(chunk) = flush.next_chunk() {             // chunk = (window: Rect, pixels: &[u8])
    display.write_window(chunk.window, chunk.pixels)?;   // blocking driver: just loop it
}   // DMA/async integration: feed chunks into your own machinery between polls — same API
```

Acceptance: change of one label flushes only its region (simulator-verifiable + probe byte counts); strip mode passes the render test suite; no regression in `draw_on_demand`/`checkbox_redraws_on_toggle`.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS6 + gates G3/G12. Verify WS2/WS5
landed (arena-owned probes; changed-set API). Land 6.9's golden harness FIRST — every
later item is reviewed by golden diff. Then 6.1 → 6.2 → 6.3 (correctness + immediate SPI
win; co-design 6.7's chunkable flush with 6.3), then 6.4 framebuffer modes (+6.8 rotation
inside it), then 6.5/6.6/6.10. The invariant from
D5-I5 holds throughout: parent redraw ⇒ child overdraw; child dirty ⇒ page dirty; O(1)
idle gate survives.
```

---

### WSi — Internals polishing (pre-API-collapse hygiene)

**Sessions:** 1 · **Risk:** low · **Directions:** D1, D4 · **Depends on:** WS1 (`try_*` machinery exists); scheduled after WS2–WS6 so their new code is held to the same bar · **Position:** deliberately right before WS7, so the breaking API pass starts from a panic-clean, lint-clean base (A3, maintainer-requested placement).

Why: "UI must never panic" is currently enforced by review, not by the compiler. Make it a gate before the API surface is finalized, so nothing panic-shaped survives into the breaking release.

Work items:

- [ ] **i.1 Unwrap/expect lint ratchet:** `deny(clippy::unwrap_used, clippy::expect_used)` via the workspace lints table in all lib crates; tests/benches/examples exempt; genuinely-unreachable cases get a scoped `#[allow]` + a one-line justification comment.
- [ ] **i.2 Burn-down:** convert the remaining lib-code unwraps (~180 at audit time; WS1–WS6 will have consumed many) to `try_*` + `log::error!` degrade paths (WS1.8 machinery). Render/event/nav paths first.
- [ ] **i.3 Panic-message audit:** any deliberately-retained panic (true invariant violations) carries a contextful message under `debug-info`.
- [ ] **i.4 Misc hygiene sweep:** `Arena::expect` naming collision (returns `Option` and logs — collides with std `expect` semantics; audit finding); leftover `#[allow(unused)]` scaffolding; dead branches found en route (report, don't silently delete `Note:`/`TODO:` markers).

Acceptance: workspace builds with the deny lints active; zero unallowed `unwrap`/`expect` in lib code; UI suite green; a before/after grep-count table recorded here.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WSi. Verify WS1 landed (try_*
APIs) and check how many unwraps WS2–WS6 already removed (grep count first). Enable the
deny lints per crate, burn down remaining sites with log-degrade fixes (TDD where a
behavior changes), record the before/after table in the roadmap. Do NOT delete Note:/TODO:
comments; report anything that looks like a real invariant instead of forcing an allow.
```

---

### WS7 — API collapse: events, WidgetCtx, Stylist, Widget trait (the breaking batch)

**Sessions:** 2–3 · **Risk:** medium (mechanical but wide) · **Directions:** D2, D6(P2a) · **Depends on:** decisions G4/G5/G9 now; execution after WS2/WS4/WS5 · **This is deliberately LATE per the deep-first philosophy — but its decisions are locked at gate time so core work doesn't build on doomed shapes.**

The staged collapse (D2's analysis, amended by G5: widgets vary over exactly TWO degrees of freedom — the renderer and, per the maintainer's decision, the custom-event type; `PageId` never reaches widget code; `Stylist` reaches it only through `get_style`):

- [ ] 7.1 **Events (G5: `Event::Custom` KEPT)**: custom events remain the extension point for app-defined widgets (user widgets are concrete over their own ctx and can match `Event::Custom(MyEvent::…)`). Scope reduces to: the `MoveEvent`→`InputEdge` rename TODO, reserving room for future `Key(u8)`/`Char`, making `E = ()` the zero-ceremony default (via G4's blanket ctx impl) so apps without custom events never have to name it, and **(final sweep) fixing the Wheel event's `Point` overload** — the simulator stuffs scroll-delta into `MouseEvent::Wheel(Point, _)` (`event/simulator.rs:78-86`) which `interpret_as_rotation` reads as a delta but `cursor_point()` returns as a *position* preferred over the real pointer position (`event/mod.rs:177`, `el/event.rs:226-230`); wheel needs a delta type, not a `Point` (precondition for WS10.6's bounds pruning, which would otherwise route wheel events by the bogus position (0, ±1)).
- [ ] 7.2 **De-generic widget-local params** (D6): `Dir: Direction` → runtime `Axis` field (constructor arg; `model_flex` is already shared), `V: RangeValue` → canonical numeric at the boundary. Measured: ~2.4 KiB flash per instantiation removed. (Cheap; can ride early.)
- [ ] 7.3 **PageId demotion**: `UI<R, P>`/`Page<R>`/`UiQueue<P>` driver-level only (13 sites, none in widgets).
- [ ] 7.4 **Stylist redesign: TypeId styler registry (G9 ✓)**: sorted `Vec<(TypeId, Box<dyn Any>)>` on the theme (binary search, no hashing, ~24 B/entry); `theme.styler::<S>(closure)` registers, `ctx.style::<S>(class)` resolves `S::base()` → registry hit → per-instance `.style()` closure; built-ins ship as **pre-registered stylers** on `Theme::light()/dark()/BinaryTheme` — adding a built-in widget becomes one registration line, never a breaking theme change (also dissolves the BinaryColor trait-impl coherence pain). `RenderShared.stylist` dyn-erased (cold path). Add the `style_resolution` micro-bench to the 0.3 snapshot (G9's tripwire; pre-approved fallback = build-time hoisting, 8 B/widget). Replace `derivative` in `declare_widget_style!` (unmaintained, RUSTSEC-2024-0388).
- [ ] 7.5 **`WidgetCtx` → two associated types (shape pending G4 sign-off)**: `WidgetCtx { type Renderer; type Event; type Color; }` where `Color` is set from the renderer's color in every impl (kept so `W::Color` keeps compiling), with a blanket `impl<R: Renderer> WidgetCtx for R { type Event = (); }` — `type W = MyRenderer` works with zero ceremony; apps with custom events write one small ctx impl. Existing `impl<W: WidgetCtx> Widget<W>` code compiles **unmodified**; delete `Wtf` + the `PhantomData` plumbing. Optional `default-backend` feature exporting `type W` sugar (D7-friendly). **Fold into the decision paper (final sweep):** `Renderer::set_options`/`type Options` is dead API (zero callers) while AA is a type-level parameter with a commented-out runtime ambition (`renderer.rs:12-41`) — decide runtime-vs-type-level AA before this trait shape locks and before WS16.1 designs the IR.
- [ ] 7.6 **Widget trait method set**: remove `update` (no widget overrides it — framework applies Update bookkeeping directly to `ElState`), add the hooks widgets actually need (`post_render` for Scrollable's scrollbar overlay; child-focus notification for Select — their own TODOs). Decide `Widget: Any` (downcast: devtools plan or delete).
- [ ] 7.7 `#[derive(View)]`: detect the `WidgetCtx`-bounded param instead of hardcoding the ident `W` (D2-F8).

**Design sketch:**

```rust
// 7.5 — WidgetCtx collapses 5 → 2 real degrees of freedom (G4 shape, pending sign-off)
pub trait WidgetCtx: 'static {
    type Renderer: Renderer;
    type Color: Color;      // set = Renderer::Color in every impl (keeps W::Color compiling)
    type Event: 'static;    // custom events (G5: KEPT)
}
impl<R: Renderer + 'static> WidgetCtx for R {              // zero-ceremony default
    type Renderer = R; type Color = R::Color; type Event = ();
}
type W = EgRenderer<Rgb565>;                               // common case: the renderer IS the ctx
// custom events: one 3-line ctx —
struct App; impl WidgetCtx for App {
    type Renderer = EgRenderer<Rgb565>; type Color = Rgb565; type Event = AppEvent;
}
// existing `impl<W: WidgetCtx> Widget<W> for Button<W>` blocks compile UNMODIFIED

// 7.4 — styler registry (G9): sorted Vec<(TypeId, Box<dyn Any>)>, binary search, no hashing
let theme = Theme::dark()
    .styler(|base: ButtonStyle<Rgb565>, status| base.border_radius(0))       // tweak built-in
    .styler(|base: GaugeStyle<Rgb565>, _| base.needle_color(RED));           // theme 3rd-party
// widget render:
let style = ctx.style::<GaugeStyle<W::Color>>(self.class());
// resolution = GaugeStyle::base() → registry hit by TypeId → per-instance .style() closure
```

Acceptance: one (or two) clearly-labeled breaking releases; UI suite green; a widget file compiles unmodified across 7.5 (proof of the blanket-impl claim); size-probe delta ≈ 0 for 7.5 (expectation-setting) and −10–20% rsact_ui `.text` for 7.2.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS7 + gates G4/G5/G9 (their
recorded answers). STOP if G4 is not signed off — 7.5 and 7.1's E=() default are
blocked on it (7.2/7.3 may proceed regardless). Verify WS2/WS4/WS5 landed. Execute the
staged collapse 7.1→7.7 in order (each stage is sed-able and independently testable;
7.2 may already have landed early — check the checkboxes). Batch into at most two
breaking releases. The compat claim to preserve: existing `impl<W: WidgetCtx> Widget<W>`
blocks keep compiling through 7.5 via the blanket ctx impl.
```

---

### WS8 — Reactive primitives completeness

**Sessions:** 1–2 · **Risk:** low-medium · **Directions:** D1(F) · **Depends on:** WS2 (ownership/cleanup machinery), WS3.

Work items (build-list per D1's triage; YAGNI'd: stores/lenses, more batching entry points):

- [ ] 8.1 `create_memo_with(f, eq_fn)` custom comparator (EVOLUTION.md idea; `MemoCallback` already isolates the comparison; `Computed` becomes `eq = |_,_| false` sugar).
- [ ] 8.2 `on_cleanup` per-node callbacks (drained on dispose/re-run) — if not already landed as part of WS2's owned-value story.
- [ ] 8.3 `watch(deps_fn, run_fn)` explicit-deps effect (~50 LOC; avoids accidental over-tracking).
- [ ] 8.4 **Keyed list reactivity**: `KeyedSignal<K, T>`-style stable per-key child scopes with diffing on write — designed against rsact-ui's `Signal<Vec<El>>`/`Dynamic` (the bread-and-butter menu/list widget path; currently O(n) compare + full rebuild). Needs WS1.1 scopes + WS3 subtree disposal.
- [ ] 8.5 `what_changed()` under `debug-info` (EVOLUTION.md TODO; the breadcrumbs already exist in `ValueDebugInfoState`).
- [ ] 8.6 **(final sweep) `Resource`/`async` subsystem adoption**: `resource.rs` + `async_rt.rs` (behind the `async` feature) are a complete, tested primitive (generation-guarded cancellation, executor-agnostic) with **no owner in the roadmap** — audit them against WS3's scope ownership and WS9b's storage rework, add `async` to the sanctioned feature axes, and give it a showcase in WS10.4's embassy example.

**Design sketch:**

```rust
pub fn create_memo_with<T: 'static>(                                  // 8.1
    f: impl Fn() -> T + 'static, eq: impl Fn(&T, &T) -> bool + 'static) -> Memo<T>;
// Computed becomes sugar: create_memo_with(f, |_, _| false)

pub fn watch<D: PartialEq + 'static>(                                 // 8.3 explicit deps
    deps: impl Fn() -> D + 'static, run: impl FnMut(&D) + 'static);
// only `deps` is tracked; `run` reads freely without subscribing (kills over-tracking)

pub struct KeyedSignal<K: Eq, T> { /* Vec<(K, Signal<T>, ScopeHandle)> */ }   // 8.4
// write diffs BY KEY: unchanged keys keep their Signal identity → no El rebuild;
// removed keys drop their scope (WS3 disposal); inserts mint scope + signal.
// Replaces today's O(n) whole-Vec PartialEq + full rebuild in Dynamic/Signal<Vec<El>>.
```

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS8. Verify WS2/WS3 landed.
8.1/8.3/8.5 are small and independent (TDD each). 8.4 is the design item: write the
design (API + diff semantics + disposal) against rsact-ui's Dynamic/Signal<Vec<El>>
consumers before implementing in rsact-reactive.
```

---

### WS9 — Engine & footprint diet

**Sessions:** 2 (9a early ∥ anything; 9b after WS4) · **Risk:** 9a low / 9b high · **Directions:** D6, D1(a,f).

**9a — collection & flash diet (independent, low risk, do whenever):**

- [ ] 9a.1 `pending_effects` BTreeSet + `run_effects` quicksort → height-bucketed vec (kills the only steady-state write alloc — 2 allocs/112 B per write — AND ~1.6 KiB quicksort + BTree flash).
- [ ] 9a.2 Sorted-vec/linear structures for the tiny collections (pages, fonts, layers); icons: honor per-size features, make size dispatch prunable (46 KiB retention hazard: `CommonIcon::size` runtime-matches every compiled size).
- [ ] 9a.3 Outline untyped cores in rsact-reactive (`add_value_raw(Rc<RefCell<dyn Any>>, kind)` etc. + `#[inline]` typed shims) — collapses the ×9–×14 per-`T` instantiation spread.
- [ ] 9a.4 Logging policy — **pending G12's still-open logging remainder** (decide there first: `log max_level_off` vs defmt); `WidgetFlags` → bitflags; drop `itertools` (4 call sites). (`derivative` replacement is owned solely by 7.4 — not duplicated here.)
- [ ] 9a.5 `subscribe` linear-scan dedup revisit (post-WS2, only if profiling still shows it).
- [ ] 9a.6 **(final sweep) `Color::mix` integer blend**: per-channel f32 math in the AA hot loop (`color.rs:42-47` — its own TODO, with a commented-out integer version at `:33-41`); called per AA pixel. An 8.8 fixed-point blend removes ~6 soft-float ops per blended pixel on FPU-less parts.

**9b — deep engine surgery (own branch, after WS2+WS4; the two highest-risk core changes):**

- [ ] 9b.1 **Drop the per-value `Rc`** (D1 rethink a): take-value-out-during-update (leptos-style) instead of `Rc<RefCell<dyn Any>>` clone per access; fold edge SecondaryMaps into the `Value` slot (per-value RAM 150–250 B → target <100 B).
- [ ] 9b.2 **Pull-phase iterativization** (D1 rethink f, staged): first the cheap tier (iterative Check-walk for clean nodes + frame diet), then incremental height maintenance + height-ordered recompute queue. The ~2 KB/level recursion is a _correctness_ hazard on 4–8 KB embedded stacks. Verification: `DEPTH=10_000` chain test in a stack-limited thread; full bench suite before/after.

**Design sketch (9b.1 — drop the per-value `Rc`, leptos-style):**

```text
BEFORE  Value { value: Rc<RefCell<dyn Any>>, kind, state, height }
        every access: Rc clone (refcount churn) + RefCell borrow + downcast
AFTER   Value { value: Option<Box<dyn Any>>, kind, state, height }
        update(): TAKE the box out of the slot → run callback (storage borrow released,
        re-entrant reads see None → logged degrade) → PUT it back
        −1 heap alloc + −Rc header per node · edge SecondaryMaps folded into the slot
```

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS9 (9a vs 9b are separate
sessions with different risk profiles). 9a: verify current alloc numbers with
benches/allocations.rs first, then diet items with size-probe deltas recorded. 9b:
own branch; verify WS2+WS4 landed; deep_chain test at DEPTH=10_000 in a stack-limited
thread is the acceptance gate; full criterion A/B required.
```

---

### WS10 — Embedded platform layer

**Sessions:** 2 · **Risk:** medium · **Directions:** D4, D1(d) · **Depends on:** WS3 (PageState pruning) loosely; G7, G10.

Why: the things an embedded product actually needs that no core WS covers: interrupt-sourced input, focus traversal for encoder-only devices, app messaging, input drivers.

Work items:

- [ ] 10.1 **ISR-safe reactivity investigation** (G7 reframed 2026-07-07; mailbox REJECTED — no extra primitives): (a) **CS-narrowing** — user closures (probe polls first: renders are the long closures; then memo/effect callbacks in `update()`) run _outside_ the critical section, only individual storage ops guarded; (b) **deferred-effect write variant** for ISR contexts: value write + `mark_dirty` under a short CS, effects queued (existing `defer_effects` semantics as a per-call entry point), flushed at next `tick` — an ISR never runs the effect cascade; (c) **hazard test suite**: ISR-timed writes mid-pull (lost-update via `mark_clean` consuming a concurrent dirty mark — consider a per-node write-generation check before `mark_clean`), re-entrant `mark_dirty` during unguarded closures, assert no effect flush ever happens in ISR context. Document the interim CS-blackout honestly until (a) lands.
- [ ] 10.2 **Focus navigation reinstatement — mechanism pending G10 (postponed).** Candidates on record: tree-order traversal over the arena on PageState (audit recommendation) vs the absolute-index model vs the `event/select.rs` chain stub. Once G10 is decided: wire `interpret_as_focus_move`, make `auto_focus` real, retire the losing models. Acceptance: the 3d_printer page fully drivable with Move/Press only.
- [ ] 10.3 **UiQueue user messages**: `UiMessage::Custom(M)` + `on_message` hook — the driver-level app-messaging channel (widget-level app events are `Event::Custom`, kept per G5; the two channels serve different consumers and coexist).
- [ ] 10.4 **Input-driver layer**: `trait InputSource` + reference quadrature-encoder (debounce/accel) and button impls; one embassy/RTIC example compile-checked in CI (showcase the `async` `Resource` primitive here — 8.6).
- [ ] 10.5 **`on_change` sugar** on value widgets (checkbox/slider/select) unifying with `on_click`; deprecate two of the three goto spellings.
- [ ] 10.6 Event-pass pruning: bounds-check descent for pointer events (cheap intermediate before any spatial index — D2-F6).
- [ ] 10.7 **Keypad support end-to-end (A12):** matrix-keypad `InputSource` impl + key→focus/activation semantics + example, using WS7.1's reserved `Key(u8)` variant (if WS10 runs before WS7.1 lands, add the variant here and coordinate).

**Design sketch:**

```text
10.1 critical-section span (single-thread backend), before → after
BEFORE  cs::with(|| { resolve rt · subscribe · maybe_update · USER CLOSURE (whole render) · mark_clean })
        └── interrupt latency = worst frame time
AFTER   cs[resolve + subscribe + state walk] → closure OUTSIDE cs (each read = its own short cs)
        → cs[mark_clean, guarded by a per-node write-generation stamp]
        hazard: ISR write lands BETWEEN slices → stamp check prevents the lost-update
```

```rust
// deferred-effect write for ISR contexts — existing machinery as a per-call entry point
sig.set_deferred(v);   // value write + mark_dirty under a SHORT cs + queue effects
                       // NO run_effects here — tick() flushes; an ISR never runs the cascade

// 10.4 input-driver boundary
pub trait InputSource<E = ()> {
    fn poll_events(&mut self) -> impl Iterator<Item = Event<E>> + '_;
}   // reference impls: quadrature encoder (debounce+accel), buttons, matrix keypad (10.7)
```

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS10 + gates G7/G10. G10 is
POSTPONED — confirm it has been decided before starting 10.2; if not, skip 10.2 and
report. Verify WS3 landed (stale-ElId pruning) before 10.2. 10.1 is rsact-reactive +
docs; 10.2–10.7 are rsact-ui. Acceptance for 10.2: a test drives a page top-to-bottom
with encoder events only (Move/Press), focus traverses per the G10-decided model.
```

---

### WS11 — Exterior polish (LAST)

**Sessions:** 1–2 · **Risk:** low · **Directions:** D4(quick) · **Depends on:** WS7 (API stable).

- [ ] 11.1 Rewrite all examples to the final API; `cargo build --examples` CI gate (also resolves the examples-required-features breakage).
- [ ] 11.2 README quickstart: 15-line skeleton, component-function pattern, feature-matrix table, tick/tick_time contract, heap sizing with measured numbers, ISR pattern, e-paper pattern.
- [ ] 11.3 Literal ergonomics: `IntoMaybeReactive` impls / `Into`-first setter shapes so `.padding(2)` works without `u32` suffixes (the single most visible user-code wart).
- [ ] 11.4 Naming pass: `SignalMapRefMaybeReactive` → user-facing alias (`IntoText`-ish); constructor-naming consistency (`create_signal` vs `Signal::new`); maintainer's rename TODOs (`Capture`→`Eat` etc.); doc-comment every builder setter with an example.
- [ ] 11.5 Label `Cow<'static, str>` storage (kills per-label String alloc).
- [ ] 11.6 Publish the observability story (mermaid graph export, `what_changed`, profile, DevTools) — it's a hidden selling point.
- [ ] 11.7 Size/RAM numbers in README from the WS0 CI (the LVGL-comparison headline; target claim per the decided floor: a 10-widget mono UI fits the Blue Pill — framework ≤ ~48 KiB flash, ≤ 20 KiB RAM total). Best sourced from WS17's measured hardware results if it has run.
- [ ] 11.8 **Rustdoc ratchet (A15):** `deny(missing_docs)` on lib crates (ratcheted allow-list), doc example on every public item, `doc(cfg)` feature annotations, intra-doc-links pass.
- [ ] 11.9 **mdbook guide (A16):** architecture tour, reactivity mental model (signal/memo/effect/probe), embedded bring-up (display driver + input + heap sizing), theming via the styler registry, writing a custom widget, the minimal/e-paper pattern. Numbers cited from WS17.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS11. Verify WS7 landed (API
stable — otherwise STOP and report). Execute the polish list; every README/guide number
must come from the WS0 CI or WS17's hardware results, never estimates (11.7/11.9 note
which). Examples must compile in CI before this WS closes.
```

---

### WS12 — API freeze & release engineering (B1)

**Sessions:** 1–2 · **Risk:** low · **Depends on:** WS7 + WS11 (surface final) · The bridge from "stabilized" to "shipped".

- [ ] 12.1 Prelude curation: one blessed prelude per crate; audit what's `pub` that shouldn't be (overlaps WS11.4's naming pass — sequence after it).
- [ ] 12.2 `#[non_exhaustive]` pass over public enums/structs likely to grow (audit: ~1060 public items at 0.1.0; respect the maintainer's earlier note that it also affects constructibility — decide per type, not blanket).
- [ ] 12.3 `cargo-semver-checks` in CI + MSRV policy (pinned + tested).
- [ ] 12.4 CHANGELOG discipline + `cargo-release` + crates.io publish order (rsact-reactive, rsact-render, rsact-macros, rsact-tiny-icons, rsact-ui, `rsact` facade) + 0.x versioning cadence. **Final-sweep blocker: rsact-tiny-icons cannot be published as-is** — its build script writes into the package dir (`build/main.rs:55` → `src/rendered/`, gitignored) with zero `cargo:` directives: registry builds can't mutate the package, rerun-if-changed is missing (fingerprint churn — the script mutates files it depends on), and generated content varies by feature in one shared dir. Fix: generate into `OUT_DIR` + `include!`, or commit the generated code.
- [ ] 12.5 Crate metadata polish (descriptions, keywords, docs.rs feature config).

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS12. Verify WS7 and WS11 landed
(API surface final). Execute 12.1–12.5; every decision that narrows the public API gets a
one-line rationale in the roadmap. Do not publish without maintainer confirmation.
```

---

### WS13 — Views as widget builders (B2 — maintainer RFC from EVOLUTION.md)

**Sessions:** 2 (design + prototype, then rollout gate) · **Risk:** medium-high (design) · **Depends on:** WS7 (Widget trait settled) · Goal: stop carrying build-only props through the widget's whole lifecycle — RAM per widget + a cleaner `build`.

- [ ] 13.1 Design doc: builder/runtime split options — separate Builder types per widget vs a generic builder layer vs build-consumed fields (`Option::take` at build). Note the existing hint: `El`'s two-state `New(ElData)/Stored` enum is already half of this idea. Quantify candidate RAM savings with the 0.4 probe before choosing.
- [ ] 13.2 Prototype on two widgets (Button, Flex) + measure (RAM/flash/ergonomics diff).
- [ ] 13.3 **Rollout decision gate** — maintainer sign-off on the measured prototype before any fleet conversion.
- [ ] 13.4 Fleet conversion (if approved) + `#[derive(View)]` adjustments.

**Design sketch (the three 13.1 candidates — decide by measured RAM, not taste):**

```rust
// (a) two types per widget:
struct ButtonBuilder { padding: Padding, on_click: Option<F>, .. }   // dies at build
struct Button        { state: Signal<ButtonState>, .. }              // lives in the arena
// (b) build-consumed fields (cheapest to adopt):
struct Button { padding: Option<Padding>, /* Option::take()n at build; None thereafter */ }
// (c) generic builder layer: the View tree stores builders; the arena stores runtime widgets.
// The codebase already hints at the split: El::New(ElData) vs El::Stored { id, layout }.
```

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS13 + the "Views as widget
builders" RFC section in EVOLUTION.md. Verify WS7 landed. This is design-first: write
13.1 with measured numbers, prototype 13.2, then STOP for the 13.3 maintainer gate.
```

---

### WS14 — DevTools v2 (B3 — maintainer idea from EVOLUTION.md)

**Sessions:** 2–3 · **Risk:** medium · **Depends on:** WS8 (`what_changed`), WS11; A10's snapshot infra helps.

- [ ] 14.1 DevTools UI rendered **with rsact itself** in its own simulator window (host) — dogfooding.
- [ ] 14.2 Host↔device debug protocol design: transport-agnostic (RTT/serial), feature-gated device probe exposing runtime profile, node graph, layout tree (the EVOLUTION "debug the device from the computer" idea — feature sets differ per target, hence a protocol, not a shared binary).
- [ ] 14.3 Integrate `what_changed`, the mermaid graph, and draw-call counters into the panel.
- [ ] 14.4 Hover-inspect parity with the current in-app overlay, then retire the overlay.

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS14. Verify WS8.5 (what_changed)
exists. 14.2 is the design item — protocol doc first, reviewed before implementation.
```

---

### WS15 — Font stack maturation (B4)

**Sessions:** 2 · **Risk:** medium · **Depends on:** WS5 (measure paths stable; co-design with 5.4's cache).

- [ ] 15.1 Measure-API audit: a single `FontProvider`-style trait boundary so new providers slot in without touching layout.
- [ ] 15.2 `fontdue` feature (verify no_std+alloc reality) for scalable fonts + flash/RAM measurement vs bitmap fonts (EVOLUTION TODO).
- [ ] 15.3 u8g2 soft-wrap gap (`font/fixed.rs:103`).
- [ ] 15.4 `TextStyle` widget subsuming the `FontProps` TODO (`font/mod.rs`).
- [ ] 15.5 Build-time glyph-subsetting notes/tooling (per-app font subsets for flash budgets).

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS15. Verify WS5 landed and read
5.4's cache design first (the measure boundary must serve both). 15.1 before 15.2.
```

---

### WS16 — Desktop/std tier: command-buffer renderer (B5 — un-parked)

**Sessions:** 3+ · **Risk:** high · **Depends on:** WS7 · Strictly feature-isolated: the embedded profile must show **zero** size delta.

- [ ] 16.1 Retained primitive IR design (seed vocabulary: `DrawCommand`/`DrawQueue` in `widget/canvas.rs`).
- [ ] 16.2 Per-node command buffers + diff → pixel-precise damage (the D5 alt-b design, viable on desktop RAM).
- [ ] 16.3 std renderer backend over tiny-skia consuming the IR.
- [ ] 16.4 Multi-renderer validation (G4's escape valve): one host binary driving simulator panel + a second view — the EVOLUTION devtools-mirror dream becomes testable.
- [ ] 16.5 Embedded regression guard: minimal-profile size CI row asserted unchanged (the feature-isolation proof).

**Design sketch:**

```rust
// 16.1 IR — canvas.rs's DrawCommand vocabulary, promoted and extended
enum DrawCommand<C: Color> { Rect{..}, RoundRect{..}, Arc{..}, Line{..}, Text{..},
                             Image{..}, PushClip(Rect), PopClip }
// per-node retained buffer inside a memo:
//   widget render ─▶ Memo<Vec<DrawCommand<C>>> ─▶ diff(old, new) ─▶ damage rects ─▶ replay
// desktop RAM makes double-buffered diffing viable; the MCU tier never compiles this (16.5)
```

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS16 + the parked-register entry
it supersedes. Verify WS7 landed. 16.1 is a design doc first; 16.5's zero-delta guard is
non-negotiable and lands with the first code commit, not the last.
```

---

### WS17 — Hardware validation & the LVGL comparison (B6)

**Sessions:** 2 (bench work, partly hands-on maintainer time) · **Risk:** low-medium · **Depends on:** WS0 + WS6 (earliest start) · Feeds 11.7/11.9's published numbers. This is where "beat LVGL" becomes evidence.

- [ ] 17.1 Reference firmware: **Blue Pill + SSD1306 128×64** (the G3 mono pair) — repo under `examples/` or a sibling dir, built in CI.
- [ ] 17.2 Reference firmware: **Black Pill + ST7789 240×240** (the G3 color pair) — strip mode + regions flush exercised for real.
- [ ] 17.3 `probe-rs` flash + RTT smoke script (repo tooling; not necessarily CI-gated).
- [ ] 17.4 Measured comparison methodology vs LVGL: the same UI implemented on both, published numbers — flash, RAM, change-frame ms, idle current.
- [ ] 17.5 Results flow into README (11.7), the mdbook (11.9), and the Baselines table here.

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS17 + gate G3 (reference pairs).
Verify WS0 (thumb builds) and WS6 (regions/strip) landed. Hardware-in-the-loop steps that
need a physical board get written as runnable scripts + docs for the maintainer to execute.
```

---

### WS18 — Fixed-capacity / no-alloc storage mode (B7)

**Sessions:** 2–3 · **Risk:** high · **Depends on:** WS9b — and **gated on its numbers**: only pursued if the Rc-free storage still isn't enough for the smallest tier.

- [ ] 18.1 Const-generic slab storage behind a `fixed-capacity` feature (Storage's narrow internal API was deliberately kept to make this possible).
- [ ] 18.2 Heapless edge lists (const max fan-out; overflow policy = log-degrade, never panic).
- [ ] 18.3 Allocator-free minimal-tier probe app + its own size/RAM CI row.
- [ ] 18.4 Capacity-planning contract documentation (peak-sizing philosophy — closes the loop with 4.6).

**Design sketch:**

```rust
// same narrow Storage API, allocator-free backing:
pub struct FixedStorage<const NODES: usize, const FANOUT: usize> {
    slots: [Slot; NODES],                  // Slot = { value: union-of-known + dyn escape, gen }
    edges: [heapless::Vec<ValueId, FANOUT>; NODES],
}
// overflow policy: log-degrade (never panic); NODES/FANOUT are the compile-time contract
```

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS18. Verify WS9b landed and check
its measured numbers first — if the smallest tier already fits, report that and STOP (this
workstream is conditional by design).
```

---

## Parked / rejected register (do not resurrect without new evidence)

- **Per-node layout memos** (D3 candidate b): 350–500 B/node graph freight — disqualified on M0 RAM.
- **Layered `rsact-core` crate split** (D7 option b): hard DCE guarantee not worth 2× maintenance + binding-generic API leak; `rsact-render` remains the one true reactivity-free layer.
- **Command-buffer rendering + diff** (D2 opt 2 / D5 alt b): RAM-prohibitive on MCU; the right future for a desktop/`std` tier and the only justification for multi-renderer binaries — keep the `DrawCommand` vocabulary in `canvas.rs` as seed. **UPDATE 2026-07-07: promoted to WS16** (desktop tier, feature-isolated, embedded zero-delta guard).
- **Versioned-reads render gate** (D5 alt d): worse idle scaling as the default; candidate for a measured minimal-mode experiment only.
- **`pending_effects` Vec+dedup rework** (old Phase 2.3): superseded by WS9a.1's height-bucketed design.
- **ISR write-mailbox (`IsrSetter` + SPSC queue)**: REJECTED (2026-07-07, maintainer — reactive ops must stay small; no extra primitives). Replaced by the G7 investigation: CS-narrowing + deferred-effect writes (WS10.1). CS-narrowing itself is thereby **un-parked**.
- **Multi-runtime public API**: recommend hiding (G-adjacent; maintainer TODO at `runtime.rs:66`); `Runtime::enter` guard is the future shape if multi-UI-on-host returns.
- **Spatial hit-test index**: WS10.6's bounds-pruning first; index only if profiling demands.

## Proposed work-item backlog (preserved; not commitments)

Menu generated 2026-07-07 from audit-report material not yet scheduled + the post-plan horizon. **ADOPTED** items were promoted into workstreams (pointer given); **BACKLOG** items are preserved for later promotion — they carry no commitment. When promoting one, give it a proper charter in its target WS.

| ID  | Item                                                                                                                                          | Status               |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| A1  | QEMU thumbv7m test run in CI (semihosting runner) — catches target-only breakage (atomics, alignment, stack) continuously                       | BACKLOG              |
| A2  | Miri pass on rsact-reactive in CI (unsafe dispose/storage soundness net)                                                                        | BACKLOG              |
| A3  | Unwrap/expect lint ratchet + burn-down                                                                                                          | ADOPTED → WSi        |
| A4  | Arena invariant `debug_assert`s (single-parent, `set_single_child`)                                                                             | BACKLOG              |
| A5  | Probe docs + third-party render-engine pattern                                                                                                  | ADOPTED → WS2.5      |
| A6  | Storage capacity management (high-water metrics + policy)                                                                                       | ADOPTED → WS4.6      |
| A7  | Persistent text-measure cache                                                                                                                   | ADOPTED → WS5.4      |
| A8  | Non-blocking flush — sans-IO chunkable regions (no async deps)                                                                                  | ADOPTED → WS6.7      |
| A9  | Display rotation/orientation                                                                                                                    | ADOPTED → WS6.8      |
| A10 | Golden-image render tests (tiny-skia snapshots + draw-call goldens)                                                                             | ADOPTED → WS6.9      |
| A11 | Renderer parity audit EG vs tiny-skia (EVOLUTION TODO)                                                                                          | ADOPTED → WS6.10     |
| A12 | Keypad support end-to-end (`Key(u8)`)                                                                                                           | ADOPTED → WS10.7     |
| A13 | Touch gesture primitives (tap/long-press/drag) as an input-layer helper                                                                         | BACKLOG              |
| A14 | Power-idle pattern: next-animation-deadline API + documented WFI loop (`render() -> bool`)                                                      | BACKLOG              |
| A15 | Rustdoc ratchet (`missing_docs` deny + examples + `doc(cfg)`)                                                                                   | ADOPTED → WS11.8     |
| A16 | mdbook guide                                                                                                                                    | ADOPTED → WS11.9     |
| A17 | Simulator input fidelity (final sweep): exact-equality keymod match breaks Ctrl+Shift+D-style combos; arrows fire only on KeyUp — no key-repeat, can't hold-to-scroll (`event/simulator.rs:17-24`) | BACKLOG              |
| B1  | API freeze & release engineering                                                                                                                | ADOPTED → WS12       |
| B2  | Views as widget builders (EVOLUTION RFC)                                                                                                        | ADOPTED → WS13       |
| B3  | DevTools v2 (separate window + host debug protocol)                                                                                             | ADOPTED → WS14       |
| B4  | Font stack maturation (fontdue, u8g2 wrap, TextStyle, subsetting)                                                                               | ADOPTED → WS15       |
| B5  | Desktop/std command-buffer tier                                                                                                                 | ADOPTED → WS16       |
| B6  | Hardware validation & LVGL comparison                                                                                                           | ADOPTED → WS17       |
| B7  | Fixed-capacity no-alloc storage mode                                                                                                            | ADOPTED → WS18       |
| C1  | embassy/RTIC integration crates (async tick adapter, executor examples)                                                                         | BACKLOG (post-1.0)   |
| C2  | Display-driver adapter kit — mipidsi/ssd1306/epd-waveshare glue for the regions API                                                             | BACKLOG (post-1.0)   |
| C3  | Multi-UI / multi-display (`Runtime::enter` on host; dual-panel on device)                                                                       | BACKLOG (post-1.0)   |
| C4  | Theme-designer playground (web tool exporting Rust theme code)                                                                                  | BACKLOG (post-1.0)   |
| C5  | Mutation testing (`cargo-mutants`) + fuzz corpora for text measurement and event streams                                                        | BACKLOG (post-1.0)   |
| C6  | i18n/text shaping — desktop tier only, explicitly out of MCU scope                                                                              | BACKLOG (post-1.0)   |

## Cross-cutting invariants (any WS must respect)

- **I1–I7 (render gating, from D5):** identity aliasing impossible by construction; records live/die with their element; nothing consumes a part's dirtiness except executing it; deps re-tracked per execution; parent redraw ⇒ child overdraw, child dirty ⇒ page dirty, O(1) idle gate survives; 0 idle allocs / O(1) change allocs; observer-cell restore stays panic-safe.
- **Layout stop rule (from D3):** upward propagation may stop at a node only if BOTH resolved `outer_size` AND `min_size` are unchanged under unchanged inputs (min_size feeds parent wrap decisions and min-clamps over max in fluid children).
- **Arena↔layout structural parallelism** is load-bearing (positional zip in event+render passes) until WS5.1 makes identity explicit; divergence must degrade (log), never panic (WS3.5).
- **"UI must never panic"** (EVOLUTION.md): every new code path logs and degrades; no new `unwrap` on render/event/nav paths; prefer exposing `try_*`/`Option` to the user where possible (WS1.8).
- **Crate encapsulation (maintainer, 2026-07-06):** rsact-reactive stays UI-vocabulary-free — no `ElId`/part/page knowledge in the core; ownership maps for reactive handles live with their owner (rsact-ui), never in a core registry.
- **No feature-flag sprawl:** the only sanctioned axes are storage backend (std | single-thread | unsafe-single-thread; `fixed-capacity` joins this axis if WS18 runs), math backend (libm | micromath — WS0.1), render backend, font provider, extras (simulator/anim/tiny-icons/debug-info), dev-only `test-utils` (never in a production graph — enabled via dev-dependencies), plus `incremental-layout`/`layout-counters` while maturing. Anything else must be architectural (pay-per-use by construction).
- **`Note:`/`TODO:` comments are never deleted** unless the referenced work is 100% done; EVOLUTION.md checklist protocol applies on every pass.
- **Serial tests always:** `-- --test-threads=1`; host tests need `--features std`.

## Baselines & verification commands (audit measurements taken 2026-07-05 at dc9bf3e+dirty; WS0.1–0.4 landed 2026-07-07 as 795d6ba…5f95e8d — re-verify numbers against the 0.3 snapshot tool)

```sh
cargo test -p rsact-reactive --features std -- --test-threads=1   # 54 pass / 2 known-fail:
#   maybe::tests::static_wrapper                      → WS4's acceptance test (must flip to pass)
#   runtime::tests::observe_recreates_disposed_child_observer → rewritten in WS2 per G2
cargo test -p rsact-ui --features std -- --test-threads=1          # 44 / 0
cargo test -p rsact-render --features "std,embedded-graphics,tiny-skia" -- --test-threads=1  # 6 / 0
cargo bench -p rsact-reactive --features std                       # criterion + allocations harness
cargo hack check --feature-powerset --no-dev-deps --all \
  --mutually-exclusive-features std,single-thread,unsafe-single-thread \
  --at-least-one-of std,single-thread,unsafe-single-thread
```

Measured reference numbers (recorded by the 2026-07-05 audit; re-verify before relying on them):

| Metric                                   | Value                                                                                                                                      |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Reactive core, thumbv6m opt-z fat-LTO    | `.text` 16.8 KiB (probe total 24.9), statics ~56 B, 0 steady-state allocs                                                                  |
| Engine allocs/op                         | create_signal 1/144 B · create_memo 3/319 B · create_effect 3/702 B · write w/1 effect **2/112 B** (BTreeSet churn → WS9a.1)               |
| Idle frame (page gate)                   | 16 ns, 0 allocs (host); est. ~25 µs on M0@48 MHz                                                                                           |
| Change frame @100 parts (today)          | ~2–4k cycles/part bookkeeping ≈ 4–8 ms on M0 + ~100 malloc/free (→ WS2 target: 0 allocs, ≥3× faster)                                       |
| Static UI cost                           | ~3 nodes / ~1.7 KB heap per label; audit's mixed 10-widget bin = 46 nodes, **31 inert** (0.4's canonical 10-label page = 42 nodes — different probes) (→ WS4 target: ≈0 nodes) |
| Per reactive value (32-bit)              | ~150 B signal / ~250 B memo incl. edge maps (→ WS9b.1 target < 100 B)                                                                      |
| 10-widget page RAM (32-bit est.)         | 13–17 KiB + framebuffer + ~8 KiB first-render transient                                                                                    |
| Framebuffer                              | 240×240 RGB565 = 112.5 KiB heap (→ WS6.4 strip mode < 20 KiB); mono 128×64 = 1 KiB                                                         |
| Host flash proxy (opt-z)                 | rsact stack ≈ 106 KiB (5 widgets) / 129 KiB (10); +2.4 KiB per widget-type instantiation; thumb estimate 70–90 KiB                         |
| Layout, one label change on 30-node page | ~180 node computations, 60–120 text measures, full-tree PartialEq, full repaint (→ WS5 target: 1 visit / ≤2 measures / label-only repaint) |
| Competitive frame                        | LVGL ~50–120 KiB flash, 8–48 KiB RAM; Slint MCU ~300 KiB class                                                                             |
| Floor targets (decided 2026-07-06)       | **Blue Pill** F103/thumbv7m (64–128 K flash, 20 K RAM): 10-widget mono UI must fit (framework ≤ ~48 K flash, ≤ 20 K RAM total). **Black Pill** F401CE/thumbv7em-hf (512 K, 96 K): color-QVGA via WS6 strip mode. thumbv6m compile-only |

## Source reports

The seven full analysis reports (with file:line evidence, alternatives considered, and per-direction open questions) were produced in the 2026-07-05 audit session. Their substance is condensed into this roadmap; the interactive version of this roadmap (browsable workstreams + copyable session prompts) is published as a Claude artifact — see MEMORY/`rsact-evolution-roadmap` note for the URL.
