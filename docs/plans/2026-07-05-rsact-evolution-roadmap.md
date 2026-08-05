# rsact Evolution Roadmap

**Target class (decided 2026-07-06):** the floor is the **Blue Pill** (STM32F103, Cortex-M3/thumbv7m: 64–128 K flash, 20 K RAM); the comfortable tier is the **Black Pill** (STM32F401CE, Cortex-M4F/thumbv7em+FPU: 512 K flash, 96 K RAM). Cortex-M0/thumbv6m is a dream tier, kept **compile-only** in CI (portable-atomic makes that ~free) with no size budgets asserted. Note the relaxation changes little in substance: Blue Pill's 64 K flash is still below today's 70–90 K framework estimate (WS4/WS9 stay load-bearing), and Black Pill's 96 K RAM still cannot hold a full 240×240 RGB565 framebuffer (WS6.4 partial-buffer mode stays existential for color — **the mechanism changed 2026-08-01 from screen strips to damage tiles; the conclusion did not**).

> **For agentic workers:** each workstream (WS) below is designed to run as its **own Claude Code session** (some as 2–3 sessions — noted per WS). At the start of a WS session: (1) read this file's WS section + the "Cross-cutting invariants" and "Baselines" sections; (2) **verify the current state first** — earlier sessions may have shifted the ground (re-run the baseline test/bench commands, re-read the cited code); (3) use the superpowers:writing-plans skill to expand the WS charter into a bite-sized TDD plan before touching code; (4) when a work item lands, mark its checkbox here `[x]` and record the commit hash; never redo a checked item; (5) follow the EVOLUTION.md TODO protocol (report conflicts, don't silently fix); (6) `> comment:` blocks are the maintainer's live review notes — **never delete or resolve them yourself**; an item carrying an unresolved comment is still under discussion and is **not ready to execute** — skip it and note that in your report. Items conflicting with reality get reported back, not forced.

**Goal:** evolve rsact into the lightest credible reactive GUI framework for embedded — beating LVGL on RAM/Flash for its target class, without feature-flag sprawl and without a v2.0 that eats 10× more memory.

**Philosophy (maintainer's):** polish from the deep first, moving up the abstraction stack. Core changes may force full API reimplementation, so API-surface work is _decided early on paper_ but _executed late_, batched into at most two breaking releases. Exterior polish (docs, examples, naming) is scheduled last.

**Method:** seven parallel adversarial deep-analyses (2026-07-05) over the working tree — reactive core (D1), UI core design (D2), incremental layout (D3), embedded-dev ergonomics (D4), reactive rendering (D5), RAM/Flash footprint (D6), minimal-mode architecture (D7) — then cross-direction reconciliation. Several findings were **empirically confirmed** with scratch harnesses and real thumbv6m builds; measured numbers are recorded in "Baselines" below. Prior context: the 86-finding audit (report artifact `ed1e601a`; 27 done / 6 partial as of this date), phases 1–4 + storage-soundness already landed.

---

## The one-page picture

### What is already strong (measured, defend it)

- **The reactive core is genuinely small on target:** 16.8 KiB `.text` on a real thumbv6m build (audit's minimal probe), ~56 B of statics, **zero steady-state allocations** after warm-up. Now continuously measured in-repo (0.3 Layer-2 size-probe, opt-z fat-LTO incl. cortex-m-rt + embedded-alloc): reactive bin `.text` ≈ 27.4 K (thumbv7m) / 27.5 K (thumbv6m); ui bin ≈ 79 K / 84 K — `.text/.rodata` are the flash signal (`.bss` is the probe's own heap buffer).
- **Idle frames are already free:** 16 ns / 0 allocs per no-change frame (host); the page-level observe gate works. "Reactivity overhead" is _not_ an idle-CPU problem.
- **The architecture is right for embedded:** consumer-side change detection + per-part render observers + `render() -> bool` is what people hand-build on LVGL. Monochrome-first theming, encoder-first events, packed 1-bit framebuffer, MIT license, mermaid graph export — these are real differentiators.

### The six structural problems (what the whole plan is organized around)

| #   | Problem                                                                                                                                                                                                                                                                                                                                                         | Evidence owner |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| P1  | ✓ **RESOLVED by WS0 (2026-07-07).** ~~The framework can't build for its own target.~~ std-only f32 math → `FloatExt` backends (0.1); ARMv6-M `fetch_add` → portable-atomic (0.2); size numbers now measured in-repo per commit (0.3 size-probe + CI).                                                                                                             | D4, D6         |
| P2  | ✓ **RESOLVED by WS1.1 + WS2 + WS3 (2026-07-08).** ~~Nothing is ever disposed.~~ core scope chain fixed (WS1.1); probes dispose with their element/page + `clear_sources` per poll (WS2); **per-page scope disposes everything a page built on navigation (WS3.1), killing the disposed-arena delayed panic + the navigation leak; PageState pruned on element removal (WS3.3); `render_once` one-shot for static displays (WS3.4).** Subtree disposal (WS3.2) verified already subsumed by effect-owned cleanup + the page scope — no per-subtree machinery needed. `leak_report` (WS3.0a) attributes any residual leak to its creation site. | D1, D2, D5     |
| P3  | ✓ **RESOLVED by WS4 (2026-07-09, PR #11).** ~~Statics pay reactive freight.~~ `Inert(T)` is stored inline (zero runtime nodes; blanket `Copy` dropped per G1, `static_wrapper` passes); builder literals mint no nodes (4.2 verified); widget constructor signals audited/de-greeded (4.5, bar the `icon.rs` repair); storage high-water metrics landed (4.6). Remaining tail rides WS5: the ~9 app/page singletons (4.3 relocated) dissolve with the RenderShared+memo rework.                                                                                                                 | D7, D6         |
| P4  | ✓ **RESOLVED by WS1.7 + WS2 (2026-07-07).** ~~Change-frames burn bookkeeping; identity is fragile.~~ `format!` killed (1.7); the hash registry is **deleted** — render identity is the owned `Probe` handle in `ElState`/`Page`, disposed with its element, aliasing impossible by construction; `ahash` gone from no_std.                                        | D5             |
| P5  | **One change relayouts and repaints everything.** Whole-page `Memo<LayoutModel>`; `min_size` recursion makes it O(N·D); no text-measure caching; `force_redraw.set(true)` _inside_ the memo forces every widget to repaint even when the tree is identical; and the flush then streams the **entire viewport per-pixel** to the display.                        | D3, D4         |
| P6  | **Flash diet — MOSTLY RESOLVED by WS9a (2026-07-09, PR #5).** BTree collections → sorted vecs ✓ (9a.2); per-`T` reactive shims outlined ✓ (9a.3, −2.8 KiB `.text`); icon per-size features + prunable dispatch ✓ code-complete (9a.2; ~46 KiB win realized once `icon-libs` regen lands); `defmt` forwarded ✓ (0.7). Remaining: widget-local type params `Dir`/`V` ≈ 2.4 KiB per instantiation (7.2, folded into WS13′); logging sliver blocked on G12. The viral `W: WidgetCtx` is an API/coupling problem, **not** a measured flash problem (exactly one `Wtf` per firmware). | D6, D2         |

### Workstream map (deep → surface)

```
            ┌─────────────────────────────────────────────────────────────┐
 LAYER 0    │ WS0 ✓ DONE (0.1–0.9)       WS1 ✓ DONE    WS1b ✓ DONE        │  ← layer 0 complete
            │                                                             │
            └───────────────┬──────────────────────┬──────────────────────┘
                            │                      │
 LAYER 1    ┌───────────────▼──────────┐  ┌────────▼─────────────────────┐
 (engine)   │ WS2 ✓ DONE — probe/render│  │ WS9a engine diet: collections │
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
            │ ∥ WS19 website: VitePress + Pages (independent — any time)    │
            └───────────────────────────────────────────────────────────────┘
```

**Suggested execution order:** WS0 ✓ ∥ WS1 ✓ ∥ WS1b ✓ → WS2 ✓ → (WS3 ✓ ∥ WS4 ✓ ∥ WS9a ✓) → **WS13′ ✓ COMPLETE** (13.1–13.4 all merged; fleet split via PR #19, 2026-07-13) → WS5 → WS6 → WSi → WS7 (remainder 7.1/7.3/7.4/7.5) → (WS8 ∥ WS10 ∥ WS9b) → WS11 → WS12 → (WS14 ∥ WS15 ∥ WS16 ∥ WS18) _(WS13 pulled early — see 2026-07-09 decision)_. WS17 may start any time after WS6 — ideally before WS11.7 needs its README numbers. **WS19 ✓ v1 SHIPPED** (19.1–19.5 + 19.7 + 19.8 A/B live at hazer-hazer.github.io/rsact, site brand applied via PR #16; only the "later" 19.6 items — blog/custom domain/og-cards — remain, any time). **Now actionable (2026-07-13): WS5 — the layout workstream is UNBLOCKED** (WS13′ fully merged via PR #19: fleet split, icon repaired, `tiny-icons` compiles again; suites 77/0 ui · 76/0 reactive · zero known-fails). Launch order per the WS5 section: **5.0 quick wins may run first or parallel** (independent since WS0.5); **5.1 layout-off-graph is the core session** — consumes WS4.0's Layout analysis + the fresh `Build<W>` transform protocol (reactive-layout bindings defer to build time; the builder is their transient home), **+ the relocated 4.3 singleton de-greed rides here**. Re-verify the WS5 section's pre-resequencing assumptions against current master before executing (its items were written before WS13′ landed). **PR #6 (WS9b design, DRAFT) is UNBLOCKED by events:** WS4 merged (PR #11) = Decision 1's recommended option (a) satisfied; before 9b code, answer **Decision 2** (9b.1 read-path fork — re-evaluate the hybrid-iii recommendation against post-WS4 storage, where inert values no longer flow through reads) and have the returning session re-verify the design doc + rebase onto current master. Its execution slot is unchanged (own branch; both WS2+WS4 dependencies are now merged — may run early if the maintainer chooses). Parallel sessions need separate worktrees. **WS20 (reactive node-storage AoS — fold `subscribers`/`sources`/`owned` into the `Value` node) is a new expansion-layer item gated "do after WS5"** — it must re-baseline against WS5's node/edge-count changes and coordinate with WS9b's read-path fork (Decision 2) so the storage read path is not churned twice.
WS7's _decisions_ are locked at Gate time (now); only its _execution_ is late. 7.2's cheap `Dir`/`V` de-genericization may ride along with WS2/WS4 if convenient — it's zero-user-impact. (7.1 no longer qualifies: G5 keeps `Event::Custom`, and its remaining scope carries a breaking rename plus a G4-dependent default.)

---

## Decision gates (answer these; each gates the marked WS)

Recommendations reflect the agents' converged analysis; where two directions disagreed, the resolution is noted.

- [x] **G1 (gates WS4) — DECIDED IN DIRECTION (2026-07-06): yes, drop blanket `Copy` on `MaybeReactive<T>`/`Inert<T>`** (`Copy` only for `T: Copy`, `Clone` otherwise — the `MaybeSignal::Inert` precedent). Execution is gated on the **WS4.0 analysis** (maintainer-authored): `Layout` cannot be inlined freely (needs shared identity; `Widget::layout` may need to return `&Layout`; `Copy` removal from `Layout` for safety) — all `MaybeReactive` usages rethought with Copy-loss in mind before 4.1 lands. **EXECUTED (2026-07-09, WS4.1 `33e7ad8`):** blanket `Copy` dropped (`Copy` iff `T: Copy`); `Memo::Inert` removed so `Memo<T>` stays unconditionally `Copy`; beyond the compiler-found `Copy`-inner sites the only forced Copy-loss was the layout _payload_ tower (`ContentLayout`/`FlexLayout`/`LayoutKind`/`LayoutData` → `Clone`) + a `W::Stylist: Clone` bound — the `Layout` handle type and WS5.1 redesign stay untouched (`Layout` analysis delivered as WS5.1 design input). `static_wrapper` passes.
- [x] **G2 (gates WS2) — DECIDED (2026-07-06): no** — children re-run iff their own deps changed or the caller passes `force` (what today's working render path does via `parent_dirty`). The failing test `observe_recreates_disposed_child_observer` encodes the _old_ cleanup-dispose-revive contract and gets **rewritten**, not satisfied. (D1's "parent forces children" rejected: it's exactly what the explicit `force` param already expresses.)
- [x] **G3 (gates WS6) — DECIDED (2026-07-07):** measurement stays **display-agnostic**; the reference displays for the floor pair are **128×64 mono OLED (SSD1306-class) on Blue Pill** and **240×240 16-bit ST7789 on Black Pill**. Consequence: 240×240 RGB565 = 112.5 KiB > Black Pill's 96 K RAM ⇒ **WS6.4 strip/partial modes are existential** for the color reference; and ST7789 has its own GRAM with window addressing, so the WS6.3 regions API maps directly onto partial window writes. **Refined 2026-08-01:** the partial mode is **damage tiles**, not screen strips (see 6.4) — and note the correlation that makes this cheap: buffer pressure and addressing freedom move _together_. The controllers with awkward addressing (SSD1306/SH1106, page-based) hold 1 KiB framebuffers and need no partial mode at all; the ones that force it (RGB565 TFT ≥240×240) have free rect windows via CASET/RASET. The genuine both-constraints case is large mono e-paper, handled in 6.5.
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

Why (historical): rsact-ui could not link for any thumb target (P1), so nothing about the embedded goal was falsifiable. **STATUS 2026-07-07: WS0 IS COMPLETE (0.1–0.9 all landed, incl. the 0.7 review fixes, the 0.8 post-commit hook, and 0.9 CI).** Remaining loose ends live elsewhere by design: `anim` feature-gating (deferred — needs the num→FloatExt unification), root-facade passthrough (WS12.5), thumbv7em-hf size row + budget thresholds (WS10-adjacent), and 0.9b's orphan-branch/Pages orchestration needs a real GitHub push to observe end-to-end.

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

- [x] **0.8 Commit-time metrics automation (maintainer-proposed 2026-07-07; design agreed)** — landed `4b965d2`: `.githooks/post-commit` + `metrics-probe hook-install` (sets `core.hooksPath .githooks`), `metrics/hook.log` git-ignored, documented in README + metrics/README.md. Verified: commit returns instantly (record runs detached); a broken-build commit still commits (hook logs + exits 0); keys `<rev>.json` on a clean tree (`-dirty` when the tree is dirty — as it was here, the roadmap-editing workflow dirties the tree mid-record). Design as specified: a **post-commit** hook — NOT pre-commit: at pre-commit time HEAD is still the parent and the tree is dirty, so the snapshot would be keyed to the wrong hash (`-dirty`); post-commit sees the new hash and a clean tree, matching the tool's own keying. Behavior: runs `metrics-probe record` **Layer-1 only** (no `--sizes` — opt-z thumb builds are too slow for commit cadence; sizes stay on-demand + CI), **in the background** (commit returns instantly; output → `metrics/hook.log`), **never blocks or fails the commit** (metrics observe, they don't gate — the 0.4 regression test is the hard gate; CI PR deltas are the review surface), **skips** during rebase/cherry-pick and when HEAD's snapshot already exists. Distribution: committed `.githooks/post-commit` + one-time `git config core.hooksPath .githooks` documented in README + `metrics/README.md` (optionally a `metrics-probe hook-install` subcommand that sets the config). Caveat on record: snapshots are git-ignored, so the hook completes the *local* timeline only — the durable shared record remains 0.3's CI half; the pair together is the full answer. Acceptance: two consecutive commits → two keyed snapshots, zero perceptible commit latency; a broken-build commit still commits (hook logs and exits 0).

- [x] **0.9 CI — the actual automation (chartered 2026-07-07 after the maintainer asked "where does it run?"; honest answer: nowhere — the repo has NO `.github/workflows` at all).** Two workflows, in order:
  - [x] **0.9a `ci.yml` — baseline CI (prerequisite):** — landed `e10e522` (scripts verified green; Actions needs a real push to observe) on push + PR — the four test suites (commands per the WS0-gotchas notes: ui-lib needs `--lib --features "std,embedded-graphics"`; reactive's 2 known-fails need explicit handling so the job is green-by-baseline, red-on-new-failure), per-crate cargo-hack powersets (exact commands in `docs/features.md`; includes the new libm/micromath group after 0.7a), thumbv7m rsact-ui build check. Cache cargo; target < ~10 min wall.
  - [x] **0.9b `metrics.yml` — the CI half of 0.3:** — landed `88e2a30` (YAML + record/diff verified locally; the orphan-branch/Pages/sticky-comment orchestration + one-time Pages setting `metrics-data` branch need a real push) _on master push_: `metrics-probe record --sizes` → commit the snapshot JSON to an orphan **`metrics-data` branch** (the durable, GitHub-browsable per-commit record; master stays clean) → regenerate the HTML viewer over the branch → publish via **GitHub Pages** (the trend dashboard). _On PR_: record for the PR head, fetch the merge-base snapshot from `metrics-data` (record on-the-fly if missing), `metrics-probe diff`, post one **sticky PR comment** with the delta table (edited in place per push, never spammed). Informational, never gating — the 0.4 test is the hard gate; budget thresholds arrive with G12 Layer-2 limits. Document the caveat: node/alloc counts are machine-independent, heap **bytes** differ between the CI runner and local machines — trends comparable within each store only.
  - Acceptance: a master push produces a new JSON on `metrics-data` + an updated Pages dashboard; a test PR gets exactly one delta comment that updates on force-push; `ci.yml` goes red on a deliberately broken test and green on the current baseline.
  - [x] **0.9c Per-pushed-commit granularity (maintainer challenge, 2026-07-07: "we can't see the difference we gain from each commit").** Today `metrics.yml` records on master pushes only → with a PR workflow the Pages trend is _PR-level_, while true per-commit history exists only in the local git-ignored store (with local-machine numbers). Fix: trigger the record job on **all branch pushes** — Layer-1 always (seconds), `--sizes` stays master-only (thumb opt-z builds are minutes) — still writing keyed JSON into `metrics-data`. Rationale for NOT git-tracking `metrics/` in the main repo, on record: (a) hash chicken-and-egg — a snapshot keyed by commit X cannot live inside commit X, and the post-commit hook writing after each commit would dirty the tree into an infinite metric-commit loop; (b) machine-dependence — local heap bytes ≠ CI bytes, one store must not mix them; (c) JSON churn/conflicts in every PR. The orphan branch avoids all three. Acceptance: push a 3-commit branch → 3 keyed JSONs on `metrics-data`; Pages charts them. **DONE — `f42c154` (branch `ci-metrics-actualize`).** `metrics.yml`: `on.push` now `branches-ignore: [metrics-data]` (all branches); record step passes `--sizes` only when `github.ref == refs/heads/master`; top-level `concurrency: metrics-${{ github.ref }}` + `cancel-in-progress`. _CONFIRMED on real pushes (2026-07-08): the record job fires on branch pushes with `section_sizes` **empty** + `bench_medians` present (`aa4547b` on ws9-footprint-diet, `9f2b884` on ci-metrics-actualize), while master pushes populate `section_sizes` (`ee5f724`, `1559f2f`) — the `--sizes`-master-only gate is provable from the committed data; 12 keyed JSONs accumulated on `metrics-data` with no publish races (`keep_files:true` held). See the checklist below._
  - [x] **0.9d Benchmark trends (same challenge, wall-clock half).** Deterministic bench outputs (allocs/op, bytes/op, counters) are already in the snapshot. Wall-clock: criterion history stays untracked locally for the same reasons + runner noise (shared GitHub runners ±10–30%). Fix: extend the CI snapshot with **criterion medians** (reactivity + layout bench groups) recorded on the CI runner only — a self-consistent trend, charted on the same Pages dashboard with explicit ±runner-noise framing (wide error bars, NO red-alert thresholds). Local criterion baselines remain the decision-grade A/B instrument (proven by 1.3b's catch). Alternative on record: `github-action-benchmark` with a generous threshold. Acceptance: dashboard shows a wall-clock series per bench group; a deliberate 2× slowdown on a scratch branch is visible; nothing gates. **DONE — `f42c154`.** Snapshot schema gains `bench_medians: Vec<BenchMedian { id, median_ns, ci_half_ns }>` (serde(default), 0.7f test extended); `metrics-probe`'s new `benches.rs` reads `target/criterion/<id>/new/estimates.json` (median point estimate + CI half-width) under a `--benches` flag; `diff`/`print` and the HTML viewer show medians with explicit ±runner-noise framing and NO thresholds; `ci-metrics.sh` runs the `reactivity`+`layout` groups with bounded criterion times (`--warm-up-time 0.5 --measurement-time 1.0 --sample-size 10`, tunable) after `rm -rf target/criterion`, then records `--benches` on every push. Verified local: parser reads real criterion output (78 medians), both bench bins accept the bounded args, viewer renders, forward-compat test green. _Pages chart CONFIRMED (2026-07-08): the bench-medians table (53 series, ±CI on hover, caption "±noise, informational", NO thresholds) renders live at `hazer-hazer.github.io/rsact/`. The deliberate-2×-slowdown demo was intentionally SKIPPED (maintainer decision 2026-07-08, checklist item 5) — the charting mechanism is already proven by 12 real snapshots' per-column median variance, and a fake spike would durably pollute the Pages-published trend (`keep_files:true`)._
  - **CI hardening (out-of-band, `02354de`) — close the size-probe silent-skip gap.** Motivated by WS9a.1's discovery that the reactive size-probe had been un-buildable (stale `runtime::observe()`) since WS2, silently dropped from every `--sizes` snapshot because `sizes.rs` skips a failed probe build like a missing toolchain. New `scripts/ci-size-probe-check.sh` compiles `reactive`+`ui` for thumbv7m in the size-probe release profile; wired into `ci.yml`'s hard gate so probe bit-rot fails a PR loudly. `metrics.yml` stays informational/non-gating (the compile gate lives in `ci.yml`, not the metrics job). The size-probe fix itself is `c7166ea` (cherry-pick of WS9a.1's `dfaf2b5`).
  - **Push-validation checklist (0.9c/0.9d — can only be observed on GitHub):** (1) push `ci-metrics-actualize` → `ci.yml` runs the size-probe compile gate green; (2) the metrics `record` job fires on the branch push (not just master), writes a keyed JSON to `metrics-data`, Layer-1 + bench medians present, `--sizes` **absent** (branch, not master); (3) push ≥2 commits → that many keyed JSONs accumulate; (4) Pages dashboard renders the new "bench medians" table; (5) a deliberate 2× slowdown on a scratch branch shows in the trend; (6) watch for cross-branch publish races on `metrics-data` (concurrent pushes from different refs; `keep_files:true` + per-ref concurrency mitigate but don't fully serialize) — document if observed; (7) confirm CI-minute cost of bench runs on every push is acceptable, else trim `crit_args` / restrict benches to master.
  - **VERIFIED 2026-07-08 — 6/7 confirmed from real WS9a/WS9b pushes; no dedicated scratch push needed** (the durable `metrics-data` history + Actions logs already carried the evidence): (1) ✅ `ci-metrics-actualize` runs `28896554403` (PR) + `28896135073` (push) green; (2) ✅ branch pushes `aa4547b` (ws9-footprint-diet) + `9f2b884` (ci-metrics-actualize) recorded with `section_sizes` **empty** + `bench_medians`=53, while master `ee5f724`/`1559f2f` populate `section_sizes` (4 rows) — the `--sizes`-master-only gate is provable straight from the committed JSON; (3) ✅ 12 keyed JSONs on `metrics-data`; (4) ✅ Pages dashboard (`hazer-hazer.github.io/rsact/`, status `built`) renders the bench-medians table (53 series, ±CI hover, "±noise, informational", NO thresholds); (6) ✅ no publish races — all 12 snapshots intact, all metrics runs green (`keep_files:true` + per-ref `concurrency` held across the WS9a/WS9b/master push mix); (7) ✅ minute cost acceptable — branch push ~4m45s, master ~5m, PR diff ~42s. **(5) intentionally SKIPPED** (maintainer decision 2026-07-08): a deliberate 2× slowdown would write a bogus doubled-median column into the durable Pages trend; the charting mechanism is already proven by the 12 snapshots' per-column median variance, so the demo's marginal value doesn't justify polluting the shared record. Also confirmed incidentally: PR #4 + PR #5 each got **exactly one** sticky `rsact-metrics` comment (`github-actions[bot]`), and the one-time GitHub Pages setting (source = `metrics-data` `/`) was already in place — nothing manual left to do.
  - [x] **0.9e Historical backfill + per-commit trend charts (maintainer ask, 2026-07-08: "fetch metrics from all previous commits so Pages charts every property per commit").** Three parts. **(a) Backfill/gap-fill job** — `workflow_dispatch` (optional `range` input): for each commit in `git rev-list --first-parent f298b98..HEAD` **without** a JSON on `metrics-data`: `git worktree add` that commit → run **that commit's own** `metrics-probe -- record` (+`--benches`) → copy the snapshot out → batch-commit to `metrics-data` → regenerate Pages. Idempotent by construction (skips existing), so the same job also repairs CI-missed pushes forever. Must run on the CI runner class (local heap bytes aren't comparable); cargo cache keeps per-commit builds cheap; `--sizes` only for a sparse subset (every Nth/tagged commit — thumb builds are minutes each). **Hard boundary on record:** a commit can only be measured by instruments that exist *inside it* — metrics-probe was born at `257f587`, so backfilled history starts there (schema evolution is safe thanks to 0.7f's `serde(default)`); earlier commits stay covered by the frozen audit rows in Baselines. **(b) Ordering index** — snapshots are keyed by hash and hashes don't self-order: CI/backfill emits an `index.json` on `metrics-data` (rev → commit date, parent, branch-hint) so the viewer sorts topologically without needing git. **(c) Time-series viewer mode** — extend the 0.9d dashboard with per-metric line charts across ordered commits; **absent metrics render as gaps, never zeros** (the 0.7e/0.7f lesson — a metric added later must not look like a regression from zero); series pickable per scenario/metric/bench-group. Acceptance: after one backfill run, Pages charts every Layer-1 metric + bench medians from `257f587` to HEAD with visible gaps where instruments didn't yet exist; re-running the job is a no-op. Design refined in `docs/plans/2026-07-08-0.9e-backfill-and-trend-viewer-design.md`. **DONE (code + local verification) — `f479070` on `ws0-9e-backfill-trends` (PR to master).** (a) `scripts/ci-backfill.sh` + a `workflow_dispatch` `backfill` job in `metrics.yml`: worktree per commit → THAT commit's own `metrics-probe record`, range clamped to tool-birth `257f587`, `--benches` only for ≥`f42c154`, sparse `--sizes` every 5th, idempotent skip, per-commit best-effort, `DRY_RUN` for local orchestration tests. (b) `metrics-probe/src/index.rs` — `index.json` (rev→{date,parent,branch}); `record` merges HEAD incrementally (shallow-safe), new `index` subcommand rebuilds all from full history; `topo_order` + 7 TDD tests; `ci-metrics.sh` pulls `index.json`. (c) `html.rs` viewer rewrite — topo-ordered table with domain-aware ▲/▼ markers (lower-is-better), click-to-expand inline charts, right sidepanel overlaying selected series each normalized to its own 0..max with a hover tooltip; gaps≠zeros; self-contained `file://`, zero deps. Verified local: 10/10 metrics-probe tests, viewer over a hand-built fixture (`node --check` + a `vm` behavioral test of arrows/gaps/ordering), backfill dry-run (clamp / idempotent no-op / sparse selection). _Corrected the range default from the roadmap's `f298b98` (predates the tool by 4 commits) to tool-birth `257f587`._ _Needs a `workflow_dispatch` on master post-merge to confirm: backfill charts `257f587..HEAD` with gaps, an idempotent re-run is a no-op, and the actual CI-minute cost — recorded here after the run._

**Session prompt (0.9e — backfill & trend charts):**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS0 item 0.9e (design + the
tool-birth hard boundary inline). Verify current state: 0.9c/0.9d landed and VERIFIED
(metrics-data live, Pages built). Build (a) the workflow_dispatch backfill job (worktree
per commit, that commit's OWN tool, idempotent skip, batch-commit), (b) the ordering
index.json, (c) the time-series viewer mode (gaps ≠ zeros; series picker). Test the
viewer locally against a hand-built multi-snapshot dir BEFORE touching CI; the backfill
run itself is observable only on GitHub — trigger it once, verify charts + idempotent
re-run, record actual CI minutes in the roadmap. Do NOT backfill --sizes for every
commit (sparse subset only).
```

  - [ ] **0.9e Historical backfill + per-commit trend charts (maintainer ask, 2026-07-08: "fetch metrics from all previous commits so Pages charts every property per commit").** Three parts. **(a) Backfill/gap-fill job** — `workflow_dispatch` (optional `range` input): for each commit in `git rev-list --first-parent f298b98..HEAD` **without** a JSON on `metrics-data`: `git worktree add` that commit → run **that commit's own** `metrics-probe -- record` (+`--benches`) → copy the snapshot out → batch-commit to `metrics-data` → regenerate Pages. Idempotent by construction (skips existing), so the same job also repairs CI-missed pushes forever. Must run on the CI runner class (local heap bytes aren't comparable); cargo cache keeps per-commit builds cheap; `--sizes` only for a sparse subset (every Nth/tagged commit — thumb builds are minutes each). **Hard boundary on record:** a commit can only be measured by instruments that exist *inside it* — metrics-probe was born at `257f587`, so backfilled history starts there (schema evolution is safe thanks to 0.7f's `serde(default)`); earlier commits stay covered by the frozen audit rows in Baselines. **(b) Ordering index** — snapshots are keyed by hash and hashes don't self-order: CI/backfill emits an `index.json` on `metrics-data` (rev → commit date, parent, branch-hint) so the viewer sorts topologically without needing git. **(c) Time-series viewer mode** — extend the 0.9d dashboard with per-metric line charts across ordered commits; **absent metrics render as gaps, never zeros** (the 0.7e/0.7f lesson — a metric added later must not look like a regression from zero); series pickable per scenario/metric/bench-group. Acceptance: after one backfill run, Pages charts every Layer-1 metric + bench medians from `257f587` to HEAD with visible gaps where instruments didn't yet exist; re-running the job is a no-op.

**Session prompt (0.9e — backfill & trend charts):**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS0 item 0.9e (design + the
tool-birth hard boundary inline). Verify current state: 0.9c/0.9d landed and VERIFIED
(metrics-data live, Pages built). Build (a) the workflow_dispatch backfill job (worktree
per commit, that commit's OWN tool, idempotent skip, batch-commit), (b) the ordering
index.json, (c) the time-series viewer mode (gaps ≠ zeros; series picker). Test the
viewer locally against a hand-built multi-snapshot dir BEFORE touching CI; the backfill
run itself is observable only on GitHub — trigger it once, verify charts + idempotent
re-run, record actual CI minutes in the roadmap. Do NOT backfill --sizes for every
commit (sparse subset only).
```

**Session prompt (0.9c + 0.9d — metrics granularity & bench trends):**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS0 items 0.9c/0.9d (rationale +
acceptance inline). Verify current state: metrics.yml exists, metrics-data is live on
origin. 0.9c: record job triggers on all branch pushes (Layer-1 only; --sizes stays
master-only); add concurrency-cancel for superseded pushes to control CI minutes. 0.9d:
extend the snapshot schema (remember serde(default) — 0.7f) with criterion medians
recorded in CI; chart with ±noise framing; do NOT add alert thresholds. Verify with a
scratch branch push; record in the roadmap anything only observable after a real push.
Mark done with commit hashes.
```

**Session prompt (0.9 — CI):**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS0 item 0.9 (+ docs/features.md
for the exact test + powerset commands). Verify current state first: 0.7/0.8 are landed;
confirm the known-fail baseline (reactive 54/2). Build 0.9a first and get it green on the
current tree, then 0.9b (orphan metrics-data branch bootstrap, Pages publishing, sticky
PR comment via the github-script pattern). You cannot fully verify Actions locally —
structure workflows so each step is also a runnable script (act-compatible where
possible), and note in the roadmap what needs a real push to validate. Do not gate merges
on metrics — informational only. Mark sub-items done here with commit hashes.
```

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

- [x] **1.1 Scope parent chain** (`runtime.rs:373,732-746`, `scope.rs`): `new_scope` overwrites `current_scope`, `drop_scope` never restores → values created after an inner scope drops are owned by nothing and leak forever. **Decided:** `parent: Option<ScopeId>` in `ScopeData` — the parent pointer _is_ the stack (intrusive, no `Vec`); `drop_scope` restores `current_scope` to the dropped scope's parent **only if** the dropped scope is current; out-of-order drops (page scopes are held across frames and dropped non-lexically) leave it untouched. A store-nothing RAII guard was rejected: guards assume LIFO order, page scopes don't obey it.
- [x] **1.2 Multi-runtime: hide + fix, postpone the rest (decided):** move `create_runtime`/`with_new_runtime` behind a **`test-utils` feature** — dependents enable it via **dev-dependencies only** (`[dev-dependencies] rsact-reactive = { features = ["test-utils"] }`), so the API doesn't exist in any production build graph. Fix the restore bug _inside_ it in the same commit (RAII guard restoring `prev` — `runtime.rs:98-111,49-64` currently discards it, bricking the runtime; our own tests call it ~15×). Single global runtime is the only public reality. No compound `ValueId`+`RuntimeId` — postponed indefinitely.
- [x] **1.3a Pin the push-queues-effects invariant with a test first:** `update()`'s commit path (`runtime.rs:938-950`) marks subscribers Dirty with bare `storage.mark`, which only flips the state byte — it does **not** enqueue effect-subscribers into `pending_effects` (only `mark_node` does). It works today solely because the write-time `mark_dirty` push already queued every transitively-reachable effect — an undocumented invariant. Test: an effect whose memo source recomputes during a pull must already be queued (push suppressed variants). **1.3b** then two one-line hardenings: commit path uses `mark_node` (pull becomes self-sufficient — insurance for WS5's lazier marking), and the cycle-degradation skip path (`try_borrow_mut` fail at `runtime.rs:924-930`) leaves the node's state untouched instead of marking the never-recomputed node Clean (`:952-954` — stale-but-Clean bug).
- [x] **1.4 Delete `Debug`/`Display` impls on reactive handles entirely (decided):** `read.rs:195-291` currently implements them via tracked `with()` — a debug print inside any observer subscribes it permanently. Formatting a signal becomes a **compile error**; users write `signal.with(|v| ...)` so the read is visible and deliberate. `PartialEq`/arith ops **stay tracked** — they are dataflow (a memo computing `a == b` must re-run when either changes). If `Debug` ever returns, only the id-only form (never reads the value) is acceptable — not now.
- [x] **1.5a Check-residue correctness suite first (decided: prove before implementing):** memo-cut diamond (after an equal-value recompute, downstream is Clean and the next idle read does zero source re-walks — uses 0.3/0.5 counters), the checkbox nested-observer scenario re-run, cut-then-real-change (a genuine change after a cut still propagates), dynamic-deps (source set changes across runs, then cut, then change through the new source), and a property test: random small graphs + random writes vs a recompute-everything oracle. **1.5b** then the fix in `maybe_update` (`runtime.rs:834-869`): after a **completed** source walk finds nothing dirty, downgrade `Check → Clean`. Safety argument: the walk recursively freshens every source; a changed source would have marked this node Dirty; walked-and-still-not-Dirty ⇒ genuinely unchanged. Invariant: only `Check → Clean`, **never** `Dirty → Clean` — the past consumed-dirtiness bug class stays impossible.
- [x] **1.6 Remove `SignalMapReactive` entirely (decided — it's the anti-pattern, its own TODO at `maybe/mod.rs:11` agrees):** slider `step` becomes an explicit match at the call site (inert → compute once; signal → `.map()`); flex `layout_children` keeps a local, honestly-named helper until WS5 dissolves the need for an always-memo. Kills both the inert-arm live-memo-cloning-per-read and the `MaybeReactive` impl's double node (`.map(map).memo()`). Fix the stale `IntoMemo` doc alongside.
- [x] **1.7 Kill render-path `format!`** (`el/render.rs:290-298`): `render_self` key becomes `&'static str` (`"self"`); tighten `render_part` keys to `&'static str`. This stops the per-frame heap churn now; the identity/ownership redesign is WS2 (the maintainer's encapsulation constraint is recorded there).
- [x] **1.8 `try_*` APIs + contextful errors (decided: expose Option to the user where possible):** public `try_with`/`try_get`/`try_update` on the read/write traits returning `Option` (dead handle → `None`, logged); the panicking APIs become thin wrappers over them with contextful messages (id/type/creation-site under `debug-info`) replacing the bare unwraps at `storage.rs:56,118`; rsact-ui render/event paths migrate to `try_*` + `log::error!`. Full `Result<_, ReactiveError>` plumbing through widget APIs stays rejected (flash cost, signature infection).

Acceptance: rsact-reactive ≥ 54 pass + new regression tests (incl. the 1.3a invariant test and the 1.5a suite); rsact-ui 44/0 unchanged; `benches/allocations.rs` shows no regressions and change-frame allocs drop (no `format!`); a compile-fail check covers `format!("{:?}", signal)`.

**STATUS 2026-07-07: WS1 COMPLETE (1.1–1.8 landed).** Commits: 1.1 `7e3afad`; 1.2 `da8998f`; 1.3a `dfe2896`; 1.3b `e3d9753` (+ revise `be2e327`); 1.4 `d3f0d65`; 1.5a `ba25ce4`; 1.5b `c8f217d`; 1.6 `3d5d0df`; 1.7 `d51ffec`; 1.8 `6a626a7`. Final: rsact-reactive **66 pass / 2 known-fail** (lib), rsact-ui **44/0**, rsact-render **6/0**, metrics-probe **3/0**, reactive feature-powerset green, `benches/allocations.rs` unchanged at baseline. Execution notes for review (per protocol — reported, not silently forced):

- **1.3b — mark_node commit-path change reverted; cycle-skip kept.** The first hardening (commit path uses `mark_node` so the pull enqueues effects itself) was implemented (`e3d9753`) and then reverted (`be2e327`) because it **doubled effect-rerun allocations** — `benches/allocations.rs` went 2/112 → 4/224 B on `effect_rerun_1_signal`/`_10`/`_100`/`batch_100`/`signal_write_noop_equal`. Cause: it re-enqueues effects the write-time push already queued (the 1.3a invariant), i.e. a redundant BTreeSet insert into the just-drained queue + an extra flush round, for **zero benefit today**. It conflicts with the "no bench regressions" acceptance. The self-sufficient-pull "insurance for WS5's lazier marking" is therefore **deferred to WS5**, which owns the lazier marking and must introduce it without this re-enqueue cost. The cycle-degradation skip fix (`try_borrow_mut` fail → leave state untouched, never `Dirty → Clean`) is landed.
- **1.2 — `test-utils` gating enabled for benches/doctests via a self-dev-dependency.** `create_runtime`/`with_new_runtime`/`RuntimeId::leave` are `#[cfg(any(test, feature = "test-utils"))]`. rsact-reactive's own benches, doctests and examples are *external* consumers of these, so `--features std` alone (the fixed baseline `cargo bench`/doctest command) would not see them; rsact-reactive lists **itself as a dev-dependency with `features = ["test-utils"]`** to enable them for its dev targets only (resolver 3 keeps this out of the normal build graph). rsact-ui enables it in dev-deps; **metrics-probe** uses `with_new_runtime` in its non-test `record` path, so it takes `test-utils` as a *normal* feature (it is `publish = false` host tooling, never in a shippable graph).
- **1.6 — `SignalMapReactive` had zero call sites.** The slider-`step`/flex-`layout_children` call sites described in the item do not exist in the current tree (they were changed or never landed), so removal was a clean dead-code deletion with no call-site rewrites needed.
- **Pre-existing, out of scope (reported, not fixed):** `cargo test -p rsact-reactive --features std` has **4 failing doctests** in the `maybe` module (`maybe_reactive::MaybeReactive`, `maybe_signal::{MaybeSignal, IntoMaybeSignal}`) — they import through private `rsact_reactive::maybe::…` paths (`MaybeReactive`/`MaybeSignal` are private there; the public paths are `prelude::…`). These fail identically on clean master before this WS (verified at HEAD without the WS1 changes) and are unrelated to reactive-core correctness; the "54/2" baseline refers to the lib unit-test binary. Left for a docs pass.

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

- [x] **b.1 Canvas blanks after any forced redraw.** `DrawQueue` drains on render (`widget/canvas.rs:201-205`, consumed at `:249`) — commands are gone after one execution, and `force_redraw` fires on every relayout, so any relayout/navigation/devtools toggle repaints the background and the Canvas renders nothing. Decide the model — immediate-mode (redraw callback re-issues per frame) vs retained replay (keep last command list, re-play on overdraw) vs `Memo<Vec<DrawCommand>>`-only — record the rationale, fix accordingly. Related: `Image` `PartialEq` returns false for `Owned == Owned` (`rsact-render/src/image/mod.rs:40`), defeating memo-diffing of command lists. The decision is design input for WS16.1.
  - **Decision (2026-07-07, maintainer): immediate-mode via a single draw closure — no command buffer.** `Canvas::new(move |renderer| { … })` stores one `Box<dyn Fn(&mut W::Renderer) -> RenderResult>` and calls it inside `render_self` through `clip_inner` (the closure receives the renderer already clipped to the Canvas rect, so it can only draw inside its own bounds). This deletes the entire `DrawQueue` / `DrawCommand` / `CanvasImage` / `ImageStorage` machinery — that is the "simpler and more memory-efficient" win (no per-frame `VecDeque` + two signals, just one boxed closure). **Why it fixes the bug:** `render_self`'s `observe_with_force` already tracks any reactivity the closure reads (so it re-runs when a signal it read changes) and always re-runs on `force_redraw`/relayout like every other widget — the drain-once queue is gone, so there is nothing left to blank. **Consequences:** drawing is absolute-coordinate + clipped, exactly as the old command model was; there is no per-frame command list, so the `Image PartialEq` memo-diffing concern is moot _for Canvas_ (it stays relevant to any future retained `Memo<Vec<DrawCommand>>` layer, which WS16.1 can still build on top). Examples `animation.rs` / `tiny_skia.rs` ported to the closure API. Rejected: retained-replay and `Memo<Vec<_>>`-only both keep a command buffer the maintainer wanted gone, and the latter's headline memo-diffing benefit is blocked now by the rsact-render `Image PartialEq` bug (WS6, off-limits this session).
- [x] **b.2 Animation correctness trio** (`rsact-ui/src/anim/mod.rs`): (a) restart silently no-ops every other `start()` — completion is checked against a stale `last_tick` (`:298`) that neither `AnimHandle::start` (`:161-163`) nor the `StartRequested→Running` transition (`:274-279`) resets; (b) u32 clock wrap breaks running animations (`:331-332`; `ui.rs:318`'s `% u32::MAX` is an off-by-one modulus); (c) `AnimCycles::N(0)` plays one full cycle (`:24`); (d) `Easing::EaseOutSine` is inverted (`easing.rs:92` runs 1→0; easings.net defines `sin(x·π/2)`).
  - **Fixed 2026-07-07** (4 failing tests first, in `anim::tests`): (a) reset `state.last_tick = 0` in the `StartRequested→Running` arm; (b) `last_tick = now_millis.wrapping_sub(start_time)` in place of `.abs()`, and `ui.rs` modulus is now `% (u32::MAX as u128 + 1)`; (c) guard at the top of the value memo returns `dir.start_point(0)` for `AnimCycles::N(0)` (never runs, never depends on the clock); (d) `EaseOutSine` is now `sin(x·π/2)`.
- [x] **b.3 `FontProps::has_any()` implements has-ALL** (`font/mod.rs:77-87`): layout stores resolved text props only when all three fields are `Some` (`layout/model.rs:270-278`) while measurement always merges (`layout/mod.rs:117-121`) — so `label.font_size(20)` alone is *measured* at 20 but *drawn* at the inherited size. Align measure and draw; note the fix in WS15.1's measure-parity scope.
  - **Fixed 2026-07-07** (failing test first, `font::tests::has_any_reports_any_set_field_not_all`): `has_any` is now `font.is_some() || font_size.is_some() || font_style.is_some()`, so a partial override (e.g. font-size-only) is stored on the layout model and read back at draw — measure and draw agree. **WS15.1 note:** with this fixed, measure/draw font-prop parity holds for single-field overrides; the remaining measure-parity work there is unaffected.
- [x] **b.4 `declare_widget_style!` broken macro arm** (`style/mod.rs:151-155`): `$crate::stable::` path typo + unbound `$field` in the no-opts `text_color: color` arm — any widget declaring it without an explicit opts block gets an incomprehensible error. Two-line fix now; WS7.4 keeps the macro.
  - **Fixed 2026-07-07** (failing test first, `style::tests::bare_text_color_generates_transparent_text`): it was actually **three** defects in that one arm, not two — `$crate::stable::`→`$crate::style::`, unbound `$field`→literal `text_color`, and the inner call was parens-delimited (`!(…)`) while it expands to *items* (methods), which Rust requires be brace-delimited or `;`-terminated. Rewrote it as a braced `!{ @opt_method_list text_color: color { transparent_text: transparent } }` matching the sibling arms. **Sibling bug found (not fixed — out of b.4 scope, report-don't-fix):** the `(@opt_method_list border: border)` arm (`style/mod.rs` ~`:229`) has the same unbound-`$field` defect but is likewise latent (every in-tree `border` field passes an explicit opts block). Worth folding into WS7.4's macro rework.

Acceptance: each bug lands with a failing test first; UI suite stays green (44/0 baseline); the b.1 canvas decision recorded in this file.

**Done 2026-07-07 — commit `b793577`** (WS1b b.1–b.4). Each bug was verified to
reproduce with a failing test first (b.1 via the old drain-on-render `DrawQueue`
before the rewrite; b.2 four tests in `anim::tests`; b.3 `font::tests`; b.4 a
macro-instantiation compile-fail in `style::tests`), then fixed. rsact-ui lib
suite `44 → 51/0` (`cargo test -p rsact-ui --features "std,embedded-graphics"
--lib -- --test-threads=1`; note plain `--features std` does not compile without
a font backend). No rsact-reactive or rendering-internals changes. Two follow-ups
handed off: the `Image PartialEq` fix (rsact-render, WS6 → design input for
WS16.1) and the sibling `border: border` macro-arm defect (WS7.4).

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

- [x] 2.1 Core `Probe` handle API (`create_probe`/`poll`) + `run_probe` refactor + the `clear_sources` split (+ tests: conditional-dep unsubscribe, nested-probe survival across parent re-runs, owned-value story). **Done — session 1 (commit `faa2e08`).** See the session-1 execution note below.
- [x] 2.2 rsact-ui arena-owned probes (`ElState.part_probes` keyed by `&'static str` with the `PartId(u16)` TODO; `Page.render_probe`); borrow-choreography prototype first. **Done — session 2 (`2a83c7a` part probes, `07f3fa7` page probe).** Pre-extraction choreography (no `RefCell`) exactly as the session-1 prototype predicted.
- [x] 2.3 Lifecycle: dispose on `remove_subtree` + `Page::drop`; leak regression test. **Done — session 2 (`2ce8e75`).** Probes created **untracked** (owned by no observer/scope → no cascade double-dispose); `dispose_probes` disposes an element's set, called from `remove_subtree` (subtree removal) and a new `ElArena::dispose_all_probes` on `Page::drop`. Two leak tests, each verified to fail when disposal is neutered: `page_drop_disposes_all_probes` (100 create→render→drop = goto round-trips, was leaking 200) and `subtree_removal_disposes_part_probes` (`set_children(root, [])` → `remove_subtree`). _Note: the "100 set_children reconciliations via Dynamic-rebuild + render" form was not viable — the null-theme harness hits a pre-existing disposed-content render panic on rebuilt children (same class the sibling `arena_rebuild_does_not_leak_subtree` avoids by not rendering); the direct `set_children` test covers the same disposal path._
- [x] 2.4 Delete `static_observers`/`observer_hashes`/`hasher` (+ `ahash` from the no_std path per G6); delete the dead revive branch; remove the keyed `observe()` API. **Done — session 2 (`5f5b7c8`).** No thin compat wrapper kept: `observe()` fundamentally needs the registry, so it was removed outright. `use_observe`'s body was already the identity-free `run_probe` (session 1). The semantics test was rewritten in session 1 (`child_observer_reruns_only_on_own_dep_change`) and then, with the registry gone, consolidated into `probe.rs` along with the other observe-vehicle tests (7 deleted as redundant/registry-specific; 2 unique semantics — memo-cut, re-entrant self-write — re-added as probe tests). Perf benches (`observe_redraw_1_of_n`, `observe_noop_frame`, `observe_redraw_1_of_16`) and the metrics `reactive_only` scenario migrated to `create_probe`/`poll` (node counts preserved: `observers == 17`/`== 11` hold). Also removed rsact-ui's now-unused `WithElId`.
- [x] 2.5 **Probe documentation & third-party pattern** (A5). **Done — session 2 (`10ea2c8`).** Module taxonomy doc + handle rustdoc (session 1) plus a compiling doctest **"Driving probes from an external render engine"** documenting the ownership contract (store one probe per reactive region, poll each frame, dispose when the region goes away) as the public contract for out-of-tree render paths.

**STATUS 2026-07-07 — WS2 SESSION 2 COMPLETE (2.2–2.5 landed).** Render identity is now the owned `Probe` handle end-to-end; no registry, no `ahash`, cross-page/context aliasing impossible by construction. Commits: `2a83c7a` (2.2a part probes), `07f3fa7` (2.2b page probe), `2ce8e75` (2.3 disposal + leak tests), `5f5b7c8` (2.4 registry deletion), `10ea2c8` (2.5 docs). Results: rsact-reactive **68 pass / 1 known-fail** (`static_wrapper` = WS4; the 7 observe-vehicle tests were consolidated into `probe.rs`, net 73→68), rsact-ui **53/0** (51 + 2 leak tests), rsact-render **6/0**, metrics-probe **3/0** (probe counts 17 reactive / 11 ui unchanged), reactive feature-powerset green (ahash-free no_std), benches compile as probe benches. Session-2 execution notes (protocol — reported, not silently forced): (1) `observe()` removed outright, not kept as a compat wrapper — it needs the registry that G6 deletes. (2) Probes created **untracked** to make disposal ownership explicit (ElState/Page) rather than incidental (the creating observer) — avoids cascade double-dispose. (3) The debug-only `("page_force_redraw", id)` observer was dropped (its `force_redraw` dep is tracked by the page render probe) rather than turned into a debug-only probe. (4) The `PartId(u16)` key compaction remains a `TODO` in `ElState` (kept `&'static str` content-compare keys). (5) A pre-existing null-theme render panic on rebuilt `Dynamic` children blocked the Dynamic-rebuild form of the 2.3 leak test (used direct `set_children` instead) — worth a look when WS6/theme work lands.

**STATUS 2026-07-07 — WS2 SESSION 1 COMPLETE (2.1 landed; 2.2 prototype done).** Commit `faa2e08`. rsact-reactive core is UI-vocabulary-free (I5 upheld). Deltas: new `probe.rs` (`Probe(ValueId)` Copy handle · `create_probe` · `Probe::poll(force,f)`); `Runtime::run_probe` = the identity-free reaction core (`is_alive`→None · `subscribe` · `maybe_update` · if changed‖force: `clear_sources` · `with_observer(f)` · `mark_clean`); `cleanup` split into `clear_sources` (edge detach only) + owned-value disposal; `ValueKind::Observer`→`ValueKind::Probe` (+ `ValueKindTag`, Display, debug fmt); `use_observe` now delegates to `run_probe` (registry kept as a thin wrapper — session 2 deletes it). Tests: 6 new in `probe.rs` (born-dirty/no-op/dep-change, force, disposed⇒None + recreate-runs-once, conditional-dep unsubscribe, nested-probe survival across parent re-run, owned-value survives re-run + disposed-with-probe). Results: rsact-reactive **73 pass / 1 known-fail** (`static_wrapper` = WS4; the old `observe_recreates_disposed_child_observer` was **rewritten to G2** as `child_observer_reruns_only_on_own_dep_change`), rsact-ui **51/0**, rsact-render **6/0**, metrics-probe **3/0**, reactive feature-powerset green, `benches/allocations.rs` unchanged.

Execution notes for review (protocol — reported, not silently forced):

- **Semantics-test rewrite done in session 1, not deferred to 2.4.** Item 2.4 lists "rewrite the semantics test," but session 1 is what *establishes* the G2 semantics it encodes (`clear_sources` split + no owned-disposal on re-run). Leaving it red through session 1 would contradict "suites green," so it was rewritten now; 2.4 (registry/revive deletion) remains session 2.
- **`clear_sources` on the poll path is a behaviour change for rsact-ui render observers** — they never cleared sources before (`cleanup` was commented out in the old `use_observe`), so stale conditional deps accumulated. Now every executed poll re-tracks (invariant I4). Verified non-regressive: rsact-ui **51/0**.
- **Metrics field still named `observers`** (it now counts `ValueKind::Probe`). Rename→`probes` deferred to a dedicated metrics pass to avoid churning the metrics-data snapshot schema (G12 regression surface). Carries a `TODO:` at the count site.
- **Revive branch + registry deletion + `observe()` deprecation stay in session 2 (item 2.4).** `use_observe`/`static_observers`/`observer_hashes` are untouched except that `use_observe`'s tail is now `run_probe`.
- **Pre-existing, out of scope (reported, not fixed):** two `unused_mut` warnings in `runtime.rs` reactive tests (`let mut a`/`let mut b`, ~`:2226,:2282`) predate this session.

**2.2 borrow-choreography prototype result (session 1, validated by a standalone compile-spike).** `Widget::render(&self)` (`widget/mod.rs:66`) means `ElData`/`ElState` is only *shared*-borrowed during render, and `RenderCtx` copies state fields out + carries `dirten: &mut bool` and `needs_redraw` (`el/render.rs:69`, extracted at `:450-452`). **Recommendation: pre-extraction (roadmap "option a"), NO `RefCell`.** In `render_subtree_body`: `mem::take` the element's `part_probes` via `expect_mut` *before* the shared `data`/render borrow (mirroring the `needs_redraw` take), pass `&mut TinyVec<[(&'static str, Probe); 2]>` into `RenderCtx`, write it back via `expect_mut` after `render` returns. `render_part(key, f)` then does: linear scan for `key` (content compare — `&'static str` pointer identity is not codegen-stable) → `create_probe()` + push if absent → **the `Probe` is `Copy`, so the `part_probes` borrow ends there** → `probe.poll(redraw, closure)` where the closure captures `&mut self` and reborrows `self.dirten`/`self.part_probes` to build the child `RenderCtx` — structurally identical to today's `observe_with_force(WithElId::new(id,key), redraw, closure)`. A compile-spike (scratch) exercising nested `render_part` (`self`→`options`→`thumb`) + a sibling part on the same element compiles and runs. `RefCell<part_probes>` is the fallback only if the write-back proves awkward; the spike shows it does not. `Page.render_probe` is the same pattern at the page level (one probe, no map).

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

- [x] **3.0a Leak-attribution diagnostics** (`debug-info`): a `leak_report(snapshot)` API — snapshot the live node-set before a page/subtree build, diff after its disposal; survivors reported **with their creation site** (the `Location` breadcrumb already exists in `ValueDebugInfoState` — this is plumbing, not new tracking). The 0.3 metrics _detect_ leaks (counts moved); 3.0a _attributes_ them (which `file:line` created the survivor). **Done `4a32db0`** — `rsact-reactive/src/leak.rs` (`leak_snapshot`/`leak_report`/`LeakReport`); the snapshot/diff (survivor ids + kinds) is always available, the `file:line` attribution is `debug-info`-gated; `Debug` derived on `ValueKindTag`. It is what re-surfaced the now-silent leak in the 3.1 repro (WS1.8's `try_*` turned the D2-F1 panic into a silent leak — see the discrepancy note below).
- [x] **3.0b Full disposal audit of rsact-ui**: inventory every `create_signal`/`create_memo`/`create_effect`/`Layout` creation site (widgets, page, ui, event — **explicitly including the easy-to-miss ones**: `Anim::handle` mints a signal + memo per animation (`anim/mod.rs:248,255`), `DrawQueue::new` two signals (`canvas.rs:74-79`), `UiQueue::new` two signals + a memo (`event/message.rs:40-47`), all unowned today); record who _should_ own each; one lifecycle test per path (page drop, `set_children` subtree replace, `Dynamic` rebuild, navigation round-trip) asserting counts return to baseline. **Answer on record to the maintainer's question ("do widget-stored signals get disposed?"): no — nothing disposes them today.** Widget structs hold `Copy` handles; dropping `ElData` drops 8-byte keys while the runtime nodes live forever. The scope model is the fix: widgets are _built_ inside a scope (3.1 per-page, 3.2 per-subtree), so build-time `create_*` calls are scope-owned and die with it; the widget's now-dangling handles are safe post-disposal (the element is gone; any straggler read becomes a logged no-op via WS1.8 `try_*`). This audit's inventory is the direct design input for 3.2. **Done `06f75ee`** — the inventory (who creates each node, in which build phase, owner-today vs owner-after) is the table in `docs/plans/2026-07-08-ws3-scopes-page-disposal.md`; lifecycle tests landed per-item (page drop + navigation in 3.1, `Dynamic` rebuild in 3.2, `set_children`/pruning in 3.3). **Two discrepancies reported (protocol — see below): `DrawQueue` no longer exists (WS1b.1 deleted it); `UiQueue`'s signals are correctly UI-lifetime (created before any page), NOT a leak to page-scope.**
- [x] 3.1 Per-page `ScopeHandle` created in `UI::load_page` (`ui.rs:212-230`); `Page::drop` disposes the scope (everything the `PageInitFn` created). Regression test: the disposed-arena effect panic repro; navigation leak test counting runtime nodes. **Done — `fae7723` (Task 1: `ScopeHandle::leave` non-lexical scope exit in rsact-reactive) + `51424ae` (per-page scope in `load_page`).** The scope wraps `page_fn.init_page()` **and** `Page::new` (both build phases — the `Dynamic` layout effect is created in the former, its build effect in the latter; both must die with the page), then `leave`s to restore `current_scope`. Arena keeps its explicit WS2 disposal (created outside the scope); the `scope` field drops after the `Drop` body so redundantly-owned nodes are skipped by `drop_scope`'s `is_alive` guard. Tests: `disposed_page_effect_does_not_panic_on_app_signal` (14 leaked nodes → 0), `page_navigation_round_trip_is_leak_free` (node total flat over 50 goto cycles).
- [x] 3.2 Subtree disposal: `remove_subtree` (`el/arena.rs:170-179`) disposes widget-owned reactive nodes (scope-per-subtree or a `Widget`-level dispose hook — design with WS2's ownership model). **Done `2e28135` — VERIFIED SUBSUMED, no new machinery (protocol: report-don't-force).** The target leak is already closed by the existing ownership model: **Dynamic rebuild** → the factory runs inside the layout effect, so the old subtree's reactive nodes are effect-owned and disposed by `cleanup`'s owned-child recursion on re-run (verified by `dynamic_rebuild_does_not_leak_reactive_nodes`); **page drop** → the 3.1 page scope. Every `set_children`/`set_single_child` caller is a one-time `build()` (container/button/flex/scrollable/show) or the `Dynamic` build effect — no widget removes children outside an effect, so the "page-scope-owned subtree removed mid-page-life" path is unreachable with the current widget set. A per-subtree scope would create double-ownership tension with effect-cleanup for an unreached path; if a future widget manages children directly, the fix is to build them in an effect/scope (the established pattern).
- [x] 3.3 PageState pruning on element removal (`ctx.rs:97-99` TODO): invalidate `focused`/`captured_by`/`hovered` referencing freed ids (D2-F5). **Done `affaf08`** — `PageState::retain_existing(arena)` (backed by `ElArena::contains`) drops `focused`/`captured_by`/`hovered`/`pressed` refs to ids no longer in the arena (+ clears `focus_pressed`); wired into `Page::send_event` before routing (lazy validate-on-use — the arena mutation happens deep in a reactive flush with no `PageState` in hand). Test: `pagestate_forgets_removed_element_ids`.
- [x] 3.4 `rsact::render_once` one-shot sugar (D7): build → layout → render → drop; heap returns to ~baseline. Acceptance: e-paper sketch from the D7 report compiles as a doc-test/example. **Done `d63d832`** — `render_once(build, target)` wraps the whole UI construction + one render in a scope so the UI's own signals AND every page node dispose on return; `rsact_ui::ui::render_once` + prelude + root facade `rsact::render_once` (facade itself is pre-existing-unbuildable until WS12.5). Tests: `render_once_returns_heap_to_baseline` (leak_report empty after a full cycle) + a `no_run` doc-test (the e-paper sketch compiles).
- [x] 3.5 `zip_eq` → checked zip + `error!` in both hot passes (`el/event.rs:89,126`, `el/render.rs:499`) — arena/layout divergence must degrade, not abort (D2-F3). **Done `abf7849`** — shared `el::check_children_parallel(pass, id, arena_len, layout_len)` logs the divergence; callers keep `.zip()` (common-prefix degrade); `LayoutModelNode::children_len()` exposes the layout count. Tests: `check_children_parallel_flags_only_divergence` (unit) + `arena_layout_divergence_degrades_without_panic` (integration). **Discrepancy reported: the cited sites already used `.zip()` (silent truncate), NOT `zip_eq` (panic) — so no panic existed to fix; the real gain is observability (divergence is now logged, not silently truncated).**

**STATUS 2026-07-08 — WS3 COMPLETE (3.0a–3.5 landed).** Branch `ws3-scopes-page-disposal`, commits `4a32db0`, `06f75ee`, `fae7723`, `51424ae`, `2e28135`, `affaf08`, `d63d832`, `abf7849`. Deps verified landed first: WS1.1 (scope parent-chain + `value_after_inner_scope_drop_owned_by_outer` test), WS2 (`Page.render_probe` / `dispose_all_probes` / `remove_subtree` → `dispose_probes`). Final suites: rsact-reactive **75 pass / 1 known-fail** (`static_wrapper` = WS4), rsact-ui lib **60/0** (+7 WS3 tests), rsact-render **6/0**, metrics-probe **3/0**; thumbv7m `unsafe-single-thread,embedded-graphics,libm` build green. Discrepancies reported (protocol — not silently fixed): (1) 3.0b's `DrawQueue::new` creation site no longer exists — WS1b.1 replaced Canvas with a `Box<dyn Fn>` closure that mints no reactive nodes; (2) `UiQueue`'s signals are created at UI construction (before any page) → correctly UI-lifetime, must **not** be page-scoped; (3) 3.5's cited sites used `.zip()` (silent truncate), not `zip_eq` (panic), so the D2-F3 panic was already absent — the change adds the missing `error!` log; (4) the D2-F1 delayed panic is now a **silent leak** (WS1.8's `try_*` swallows the disposed-arena access) — 3.0a's `leak_report` is what re-surfaces it. The design sketch's `with_scope(&scope, || init_fn())` primitive does not exist and was **not** added; the equivalent is `new_scope()` + build + `ScopeHandle::leave()` (the scope must stay current across `Page::new`, so an enter/build/leave shape, not a wrap-a-closure shape).

**Post-review follow-ups (maintainer Q&A, 2026-07-08):**

- **Q1 (landed, `551b590`):** `Page.scope` is now a **required** `ScopeHandle`, not `Option`, passed into `Page::new` (caller creates it with `new_scope()` so it's current while `init_page()`/`root` and the per-page nodes build; `Page::new` `leave`s it and takes ownership). Removed `set_scope` + the "intentionally unmanaged page" arm — there was no live scopeless-page path (G11: page-created is page-owned). The constraint that keeps the scope caller-created: `init_page()` is the `root` arg, evaluated before `Page::new`'s body, so the scope must already be current at the call site.

- **Q2 (OPEN — needs a gate; do NOT land in WS3):** render/`on_event`/`update` must **not create reactive values at all**; a violation is a user bug we must **log, never panic** ("UI must never panic"). Direction: run each pass inside a **`deny_new` scope that logs instead of panicking** (today `new_deny_new_scope` *panics* in `add_value_raw`; add a logging mode). **Critical dependency found while scoping it:** a naive deny-scope around the passes is *wrong* — the passes legitimately create values inside observers (the render `Probe`/part probes; the deferred effect flush rebuilding a `Dynamic` inside `with_observer`). `add_value_raw` currently appends every new value to **both** the current scope AND the current observer, so a per-pass scope would capture (and, if disposed at pass end, **destroy**) the `Dynamic`'s just-built subtree — catastrophic. So Q2 requires first changing `add_value_raw` to **own-by-innermost** (observer active ⇒ do *not* also scope-add), then the deny/log check fires only for creations that land in the pass-scope with **no observer active** (genuine bare-`create_*` misuse). That's a **reactive-core semantics change** (it also removes the current "scope backstop for effect-escaped values" that the `Runtime::cleanup` FIXME relies on) → belongs in a WS9b-adjacent item behind its own decision gate, not WS3.

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

- [x] **4.0 Analysis first (maintainer-authored): can `Inert` be stored inline, and what breaks?** The two user classes of `MaybeReactive`/`Inert`: (1) **user-facing props** (reactive-or-inert) — the inline target; (2) **`Layout`**, which uses `Inert` for static layouts and **cannot be inlined freely**: it needs shared identity (the widget's copy and the parent's children-vec copy must observe the same data), and mutating-through-`Copy` already produced the `LayoutMut` bug class — so analyze `Widget::layout` returning `&Layout` and removing `Clone`/`Copy` from `Layout` for safety; there may be deeper problems buried. Inlining also removes blanket `Copy` from `MaybeReactive`, so **all usages are rethought with Copy-loss in mind** (G1; the compiler finds every site, `MaybeSignal::Inert` is the in-tree precedent). **Agreed scope cut:** `Layout` is out of WS4's execution scope — it is resolved by WS5.1's off-graph shared handle; 4.0's Layout analysis is the design input for WS5.1, not a blocker for 4.1. Deliverable: written analysis + go/no-go for 4.1.
- [x] **4.1 Inline `Inert(T)`** (gated on 4.0): in rsact-reactive (`inert.rs`, `maybe/maybe_reactive.rs`, `memo.rs::Memo::Inert`) mirroring `MaybeSignal::Inert`; `Copy` for `T: Copy` only. Flip `maybe::tests::static_wrapper` to **passing** — it is the acceptance test. Ripple through rsact-ui (`label.rs` double-node fix included, setter traits); `Layout` untouched (4.0 scope cut).
- [x] **4.2 Builder-literal leak — becomes a verification item after 4.1.** The problem today (yes — exactly as the review guessed): every `.gap(2u32)`-style call converts the literal into an `Inert` **runtime node**; the setter reads it once, and because view construction runs with no scope active the node is owned by nothing — not merely undisposed but _undisposable_: one permanent node per builder call, forever. 4.0/4.1 dissolve the mechanism (the value arrives inline, by move — no node ever exists). 4.2 = sweep the setter paths (`layout.setter`, the `widget/mod.rs` builder traits) confirming no node-creating conversion path survived, asserted via the 0.4 node-count test.
- [~] **4.3 Singleton de-greed.** **RELOCATED TO WS5 (maintainer decision, 2026-07-09):** the singleton demotions are moved to WS5 — they entangle with the `LayoutModel` memo + the `Copy` `RenderShared`, which WS5 reworks anyway. WS4 delivered the precise site map (every singleton's create/read/write + reactive-vs-habit verdict, see the WS4 STATUS block) as WS5's input; WS5 executes it (see WS5 stage 5.0b). Original charter: The issue: every app/page allocates ~9 reactive nodes unconditionally for things that never change reactively — pure graph freight. Each gets demoted to the cheapest primitive its real use supports: renderer signal → plain field (its own TODO at `ui.rs:61-64` already says it shouldn't be reactive); dev_tools signal → exists only under `simulator`; fonts signal → plain data + an explicit "fonts changed → relayout" call (fonts change at startup, not reactively); page style signal → `MaybeSignal`; `force_redraw` signal → imperative flag through the existing `force` path (fully dies in WS5/WS6); viewport stays inert; the eager per-page `LayoutModel` memo → dissolved by WS5. Payoff beyond node count: every render probe currently tracks `force_redraw`, so that one demotion removes a page-wide fan-out edge per part.
- [x] **4.4 Zero-source render probes — demote, not dispose (investigation; maintainer correction folded).** A part with no reactive deps still needs _force_-rerendering (parent overdraw / damage) — but force needs no graph node: `probe.poll(force=true, f)` runs `f` unconditionally, and the node's only job ("did my deps change?") is eternally "no" for zero deps. So: pull the node out of the runtime and keep the closure as a plain render function — `ElState`'s part entry becomes `enum PartGate { Static, Probe(Probe) }` — saving runtime slots and dispatch. **Soundness hazard to resolve first: conditional first runs** — `if state.expanded { signal.get() }` with `expanded == false` on run #1 would demote the part, and the later reactive read would never re-register → stale UI. Options on record: (a) **opt-in** — the widget declares a part static (safe, zero magic — the default posture); (b) demote-after-N-clean-runs (heuristic, unsound — rejected); (c) **lazy re-promotion** — demoted parts run under a sentinel observer that mints a real `Probe` on the first `track()` call and re-registers (sound and automatic; costs one branch on the track path). Separate investigation; not blocking WS4's main line.
- [~] **4.5 Interactive-widget signal audit.** The issue: widget _constructors_ create real `Signal`s unconditionally, whether or not that instance is ever interactive — checkbox value (`checkbox.rs:36`), slider state+value (`slider.rs:81-82`), knob state, scrollable state (`scrollable.rs:106`), select (`select.rs:143,166`), and **icon allocates a signal AND a memo just for its size** (`icon.rs:74-78`). Measured: 10 static checkboxes → 16 signals. Per constructor, ask "does interactivity _require_ this node, or is it reactive-by-habit?" — keep the former (checkbox's value signal is its job), demote the latter to plain fields / `MaybeReactive` props (icon size is the poster child). Acceptance: 10-checkbox probe 16 → ~10 signals; a static icon → 0 nodes. (Whether interactive state should later move to `ElState`-style flags the way hover/press did: separate topic, parked.)

- [x] **4.6 Storage capacity management (A6).** Fact on record (maintainer question answered): `SlotMap::remove` **does** drop the stored value immediately — the heap payload (`Rc` + boxed data) is freed on dispose. What never shrinks is the backing **capacity**: removed slots become vacant entries reused by future inserts, but the slot array and our dense `SecondaryMap`s (sources/subscribers/owned/mark_seen) keep their high-water size forever — peak node count = permanent RAM. On embedded that peak-sizing is often _desirable_ (deterministic memory). Work: expose high-water marks + vacant counts in `Profile` (extends 0.4), then decide policy — document "capacity = peak" as the contract, and/or an explicit `shrink`-style compaction call for host/long-running use. Coordinate with 9b.1 (the storage rework changes the layout this measures).

**STATUS 2026-07-09 — WS4 COMPLETE (maintainer-confirmed 2026-07-09; PR #11).** 4.0/4.1/4.2/4.4/4.6 landed; **4.3 relocated to WS5** (maintainer decision — singleton de-greed rides WS5's RenderShared+memo rework; WS4 delivered the site map); 4.5 done bar the icon repair (pre-existing `tiny-icons` breakage + needs `SignalOnWrite` — its own pass). _(Correction 2026-07-13, WS13.4: the "needs SignalOnWrite" half was a misdiagnosis — icon's 5 compile errors were all mechanical, repaired in `7cfbc38` without touching rsact-reactive; the SignalOnWrite TODO remains open as a design question but blocks nothing.)_ Branch `ws4-zero-cost-statics`. Commits: 4.0 `65c9095` (analysis doc); 4.1 `33e7ad8`; 4.5 `caa7789`; 4.6 `e06127c`; 4.4 `1b9462c` (investigation doc). **G1 now fully resolved** (was "decided in direction, gated on 4.0"): blanket `Copy` dropped from `Inert`/`MaybeReactive` (`Copy` iff `T: Copy`). Suites: rsact-reactive **76 / 0** (`static_wrapper` flipped to passing — was the last known-fail; **75/1 → 76/0**), rsact-ui lib **60 / 0**, rsact-render **6 / 0**, metrics-probe **11 / 0**; reactive no_std (`unsafe-single-thread`) builds.

**Re-baselined node counts** (`metrics-probe -- record`; deterministic — heap bytes omitted, machine-dependent):

| Scenario | total (old→new) | stored/Inert (old→new) | note |
| --- | --- | --- | --- |
| reactive_only_16 | 33 → 33 | 0 → 0 | no `Inert` — unchanged (as predicted) |
| ui_labels_5 | **32 → 29** | **9 → 6** | 3 builder-literal/prop `Inert` nodes inlined; residual 6 = `Layout::Static` (WS5.1) |
| ui_labels_10 | **52 → 49** | **14 → 11** | residual 11 = `Layout::Static` (10 labels + flex) |

`node_count_regression` (`scenarios.rs`) updated to 33/29/49 and now also **asserts `stored`** (6/11) = the Layout-only residual — this is the 4.2 verification, locked. New 4.6 capacity metrics (host): reactive_only cap 63/vacant 30, ui_labels_5 cap 31/vacant 2, ui_labels_10 cap 63/vacant 14.

**Per item:** **4.0** ✓ analysis doc `docs/plans/2026-07-09-ws4.0-inline-inert-analysis.md` (GO). **4.1** ✓ `Inert<T>(T)` inline; `Memo::Inert` **removed** (keeps `Memo<T>: Copy` unconditional → `MemoTree`/`Page.layout` untouched); `IntoMemo for Inert/MaybeReactive` now mints a real constant memo node (cold path, `T: Clone`). **4.2** ✓ verified: `Layout::setter`'s `Inert` arm writes in-place (`update_untracked`) — no node; asserted via the `stored` counts above. **4.4** ✓ investigation only, no mechanism landed (`docs/plans/2026-07-09-ws4.4-*`); resolves the conditional-first-run hazard (recommend opt-in `PartGate::Static` first; lazy re-promotion gated behind its own gate; N-clean-runs rejected). **4.6** ✓ `values_capacity`/`values_vacant` in `Profile` + snapshot (`serde(default)` forward-compat kept); policy = **capacity = peak** (recorded, not gated; compaction → WS9b.1).

**4.5 (partial)** ✓ slider/knob `state` → plain `Copy` field (only touched in `on_event`, never render/layout → −1 signal each); select `state.selected → selected` setter gated on `selected` being reactive (skips an orphan Signal+Memo+Effect for a static select). KEPT (interactivity requires; read reactively in render): checkbox value, slider/knob value, scrollable state, select state. **Icon DEFERRED (poster child) — reported:** `widget/icon.rs` is behind `#[cfg(feature="tiny-icons")]` and **does not compile there today** — pre-existing breakage (`aec0309`): `Icon::new` arity/move bug, a stale `render(&mut RenderCtx)` signature (trait takes it by value), duplicate `transparent` in `style/mod.rs:174`, lifetime bounds. Node-free static icon additionally needs the `SignalOnWrite` reactive-on-write type (the `icon.rs:66` TODO — the layout must observe the *same* size, so a plain `MaybeSignal` copy won't do). Both need a dedicated icon-repair pass.

**4.3 (investigation complete in WS4; execution RELOCATED TO WS5 — maintainer decision 2026-07-09).** Precise site map delivered (all singletons' create/read/write sites + reactive-vs-habit verdicts) as WS5's input; WS5 stage 5.0b executes the demotions. Rationale for the relocation — the coupling the roadmap's one-liner understated: `fonts` (layout memo _depends on_ it, `page/mod.rs:136`) and `force_redraw` (layout memo _writes_ it, `:163`) are entangled with the **`LayoutModel` memo, which is DEFER-TO-WS5**; `page_style` threads through the `Copy` `RenderShared` (`el/render.rs:29`) alongside them and has no live writer — best demoted with them in one WS5 RenderShared+memo pass. `renderer` (its own TODO wants `Rc<RefCell>`) and `dev_tools` are _not_ memo-coupled and _not_ in RenderShared, but both are shared-mutable UI↔Page state needing `Signal→Rc<RefCell>` surgery in the render **poll closure** (the `?`-in-closure + borrow choreography) — a focused, verifiable follow-up rather than rushed render-path edits. `viewport` already inert (no change); `LayoutModel` memo is WS5. So WS4.3's node-count win lands in WS5 (stage 5.0b); the analysis + sequencing is WS4's deliverable.

**Protocol-reported deviations (approved / for review):** (1) **"Layout untouched" scope nuance (maintainer-approved 2026-07-09):** 4.1 forced the layout _payload_ structs `ContentLayout`/`FlexLayout`/`LayoutKind`/`LayoutData` from `Copy` to `Clone` (they embed `MaybeReactive<NonCopy>` directly; node-free static text is impossible otherwise). The `Layout` _handle_ type, `now_reactive`/`layout_mut`, and the WS5.1 redesign are untouched. (2) **`Select.options` wrapped in `Rc`** — `MaybeReactive<Vec<SelectOption>>` lost blanket `Copy` and `SelectOption` is neither `Copy` nor `Clone`, yet is read by two long-lived owners (the setter effect + the widget); ties to the 4.5 select audit — **filed as backlog A18** (post-core widget-structure refactor, PR #11 review). (3) **`WidgetCtx::Stylist: Clone`** bound added (inline `Inert<W::Stylist>` is cloned into each page; all concrete stylists are `Copy`) — **follow-up to drop the bound documented in WS7.4** (dyn-erasure removes it for free; PR #11 review). (4) **Discovered pre-existing breakage:** the `tiny-icons` icon module does not compile (see 4.5 above) — independent of WS4.

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

**Sessions:** 2–3 · **Risk:** medium-high · **Directions:** D3 + D7(P2) merged · **Depends on:** WS4 (MaybeReactive/Layout field ripples), G8, **WS13 (builder/widget split — resequenced before WS5, 2026-07-09; gates the clean node-free/`Rc`-free/`ElId`-identified 5.1)** · **Feeds:** WS6, **WS20** (reactive node-storage AoS rework — "do after WS5": WS5 changes the node/edge counts and how layout subscribes, so WS20 must re-baseline against WS5's post-off-graph graph).

**Sequencing note (2026-07-09):** WS5.1's off-graph Layout was going to reach for `Rc<RefCell<LayoutData>>` (design sketch below + WS4.0 §100-107). The maintainer chose the cleaner destination — arena-owned, `ElId`-identified, node-free layouts — which requires the builder/widget split first (WS13, now resequenced ahead; see its decision block + `docs/plans/2026-07-09-ws13-views-as-builders-analysis.md`). Consequence: the earlier 5.0 quick-win _per-pass measure cache keyed by `ValueId`_ is **dropped** — it would key on the fake-inert being removed; the ElId-identified incremental relayout (5.2) supersedes it. The 5.0 `force_redraw` purification folds into 5.0b/5.1's off-graph dirty-set. Independent 5.0b bits (`renderer`/`dev_tools` → `Rc<RefCell>`) remain free-standing.

Why: P5's layout half. Whole-page memo, O(N·D) min*size recursion, no measure caching, side-effecting memo, O(N) tree PartialEq. The reconciled design (D3×D7): layout data lives **outside the reactive graph** (`Rc<RefCell<LayoutData>>`, identity preserved — retiring the `layout_mut` reactive-on-write trap class entirely); each \_reactive binding* creates exactly one effect that writes the data **and marks the node id in a page-level dirty set** — D7 gets pay-per-binding, D3 gets write→node invalidation, from the same mechanism.

Stages:

- [x] **5.0 Quick wins (independent, can run any time after WS0.5):** per-pass `min_size`/`ContentSizing` reuse in `model_flex`/`model_layout` (kills O(N·D) → O(N), zero retained RAM); move `force_redraw.set(true)` out of the memo (`page/mod.rs:134`), fire only when the model actually changed (e.g. `Keyed` generation instead of O(N) `PartialEq`) — WS4.3 did **not** land the `force_redraw` demotion (relocated here, see 5.0b), so this stage owns making it an imperative flag. Expected: 3–10× on deep pages, repaint-on-no-change gone. **DONE 2026-07-13 (`7153934`, branch `ws5-incremental-layout`) — but narrower than the pre-resequencing charter above; re-verified per the "Now actionable" instruction.** What landed: the within-`model_flex` per-pass `min_size`/`size` reuse — the placement pass reran `child.min_size(ctx)` (a full subtree descent, one text measure per content leaf) and discarded it for every non-fluid child; now the sizing pass stashes `(size, min_size)` on `FlexItem` and the placement pass reuses them. Locked baseline `ui_labels(10)` single-leaf change: **measures 40 → 30** (−25%; each label 3× not 4×), **visits unchanged at 11**. Zero retained RAM, no `model_layout` signature change (survives 5.1). **Reconciliation of the stale charter (report, per EVOLUTION protocol):** (1) the **"3–10× on deep pages" / "kills O(N·D)→O(N)"** promise assumed the _per-pass measure cache keyed by `ValueId`_ that the 2026-07-09 sequencing note **dropped**; the deep-page O(N·D) descent (the repeated full-subtree `min_size` at `flex.rs:127`) is now owned by **5.2** (ElId-identified incremental relayout skips clean subtrees — strictly better than a per-pass cache), so 5.0's realistic acceptance is "per-pass measure dedup," not "3–10× deep." (2) The `model_layout`-side `ContentSizing` dedup (the sizing `min_size` vs the layout `content_sizing` for the same leaf — measures 30 → ~20) needs a `model_layout` signature/param change, which 5.1 reworks anyway, so it is **deferred into 5.1's kernel rework** rather than churned twice. (3) **`force_redraw` was NOT touched here** — per the sequencing note it folds into 5.0b/5.1's off-graph dirty set; that remains open.
- [ ] **5.0b Singleton de-greed (absorbed from WS4.3, 2026-07-09).** Execute the demotions WS4.3 mapped (site map in the WS4 STATUS block): `renderer` → `Rc<RefCell>` (its own `ui.rs` TODO), `dev_tools` → `Rc<RefCell>` (simulator-gated), `page_style` → `MaybeSignal` (never written), `fonts` → plain data + an explicit "fonts changed → relayout" call, `force_redraw` → imperative flag through the existing `force` path (removes the page-wide per-part fan-out edge). `fonts`/`force_redraw`/`page_style` thread the `Copy` `RenderShared` and entangle with the `LayoutModel` memo — do them **with** the 5.1 off-graph rework; `renderer`/`dev_tools` are independent `Signal→Rc<RefCell>` swaps in the render poll closure. `viewport` stays inert. Re-baseline the 0.4 node counts (ui_labels signal counts drop) in the same commit.
- [x] **5.1 Layout off-graph:** **DONE — PRs #21/#23/#25 (branch `ws5.1-off-graph-removal`), merged → master.** Arena owns `LayoutData` by `ElId`, the retired `Layout` node type is deleted, and passes dispatch by `layout.id()` (arena↔layout positional zip gone). `Layout` → shared `LayoutData` handle outside the graph + binding-effects + page dirty set; node identity = `ElId` recorded at build (also makes the arena↔layout `zip_eq` invariant explicit); `transparent_layout` maps to parent. Consumes WS4.0's Layout analysis (`Widget::layout` returning `&Layout`, `Clone`/`Copy` removal from `Layout` for mutation safety).
- [x] **5.2 Retained tree + boundary stop rule:** **DONE — PR #24 (branch `ws5.2-incremental-relayout`), merged → master.** Off-by-default `incremental-layout` feature; page memo splices the prev `LayoutModel` + recomputes only dirty subtrees with the `(outer_size, min_size)` stop rule; 500-seed differential fuzz (incremental == full) + a Fixed×Fixed 1-visit acceptance. per-node `(last_inputs, outer_size, min_size, flags)` ≤ 64 B/node (feature-gated `incremental-layout`); skip-and-splice clean subtrees; recompute dirty via the unchanged `model_layout` kernel; stop upward when `(outer_size, min_size)` unchanged (tight limits / Fixed×Fixed / `InfiniteWindow` scrollables are natural boundaries). **Differential fuzz test**: random trees + random single mutations, incremental result `==` full recompute.
- [x] **5.3 Changed-set output:** **DONE — PR #26 (branch `ws5.3-changed-set`), merged → master 28ec8b8.** `Rect::union` (rsact-render) + `layout_changed_set(prev, new) -> Vec<Rect>` (feature-gated); geometry channel only (same-size content change moves nothing → repaints via its render probe); 500-seed brute-force fuzz cross-check. relayout returns the list of nodes whose absolute rect changed (old∪new rects) — the damage channel WS6 consumes.
- [ ] **5.x Inherited from WS1.3b (deferred hand-off):** the "self-sufficient pull" hardening (commit path enqueues effects via `mark_node`) was implemented and **reverted** in WS1 — it doubled effect-rerun allocations because the write-time push already queues everything (see WS1's execution notes). WS5 owns the lazier marking that breaks that invariant, so WS5 must (re)introduce pull-side effect enqueueing **together with** its dirty-set marking, without the redundant re-enqueue cost — and re-run `benches/allocations.rs` as the gate.
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

**Sessions:** 3–4 (was 2; 6.4's a–d split + 6.7's implementation land here) · **Risk:** medium · **Directions:** D5(C), D3(P4), D4, D6(P1a) · **Depends on:** WS2, WS5 (changed-set) · **Gated by:** G3, G12.

**Status 2026-08-01:** 6.1 ✓ (PR #29) · 6.2 ✓ + 6.3a ✓ (PR #28) · 6.3b ✓ (PR #30 → e4c0ca0) · 6.9 op-log half ✓ (PR #27) · **6.7 researched and decided** (sans-IO owned tiles) · **6.4 redesigned** from screen strips to damage tiles, split into 6.4a–d with 6.4a gating 6.4d. Remaining in 6.3: **6.3c** (flush-side `fill_contiguous`).

**Status 2026-08-04 (design session — output-type generalization):** 6.4's _interface_ is now specified (**6.4.0**, new): a short-lived `Frame` handle, a **type-level** `FramePolicy` with a compile-time capacity proof, and the surface kept entirely out of rsact. Prompted by a maintainer question this session — "generalize over output type so one API serves a framebuffer over SPI **and** a GPU, without breaking sans-IO." Resolution: GPU _drawing_ is not IO (encoding is pure CPU; `submit`/`present` are the IO), so the existing `Renderer` / `FinishRender` seam was already in the right place and no `dyn` erasure is needed. **Two decisions taken:** region shape is **tight damage rects with an area-test merge** (was: row-bands — see 6.4d), and `Renderer` **stays in `WidgetCtx`** (static dispatch throughout; the `dyn Renderer` option was costed and rejected, see 6.4.0). **One retraction:** a mid-session proposal to add `Probe::is_dirty` + `Probe::poll_again` to rsact-reactive was **withdrawn — 6.4c already supersedes it** and does so better; the reactive crate needs **no changes** for 6.4 (details in 6.4c). Baseline re-verified on `27115cc` (PR #32 merged): suites 75/93/21/16 green, size-probe gate green, benches compile.

**Status 2026-08-05 (design session — 6.4c's mechanism, before any code):** 6.4c's specified mechanism turned out to be unreachable and was re-derived from scratch with the maintainer; the *semantics* survive untouched, the *implementation site* moves. Five decisions, all in 6.4c(A)–(G): **(1)** the collect pass mutes at a seam rsact owns — `impl Renderer for RenderCtx`, `renderer` field **private** — not at the backend (`set_muted` rejected as "logic only we need"), not by substituting a renderer type (impossible: `Widget::render` is reached through `dyn Widget<W>`), and `NullRenderer` goes back to being a test stub; **(2)** `RenderMode::{Fused, Collect, Paint}` with **Fused kept as the default**, so goldens and metrics are untouched; **(3)** `Canvas`'s closure takes `&mut RenderCtx` now and a purpose-built **`CanvasRenderCtx`** later (documented in full at the maintainer's request — the motive is rsact styles inside canvas closures, which a bare `Renderer` can never offer); **(4)** `&mut dyn Renderer` **rejected on a hard fact** — `E0038`, the trait is dyn-incompatible because of `SURFACE_UNITS`, i.e. 6.4.0(iii)'s compile-time tile-capacity proof, verified both ways; **(5)** the traversal prune needs **no per-node storage** — 6.4b's costed `subtree_fits`-vs-union-`Rect` decision is **struck**, because a widget-declared clip makes containment structural and `clip_bounds()` already reports the composed rect. Two follow-ons: clipping becomes **widget behaviour** rather than a render-body call (also unblocks the scrollable and 6.11), and the "a widget never draws outside `outer`" invariant is audited, found **already false**, and filed as **ISSUE-3** with an LVGL-style `paint_bounds`/`ext_draw` extension point designed now and computed later. No code yet — nothing in this status line is measured.

**Status 2026-08-05 (execution session):** **6.4b paint side DONE** (branch `ws6.4b-culling`) — `Renderer::clip_bounds()` + a geometry gate in `render_part` + a per-pixel filter in `DrawTargetProxy` take a region replay from ×10.00 to **×1.00–1.02**, i.e. onto the `required` floor, with `wasted` down from tens of thousands of ops to 0–5. **The precondition for 6.4d is discharged.** Also fixed a latent bug that 6.4d would have hit: **nested clips did not compose**, so a widget clip inside a region clip would have let drawing escape its tile. Traversal (`visits` ×10.00 vs a ×1.19–2.85 floor) **deferred into 6.4c** with its storage choice costed — the collect pass visits every node anyway, so it belongs inside that split. The closed-form text-run culling half of (ii) moves to **WS15 / the `Clip`/`Ellipsis` TODO**: it needs rsact to own the line loop, and the pixel filter already took the measurable cost to its floor. **NEXT: 6.4c.**

**Status 2026-08-04 (execution session):** 6.4.0 **COMPLETE** — (i)+(ii)+(iii) merged as PR #33, (iv) as PR #34 (layout de-memoized: `Memo<LayoutModel>` → owned field + `relayout_if_needed`; `force_redraw`/`full_flush` demoted to plain `bool`s, killing the per-part broadcast; −3 reactive nodes per page). **6.4a DONE** (branch `ws6.4a-tile-measurement`) — the estimates are now measurements; see the item for the table and the five findings. Net effect on the plan: **6.4b is promoted to a hard precondition for 6.4d** (uncalled ×emit is the region count outright), 6.4b's own subtree-prune spec is corrected for soundness, 6.4d(1)'s merge threshold is set at area ×2.0, and one new gap is filed as **ISSUE-2** (text changes bypass the arena dirty set, so every one is a blanket relayout — with `incremental-layout` on as well). Suites 75 / 95 / 104 / 5 / 48+3+2 / 19.

Why: P5's render half — the biggest _practical_ gap vs LVGL/Slint. Fine-grained observers already know what re-rendered, then `finish_frame` streams **every pixel of the viewport** through a per-pixel iterator (`eg/framebuf.rs:147-161`, `eg/output.rs:7-25`); no dirty rects; e-paper region refresh impossible; full-screen SPI transfer per change. Also the blanket `force_redraw` defeats per-part gating (three directions demanded its death).

Work items:

- [x] 6.1 **Replace blanket `force_redraw`** with targeted invalidation: WS5's changed-set → `needs_redraw` on affected `ElId`s (mechanism exists: `ElState::take_needs_redraw`, `observe_with_force`); clear only old∪new rect unions. Keep an explicit full-invalidate escape hatch (page enter, dev tools). **DONE — PR #29** (feature-gated `incremental-layout`): `layout_repaint_roots(prev,new)` picks the nearest SIZE-STABLE ANCESTOR of each moved node (its clear covers old∪new child positions — no ghost; 500-seed fuzz-verified coverage); the page memo marks those (`RedrawReason::LayoutChange`) instead of the blanket `force_redraw`/`full_flush`, which stays for full relayouts + root-reached changes (`RepaintRoots.full`). **Key render-pass fix:** transparent containers (Flex) have no-op renders that never `render_part`, so `render_subtree_body` now clears a transparent `LayoutChange` repaint root's outer + records damage + propagates `parent_dirty` itself. Integration test: child grows in a fixed 40x40 parent → damage == [40x40], not the 64x64 viewport.
- [x] 6.2 **Dirty-region accumulation**: render pass records executed parts' clip rects into a small LVGL-style joined-areas list (4–8 rects) on `RenderShared`. **DONE (paint channel) — PR #28** (`ws6.1-damage-pipeline`): `RenderShared.damage: &RefCell<Vec<Rect>>`; the pass pushes each REDRAW-ROOT's absolute `outer` (a part repainting without a parent clear — `!parent_dirty`, so no redundant sub-rects); `Page.render` flushes the set via `finish_frame_regions`; idle frame → empty → flush nothing; full-invalidate (layout/first render) → whole viewport via a NON-reactive `full_flush` flag (reading `force_redraw`'s value spuriously re-dirties the render probe). Not yet joined/deduped (overlaps double-flush, harmless — LVGL-style area-join is a follow-up). Content change = minimal flush NOW; layout change still full (targeted in 6.1).
- [ ] 6.3 **`FinishRender` regions API** + row-contiguous output: `finish_frame_regions(target, &[Rect])` using `fill_contiguous`/scanline runs instead of the per-pixel iterator (likely 5–10× SPI win even before partial flush — D4 measured path). **Final-sweep addition — the render side too:** `PackedFramebuf` implements only `draw_iter`, so every background/clear/fill is per-pixel bit-twiddling (`eg/renderer.rs:300-310`, `eg/framebuf.rs:201-213`); implement `fill_solid`/`fill_contiguous` on it (whole-byte writes for mono, `slice::fill` runs for RGB) — an 8–32× render-side win, distinct from the flush-side fix. **6.3a DONE (regions API) — PR #28:** `FinishRender::finish_frame_regions(target, &[Rect])` with a default full-flush fallback (safe for every backend); `Framebuf::output_region` (rect ∩ viewport) + EG `renderer_output_regions` + region-aware tiny-skia. **6.3b DONE (fill_solid) — PR #30:** `PackedColor::solid_storage` + `PackedFramebuf::fill_solid` — per-row three-part split (partial head word · run of WHOLE words `slice::fill`ed = whole-byte mono / RGB run · partial tail word); whole words are inside the row so no cross-row (shared edge-byte) corruption — NO per-row stride needed (the earlier stride worry was overcautious for a solid fill). `EGRenderer::fill_solid` (DrawTarget) override routes solid fills there instead of the default per-pixel fan-out (mirrors `draw_pixels`' viewport dispatch). Differential-fuzz-proven byte-identical to `draw_iter` (300 RGB + 300 mono) + EGRenderer-level test. **Still deferred:** `fill_contiguous` (varying-colour runs — no `slice::fill` benefit, rarer), LVGL area-join of overlapping damage rects (6.2 follow-up), flush-side scanline batching to the display driver (belongs with 6.4 strip/regions output).
  - [ ] **6.3c Flush-side `fill_contiguous` (split out 2026-08-01).** `Framebuf::output_region` still iterates `region.points()` and emits **individual pixels**, which on a mipidsi-class driver is an address-window set **per pixel** — the single largest defect on the flush path, and the reason any "N ms SPI flush" figure is currently fiction. Fix: stream each region row-run through `DrawTarget::fill_contiguous`. **Kept as its own item even though 6.4d's tiled path bypasses it** — the `DrawTarget` route still serves the simulator, the 6.9 goldens, and every generic embedded-graphics driver, and it has no dependency on tiles. Note `fill_contiguous` _is_ the address-window abstraction: mipidsi issues CASET/RASET once and streams; ssd1306/sh1106 own their page loops internally. rsact needs no controller knowledge here.
- [ ] 6.4 **Partial-buffer (tiled) rendering** — **REDESIGNED 2026-08-01** (design session; supersedes the original "full / N-line strip / direct-to-target framebuffer modes" framing). Acceptance unchanged: 240×240 RGB565 heap 112.5 KiB → <20 KiB (D6 P1). Reference target: **ST7789/ST7735 SPI RGB565** (G3's color reference).

  **Why the framing changed.** Screen strips and probe-gated damage rendering _are_ mutually exclusive — but the conflict is with **probe-as-paint-gate**, not with damage rendering. `render_part`'s `probe.poll` both detects change _and_ authorizes paint, so a second pass over a strip finds every probe clean and paints nothing. Separating those two roles dissolves it (6.4c). And once rendering goes **direct-to-tile** (6.7 decision 1), the tile buffer holds exactly its sub-rect in its own scan order, so it is contiguous by construction for _any_ rect anywhere on screen — damage never needs reshaping into full-width strips at all. Strips survive only as the degenerate chunking of a full-screen damage rect. **General principle worth reusing** (from the 6.7 research): a constraint that looks like it belongs to the _data_ (damage shape) often belongs to the _storage decision_ (who owns the pixels).

  **Cost model** (AA rasterizers deliberately excluded — they are acknowledged stubs, see backlog; centering the decision on them would be measuring the wrong thing). Per-pixel work is **invariant**: each output pixel is written once across the whole schedule. Only **per-object** work repeats — node visit, style resolve, primitive setup, text-run dispatch — and at ×1.6–2.0, not ×K: an object of height `h` over bands of height `s` is visited `1 + (h−1)/s` times, leaves dominate by count, and full-height containers are transparent no-ops. Estimated cold-frame cost **+14%** (100 nodes) to **+49%** (500 nodes); interactive frames **+0%** (damage fits one tile, no repetition at all). **MEASURED 2026-08-04 by 6.4a — the model holds where it was checkable, with two amendments.** Per-object ×1.42–2.17 at 10 regions (estimate was ×1.6–2.0 ✓) and per-pixel ×1.00 exactly ✓. Amendment 1: **traversal is worse than drawing** (×1.19–2.44), because "full-height containers are transparent no-ops" is true for ops and false for node visits. Amendment 2: **"interactive +0%" holds only for paint-only changes** — a text change is a full cold frame today (ISSUE-2). All three figures are the *culled* floor; uncalled, ×emit is the region count outright, which is why 6.4b became a precondition. Cold frames are **flush-bound** — 57600 px × 16 bit @40 MHz SPI ≈ 23 ms against 4–9 ms of paint — so with 6.7's ping-pong the paint hides inside the transfer and tiled is expected **~20% faster wall-clock** than a single framebuffer, which cannot pipeline paint against DMA (one buffer is both paint target and DMA source; double-buffering recovers the pipeline at 225 KiB). **These are estimates. 6.4a exists to replace them with measurements before 6.4d is committed.**

  - [~] **6.4.0 Prerequisites + interface spec (added 2026-08-04 — do these BEFORE 6.4a/b/c/d).**

    **Status 2026-08-04: (i)+(ii)+(iii) DONE — PR #33 OPEN (branch `ws6.4.0-render-prep`, NOT merged; `CLEAN`/`MERGEABLE`, CI green).** Seven commits `97c4f85..54ca9ea` plus merge `4eab63b` (see below). Suites went render 21 → **26 unit + 3 doctests + 2 `compile_fail` doctests**; reactive 75 / ui 93 / metrics 16 unchanged; powerset, thumbv7m floor, size-probe, fmt all green — re-run after every commit, not only at the end. **REMAINING: (iv) only** (the layout de-memoization; wants a clean tree, so after #33 merges). **MERGE NOTE:** PR #31 (delete the layer dimension) landed mid-flight and restructured the same files; #33 carries a merge commit rather than a rebase because 5 of its 7 commits touch `renderer.rs`/`eg/renderer.rs`/`tiny_skia`, so a rebase meant five resolutions against partially-migrated intermediate states instead of one against the final shape. Two adaptations worth knowing: `Viewport { layer, kind }` collapsed to a bare `ViewportKind` (so `current_viewport().kind` → `current_viewport()`), and `sub_viewport` was deleted (push `ViewportKind::Clipped(area)` directly). None of these depend on tiles; three are live bugs and the rest are cleanups 6.4 would otherwise have to make mid-flight. **Ordering:** (i) the three render-crate bugs, (ii) the render-trait cleanups, (iii) the capacity chain, (iv) the layout de-memoization (largest blast radius, wants a clear tree), then 6.4a.

    **The interface (settled this session).** `ui.start_frame::<P: FramePolicy>() -> Frame<'_, W, P>` — a short-lived handle borrowing `&mut UI`; `frame.next_region() -> Option<Rect>` (a `&mut self` cursor, not an iterator, so it cannot alias `render`); `frame.render(&mut W::Renderer, region)`; `Drop` releases. The `&mut UI` borrow makes `tick()` mid-frame a **compile error** rather than a documented contract (upgrades 6.4c's `debug_assert`). Three properties this buys, in the maintainer's words as constraints: **IO is the user's** (every `.await` is in their loop — verified against a real Embassy ST7789 two-task `FREE`/`READY` design), **rsact never holds a framebuffer** (see surface ownership below), and **neither the GPU nor the framebuffer path imposes API on the other** (`FramePolicy` decides how many regions a frame is cut into; a GPU takes `Whole<W,H>` = one region, one walk, one scissor).

    **Surface ownership — rsact never learns what a Surface is.** The framebuffer enters the renderer through **inherent** backend methods the _user_ calls before handing the renderer to rsact (`EgTileRenderer::attach(buf)` / `detach() -> buf`; a GPU has `scissor`/`submit` instead), so no `type Surface` appears anywhere in rsact and the two backends never see each other's vocabulary. This also keeps `W::Renderer` free of a lifetime parameter, which is what makes `WidgetCtx: 'static` survive — a renderer that _borrowed_ the tile would be `TileRenderer<'a, C>` and unnameable as an associated type of a `'static` ctx. **Amends 6.7 §11-Q3** ("buffer pool registered once at UI construction"): the pool is the _user's_ (`StaticCell` + channels), never registered with rsact. **Rejected:** passing the buffer to `Frame::render` (a GPU would receive something it has no business with) and moving the surface into `Renderer::begin_tile(Self::Surface)` (reintroduces `type Surface`, and an owned-buffer instantiation would be a multi-KB by-value move on a Cortex-M stack).

    **Static dispatch confirmed; `dyn Renderer` costed and rejected.** Object safety is needed for the _widget_ tree (`El<W>` is `Box<dyn Widget<W>>`, `el/mod.rs:47`) and is already satisfied by `W::Renderer` being reachable from `W` — a generic `render<R>` method would make `Widget<W>` non-object-safe and the arena unbuildable, and `Canvas`'s `Box<dyn Fn(&mut W::Renderer))` (`canvas.rs:52`) cannot be written generically at all (`dyn for<R: Renderer> Fn(&mut R)` has no syntax). The erasure cost that decides it: `DrawTargetProxy::draw_iter` (`eg/renderer.rs:56`) calls `renderer.pixel()` **once per glyph pixel** and is the only path fonts use (`font/fixed.rs:122`, both the embedded-text and u8g2 arms), so `&mut dyn` would mean a vtable call per pixel with no inlining into the framebuffer store, on O(10⁴) covered pixels per text-heavy frame. Memory saving would be nil (one renderer variant per binary ⇒ no monomorphisation fan-out). **Escape hatch if runtime backend switching is ever wanted: a user-side `enum AnyRenderer` implementing `Renderer` by matching — ~40 lines, zero framework cost.** Consequence: **6.3c stays a pure optimization** (it was briefly a prerequisite under the erased design, and its signature is free to be `impl Iterator` again).

    - [x] **(i-1) 🐞 `EGRenderer::pixel_alpha` read/write asymmetry** (`eg/renderer.rs:173`): reads `current_canvas().pixel(p)` **raw** while writing through `draw_pixels`, which applies the viewport. Latent today (nothing constructs `ViewportKind::Cropped`) and guaranteed wrong the moment a tile origin exists. Fix before any translation lands. See also constraint (b) below for the deeper RMW consequence.
    - [x] **(i-2) 🐞 `fill_solid` duplicates the coordinate math** (`eg/framebuf.rs:265-283`, new finding 2026-08-04): it intersects against `Rectangle::new(Point::zero(), size)` and indexes `area.top_left` directly, **bypassing `point_to_subpart`** (`:203`) which every other path uses. Add a tile origin and fix only one of them and you get the worst outcome — per-pixel writes land correctly while WS6.3b's fast solid fills land in the wrong row, i.e. a _plausible_ image rather than an obvious failure. Extract one shared `to_local(&self, Point) -> Option<(usize, usize)>` (or at minimum a shared origin+bounds step) so the two paths cannot drift again. General lesson worth keeping: 6.3b bought a real 8–32× win by writing whole words directly, but it **forked the address computation** — a fast path that duplicates addressing must share the addressing step or it silently rots.
    - [ ] **(i-3) 🐞 `area % pps == 0` constructor panic** — already owned by **6.5**; the row-padded replacement is `units_for` from (iii) below, so 6.5 consumes this item rather than re-deriving it.
    - [x] **(ii-1) `Renderer::clipped(area, impl FnOnce(&mut Self))` → `push_clip`/`pop_clip` clip stack.** Only **2 call sites in rsact-ui** (`el/render.rs:222` in `clip_inner`, `el/render.rs:477`) plus 4 backend impls. Wanted independently: a tile loop re-establishes clips per pass, which closure nesting fights, and it de-nests `clip_inner`'s struct-update expression.
    - [x] **(ii-2) Delete `Renderer::Options` + `set_options`** — dead: all 7 impls are `type Options = ()` with an empty body (`renderer.rs:74`, `record.rs:211`, `tiny_skia/mod.rs:161`, `eg/renderer.rs:353`+`:513`, `renderer.rs:224`, `page/mod.rs:1811`). Clears the trait before (ii-3) adds to it.
    - [x] **(ii-3) `Renderer::begin_region(Rect)` / `end_region()`, both defaulted to no-ops.** "rsact is about to paint this sub-rect": a tile framebuffer sets its origin offset, a GPU sets a scissor, a renderer whose surface already covers the frame ignores it — and **the default is _correct_, not merely permissive**, since a full-frame surface receives absolute coordinates and needs no offset (`NullRenderer`, `RecordingRenderer`, full-frame `EGRenderer` all work untouched). Deliberately in `Renderer` and not a separate `TileAware` trait: it says _where_ you are drawing, the same category as `size()` and `push_clip()`, so no `where` clause appears on `Frame::render`. This is a **refinement of 6.4d's** "`Renderer` never learns tiles exist, seeing only a clip rect" — it still never learns about _tiles_, only about regions.
    - [x] **(ii-4) `NullRenderer<C>` generic over colour** — the existing one is hard-wired to `type Color = NullColor` (`renderer.rs:220`). 6.4c's collect pass must run widget bodies against a no-op `Renderer<Color = W::Color>`, so a colour-generic null renderer is a hard prerequisite for 6.4c. (`RecordingRenderer<C>` from 6.9 already has the right shape and may just serve, with its op log ignored.)
    - [x] **(iii) Compile-time tile-capacity chain — VERIFIED on rustc 1.96 / edition 2024, no `generic_const_exprs`.** **DEVIATION as built (PR #33):** this item proposed a new `PackedColor::BPP`; the crate already had `pps()` (pixels per storage unit), which is the same quantity and strictly less ambiguous — `Rgb666` is 18 bits inside a `u32`, and `pps == 1` is the fact capacity depends on. So `pps()` became an associated `const PPS` with the method kept as a **defaulted forwarder**: no `C::pps()` call site changed, and the value became usable from a `const fn`, which a trait method is not on stable. Four static links: `PackedColor::PPS` + `type Storage` → free `const fn units_for<C>(w, h)` (row-padded: `ceil(w·BPP / bits_per_unit) · h`) → `PixelBuf<C>::UNITS` on the user's buffer type → `Renderer::SURFACE_UNITS` (defaulted `usize::MAX` = "my surface always covers the frame"). `start_frame` then holds one `const { assert!(units_for::<W::Color>(P::MAX_W, P::MAX_H) <= <W::Renderer as Renderer>::SURFACE_UNITS, "…") }`, so **a `Frame` whose regions could overflow the surface cannot be obtained**. Post-monomorphization error; the instantiation string names both buffer and policy, e.g. `UI::<Wtf<TileRenderer<Rgb565, [u16; 5760]>>>::start_frame::<Tiles<240, 25>>`. Protocol-agnostic by construction: `[u16; 5760]` and `AsBytes<[u8; 11520]>` both report 5760 units for a 240×24 RGB565 tile. **Coherence trap found and solved:** the natural `impl<C: PackedColor<Storage = u8>> PixelBuf<C> for [u8; N]` and `impl<C: PackedColor<Storage = u16>> PixelBuf<C> for [u8; N]` are **E0119** (Rust has no negative reasoning over associated types), so the wire-format case needs a newtype — `AsBytes<B>` — which also reads as documentation at the call site (`AsBytes` is exactly what you hand to DMA). `N / 2` truncates, which is the safe direction. **Row padding is what makes 1-bpp correct** and is the fix (i-3)/6.5 needs: a 122-px mono row is 16 bytes, not 15.25, so `Tiles<122, 24>` needs 384 units and `[u8; 360]` must fail to compile (it does). **Proof now lives in the crate, not the scratchpad** (the session scratchpad is ephemeral and its files are superseded): 3 doctests for the positive values, 2 `compile_fail` doctests for the negative cases (one row too tall; a mono buffer sized by area instead of padded rows), and a unit test pinning the padding boundaries + `AsBytes` truncation — all in `rsact-render/src/eg/framebuf.rs`. The `compile_fail` pair was verified to fail for the RIGHT reason: enlarging both buffers makes them compile, which turns those tests red.
    - [ ] **(iv) Remove `Memo<LayoutModel>`** (`page/mod.rs:56`) → an owned `Page` field recomputed by an explicit `&mut self` call. This finishes WS5.1's "layout off the reactive graph" (5.1 moved `LayoutData` into the arena; the _model_ is the last reactive holdover) and it is what makes the frame's layout snapshot sound: `Frame` needs the model stable for the whole multi-pass frame, `LayoutModel` is `Clone` (`model.rs:108`) but it is a recursive tree so cloning per frame is a deep heap copy _every frame_ — exactly what retention exists to avoid. Borrowing the memo's value instead turns any mid-frame relayout pull into a `RefCell` conflict, i.e. a **panic**, violating WS1.8. Owned-field + `&mut self` makes it a **compile error** instead, the same trick as the `Frame`/`tick()` guarantee. Maintainer's verdict on the `try_with` + log-and-degrade alternative: "a footgun and a crutch" — do not ship it.
  - [x] **6.4a Measurement harness (step 0 — gates 6.4d). DONE 2026-08-04 (PR TBD, branch `ws6.4a-tile-measurement`).** Render a representative page through 6.9's `RecordingRenderer` full-frame vs. as an N-tile schedule; diff op counts per widget class for the real per-object multiplier. Exposes any primitive whose op count fails to shrink under clipping (i.e. where clip is a write-filter rather than a loop bound). Doubles as the **tile-invariance** regression: per-tile op-logs, each intersected with its tile, must reconstruct the full-frame log. Pixel-level equality waits on 6.9's deferred PNG half.

    **As built.** `rsact_render::schedule` owns the arithmetic (`TileSchedule` · `ScheduleLog` · `tile_invariance` · `ScheduleReport` · `merge_verdict`), `rsact_ui::test_support::tile_probe` drives real pages, `rsact-ui/tests/tile_schedule.rs` runs 6 pages × 4 schedules with three blessed goldens. Three definitions carry the whole item: **emitted** = ops an N-region replay issues today; **required** = for each full-frame op, the number of regions its `DrawOp::bounds` intersects — the floor a perfect geometric cull reaches, computed from real geometry rather than estimated; **wasted** = emitted − required. `DrawOp::bounds` is documented as *the culling contract* (if the bound hits a region, the replay must emit there; a cull may skip only what the bound misses), so 6.4b must cull on the same predicate or the check becomes vacuous. Prerequisite found and fixed inside the item: `Polygon` kept only a point count and `Path`/`Image` no geometry at all, and `Path` is not hypothetical — it is Checkbox's check icon.

    **Why region passes exist before 6.4c:** a tile pass is a second pass over an already-painted frame, which the probe-gated render refuses. `force_redraw` is the way through *because* WS6.4.0(iv) turned it from a per-part subscription into a value OR-ed into every part's gate — which is exactly 6.4c's geometry-selected paint. So the harness captures the real schedule minus the culling 6.4b will add.

    **THE NUMBERS (240×240, `rows-24` = 10 regions; full table in `tests/goldens/tile_schedule_240.txt`).** `struct` = every kind but `Pixel` (the per-object term); `all` includes per-pixel work; `visits` = layout-node traversal.

    | page | nodes | ops | of which `Pixel` | `struct` ×req | `all` ×req | `visits` ×cull | ×emit (all) |
    | --- | --- | --- | --- | --- | --- | --- | --- |
    | labels-12 | 13 | 2490 | 2478 | **1.50** | 1.00 | **2.15** | 10.00 |
    | buttons-8 | 17 | 934 | 918 | **2.00** | 1.02 | **2.24** | 10.00 |
    | checkboxes-8 (no text) | 9 | 24 | 0 | **1.42** | 1.42 | **2.44** | 10.00 |
    | scrollable-20 | 42 | 576 | 552 | **2.17** | 1.05 | **1.19** | 10.00 |
    | option-rows-6 | 19 | 697 | 673 | **1.75** | 1.03 | **2.26** | 10.00 |
    | mixed | 9 | 460 | 451 | **1.67** | 1.01 | **2.44** | 10.00 |

    Five findings, in order of how much they change the plan:

    (1) **The cost model's per-object estimate is CONFIRMED, and its per-pixel invariance is confirmed exactly.** Structural ×req lands at **1.42–2.17** against the estimated ×1.6–2.0 (band schedules cluster 1.42–1.75; the 80×80 grid reaches 2.33 on labels, as expected — more boundaries). `Pixel` ×req is **1.00 on every page**: each glyph pixel belongs to exactly one region, so per-pixel work genuinely does not repeat *under a perfect cull*. Note what the same rows say about instrumentation: 96–99% of the op log is per-pixel work, which is why the report carries the structural subtotal — an undifferentiated op count measures the term the model calls invariant.

    (2) **×emit is 10.00 on every page, per-pixel work included — so 6.4b is a PRECONDITION for 6.4d, not an optimization after it.** Today's clip is a write filter (A19), so an N-region replay pays N× for *everything*: `labels-12` at 10 regions emits 24900 ops where 2496 are required — **22404 wasted**. Without culling, tiling a cold frame costs ×10 paint, which no flush saving recovers. The roadmap already sequenced 6.4b before 6.4d; this promotes that from tidy ordering to a hard dependency.

    (3) **Traversal, not drawing, is the worse multiplier — and the cost model understates it.** `visits` ×cull is **1.19–2.44**, above the structural op multiplier on every page. The model's "full-height containers are transparent no-ops" is true for *ops* and false for *visits*: a viewport-tall `Flex` emits nothing yet is visited, has its state extracted and is recursed through once per region. The traversal term is what 6.4b(i)'s subtree culling and 6.6's dirty list both attack, and 6.6's "may not clear the bar on its own" note should be re-read with these numbers.

    (4) **6.4b(i) as specified is UNSOUND — a subtree prune on `layout.outer` alone drops content.** The harness counts nodes whose `outer` escapes their parent's `outer` (`VisitReport::escaping`): **1 on the scrollable page**, 0 elsewhere. That is scrollable content, exactly what `ClipPath::InnerRect` exists for. 6.4b must prune on the subtree's union extent, or only where a clip bounds the children — recorded on 6.4b itself.

    (5) **6.4b pays today, without tiles, and the amount is now measured.** On the scrollable page a *single* full-viewport pass has required 565 < full 576: **11 of 21 `RoundedRect`s are drawn entirely off-screen every frame**, half the block work on that page. This is 6.4b's "ships value with or without tiles" claim, quantified.

    **Region shape (`tile_shape_240.txt`) — 6.4d(1) vindicated, hard.** Tight 16×16 rect around one checkbox: **3 required ops**. The 240×24 band containing it: **118** — 39× more, because the band catches the neighbouring label's glyph pixels. Both regions derived from the frame itself (the tight rect is the checkbox's own bound, i.e. what `render_part` pushes as damage), so this is real geometry, and it is asserted as an inequality, not just recorded.

    **Merge threshold (`tile_merge_240.txt`) — 6.4d(1)'s knob, measured instead of guessed.** Paint and transfer terms for three pairs on one frame: adjacent (20 px apart) ops ×1.00, area **×1.12**; far (opposite corners) ops ×3.62, area **×23.62**; overlapping ops ×0.98, area **×0.75**. Reading: the **area ratio is the discriminator** and the decision gap is enormous (1.12 vs 23.62), so the threshold is not delicate — **anything in ×1.5–×4 separates these cases; take ×2.0** (LVGL's neighbourhood) and revisit only if a real page lands inside the gap. Two corollaries: overlapping rects must **always** merge (both terms improve — the overlap was being painted twice), and a merge that leaves the op count unchanged is still a win because it halves the per-region command overhead.

    **Interactive frames (`tile_damage_240.txt`) — the "+0%" claim is HALF true.** Measured on rsact's own damage rects: cold frame 480 ops; **paint-only** change (checkbox toggle) → 1 region, 0.4% coverage, **3 required ops** — the claim holds, emphatically; **text** change → 1 region, **100% coverage, 478 ops**, i.e. a full cold frame. Cause is a channel mismatch, not a threshold: a `Label`'s text lives in its layout (`ContentLayout::text`) and is read during measurement, so it arrives through the *tracked-read* channel and never marks `ElArena`'s dirty set — WS5.2's incremental path requires a non-empty dirty set and falls through to a full recompute, so WS6.1's targeted roots are never computed and `blanket`/`full_flush` follow. **Measured identical with `incremental-layout` ON**, which is the surprising half. Filed as **ISSUE-2**; consequence for 6.4d is that the interactive win covers paint-only changes, while every text change costs the cold multiplier until that channel is fixed.

    **Also measured and worth keeping:** `FillSolid` ×req is exactly the region count (10.00) wherever a page has a full-viewport fill — the harness independently rediscovers constraint (b)'s "every tile must be initialized to the true background", at its true price of one whole-tile fill per region.

    **Test discipline.** Every assertion in the integration test passes for rsact *today* (nothing culls yet), so on its own it proves nothing about what the check would catch — the teeth are six unit tests in `schedule` that feed the checker deliberately broken replays (a dropped op, a half-drawn straddler, region-relative coordinates), plus one end-to-end inversion: making region passes skip the force turns the golden test red with 2490 `Missing` violations. `ci-test.sh` gained a job for the new target, because `--lib` cannot reach an integration test and the integration form is deliberate (metrics-probe will consume the harness from outside the crate) — same gap class as the incremental-layout job.
  - [~] **6.4b Culling** — what turns the per-object multiplier from ×K into ×1.8. **Promoted by 6.4a from "ships value" to a hard PRECONDITION for 6.4d:** measured ×emit is exactly the region count on every page (10.00 at 10 regions), per-pixel work included, because today's clip is a write filter — so tiling without culling costs ×N paint, which no flush saving recovers. **And two 6.4a corrections to this item's own spec:** (a) **(i) as written is UNSOUND** — the harness counts 1 node per scrollable page whose `outer` escapes its parent's `outer` (`VisitReport::escaping`), so a subtree prune on the parent's rect drops scrolled content; prune on the subtree's **union extent** — and note the tempting alternative is unavailable: "prune on `outer` except where a clip bounds the children" needs a clip, and `ElState::clip_path` is only ever initialised to `None` (`el/state.rs:86`, set nowhere), so `render_subtree`'s `ClipPath::InnerRect` arm is dead and `Scrollable`'s own clip call is commented out with a TODO (`widget/scrollable.rs:344-350`). Overflowing content is bounded only by the framebuffer viewport today, on every backend — which also sharpens 6.11: no widget requests clipping at all, so the observable Scrollable overflow is not tiny-skia-specific. Keep `escaping` at 0 as the guard; (b) the prune predicate must be `DrawOp::bounds`-compatible — a cull tighter than that bound passes review and cracks on screen, and 6.4a's invariance check is written against exactly that predicate. **Value without tiles, now measured:** on the scrollable page a single full-viewport pass draws 11 of 21 `RoundedRect`s entirely off-screen — half the block work on that page. (i) hierarchical subtree culling on `layout.outer ∩ tile` in `render_subtree`; (ii) arithmetic text-run culling — first/last visible glyph index is closed-form from the fixed advance (`font/measure.rs`), plus skipping lines outside the clip's y-range; converges with the existing `Clip`/`Ellipsis` TODO in `font/fixed.rs`, which already wants a custom line loop; (iii) hoist the per-pixel `layers.binary_search` out of `EGRenderer::draw_pixels`. **Ships value with or without tiles** — every `ClipPath::InnerRect` subtree and every Scrollable pays full cost for off-screen content today.

    **PAINT SIDE DONE 2026-08-05 (PR TBD, branch `ws6.4b-culling`); traversal side DEFERRED to 6.4c by maintainer decision.** (iii) was already retired by PR #31. What landed:

    **The seam.** `Renderer::clip_bounds() -> Option<Rect>` — deliberately the same one-directional contract as `DrawOp::bounds` (*anything whose bounds miss it cannot affect the output*; never report narrower than you clip), so the cull and 6.4a's invariance check are the **same predicate**. `None` = "not reported" disables culling, which is the safe direction *and* the only correct answer for `NullRenderer`, whose `size()` is zero — a default of `Rect::new(zero, size())` would read as "clips everything away" and cull every widget on every headless page (that trap is now a test). `Fullscreen` reports the **surface rect**, not `None`: reporting `None` there would have confined this whole item to tiled mode instead of paying on an ordinary frame.

    **🐞 Bug found and fixed on the way: nested clips did not compose.** `push_clip` stored the raw area and every write filter consults only the TOP of the viewport stack, so a clip *wider* than its parent widened the effective clip. Unreachable today (nothing sets `ElState::clip_path`) and live the moment either 6.4d pushes a region clip with a widget clip inside it — **drawing escaping its tile** — or `Scrollable`'s commented-out clip is implemented. Fixed via `ViewportKind::nested_in`, applied in all three stacks (`surface::Canvas`, `EGRenderer`'s inline one, `RecordingRenderer`'s new one). Intersecting on push is also what makes the top of the stack *be* the effective clip, i.e. what makes reading it for culling exact rather than approximate. Test asserts on the FRAMEBUFFER, not the stack.

    **(i) paint-level cull** — in `render_part` (the shared wrapper, per AGENTS.md, and the one place `layout.outer` is known absolute): a part whose outer misses the clip gets no clear, no paint, no damage, **not even a probe**. Two reactive properties, both tested and both verified to fail without the cull, because the op-log measurements can only count and these are graph behaviour: a culled part **does not keep the page awake** (the walk stops reading its probe, so the page probe's `clear_sources` drops the edge and the next frame is idle *even though the culled part is dirty*), and it is **skipped, not resolved** (back in view it repaints with the state it acquired while invisible). Together these make 6.4c's "an unpainted probe stays dirty" concrete: get the first wrong and every frame re-walks the tree forever; get the second wrong and scrolled-away content comes back stale.

    **(ii) per-pixel filter (the cheap half)** — `DrawTargetProxy::draw_iter` drops pixels outside `clip_bounds()` before paying `Renderer::pixel`. That path is the *only* one text takes (`embedded-text` and u8g2 both rasterise glyphs and hand them over one pixel at a time), and it is where the cost that survives the part-level cull lives: a label straddling a region boundary is culled in **neither** region, so every glyph pixel is offered twice. **Honest about what it is:** a cheaper write filter, not the loop bound. The glyph iteration upstream still runs because the line/glyph loop belongs to `embedded-text` — see the deferred half below.

    **THE NUMBERS (240×240, `rows-24` = 10 regions; ×emit = what a region replay issues ÷ one full frame):**

    | page | ×emit before | after (i) | after (ii) | `required` floor | wasted ops |
    | --- | --- | --- | --- | --- | --- |
    | labels-12 | 10.00 | 1.50 | **1.00** | 1.00 | 22404 → **0** |
    | buttons-8 | 10.00 | 1.51 | **1.02** | 1.02 | 8406 → **0** |
    | checkboxes-8 | 10.00 | 1.50 | **1.50** | 1.42 | 206 → **2** |
    | scrollable-20 | 10.00 | 1.52 | **1.08** | 1.07 | 5156 → **5** |
    | option-rows-6 | 10.00 | 1.84 | **1.03** | 1.03 | 2781 → **2** |
    | mixed | 10.00 | 1.32 | **1.01** | 1.01 | 4134 → **0** |

    **Paint is now AT the geometric floor** — `wasted` is 0–5 ops per frame where it was tens of thousands, so the reason 6.4b was promoted to a precondition for 6.4d is discharged. (`checkboxes-8` sits at 1.50 vs a 1.42 floor because it has no text at all: its residual is 2 ops whose own bounds miss a band their widget's `outer` straddles — the part-level cull is per widget, not per primitive.) **Value without tiles, delivered:** the scrollable page's ONE-region capture fell from 576 ops to 565, i.e. the 11 `RoundedRect`s that were being drawn entirely off-screen every frame are gone.

    **DEFERRED, with owners — neither is a loose end:**

    - **(i-tree) subtree culling / the traversal term.** `visits` ×emit is untouched at **10.00** against a measured floor of ×1.19–2.85 (`VisitReport::cullable`). **Maintainer decision 2026-08-05: defer to 6.4c**, on the reasoning that under 6.4c the **collect** pass must visit every node anyway (tracked + probe-gated for all widgets), so subtree culling only shrinks the K *paint* walks — it belongs inside a structure that is about to change, and 6.6's dirty-list walk attacks the same term from the other end. The storage decision travels with it, already costed: `subtree_fits: bool` per `LayoutModel` node (~1 B, often free in padding; prune on `outer` only where the subtree is contained, else descend and test children individually — same soundness as the union extent, one extra visit per escaping node, measured at 1 per scrollable page) **vs** the union `Rect` (+16 B/node, ~1.6 KB per 100 nodes against a 20 K floor, prunes an escaping subtree one level higher). Whichever is chosen must be excluded from `LayoutModel`'s geometry-only `PartialEq` and maintained across WS5.2's incremental splice. — **STRUCK 2026-08-05: neither is needed.** Both options existed only to tolerate the fact that nothing pushes clips today; with the clip pushed as widget behaviour, containment is structural and the prune tests the *composed clip* that `clip_bounds()` already reports — zero bytes per node, no `PartialEq` exclusion, nothing to maintain across the splice. Full reasoning in 6.4c(E)/(F). The `escaping` measurement stays useful as the regression that proves the clip is actually being pushed.
    - **(ii-loop) the real text-run culling.** Closed-form first/last visible glyph index needs rsact to own the line loop, which means implementing text layout instead of delegating to `embedded-text`. That is the same work `font/fixed.rs`'s `Clip`/`Ellipsis` TODO already wants (it renders as wrap-into-a-short-box today, no ellipsis glyph), so the two should land together — **WS15 (font stack) or the TODO itself**, not 6.4. The pixel filter above already takes the *measurable* cost to its floor, so this is now about the glyph iteration, not the writes.

    **Constraint pinned for 6.4d:** `clip_bounds()` reports in the **caller's (absolute) space for every variant, including `Cropped`**, whose rebasing is the renderer's private business (ii-3). Both consumers compare against it directly — the cull holds an absolute `layout.outer`, the pixel filter sees the coordinates the drawing code emitted — so a variant reporting a viewport-local rect would silently invert both tests the moment 6.4d starts constructing `Cropped`.
  - [ ] **6.4c Collect/paint split** — probes stop gating paint. `render_part` gains a mode: **Collect** (tracked, probe-gated, `NullRenderer`, pushes damage) and **Paint** (untracked, no probe, selected by geometry). The collect pass must genuinely run the body so dynamic dependencies re-track — an `is_dirty()` peek would freeze the recorded source set and silently break conditional reads. Paint passes must be **untracked** so `run_probe`'s `clear_sources` + re-subscribe + `mark_clean` round-trip is paid **once per frame regardless of tile count**; force-polling K times is K× graph churn (~24–48 ms at 8 tiles, comparable to the entire rest of the frame). Probes are marked clean **by the collect pass** (`run_probe` cleans after its closure runs) — **CORRECTED 2026-08-04, this item originally said "after the final paint pass", which is wrong and harmful**: deferring the clean until after the last paint swallows any write that lands mid-frame, so the torn region never re-plans and the tear becomes permanent instead of self-healing on the next frame. Cleaning in collect is what makes accepting class A/B skew reasonable at all. (The original wording is a leftover from when paint did the polling.) The damage sink stops clearing per pass (`page/mod.rs`) and unions across the flush boundary per 6.7's defer+coalesce. **Coherence invariant:** `tick()` between `begin_flush` and drain is a documented contract violation + `debug_assert`. Rationale: direct-to-tile makes the **widget tree** the frame snapshot, and a tree of live signals is not frozen unless something freezes it — 6.7 §8 defers new _frames_, but nothing otherwise stops a signal write between `next_tile` calls in an async app, which is the same generation-mixing §8 rejects arriving through a different door. (Copy-from-framebuffer would not have this hazard: there the framebuffer _is_ the snapshot. It is the price of decision 6.7-1.)

    **INHERITS 6.4b's traversal item (maintainer decision 2026-08-05).** 6.4b delivered the paint cull (×emit 10.00 → 1.00–1.02, at the geometric floor) but left node **traversal** at ×10.00 against a measured ×1.19–2.85 floor. It lands here rather than there because *this* item decides how many walks a frame makes: the collect pass must visit every node regardless (tracked + probe-gated for all widgets), so a subtree cull only shrinks the K paint walks — building it before the collect/paint split exists means building it against a shape about to change. Two consequences to carry: the cull predicate must stay `DrawOp::bounds`-compatible (6.4a's invariance check is written against it, and a tighter cull passes review then cracks on screen), and pruning on `layout.outer` alone is **unsound** — see 6.4b for the `escaping` measurement and the costed `bool`-flag vs union-`Rect` storage choice. 6.6's dirty-list walk attacks the same term from the invalidation end; re-measure with 6.4a before committing to both.

    **DESIGN SETTLED 2026-08-05 (maintainer design session) — the drawing seam. Supersedes 6.4.0(ii-4)'s mechanism and 6.4b's storage costing; nothing below changes the collect/paint *semantics*, only where they are implemented.**

    **(A) 6.4.0(ii-4) specified an unreachable mechanism.** It said the collect pass must "run widget bodies against a no-op `Renderer<Color = W::Color>`". It cannot: `Widget::render(&self, ctx: RenderCtx<'_, W>)` (`widget/mod.rs:86`) is a non-generic trait method reached through `Box<dyn Widget<W>>` (`el/mod.rs:47`), and `W::Renderer` is a fixed associated type — so no other renderer *type* can be handed to a built widget — while 6.4.0 separately decided `Renderer` stays in `WidgetCtx` with no `dyn` erasure. The two decisions are in direct tension. `NullRenderer<C>` reverts to what it was always for: a test stub. **The actual requirement, stated without a mechanism smuggled in: run every widget body once, tracked, and throw its drawing away.**

    There are exactly three seams between a tracked read and a pixel, and only one of them is rsact's own:

    - at the **backend** (`Renderer::set_muted`) — **rejected by the maintainer**: "asks the user to implement logic only we need, because we failed to do it other way". Nothing in the trait's vocabulary ("how to draw a primitive") justifies a frame-planner flag, and it would filter *after* eg built the pixel iterator.
    - at the **renderer type** (substitute one per pass) — **rejected**: needs `dyn Renderer` or a generic `Widget::render` (see (D)).
    - at **`RenderCtx`** — **CHOSEN.** It hands out `pub renderer: &'a mut W::Renderer` and steps aside, which is exactly why there was nowhere to intercept. The missing mechanism was never a capability; it was an **encapsulation boundary**.

    **The seam:** `impl Renderer for RenderCtx<'_, W, CtxReady>` forwarding to `self.renderer`, with the mode check in the forwarding layer, and **the `renderer` field private** — maintainer, verbatim: "I confidently tell you YES, renderer must be private to RenderCtx". That privacy *is* the enforcement: a widget that can reach the raw renderer can bypass the mode, and then the mode is advisory. Properties, all verified against the tree before committing:

    - rsact primitives keep working untouched — they already take `&mut impl Renderer` (`primitives/block.rs:35`).
    - mute short-circuits **before** rasterization, so Collect costs the body's own logic and nothing else.
    - `Collect`'s `clip_bounds()` returns `None`, which disables both 6.4b's per-node gate and the traversal prune **for free**, and is *semantically true* (collect draws nowhere). No special-casing anywhere for "the collect pass must visit every node".
    - monomorphization stays flat: primitives instantiate per receiver type — was `W::Renderer`, becomes `RenderCtx<W>` — plus 12 inlinable forwarders per `W`. No second widget tree.
    - 21 call sites in 12 widget files (`edge, icon, canvas, image, checkbox, bar, select, slider, button, knob, scrollable, container`); maintainer accepted the call-site cost.
    - folds in a simplification: `FontHandler::draw<W: WidgetCtx>` (`font/mod.rs:256`) uses `W` for nothing but `W::Color` and `W::Renderer`, so it becomes `draw<R: Renderer>(…, color: R::Color, renderer: &mut R)` — strictly simpler, and it drops a rsact-ui dependency out of the font layer. `DrawTargetProxy<'a, R: Renderer>` (`eg/renderer.rs:34`) is already renderer-generic, so text flows through the seam unchanged.
    - the seam is also the natural home for two obligations that had none: the `debug_assert` enforcing 6.4a's "a primitive must not write outside its declared bounds" (see (G)), and 6.4d's per-region coordinate translation if it is ever wanted above the backend.

    **(B) `RenderMode::{Fused, Collect, Paint}` — Fused stays the default (maintainer decision 2026-08-05).** `Fused` is today's behaviour (tracked, probe-gated, paints, pushes damage) and remains the **full-frame** strategy; `Collect`/`Paint` are the pair 6.4d drives. This is faithful to 6.4c(1) rather than a hedge: the argument that paint *must* be geometry-selected is specifically "a tile buffer is scratch with no history", and a persistent framebuffer **has** history — so probe-gating there is optimal, not merely tolerated. Practical effect: every page golden and the per-page metrics gate stay unchanged, and Collect/Paint are proven through 6.4a's tile harness instead of by re-blessing the world.

    **(C) `Canvas`, and the future `CanvasRenderCtx`.** `Canvas` is the one widget whose reactive reads sit *behind* a `dyn Fn` boundary: `draw: Box<dyn Fn(&mut W::Renderer) -> RenderResult>` (`widget/canvas.rs:52`), and its own docs make reads-inside-the-closure the contract (`canvas.rs:32-35`) — the example reads `x.get()` inside the closure body. Two consequences:

    1. **Muting must happen at the draw call, not around the closure call.** For the other 20 sites the reads are *arguments*, evaluated before the call, so muting around them loses nothing. For `Canvas`, "don't call the closure in Collect" would leave the probe with an empty source set — and because collect is the *only* tracked run under the split, that canvas would never repaint again: `x.set(…)` marks nothing, no damage, no region, silently and permanently. Under `Fused` this is invisible, which is exactly why it must be written down.
    2. **The closure's argument type must change**, because the `renderer` field is now private — `(self.draw)(ctx.renderer)` stops compiling. It becomes `&mut RenderCtx<'_, W, CtxReady>`. This is a *type* change, not a call-shape change: closure argument types are inferred from the expected `Fn` type, so `Canvas::new(move |renderer| { renderer.circle(…) })` compiles unchanged; only closures that *annotate* the old type break (one site: `canvas.rs:151`, a test).

    **`CanvasRenderCtx` — the documented future (maintainer request 2026-08-05, so it is not forgotten).** Passing `RenderCtx` is accepted *as a step*, not as the destination: `RenderCtx` is an internal pass context (probes, `part_probes`, `dirten`, the damage sink, `needs_redraw`, `RenderShared`) and user code must not reach any of it. The maintainer's framing: "I see all such types as context used in passes strictly internal … but Canvas closure should accept its own `CanvasRenderCtx` in the future … because Renderer API doesn't provide, expanding Canvas capabilities like adding rsact styles usage for example."

    ```rust
    // rsact-ui: the PUBLIC drawing context for user-authored immediate-mode drawing.
    // Wraps the internal RenderCtx and re-exports only what user code may see.
    pub struct CanvasRenderCtx<'a, W: WidgetCtx> { /* &'a mut RenderCtx<'_, W, CtxReady> */ }

    impl<W: WidgetCtx> Renderer for CanvasRenderCtx<'_, W> { /* forwards; mode-aware, same as the seam */ }

    impl<'a, W: WidgetCtx> CanvasRenderCtx<'a, W> {
        pub fn area(&self) -> Rect;                       // the canvas's own inner rect —
                                                          // today a closure cannot know where it is
                                                          // (canvas.rs:40 hardcodes absolute coords)
        pub fn style<S: Style>(&self) -> S;               // rsact styles / theme colours — the stated motive
        pub fn pseudoclass(&self) -> StylePseudoClass;    // hovered / pressed / focused
        pub fn font(&mut self, …) -> RenderResult;        // text via the font stack, not raw glyphs
        pub fn clip(&mut self, r: Rect, f: impl FnOnce(&mut Self)) -> RenderResult;
        // deliberately ABSENT: probes, part keys, the arena, the damage sink, RenderShared.
    }
    ```

    Migration is one type substitution in `CanvasBuilder::draw`; user closures that do not annotate their argument keep compiling across it, for the same inference reason as above. Do it when Canvas next gets an API pass (the file already carries a WS1b `DrawQueue` note) — the ordering constraint is only that it lands *after* the seam, never before.

    **(D) `&mut dyn Renderer` investigated and rejected — it does not compile today, and the reason is load-bearing.** Verified empirically, not argued:

    ```
    error[E0038]: the trait `Renderer` is not dyn compatible
      --> `fn _takes_dyn(_r: &mut dyn Renderer<Color = NullColor>) {}`
      note: … because it contains associated const `SURFACE_UNITS`   (renderer.rs:128)
    ```

    `SURFACE_UNITS` is 6.4.0(iii)'s **compile-time capacity proof** — the thing that makes a too-small framebuffer a compile error instead of a runtime check. `dyn` erases exactly the information that proof needs. Removing the const makes the trait dyn-compatible (also verified: the probe compiles clean with it commented out), so the choice is explicit — *give up the compile-time tile check, or split the const into a second trait, for one widget's closure*. Four further costs, recorded so this is not re-proposed: (i) `?Sized` goes viral — every primitive is `fn render<R: Renderer>(…)` with an implicit `Sized` bound, so `Block`, `Line`, the 7 other primitives, `DrawTargetProxy` and `FontHandler::draw` would all need `R: Renderer + ?Sized`; (ii) one indirect call per primitive, which is negligible for a shape-drawing canvas and **material for a pixel-loop canvas** (`renderer.pixel` per point is exactly what immediate mode is for — 240×240 is 57 600 vtable hops, and the mute/clip checks stop inlining); (iii) it *adds* binary size rather than saving it — the closure is already a single `Box<dyn Fn>` per `W`, and `dyn Renderer` introduces an extra primitive instantiation alongside the `W::Renderer` ones every other widget needs; (iv) it forecloses (C) — a `dyn Renderer` can only ever offer `Renderer` methods, and styles-in-the-canvas is the stated direction.

    **(E) The traversal prune needs no per-node storage — 6.4b's costed `subtree_fits`-vs-union-`Rect` decision is STRUCK.** That costing existed only to tolerate a defect: nothing pushes clips today (`ElState::clip_path` is `None` at `state.rs:54` and set nowhere), so scrollable content genuinely escapes its parent's rect, and per-node storage was a way to design *around* that. With the clip pushed, containment is structural — **everything a subtree draws is confined to `outer ∩ (all enclosing clips)`** — and `Renderer::clip_bounds()` already reports exactly that composed rect, because PR #36 made nested clips compose (`ViewportKind::nested_in`). So the prune test is `clip_bounds() ∩ region`, at **zero bytes per node**, and it produces precisely the maintainer's specification: "check tile intersection with the parent (scrollable) and then only render children that intersect with the tile while clipping by scrollable clip rect, so top and bottom widgets are clipped, middle children rendered fully."

    **(F) Clipping is widget *behaviour*, not a render-body call (maintainer decision 2026-08-05):** "clip must be a widget behavior backed logic, not user call in the render method, otherwise we cannot know if widget clips its contents." This is also a mechanical prerequisite for (E): `ctx.clip_inner` wraps only the widget's *own* drawing, whereas children are clipped only by `render_subtree`'s `clip_path` push — the currently unreachable `ClipPath::InnerRect` arm. So the prune, the scrollable, and 6.11 all want the same small piece of work: a widget-declared clip the framework reads before descending. `scrollable.rs:344` already asks for it by name ("Clip path in the element properties").

    **(G) The widget's painted area must be an extensible function, computed not stored (maintainer decision 2026-08-05).** Both culls currently rest on "a widget never draws outside `layout.outer`", which is a contract nothing checks — and the audit in **ISSUE-3** shows it is already false, structurally, for every focused widget. The maintainer's direction: adopt an LVGL-style *real area* / `ext_draw_size` notion, **compute it when looking for the widget's affected area rather than storing it**, treat it as the groundwork for absolute positioning, box shadows and tooltips, and **postpone the computation while designing the API for it now**. Concretely:

    ```rust
    // ONE function; four callers that must never compute this themselves.
    fn paint_bounds<W: WidgetCtx>(layout: &LayoutModelNode<'_>, ext: Padding) -> Rect {
        layout.outer.outset(ext)      // TODAY ext == Padding::zero() at every call site
    }

    // the extension point: a widget declares how far outside its own rect it paints.
    trait Widget<W> {
        fn ext_draw(&self, /* resolved style */) -> Padding { Padding::zero() }
    }
    ```

    The four callers are 6.4b's per-node gate (`el/render.rs`), the damage push (`el/render.rs:394`), 6.4c's traversal prune, and the seam's bounds `debug_assert`. WS5.3's `layout_changed_set` and WS6.1's `layout_repaint_roots` derive geometry damage from `outer` too and join the list when `ext` becomes non-zero. Two design notes to carry: a `Padding` outset rather than LVGL's scalar, because shadows are *offset* as well as spread; and the ordering hazard — the gate runs **before** the widget's style is resolved (`ctx.get_style` is inside the body), so `ext_draw` must be answerable from the widget + stylist without running the render body, which is why it is a `Widget` method with a conservative default rather than a value read off the resolved `BlockStyle`. Same shape as (F): two widget-declared geometry properties (`clips_children`, `ext_draw`), both defaulting to the conservative answer, both read by the framework and never called by user render code.

    **CONFIRMED + AMENDED 2026-08-04.** This item was re-derived from scratch in a design session and came out the same, so it is now load-bearing rather than provisional — with three additions.

    (1) **Why Paint must be geometry-selected, not dirtiness-selected — the missing justification.** A tile buffer is **scratch with no history**. On a full framebuffer, damage rendering skips a clean widget and its pixels are still there from last frame; in a tile they are _not there at all_, so skipping a clean widget flushes a tile with holes. Hence "no probe, selected by geometry" is **forced**, not merely convenient. Consequence for the cost model above: tiling converts "repaint what changed" into **"repaint everything inside the damage regions"**, so the `1 + (h−1)/s` multiplier understates it — that figure assumed only _dirty_ objects repeat. This is the finding that made region **shape** the economic lever and drove 6.4d's tight-rect decision. It also means over-planning is merely wasted paint while under-planning costs one stale frame that self-heals (an unpainted probe stays dirty), so plan/paint divergence is benign — worth knowing, because the plan must be a top-down **walk** (`parent_dirty` cascades during traversal), not a flat scan.

    (2) **The `is_dirty()` rejection here is right; a 2026-08-04 proposal to add `Probe::is_dirty` + `Probe::poll_again` to rsact-reactive is WITHDRAWN.** That proposal tried to plan the frame with a cheap dirtiness _query_ and then force-poll per tile, which needs source-set accumulation (`clear_sources` once, then track without clearing, to get the **union** of sources read across passes) plus a per-probe frame stamp. All of that machinery exists only to patch a design strictly worse than this item's: a Collect pass that genuinely runs the body has exactly **one** tracked run, so there is no union to take, no frame stamp, and no accumulate mode. Two concrete traps the query design walks into, recorded so it is not re-proposed: `maybe_update` on a Dirty node calls `update`, whose Probe arm returns `changed = true` (`runtime.rs:1018`) and marks the probe's **subscribers** dirty — so a "query" propagates dirt up to the page probe, the same shape as the `force_redraw` gotcha; and a bare `f()` for repeat passes runs under whatever observer is **ambient** (the parent probe / `Page.render_probe`, per `run_probe`'s own subscribe-the-parent-edge invariant at `runtime.rs:724`), silently subscribing the page to every leaf signal and destroying targeted invalidation. **Net: rsact-reactive needs no changes for 6.4.** The only new primitive 6.4c actually requires is a colour-generic no-op renderer for the collect pass (6.4.0 (ii-4)).

    (3) **The coherence invariant upgrades from `debug_assert` to a compile error.** 6.4.0's `Frame<'_, W, P>` holds `&mut UI`, so `tick()` between `start_frame` and `Drop` does not compile. Keep the `debug_assert` only where a borrow cannot reach (e.g. a re-entrant path inside the page). `Frame` also holds a `DeferEffectsGuard` for the frame's duration: arena rebuilds all live inside `create_effect` (`widget/dynamic.rs:37`+`:58`, `el/build.rs:161`) and `run_effects` returns early while the guard is held (`runtime.rs:1491`), so **structural** mutation cannot happen mid-frame. Note precisely what this does _not_ buy: `defer_effects` gates only the effect flush, never `mark_dirty`, so **values still change mid-frame** and cross-region value skew remains. Two classes survive — (A) one widget straddling two regions whose own value changes mid-frame, and (B) **two widgets bound to the same signal in different regions** (progress bar + percentage label), which needs no straddling and is the likelier one. Both are bounded to a single frame and self-heal. Frequency is _not_ the "user double-clicks a checkbox" rare case the session first assumed: machine-paced writes (ADC/sensor ISR, telemetry, animation timers) land mid-frame **every frame** when the frame is flush-bound, which for ST7789 @40 MHz (≈23 ms transfer vs 4–9 ms paint) is the normal case. What narrows it back down is the plan: painting only touches widgets intersecting **planned** regions, so a mid-frame write to a widget that was not already in this frame's plan cannot tear at all. **Accepted** as a bounded artifact. Escalation if a machine-paced gauge ever shows visible skew is **selective, not global** — let an individual widget opt into caching its resolved paint values for the frame (memory only where needed), never snapshot every widget. Reference point: **LVGL has neither problem, for two reasons worth separating** — its refresh never yields (it spins on the `flushing` flag, or blocks in v9's overridable `flush_wait`; even two-buffer partial mode overlaps LVGL's _own_ paint with DMA, never application code), and more deeply it is retained-mode with **eager copies** (`lv_label_set_text` copies the string), so paint reads widget-owned memory rather than resolving a live query. Fine-grained reactivity buys minimal invalidation by resolving late; tiling multiplies the number of "late" moments per frame from one to N. LVGL's third mechanism **is** worth copying and is what makes (A)/(B) self-heal: `lv_obj_invalidate()` records the dirty area at **write** time and the refresh consumes that list at its start, i.e. _a change during a refresh takes effect on the next refresh_. rsact's equivalent is that the collect pass runs before any paint and the damage sink keeps accumulating during the frame for the **next** plan — which requires the two damage channels to stay separate (see 6.4d).
  - [ ] **6.4d Tiled output.** Strategy lives in the **output path**, orthogonal to `Renderer` — which stays "how to draw a primitive" and never learns tiles exist, seeing only a clip rect — and **absent from `WidgetCtx`**, so widgets, layout and event passes stay ignorant and the const generic never infects the type family every widget is generic over. `TiledOutput<C, const MAX: usize>`: const max sizes the backing array (alloc-free, no `Vec` — a step toward 6.4's no-alloc direction and WS18), runtime `budget ≤ MAX` plus `TilePolicy` knobs. **Buffer pool registered once** at UI construction (6.7 §11-Q3). Damage → **merge overlaps** (`Rect::union`, WS5.3 — overlap is common: a widget and its parent's `LayoutChange` root) → chunk each merged rect into **row-bands** ≤ `min(budget, peripheral_max_transfer)`; the latter is a hard ceiling independent of RAM (nRF52 SPIM `MAXCNT`) and only the app knows it. Full-screen guard: if merged damage ≈ viewport, one screen rect chunked into bands — classic strips, arrived at rather than designed in, and the guarantee that tiling is **never worse than a plain strip renderer**. Tiles held in **wire format** (big-endian RGB565: `solid_storage` returns a pre-swapped pattern so 6.3b's word-fill is unaffected, `pixel()` readback pays one `REV16`). Needs a **contiguous sub-buffer seam** — `draw_buffer` exposes only the whole buffer today (6.7 §9). Full-framebuffer mode stays, same entry point, different strategy (6.7 §11-Q2, settled by the encapsulation decision above).

    **AMENDED 2026-08-04 — region shape, and the interface moves to 6.4.0.** Three changes, all maintainer decisions.

    (1) **DECIDED: tight damage rects with an area-test merge.** Supersedes "chunk each merged rect into row-bands" above. Rationale is 6.4c(1): because a tile has no history, everything intersecting a region repaints, so **region shape sets the repaint set**. A full-width 240×24 band makes a one-checkbox change repaint every widget crossing those 24 rows; a tight 16×16 rect repaints the checkbox and its backdrop. Algorithm: start from tight per-damage rects, and merge two only when the union's area is not much larger than the sum of the parts (LVGL-style join with a threshold — far-apart rects stay separate). **The threshold is a tuning knob and must be set from 6.4a numbers, not guessed.** ✓ **SET 2026-08-04 by 6.4a: area ratio, threshold ×2.0.** Measured on one frame: adjacent rects 20 px apart → area ×1.12; opposite corners → area ×23.62; overlapping → area ×0.75. The decision gap is so wide that the knob is not delicate — anything in ×1.5–×4 separates them — so take ×2.0 (LVGL's neighbourhood) and revisit only if a real page lands inside the gap. Two corollaries from the paint term: **overlapping rects must always merge** (ops ×0.98 *and* area ×0.75 — the overlap was being painted twice), and a merge that leaves the op count unchanged is still a win because it halves the per-region command overhead. And the premise itself is now measured, not argued: tight 16×16 = **3** required ops vs the 240×24 band's **118** (`tile_shape_240.txt`). Row-bands survive exactly as before but demoted to the **degenerate** case: when merged damage ≈ viewport there is nothing to be tight about, so one screen rect chunks into bands — still "classic strips, arrived at rather than designed in", still the never-worse-than-a-strip-renderer guarantee. Hardware note that makes tight rects free on the reference target: ST7789/ST7735 `CASET`/`RASET` take arbitrary rects, and a tile strided at its own region width is contiguous by construction, so a tight rect is one `set_window` + one DMA burst exactly like a band. The alignment/minimum-region constraints that _would_ favour bands belong to page-addressed and e-paper panels and stay in **6.5**'s `RegionPolicy`. Trade-off accepted: more regions ⇒ more per-region command overhead (`CASET`/`RASET`/`RAMWR`), which is why the merge test exists at all.

    (2) **`TilePolicy`/`budget` becomes a type-level `FramePolicy`; the interface spec moves to 6.4.0.** The runtime-`budget ≤ MAX` framing is replaced by `Tiles<W, H>` / `Whole<W, H>` as **types**, with a `const { assert! }` in `start_frame` proving the renderer's surface can hold the policy's largest region — so a too-small framebuffer is a **compile error**, not a runtime check. Maintainer's requirement, verbatim: "FramePolicy is not a dynamic value but one that applies a constraint over the framebuffer that can be passed … so we are sure that user cannot pass a framebuffer smaller than needed." The `peripheral_max_transfer` ceiling (nRF52 SPIM `MAXCNT`) is unaffected and still app-supplied — but note it now has a natural home: it is a _policy_ choice, so `Tiles<W, H>` is where the app encodes it. Mechanism, verification, and the coherence trap are in 6.4.0(iii).

    (3) **Buffer pool: user-owned, not registered.** Amends "Buffer pool registered once at UI construction (6.7 §11-Q3)". The buffers never enter rsact — they enter the _renderer_, through inherent backend methods the user calls (6.4.0, surface ownership). On embedded that pool is `StaticCell` + two channels of `&'static mut Tile`, which is also what keeps `W::Renderer` lifetime-free. `ConstStaticCell` over `StaticCell::init` in the docs/example: the latter builds the tile on the stack and memcpys it, which is 11.5 KiB of stack you do not have.

    (4) **Two damage channels must stay separate** (follows from 6.4c(3)). Paint-derived damage — `render_part` pushing `layout.outer` at `el/render.rs:349` — is an _output_ of painting consumed by the flush; under `Tiles` the region **is** the flush unit, so the tiled driver never reads it (it stays the full-frame path's mechanism). Write-derived invalidation is what feeds the **next** frame's plan. Conflating them self-perpetuates repaints, since every painted region would re-damage itself. Also revisit here: WS6.1's transparent-container fix (`render_subtree_body` clears a `LayoutChange` root's outer **and pushes damage**, because transparent `Flex` never reaches `render_part`) — under tiles that clear must be clipped per region and repeated per region, and its damage push must not reach the plan channel. Inherit it deliberately, not silently.

  **Final-sweep design constraints — all still apply:** (a) `PackedFramebuf`'s area-based packing asserts `area % pps == 0` — a real 122×250 mono e-paper **panics in the constructor** (30500 % 8 = 4), and rows are never byte-aligned (`eg/framebuf.rs`); minimal fix is to allocate `ceil(area/pps)` and drop the assert. **Moved to 6.5**, since the ST7789 reference target never hits it. **The per-row stride half of this constraint is now largely retired:** 6.3b found none is needed for a solid fill (whole words sit inside the row), and direct-to-tile retires the rest — a tile's own width _is_ its stride, so the sub-rect is contiguous by construction. Stride survives only for the optional zero-copy-from-full-framebuffer path; (b) `EGRenderer::pixel_alpha` blends against untranslated coordinates — dead today because `ViewportKind::Cropped` is never constructed, but **this goes LIVE with 6.4d**: a tile buffer is its own coordinate space, so the translation the bug was waiting for is precisely what direct-to-tile introduces. Also note `pixel_alpha` is a read-modify-write per pixel, which defeats write-combining; **and the RMW has a second, sharper consequence found 2026-08-04 — it constrains the tile protocol, not just performance.** `pixel_alpha` reads the **destination** (`eg/renderer.rs:173`: `canvas.pixel(p).map(|current| current.mix(blend, pixel.1))`) and the AA primitives use it heavily (`circle.rs:63-105`, `arc.rs:80-91`). Blending against an _ancestor's_ background is fine — the ancestor repaints earlier in the same pass, so painter order holds within a region. Blending against the **persistent frame** is not: full-frame rendering gets a correct backdrop for free because the framebuffer survives between frames (that is how AA edges currently compose under damage-driven partial redraw), whereas a tile buffer arrives holding whatever the previous tile left in it. So **every tile must be initialized to the true background before painting** — a full tile fill per tile, on top of the region's real work — and where a widget's backdrop is not opaque the tiled result _legitimately differs_ from the full-frame result, because "what is underneath" no longer exists. Note also `.unwrap_or(pixel.1)`: an out-of-bounds read silently yields the unblended colour, so every mistake in this area produces a plausible image rather than an error, which is why 6.4a's tile-invariance op-log check is the only real defence; (c) decide the fate of the **layer dimension** first: two parallel `Layering` impls (`layer.rs` + a private copy in `EGRenderer`, TODO at `renderer.rs`), a per-draw BTreeMap lookup, and no code path that can create layer > 0. Sharpened: `Layer::fullscreen` allocates a **full-size framebuffer per layer**, so the latent memory multiplier is L × 112.5 KiB — tiling is what would make layers affordable at all — and `finish_frame_regions` streams layer-by-layer per region, multiplying per-region flush cost too. **✓ RESOLVED 2026-08-04 by PR #31 (`render: delete the layer dimension`, master `8810554`)** — the decision was _delete_: `layer.rs` is gone, `EGRenderer` holds a single `PackedFramebuf` + a `Vec<ViewportKind>` clip stack, `Viewport { layer, kind }` collapsed to a bare `ViewportKind`, and the per-draw `binary_search` for the layer index went with it. So the L × 112.5 KiB multiplier and the per-region layer loop are both retired, and 6.4b's "hoist the per-pixel `layers.binary_search` out of `draw_pixels`" sub-item is **already done**. Nested composition, if it is ever wanted, comes from tree-depth composition rather than a surface stack (rationale in the new `rsact-render/src/surface.rs`). **Follow-up left on master by #31 (not a blocker):** a `TODO` at `eg/renderer.rs` notes `EGRenderer` still holds `canvas` + `viewport_stack` inline instead of using the shared `surface::Canvas<T>` helper the same PR introduced — worth folding into 6.4d, which touches that struct anyway.

  **New invariant (2026-08-01):** any position-dependent effect — dithering, ordered halftone, gradients, pattern fills — must be a pure function of **absolute** coordinates, never tile-relative, or it seams at every tile boundary. Same class as "a primitive must not write outside its declared bounds", which multi-pass rendering turns from cosmetic sloppiness into visible cracks. 6.4a's tile-invariance check catches both.
- [ ] 6.5 **e-paper story**: partial window refresh driven by the regions API; `render() -> bool` + regions documented as the e-paper contract. **Absorbs from 6.4 (2026-08-01):** (i) the `area % pps == 0` constructor panic (allocate `ceil(area/pps)`, drop the assert) — deferred here because the ST7789 reference target never triggers it, but 122×250 mono does; (ii) the **`RegionPolicy`** constraint model — `{x_align, y_align, min_region, max_regions}` on the output trait with permissive defaults — deferred here because ST7789/ST7735 have free rect addressing and would leave it unexercised and unvalidated. e-paper is the genuine both-constraints case: byte-aligned x plus a refresh cost so high that `max_regions = 1` (coalesce everything to one bounding box) is the only sane policy. Reference points for that model: SSD1306 has addressing-mode + column/page **range** registers (0x20/0x21/0x22) so it takes an aligned rect window directly; SH1106 does have pages (`0xB0|n`) but **no range registers**, so a windowed write is a per-page loop with no auto-wrap — that is `y_align = 8` plus higher per-region command cost, _not_ "full frame only". 6.7's owned-tile handshake covers e-paper for free: `release` is called when the BUSY pin deasserts, and a multi-second panel refresh is the same protocol as a 2 ms DMA burst.
- [ ] 6.6 (Optional, after 6.1–6.3) **dirty-list walk** (D5 Phase C): `mark_dirty` enqueues `(ElId, part)` into a page-owned map; render walks dirty paths + forced subtrees only — eliminates the O(N) tree walk on change frames. Preserve parent-clears-before-children ordering. **Interaction with 6.4 (2026-08-01):** 6.4b's hierarchical culling and this item attack the same O(N) walk from opposite ends — culling prunes by geometry per tile, the dirty list prunes by invalidation per frame. Sequence 6.4b first and re-measure with 6.4a before committing to this: the traversal term is ~1.5 ms of a ~4 ms frame at 100 nodes, so the dirty list may not clear the bar on its own. It becomes materially more attractive only if 6.4a shows the per-object term dominating at realistic node counts.
- [~] **6.7 Non-blocking flush (A8 — sans-IO, NO new deps). RESEARCH DONE 2026-08-01** — see `docs/plans/2026-08-01-async-agnostic-tile-output-research.md`. Original charter: a chunk-iterator `flush_regions()` the app drives. **Superseded by a stronger form:** the core returns **owned tiles**, not borrowed chunks — `next_tile() -> Option<Tile>` / `release(Tile)`, with the core holding a cursor into the damage list so it is re-enterable between transfers. Ownership is the load-bearing part and is not about async at all: DMA soundness (`embedded-dma`'s `ReadBuffer` is `unsafe` precisely because a borrow the core can still write through is UB, manifesting as tearing rather than a crash), borrows held across `.await` locking `&mut ui` so "render the next tile during the transfer" becomes uncompilable, and `'static` requirements in real HAL DMA APIs. One synchronous core then serves blocking SPI, polled DMA, Embassy, and RTIC ISR-completion — the RTIC row being the proof, since an async-trait design would need a waker plumbed into the ISR. **Why it escapes function coloring:** coloring propagates through _callers_, not callees; inverting the call direction is the only in-language escape on stable (no `?async`). **Decisions taken:** (1) render **directly into tiles**, double-buffered — full-framebuffer double buffering stays available; (2) buffers are **user-provided**, which also hands the app exactly the right window for cache maintenance on M7/ESP32-S3 (clean after CPU writes, before DMA reads — the interval when the app holds the tile and the core does not). Frame coherence is **defer + union-coalesce**, never abort: the pending set is a union so it cannot grow, giving bounded memory, bounded latency and no starvation. Discarding is correct only under the checkable predicate `new_damage ⊇ every pending tile` (page navigation), and even then the in-flight tile finishes rather than aborting mid-stream, which would leave the panel's write pointer at an unknown position inside its address window. `FinishRender` survives as the "I have a `DrawTarget`, just blit it" convenience layer. **Remaining work is implementation, sequenced inside 6.4d.**
- [ ] **6.8 Display rotation/orientation (A9):** 0/90/180/270 at the framebuf/regions layer; design inside 6.4's mode work (rotation interacts with strip windows and region coordinates); per-backend transform behind a target-agnostic API.
- [~] **6.9 Golden-image render tests (A10) — land FIRST in this WS:** tiny-skia PNG snapshots (host) + NullRenderer/RecordingRenderer draw-call goldens, with a blessed-image update workflow (`UPDATE_GOLDENS=1`). Every subsequent WS6 item (and WS6.10, and WS17) is then reviewable by golden diff. **Draw-op-log half DONE — PR #27** (`rsact-render`: `RecordingRenderer` + `format_ops` + `golden` bless harness; `rsact-ui/tests/goldens/` checkbox page goldens; deterministic, in the default gate). **PNG-snapshot half PENDING:** needs (a) a Pixmap-extraction path on `TinySkiaRenderer` (composites transiently, streams to target — no public pixmap accessor) and (b) a `simd` determinism decision (SIMD rounding differs across arches → PNG-byte goldens risk cross-platform flakiness; keep out of the default gate unguarded). `assert_bytes_golden` already in place. PNG only load-bearing for 6.10/6.11.
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

```text
6.4 tiled frame (0, a–d), one frame — user owns the loop, rsact owns no IO:
  ui.start_frame::<Tiles<W,H>>()  ─ const-asserts surface >= region  (6.4.0)
    ├ DeferEffectsGuard held for the frame  (no structural mutation) (6.4c)
    ├ layout model borrowed, not cloned     (owned Page field)       (6.4.0-iv)
    └ collect pass ─ tracked, probe-gated, NullRenderer<C> ─▶ damage  (6.4c)
  tight rects ─▶ merge only when union area ≈ sum of parts           (6.4d)
                 (bands ONLY when merged damage ≈ viewport)
  while let Some(region) = frame.next_region():           ← user's loop
    user: attach(buf)      ─ inherent, rsact never sees a Surface    (6.4.0)
    frame.render(&mut r, region) ─ begin_region ▸ paint UNTRACKED,
      no probe, EVERYTHING intersecting repaints (tile has no history),
      cull subtrees by outer ∩ region                             (6.4b/c)
    user: detach() ─▶ DMA / submit / blocking write ─▶ recycle       (6.7)
  drop(frame): probes clean, deferred effects flush, new damage ─▶ next plan
```

```rust
// 6.7 sans-IO owned-tile handshake — the APP owns the I/O call, therefore the color.
// Identical core for blocking SPI, polled DMA, Embassy and RTIC-ISR completion.
let mut flush = ui.begin_flush();                  // sync; no executor, no I/O, no block_on
while let Some(tile) = flush.next_tile(&mut ui) {  // None = frame done XOR no free buffer
    let xfer = spi.write_dma(tile.rect, tile);     // tile MOVES into the transfer
    if let Some(prev) = inflight.replace(xfer) {
        flush.release(prev.wait().await);          // buffer returns to the pool
    }
}   // blocking driver: spi.write(...)?; flush.release(tile); — same core, no feature flag
```

Acceptance: change of one label flushes only its region (simulator-verifiable + probe byte counts); tiled mode passes the render test suite **and** 6.4a's tile-invariance check (per-tile op-logs reconstruct the full-frame log); 240×240 RGB565 heap <20 KiB; no regression in `draw_on_demand`/`checkbox_redraws_on_toggle`.

**Session prompt:**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS6 + gates G3/G12 — AND
docs/plans/2026-08-01-async-agnostic-tile-output-research.md (6.7 sans-IO, decided).
Verify WS2/WS5 landed (arena-owned probes; changed-set API). 6.9's op-log harness, 6.1,
6.2, 6.3a and 6.3b are all DONE (6.3b merged via PR #30 — note it found NO per-row
stride is needed for a solid fill; the earlier stride worry was overcautious).

Order (resequenced 2026-08-04; supersedes the 2026-08-01 order): 6.4.0 PREREQUISITES FIRST
— the three render bugs (pixel_alpha asymmetry, fill_solid's duplicated addressing, and
6.5's area%pps via units_for), then the trait cleanups (clip stack, delete Options,
begin_region, colour-generic NullRenderer), then the const-generic capacity chain, then
the layout de-memoization. All are independent of tiles; three are live bugs; and each one
is something 6.4 would otherwise have to fix mid-flight. 6.3c fill_contiguous next — still
independent, still the reason every SPI timing figure is currently fiction, but note it is
now a pure OPTIMIZATION (it was briefly a hard prerequisite under the dyn-Renderer design
that 6.4.0 rejects). Then 6.4a measurement harness — it GATES 6.4d, so do not skip it to
"save time"; its purpose is to replace the +14%/+49% estimates with numbers, to prove the
tile-invariance property, AND (added 2026-08-04) to set 6.4d's merge threshold, which must
not be guessed. Then 6.4b culling (wins on Scrollables today, tiles or not), 6.4c
collect/paint split, and only then 6.4d tiled output.
UPDATE 2026-08-04: 6.4.0 (PRs #33 + #34) and 6.4a (branch ws6.4a-tile-measurement) are
DONE, so the live front is 6.4b — and 6.4a's numbers make it a PRECONDITION for 6.4d, not
a preceding nicety: uncalled, an N-region replay emits N× everything (per-pixel work
included), so tiling before culling is a ×10 paint regression at 10 regions. Read 6.4a's
findings before starting 6.4b: its sub-item (i) is unsound as written (scrollable content
escapes its parent's `outer`), and the cull predicate must match `DrawOp::bounds` or 6.4a's
invariance check goes vacuous. Re-run `tests/tile_schedule.rs` after 6.4b and bless the
goldens — the win IS the diff (`struct` ×req should stay put while ×emit falls toward it). 6.8 rotation designs inside 6.4d.
6.5/6.6/6.10 after. Parallelism note: 6.4.0's render bugs, trait cleanups and capacity
chain all touch framebuf.rs/eg-renderer.rs, so keep them on ONE branch (or strictly
sequence them) rather than stacking PRs — see the WS5.2 stacked-PR trap in the WS5 notes.
rsact-reactive is NOT touched by any of this (6.4c(2)).

Two invariants hold throughout. D5-I5: parent redraw ⇒ child overdraw; child dirty ⇒ page
dirty; O(1) idle gate survives. And (new) every position-dependent effect is a function of
ABSOLUTE coordinates — tile-relative dithering/gradients seam at band boundaries.

Do NOT center any decision on the AA rasterizer costs. They are acknowledged stubs slated
for rewrite or replacement (backlog); measuring tiling through them measures the wrong
thing. The cost model that matters: per-pixel work is invariant, per-object work is what
repeats.
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

**Note (2026-07-09): 7.2, 7.6, 7.7 pulled forward into WS13** (builder/widget split, resequenced before WS5 — they share the widget-struct/trait/`derive(View)` surface, reshaped once there; see the WS13 decision block). WS7's remaining scope = **7.1** (events), **7.3** (PageId), **7.4** (stylist), **7.5** (ctx-collapse — still G4-blocked). Tick the folded items' checkboxes here when WS13 lands them.

The staged collapse (D2's analysis, amended by G5: widgets vary over exactly TWO degrees of freedom — the renderer and, per the maintainer's decision, the custom-event type; `PageId` never reaches widget code; `Stylist` reaches it only through `get_style`):

- [ ] 7.1 **Events (G5: `Event::Custom` KEPT)**: custom events remain the extension point for app-defined widgets (user widgets are concrete over their own ctx and can match `Event::Custom(MyEvent::…)`). Scope reduces to: the `MoveEvent`→`InputEdge` rename TODO, reserving room for future `Key(u8)`/`Char`, making `E = ()` the zero-ceremony default (via G4's blanket ctx impl) so apps without custom events never have to name it, and **(final sweep) fixing the Wheel event's `Point` overload** — the simulator stuffs scroll-delta into `MouseEvent::Wheel(Point, _)` (`event/simulator.rs:78-86`) which `interpret_as_rotation` reads as a delta but `cursor_point()` returns as a *position* preferred over the real pointer position (`event/mod.rs:177`, `el/event.rs:226-230`); wheel needs a delta type, not a `Point` (precondition for WS10.6's bounds pruning, which would otherwise route wheel events by the bogus position (0, ±1)).
- [ ] 7.2 **De-generic widget-local params** (D6): `Dir: Direction` → runtime `Axis` field (constructor arg; `model_flex` is already shared), `V: RangeValue` → canonical numeric at the boundary. Measured: ~2.4 KiB flash per instantiation removed. (Cheap; can ride early.)
- [ ] 7.3 **PageId demotion**: `UI<R, P>`/`Page<R>`/`UiQueue<P>` driver-level only (13 sites, none in widgets).
- [ ] 7.4 **Stylist redesign: TypeId styler registry (G9 ✓)**: sorted `Vec<(TypeId, Box<dyn Any>)>` on the theme (binary search, no hashing, ~24 B/entry); `theme.styler::<S>(closure)` registers, `ctx.style::<S>(class)` resolves `S::base()` → registry hit → per-instance `.style()` closure; built-ins ship as **pre-registered stylers** on `Theme::light()/dark()/BinaryTheme` — adding a built-in widget becomes one registration line, never a breaking theme change (also dissolves the BinaryColor trait-impl coherence pain). `RenderShared.stylist` dyn-erased (cold path). Add the `style_resolution` micro-bench to the 0.3 snapshot (G9's tripwire; pre-approved fallback = build-time hoisting, 8 B/widget). Replace `derivative` in `declare_widget_style!` (unmaintained, RUSTSEC-2024-0388).
  - **Drops `WidgetCtx::Stylist: Clone` (PR #11 review, 2026-07-09).** WS4.1's `Inert` inlining forced a `+ Clone` bound on `type Stylist` (`el/ctx.rs:12`): the stylist is now stored inline in `Inert<W::Stylist>` and cloned into each page (it was a free `Copy` `ValueId` handle pre-WS4). Maintainer concern (worth resolving here, not in WS4): `Clone`/copy-everywhere is prone to "accidental global change silently not working" bugs and makes a future _dynamic_ Stylist's invalidation clumsy. This redesign removes the bound **for free** — once `RenderShared.stylist` is dyn-erased and reached only through `get_style`, the stylist is borrowed (`&dyn`), never owned-per-page or cloned, so a single shared instance can be invalidated in one place. Interim options if a fix is wanted before WS7: (a) narrow `+ Clone` → `+ Copy` (all concrete stylists — `()`, `BinaryTheme` — are `Copy`; matches pre-WS4 semantics but no better for the dynamic case); (b) hold the stylist as a UI-level `Rc<W::Stylist>` shared into pages (one instance, cheap handle clone). Preferred: no bound at all via this item's dyn-erasure.
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

**STATUS 2026-07-08 — WS9a COMPLETE (all actionable items) on `ws9-footprint-diet`.** Done: **9a.1** (EffectQueue: 0 write allocs, −6.8 KiB reactive `.text`), **9a.2** (icons L1+L2 code-complete, rides w/ Icon-widget fix for measurement; tiny-collections landed+verified), **9a.3** (untyped-core outline: alloc-neutral, −604 B reactive / −2188 B ui `.text`), **9a.4** partial (itertools dropped + WidgetFlags→bitflags; **logging sub-item blocked on G12**), **9a.6** (Color::mix 8.8 fixed-point). **9a.5 skipped** (condition not met — profiling doesn't flag `subscribe`). The WS2 size-probe bit-rot fix + CI (0.9c/0.9d) landed separately via PR #4 on master. Verified across the branch: reactive 72/1 (`static_wrapper`=WS4), rsact-ui 53/0, rsact-render 6/0; thumbv7m no_std floor build green. _(Commit hashes in the per-item notes below were rewritten by a rebase onto the post-PR#4 master — see the branch's actual log / the 9a PR for canonical hashes.)_

- [x] 9a.1 `pending_effects` BTreeSet + `run_effects` quicksort → height-bucketed vec (kills the only steady-state write alloc — 2 allocs/112 B per write — AND ~1.6 KiB quicksort + BTree flash). **Done.** New private `EffectQueue` in `runtime.rs`: height-indexed `pending` buckets (drained ascending → no sort) + a `draining` buffer swapped in per flush round (allocation-free `take()` equivalent, faithful round semantics) + a `SecondaryMap<ValueId,()>` membership set for O(1) dedup (replaces the BTreeSet's set semantics, load-bearing for batched writes). Buffers retain capacity → **0 steady-state write allocs**. Cap-break (>10 000 rounds ⇒ cycle) now clears+logs (maintainer decision) instead of leaving the queue wedged. Kept behavior identical (dedup / round-snapshot / shallower-first); glitch-freedom already comes from `maybe_update`'s source-pull, so the sort was only preserving execution order. 4 new tests lock the semantics (`effect_queue_{dedups_batched_writes,fans_out_to_all_subscribers,cascade_settles,no_glitch_across_heights}`); `dispose_removes_from_pending_effects` updated to the new membership API. Resolved the load-bearing TODO at the old field decl ("Maybe use Vec…pre-sorted…so we don't need to sort"). **Measured — alloc (`benches/allocations.rs`):** `signal_write_noop_equal_1_effect` / `effect_rerun_1_signal` / `batch_100_writes_1_effect` **2 / 112 B → 0 / 0**; `effect_rerun_10_subscribers` 2 / 184 B → 0; `effect_rerun_100_subscribers` **18.73 / 2931 B → 0 / 0**; all create/idle rows unchanged. **Flash (size-probe `reactive`, thumbv7m opt-z fat-LTO):** `.text` **23 364 → 16 580 B (−6 784 B, −29.0%)**, `.data`/`.bss` unchanged (pure code win). _Caveat: the roadmap's old 27.4 K reactive `.text` predates WS2 — the size-probe binary still called the deleted `observe()`, so `metrics-probe --sizes` had silently failed for the `reactive` bin since WS2 (see report note below). Both A/B numbers above use the corrected Probe-based probe, so 23 364 → 16 580 is the apples-to-apples 9a.1 delta. The full L2 baseline row (reactive + ui × thumbv7m/thumbv6m) is now re-recorded from the fixed tool in Baselines._
- [x] 9a.2 Sorted-vec/linear structures for the tiny collections (pages, fonts, layers); icons: honor per-size features, make size dispatch prunable (46 KiB retention hazard: `CommonIcon::size` runtime-matches every compiled size). **Both halves DONE in code (on `ws9-footprint-diet`).** **Tiny-collections (`618aafe`):** PAGES (`BTreeMap<PageId, Box<dyn PageInitFn>>`, ui.rs), FONTS (`BTreeMap<FontId, StoredFont>`, font/mod.rs), LAYERS (two `BTreeMap<usize, Layer>`, rsact-render layer.rs + eg/renderer.rs) → sorted `Vec<(K,V)>` + `binary_search_by` (N≈1–5, all keys `Ord`; drops BTreeMap monomorphization/allocation, deterministic RAM). Preserved: add_page duplicate-panic, fonts replace-on-duplicate, layers ascending compositing order, all panic messages; plain `Vec` (values aren't `Default`). Verified rsact-render 6/0, rsact-ui 53/0, thumbv7m green. **Icons (`c640083`):** both root causes fixed — **(L1 — honor features)** rsact-ui hard-forced `rsact-tiny-icons`'s default `all-sizes` (its default = `["common","all-sizes"]`) so feature unification pinned all 19 tables into every app. Fixed by `default-features = false` on the workspace dep (root `Cargo.toml`; a member can't override an inherited default to false) + keeping only `common`, plus per-size passthrough features `5px..24px = ["rsact-tiny-icons?/Npx"]` (weak `?` = no-op unless `tiny-icons` on) and an `icons-all-sizes` convenience; the 3 `tiny-icons` examples pin `icons-all-sizes` to keep today's behavior. **(L2 — prunable dispatch)** the generated `CommonIcon::size()` runtime-`match` referenced every size module unconditionally, so the linker kept all compiled tables (and any subset failed to compile — only `all-sizes` built). Fixed in the build generator (`build/main.rs`): each arm is now `#[cfg(feature="Npx")]`-gated (mirroring the `pub mod` decls), with a descending-gated `_` fallback (clamp to the largest compiled size) + a no-sizes `panic!` arm. **Verified:** feature graph — `tiny-icons` alone → `common` only (all-sizes gone), `+16px` → only 16px, `+icons-all-sizes` → all, `16px` w/o `tiny-icons` → dep absent; and the generated-code SHAPE compiles under all/subset/none combos (rustc `--cfg` mock). **NOT verifiable locally / rides with the WIP Icon-widget fix:** `icon-libs/` SVGs are absent (excluded from the repo) and `build/main.rs` embeds them via `include_str!`, so the crate can't be rebuilt/regenerated here — the maintainer regenerates `src/rendered/` (git-ignored) with `icon-libs` present to realize L2, and the ~46 KiB flash win is measured once the (excluded) rsact-ui Icon-widget path compiles.
- [x] 9a.3 Outline untyped cores in rsact-reactive (`add_value_raw(Rc<RefCell<dyn Any>>, kind)` etc. + `#[inline]` typed shims) — collapses the ×9–×14 per-`T` instantiation spread. **DONE — `da965d1`.** All 7 `create_*` constructors funnel through `Runtime::add_value<T,DT>`, which monomorphized the whole value-creation body (scope borrow, deny_new, `storage.add_value`, owned-map, state match) per `(T,DT)` though only `Rc::new(RefCell::new(value))` + debug `type_name::<DT>()` depend on `T`. Split into a non-generic `add_value_raw(Rc<RefCell<dyn Any>>, kind, state, caller[, ty])` core (compiled once) + a thin `#[inline] add_value<T,DT>` shim (allocates the cell, unsizes to `dyn Any` at the call arg, resolves the debug type name via the `#[cfg]`-on-arg pattern from `scope.rs`). **Verified:** reactive 72/1 (static_wrapper=WS4) unchanged; `benches/allocations.rs` create_* counts identical (alloc-neutral); debug-info build compiles; size-probe `.text` thumbv7m −604 B reactive / **−2188 B ui** (more widget `T`s → more collapse), `.data`/`.bss` unchanged.
- [~] 9a.4 Logging policy — **pending G12's still-open logging remainder** (decide there first: `log max_level_off` vs defmt); `WidgetFlags` → bitflags; drop `itertools` (4 call sites). (`derivative` replacement is owned solely by 7.4 — not duplicated here.) **PARTIAL DONE — `2187347` (`ws9-footprint-diet`):** dropped `itertools` (its only uses were `zip_eq` over arena-paired children↔layouts, equal-length by construction → std `zip`; dep gone from rsact-ui's shipping graph, verified `cargo tree -e normal` = 0); `WidgetFlags` five `bool`s → `bitflags` `u8` (already a workspace dep; chainable setters kept for producers, `is_*()` accessors for consumers, default `HOVERABLE_FROM_CHILDREN` preserved; 5 B → 1 B). Verified rsact-ui lib + 53 tests + thumbv7m no_std. **Still blocked:** the logging-policy sub-item (G12 `log max_level_off` vs defmt).
- [~] 9a.5 `subscribe` linear-scan dedup revisit (post-WS2, only if profiling still shows it). **SKIPPED (condition not met):** the 0.3 profile / `benches/allocations.rs` do not flag `subscribe` dedup as a hotspot (fan-in is 1–8 over TinyVec-inline lists; `subscribe`'s linear scan is not on any measured hot path), and WS2 already removed the observer-registry churn. Left for a future pass only if profiling later shows it — no change made.
- [x] 9a.6 **(final sweep) `Color::mix` integer blend**: per-channel f32 math in the AA hot loop (`color.rs:42-47` — its own TODO, with a commented-out integer version at `:33-41`); called per AA pixel. An 8.8 fixed-point blend removes ~6 soft-float ops per blended pixel on FPU-less parts. **DONE — `900b88a`.** 8.8 fixed-point: α scaled to `0..=256` once, per-channel blend `(this*(256-a) + other*a) >> 8` pure integer. API stays `f32` (callers produce coverage/blend as floats) + `clamp` for out-of-range AA coverage. rsact-render 6/0 (no pixel assertion regressed; ±1 integer-vs-f32 rounding is imperceptible).

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
- [ ] 11.9 **guide — destination AMENDED by WS19 (2026-07-08): the website's docs section, NOT mdBook** (WS19.1 delivered the docs skeleton at site/docs/ — getting-started/features/architecture; the full guide lands there.) (one docs pipeline; mdBook dropped). Content unchanged: architecture tour, reactivity mental model (signal/memo/effect/probe), embedded bring-up (display driver + input + heap sizing), theming via the styler registry, writing a custom widget, the minimal/e-paper pattern. Numbers cited from WS17. Lands on WS19.1's docs skeleton.

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

**Sessions:** 2–3 (now includes the folded WS7 items) · **Risk:** medium-high · **Depends on:** folds WS7.2/7.6/7.7 (shared widget/trait/derive surface); keeps the current `W`-generic ctx (NOT 7.5) so it is **NOT G4-gated** · **RESEQUENCED before WS5 (2026-07-09 — see decision block)** · Goal: stop carrying build-only props through the widget's whole lifecycle — RAM per widget + a cleaner `build`.

**DECISION 2026-07-09 (maintainer): RESEQUENCED — WS13 moves BEFORE WS5, bounded scope.** The clean (node-free, `Rc`-free, `ElId`-identified) form of WS5.1 (Layout off-graph) requires deferring reactive-layout bindings to build time, which forces build-only props onto the *retained* widget unless construction is split from retained state — i.e. **WS5.1's clean storage is gated on the builder/widget split.** Full rationale, blast radius, and the pivotal finding (today the fake-inert node absorbs props, so inflation is what the pure-Arena WS5.1 would *create* — the split prevents it) in `docs/plans/2026-07-09-ws13-views-as-builders-analysis.md`. **Bounded scope** (reshape the widget/trait/derive surface *once*): fold **7.6** (Widget trait method set), **7.7** (`derive(View)` param detection), and **7.2** (de-generic `Dir→Axis`/`V→numeric` — removes the `PhantomData` markers the census flags as build-only). **Keep the current `W`-generic ctx (NOT 7.5)** → not G4-gated. WS7's remainder (7.1/7.3/7.4/7.5) stays in its late slot. Recommended shape (analysis §4): an explicit builder consumed into a lean retained widget (`build` *transforms* the type), with a derive to kill boilerplate — not the `pending`-queue half-measure. The analysis already covers most of 13.1's "design doc" ask; next deliverable is the 13.1 spec proper. **Flagged discrepancies (protocol): `icon.rs` does not currently compile (known WS4.5 debt); `image.rs` disabled — both need an explicit call if the split touches them.**

- [ ] 13.1 Design doc: builder/runtime split options — separate Builder types per widget vs a generic builder layer vs build-consumed fields (`Option::take` at build). Note the existing hint: `El`'s two-state `New(ElData)/Stored` enum is already half of this idea. Quantify candidate RAM savings with the 0.4 probe before choosing. **Scoping analysis delivered 2026-07-09 (`docs/plans/2026-07-09-ws13-views-as-builders-analysis.md`) — blast radius + go/no-go + recommended shape (§4): the explicit builder consumed into a lean retained widget (`build` _transforms_ the type: builder → widget) + a boilerplate-killing derive; reject the `pending`-queue-on-the-widget half-measure. RAM caveat (§1): retained widgets are already lean — the payoff is *enabling* node-free off-graph layouts + collapsing the 3-way child encoding + retiring the fake-inert/`layout_mut` trap/WS3.5 zip, not husk-stripping alone. 13.1 REMAINING = the concrete `build`-transform protocol (how `El`/arena/`build` change), the derive design, per-widget migration + the design-around cases (`Show` layout-delegate; reactive-structure effects), and the folded 7.6/7.7/7.2 changes.** **[x] LANDED 2026-07-09 — the 13.1 spec is `docs/plans/2026-07-09-ws13.1-builder-widget-split-spec.md` (commit `5fb2ff9`): the `Build<W>` transform protocol + two-stage `ElStage` arena node + per-type identity-`Build` coexistence (no blanket) + the `#[derive(Builder)]` design + 7.6/7.7/7.2 slices + design-around cases.**
- [x] 13.2 Prototype on two widgets (Button, Flex) + measure (RAM/flash/ergonomics diff). **LANDED 2026-07-10 (`ws13-builder-widget-split`, commits `997677f..8a125e6`; rsact-ui lib 65/0). Findings for the gate: `docs/plans/2026-07-09-ws13.2-prototype-findings.md`. Result: reactive-node delta 0 (props still bind via node — the win is WS5.1's), heap −4%, flash +0.34% `.text`, retained widget −68–70% vs builder, ergonomics 4.8:1 boilerplate cut (break-even ~9 of ~15 widgets). FULL `#[derive(Builder)]` delivered (not the fallback). Deviations logged in the findings: dropped `LayoutWidget:Widget` supertrait; retained `Flex<W>` needs `PhantomData<W>`; latent split breakage in workspace-excluded `size-probe/bin/ui.rs` found+fixed. TDD plan: `docs/plans/2026-07-09-ws13.2-builder-widget-split-prototype-plan.md`.**
- [x] 13.3 **Rollout decision gate — SIGNED OFF: GO (maintainer, 2026-07-10).** Decision: **GO for the fleet conversion**, with the findings-§6 conditions **binding and sequenced FIRST**: (1) derive hardening — M1 both-attrs guard, M2 `#[layout(delegate)]` for `Show`, M3 the extra child modes; (2) sweep of workspace-excluded surfaces (size-probe bins, `examples/` call sites, cfg'd-off code); (3) landing the `icon.rs` repair (WS4.5 debt — needs `SignalOnWrite`; if it resists a contained fix, report back rather than forcing it). No fleet-wide widget conversion lands before the conditions are green. _(Original ask: sign-off on the measured 13.2 prototype; recommendation was GO-with-conditions and was accepted as-is.)_
- [x] 13.4 Fleet conversion (**APPROVED at 13.3 — conditions first**) + `#[derive(View)]` adjustments. **DONE 2026-07-13 — branch `ws13-fleet-conversion` (22 commits, `3433a28..30e603f`), every task two-verdict reviewed + final whole-branch review: MERGEABLE, no fix wave.** Conditions landed FIRST: M1 one-role-per-field derive guard `dff3a34` · M2 `#[layout(delegate = "field")]` + missing-layout diagnosis `46c4e5d` · excluded-surfaces sweep + pre-fleet example baseline `e5fad1b` (all 10 examples pre-broken — traced to pre-gate `f41a7c8`, none split-caused). Fleet: **12 widgets split** (Show `4b0b9a9` [M2 consumer], Label `504f765`, Space `a83732e`, Edge `152486d`, Bar `774b2f8`, Checkbox `4ba25cc`, Container `16a8bfe`, Slider `a86e6af`, Knob `670bdf6`, Scrollable `8ef2936`, Canvas `e3abe67`, Select `86fac76` [**no A18 entanglement found** — options `Rc`/`K` untouched], Icon `275e6e6`) + **Dynamic (5.12) and combinators/`Unit` (5.14) verified unsplit-by-design** (identity-`Build`; recorded in `2026-07-13-ws13.4-notes.md`) + M6 `ui_scenario` dedup `30e603f`. **icon.rs REPAIRED `7cfbc38` — the WS4.5 "needs `SignalOnWrite`" note was a MISDIAGNOSIS**: all 5 errors were mechanical (enum arity, stale `RenderCtx` sig, `'static` bound, style-macro two-color collision, missing `Stylist` wiring); the SignalOnWrite TODO itself is preserved and still open as a design question, but it does NOT block icon (`tiny-icons` compiles for the first time since WS4; ui **78/0** with it). **image.rs stays disabled** `18b6bd2` — corrected rationale on record: `Renderer::image` EXISTS (renderer.rs:151, both backends); revival = porting the eg-specific data model onto `ImageRef`/`DrawImage`, not a missing primitive. **7.6/7.7 delivered fleet-wide; 7.2 `Dir`→`Axis` delivered fleet-wide** (Space/Bar/Slider/Scrollable/Select; Knob never had one); **7.2's `V: RangeValue`→numeric slice DEFERRED to the WS7 remainder** (only live impl is the const-generic `RangeU8` family — no canonical numeric exists; in-code notes at bar.rs/knob.rs). Re-baseline: **node/heap unchanged vs post-13.2** (ui_labels_5 = 29 nodes / 11 823 B; ui_labels_10 = 49 / 20 730 B) — the split is the enabler; the node win lands with WS5.1, per spec §6. Suites: macros 6/0 · ui-lib 77/0 (+13 shape tests; 78/0 with tiny-icons) · reactive 76/0 (untouched, zero diff verified) · metrics-probe 15/15 · size-probe thumbv7m clean. Deferred Minors (final-review triaged, safe post-merge): M2 delegate non-identifier ICE (macros:246), Space ctor naming outlier, prototype-era comment nits. **WS5 IS UNBLOCKED once this merges.**

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
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS13 (the 2026-07-09 DECISION
block + the 13.3 GO record) + docs/plans/2026-07-09-ws13.1-builder-widget-split-spec.md
(the Build<W> transform protocol — the contract you implement) +
docs/plans/2026-07-09-ws13.2-prototype-findings.md (§6 conditions + logged deviations)
+ Cross-cutting invariants. 13.3 IS SIGNED OFF (2026-07-10): GO for the fleet —
the §6 conditions are BINDING and come FIRST.

Context already established by the analysis session (do NOT re-derive — verify, then build on):
- WS13 is RESEQUENCED BEFORE WS5, bounded. It GATES WS5.1's clean off-graph layout
  (arena-owned, ElId-identified, node-free, Rc-free): removing the Layout fake-inert
  forces reactive-layout bindings to defer to build time, which forces build-only props
  onto the RETAINED widget UNLESS construction is split from retained state. There is no
  Rc-free/node-free builder-time residence for a binding effect (proof: analysis §2).
- Bounded scope: FOLD WS7.6 (Widget trait method set), 7.7 (derive(View) W-param
  detection), 7.2 (de-generic Dir→Axis / V→numeric — removes the PhantomData markers).
  KEEP the current W-generic ctx (NOT 7.5) → NOT G4-gated. WS7 remainder (7.1/7.3/7.4/7.5)
  stays in its late slot.
- Recommended shape (analysis §4): explicit builder consumed into a lean retained widget
  — `build` TRANSFORMS the type (builder → widget) — plus a derive to kill boilerplate.
  Reject the `pending`-queue-on-the-widget half-measure.
- Facts the census pinned (§1/§3): TODAY there is no prop inflation (the fake-inert node
  absorbs props); child widgets are already mem::replace'd into the arena (only {id,layout}
  husks remain in parent fields); build(&mut self, ctx) already receives the ElId and
  Dynamic already wires build-time effects capturing ctx (the pattern to extend to props).
  Blast radius: ~15 live widgets, the builder traits in widget/mod.rs, ~50 src + ~130
  examples call sites, the 6 by-Copy layout() collection sites (flex/container/scrollable/
  button/select/page) + arena.add husk-move + model.rs *content, the page/mod.rs:1811
  reactive-setter regression tests.
- Design-around cases: (1) Show owns no layout — layout() delegates to its child husk;
  (2) reactive STRUCTURE (Flex.children / Dynamic.current / Select setter) stays on the
  existing build-time-effect path — only reactive PROPS are newly routed through build.
- Pre-existing discrepancies to decide on if the split touches them (protocol: report):
  icon.rs does not currently compile (known WS4.5 debt); image.rs is disabled.

Verify current state first (13.2's Button+Flex split + full #[derive(Builder)] are on
master via PR #17; baseline rsact-ui lib 65/0, reactive 76/0; re-read cited code — it
may have moved). Use superpowers:writing-plans to expand this into a bite-sized TDD plan,
then execute in order:
(1) CONDITIONS — derive hardening: M1 both-attrs guard (compile-fail test), M2
    #[layout(delegate)] for Show (layout() delegates to the child husk), M3 the extra
    child modes — TDD each; excluded-surfaces sweep: size-probe bins (workspace-excluded
    — latent split breakage was found+fixed there once already), examples/ call sites,
    cfg'd-off code; icon.rs repair (WS4.5 debt, needs SignalOnWrite) — if it resists a
    contained fix, REPORT back, don't force it; image.rs stays disabled (report if the
    split touches it).
(2) 13.4 FLEET CONVERSION — widget-by-widget commits (~13 remaining of ~15), folding the
    7.6/7.7/7.2 slices where they touch each widget; UI suite green after EVERY widget;
    design-around cases per the spec (Show layout-delegate; reactive STRUCTURE stays on
    build-time effects — only reactive PROPS route through build).
Re-baseline the 0.4 node/RAM numbers and record old→new in the roadmap; mark 13.4 done
with commit hashes. Fresh worktree off master (e.g. ws13-fleet-conversion). Keep
full-relayout the default path; do NOT start WS5 work in this session — WS5.1 launches
only after this lands.
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

### WS19 — Website: VitePress on GitHub Pages (maintainer-initiated 2026-07-08)

**Sessions:** 2 · **Risk:** low · **Depends on:** nothing (metrics infrastructure already delivered) · **Decisions recorded (2026-07-08):** VitePress chosen (docs+landing+blog hybrid; Vue components enable native metrics charts and future H9 WASM demos; Node toolchain isolated in `site/`); **one assembled Actions Pages deployment**; **VitePress owns all docs** — WS11.9's mdBook is dropped, the guide's destination becomes the site's docs section, rustdoc served under `/api/`; **v1 scope = landing + metrics section + docs skeleton** (blog is a later item); **github.io/rsact path** for now (`base: '/rsact/'`; custom domain = one CNAME + base switch later).

- [x] **19.1 Scaffold:** `site/` dir with its own `package.json` + committed lockfile (`node_modules` ignored — the Node toolchain stays contained in one directory of a Rust repo); VitePress init with `base: '/rsact/'`, dark/light, Rust via Shiki, nav skeleton (Docs · Metrics · Roadmap · GitHub); `npm run docs:build` green locally and as a CI check. **DONE — 69ba50e/837a733.** site/ scaffolded (VitePress + Vue 3 + TS + SCSS + Vitest); npm run docs:build green locally; PR build-check wired via site.yml (Task 19.2). node_modules ignored, package-lock committed.
- [x] **19.2 Pages deployment rework (the structural piece):** switch the repo's Pages source from the `metrics-data` branch to **Actions deployment** (`actions/upload-pages-artifact` + `actions/deploy-pages`). A `site.yml` workflow assembles ONE artifact: VitePress `dist/` + the metrics data/dashboard copied from `metrics-data` under `dist/metrics/` (+ later `dist/api/` from 19.5). Triggers: master push (content changes) **and** after `metrics.yml` records fresh snapshots (`workflow_run` or repository_dispatch; mind the publish-race notes from the 0.9c verification). **`metrics-data` stays the durable data store, write path unchanged** — it just stops being the Pages source. PR sticky comments unaffected. Metrics URLs move to `/rsact/metrics/`. **DONE (code + local verification) — c5a6f8b/da1f0a5.** site.yml assembles ONE Actions Pages artifact (VitePress dist + metrics data.json from the metrics-data branch) and deploys via actions/deploy-pages; PR = build-only CI check; metrics.yml write path + ci.yml (Node-free) unchanged. Pages-source flip to GitHub Actions + post-merge deploy verification are the documented human cutover (site/README.md).
- [x] **19.3 Metrics section:** preferably build 0.9e's time-series viewer **as a Vue component in the site** (reads the copied snapshot JSONs + `index.json`) rather than extending the standalone `html.rs` viewer — record the split: `html.rs` viewer = the _local_ store's dev view; the site component = the _CI_ store's public view. Coordinate with 0.9e — whichever session runs first implements the charts, the other consumes/ports. **DONE — 7b36f8b/8f66afc.** Ported the landed 0.9e Vue viewer into the site as <MetricsDashboard> (TS SFCs + typed pure libs with their vitest suites), fetching one assembled /metrics/data.json at runtime. Split recorded: metrics-probe/viewer = local dev view; site component = CI public view. (WS19.7 dissolved this split: the standalone viewer is removed; the site is the one viewer and its dev server charts the local store.)
- [x] **19.4 v1 content:** landing page (the pitch: _you pay for what you wire_; hardware-measured numbers only — from the CI/size-probe rows, never estimates; a real code sample; the honest LVGL/Slint comparison once WS17 provides it, placeholder marked until then); docs skeleton (getting started, feature matrix seeded from `docs/features.md`, architecture overview); roadmap page linking the artifact/repo. **DONE — 80d682c/2c2cbb3.** Landing (you-pay-for-what-you-wire pitch, real sandbox-derived sample, WS17 comparison marked placeholder), docs skeleton (getting-started/features/architecture), roadmap page.
- [x] **19.5 rustdoc under `/api/`:** CI builds workspace rustdoc (documented feature set: `std,embedded-graphics`; `doc(cfg)` annotations arrive with WS11.8) → copied into the Pages artifact. **DONE (code + local verification) — 5073bfe.** site.yml builds workspace rustdoc (std,embedded-graphics) into dist/api/ with an index redirect to rsact_ui.
- [ ] **19.6 Later (explicitly not v1):** blog scaffolding (`createContentLoader` posts index + RSS; first-post candidates on record: the evolution-plan story, the Probe redesign write-up); custom domain (CNAME + `base: '/'` switch); og/social cards; sitemap.
- [x] **19.7 Metrics viewer v2 + single-viewer consolidation (2026-07-09):** removed the standalone `metrics-probe html` viewer (deleted `metrics-probe/viewer/` + `src/html.rs` + the `html` subcommand/record-hook); the site is now the single metrics home, and its dev server charts the local git-ignored `metrics/` store via a Vite plugin sharing one `assemble()` with the CI `data.json`. Enhancements: fixed-layout sticky uniform-width first column; cell-centered inline-chart alignment (bug fix); global unchanged-commit collapse (`a..b` columns, prevPresent boundaries, nulls never split); stable per-metric colors; `layout:page` full-width; synchronized crosshair; Δ-from-baseline; only-changed filter; URL-hash state. All logic in pure vitest-tested `lib/*.ts`. **DONE — bdd9b79..ec9c427.** (Branch `ws19-metrics-v2`, stacked on PR #12.) The metrics-data branch's stale index.html/README/.nojekyll are vestigial (harmless; optional manual prune).
- [x] **19.8 Dashboard Phase A — unified grid, sticky captions, dims, commit links, Δ-overall (2026-07-09):** unified grid unifying th/td column structure; sticky tbody section captions per metric with diff highlight; per-column dim/brighten opacity toggle; Δ-overall net-arrow row showing aggregate change; clickable commit-hash headers with GitHub commit/compare links; hover legend folded into dashboard (TrendChart internal tip dropped). **DONE — 467b7df..df86a19.** Phase B (commit-message tooltip #4 + PR grouping/link #7 via build-time `subject`/`pr` enrichment of index) logged as follow-up.
- [x] **19.8 Dashboard Phase B — commit-message tooltip (#4) + PR grouping (#7) (2026-07-10):** `metrics-probe` enriches `index.json` with commit `subject` (from git) + `pr` (pure-git, ancestry-first: merge commits `M^2 --not M^1`, squash `(#N)` only where no merge covers, via `index::resolve_pr`); the site appends the subject to the column header's native `title` and groups columns by PR (fallback: the stored branch) in a third sticky header row with linked `#N`/branch labels + a `group-start` separator. Backward-compatible (`skip_serializing_if`); degrades gracefully until CI re-runs `metrics-probe index` (branch grouping works immediately; `#N` links + subjects appear after repopulation). **DONE (branch ws19-metrics-v4).** WS19.8 complete (Phase A + B).

Acceptance: site live at `hazer-hazer.github.io/rsact/` with landing + docs skeleton + a metrics section showing live per-commit data; the `metrics-data` store and PR comments provably unaffected (post-deploy snapshot lands, comment appears); `site/` build green in CI; WS11.9's text amended.

**Status 2026-07-09:** v1 code complete on branch ws19-website (19.1–19.5). Remaining = the human cutover (Pages source → GitHub Actions, merge, post-deploy metrics-data/PR-comment verification) per site/README.md.

**Session prompt (WS19):**

```
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS19 (all decisions inline).
Verify current state: Pages currently serves the metrics dashboard from metrics-data;
check whether 0.9e landed (determines 19.3's build-vs-port). Order: 19.1 scaffold →
19.2 deployment rework (the risky piece — verify metrics-data writes + PR comments
still work after the Pages source switch) → 19.3 metrics section → 19.4 content →
19.5 /api/. Landing-page numbers come from the CI store only, never estimates —
mark WS17-dependent claims as placeholders. Amend WS11.9's item text (mdBook → site
docs section) when done. Node stays inside site/; do not add Node steps to ci.yml —
site.yml owns the web build. Mark items done with commit hashes.
```

---

### WS20 — Reactive node storage: fold the edge maps into the node (AoS)

**Sessions:** 1–2 · **Risk:** medium (touches every graph-walk hot path) · **Directions:** D1 reactive core + D6 footprint · **Depends on:** **WS5 — do after** (WS5 changes how many nodes/edges exist and how layout subscribes; folding the edge maps _before_ WS5's off-graph rework lands means doing it twice) · **Relates to:** WS9b (engine surgery — coordinate so the read path is not churned twice, Decision 2) · **Feeds:** **WS18** (co-locating edges on the node is the shape WS18.2's heapless per-node edge lists want — `FixedStorage` becomes `[Node; N]` with no parallel `SecondaryMap`s).

**Origin:** maintainer question, 2026-07-29 (session on `runtime.rs`/`storage.rs`). "What if `subscribers`/`sources`/`owned` were stored directly inside `Value`, avoiding sparse `SecondaryMap` usage — better for runtime and memory, and is it even possible given overlapping borrows?" Analysis below.

Why: today the graph is **struct-of-arrays** — `Storage.values: RefCell<SlotMap<ValueId, Value>>` plus three parallel `RefCell<SecondaryMap<ValueId, IdVec>>` (`subscribers`/`sources`/`owned` in `runtime.rs`). Each `SecondaryMap` is a dense `Vec` sized to the high-water-mark index, so every live node costs an `IdVec` slot **in all three** whether or not it has edges, plus a redundant per-map version word. Graph traversal is the hot path and it touches several fields of _one_ node together (read a node's subscriber list, flip a neighbour's state) — the textbook case where **array-of-structs** (edges on the node) wins on cache locality. The prize is a constant-factor-per-node reduction (≈15–25% of graph bookkeeping + better locality), **not** a change to the O(N) shape — N itself is WS5's lever; WS20 shrinks the cost of the nodes that remain. That is _why it is sequenced after WS5, not before_.

**Why the naive field-move is impossible (borrow-liveness).** The split of `values` / `subscribers` / `sources` into _distinct_ `RefCell`s is load-bearing: a graph walk holds a shared borrow of one edge map while mutating `values` (node state) or another node's edge set. Collapse them into one `RefCell<SlotMap<Value>>` and four hot sites become `borrow`-vs-`borrow_mut` panics:

- `update` commit path (`runtime.rs:1073`) — holds `subscribers.borrow()` for `id`, calls `storage.mark(*sub, Dirty)` (`values.borrow_mut()`) per subscriber.
- `mark_check_closure` (`runtime.rs:1169`) — holds `subscribers.borrow()` for the **whole** transitive walk while marking each node (the comment at `runtime.rs:1167` documents the invariant it depends on).
- `clear_sources` (`runtime.rs:1434`) / `dispose` (`runtime.rs:775`) — read one node's list while `get_mut`-ing every neighbour's list (cross-node `get_mut` on one `SlotMap` — not safely expressible).

**Design that works (a lock-granularity change, not just a layout change):**

```rust
pub struct Value {
    pub value: Rc<RefCell<dyn Any>>,
    pub kind: ValueKind,
    state:  Cell<ValueState>,     // Copy → mutate through &Value (no borrow_mut)
    height: Cell<u32>,            // Copy → mutate through &Value
    subscribers: RefCell<IdVec>,  // per-NODE lock, not one lock over all nodes
    sources:     RefCell<IdVec>,
    owned:       RefCell<IdVec>,
}
```

- `state`/`height` are `Copy`, so `Cell::set` mutates through `&Value` — `mark_check_closure` runs the whole walk under a single `values.borrow()` and flips `child.state.set(Dirty)` with no contention.
- Edges become **per-node** `RefCell<IdVec>`, so the cross-node ops read `id.sources.borrow()` while doing `source.subscribers.borrow_mut()` — different `RefCell`s (self-subscription is already rejected at `runtime.rs:882`, so `id ≠ source`).
- Net: `SlotMap::borrow_mut` shrinks to **only** `add_value` and `dispose`'s final `remove` — a _stronger_ invariant than today's many `borrow_mut` windows.

Stages:

- [ ] **20.1 `owned` first (lowest risk):** `owned` is mostly single-node access (`runtime.rs:1463`, `:544`) — migrate it into `Value` as a proof the AoS + per-node-`RefCell` scheme compiles and holds the single-shared-borrow invariant, before touching the coupled pair.
- [ ] **20.2 `state`/`height` → `Cell`:** convert the two `Copy` scalars; rewrite `storage.mark`/`set_height` to take `&Value` + `Cell::set`; confirm `values.borrow_mut()` disappears from every walk.
- [ ] **20.3 `subscribers`/`sources` → per-node `RefCell<IdVec>` in `Value`:** migrate the coupled pair together; rewrite `update`/`mark_check_closure`/`clear_sources`/`dispose`/`subscribe`/`update_height`; delete the three `SecondaryMap`s from `Runtime`.
- [ ] **20.4 Cold-path clone audit:** `Storage::get` (`storage.rs:457`) now clones three `IdVec`s — confirm no _hot_ caller regressed (hot paths already use `state_of`/`kind_of`/`value_rc`/`get_height`, which are unaffected).

**Design sketch (borrow flow, after):**

```text
mark_check_closure:  values.borrow()  held for the whole walk
                       read  node.subscribers.borrow()        (per-node shared)
                       write child.state.set(Dirty)           (Cell, through &Value)
clear_sources:       values.borrow()  throughout
                       read  id.sources.borrow()
                       write source.subscribers.borrow_mut()  (id ≠ source → distinct cells)
add_value / dispose: the ONLY values.borrow_mut() sites (structural insert/remove)
```

Acceptance: `benches/allocations.rs` shows **no** alloc-count/bytes regression (the gate — the harness resolves single allocations, cf. the `2/112 → 4/224 B` note at `runtime.rs:1081`) and `benches/reactivity` shows a traversal win from locality; `SlotMap::borrow_mut` appears only in `add_value`/`dispose`; **no** per-visit heap allocation is introduced (i.e. no neighbour-list snapshotting — that "solution" would trade the locality win for allocations and is explicitly rejected); full reactive + UI suites green (serial, per CLAUDE.md).

**Session prompt:**

```text
Read docs/plans/2026-07-05-rsact-evolution-roadmap.md — WS20. VERIFY WS5 landed first
(this is a "do after WS5" item — re-baseline against WS5's post-off-graph node/edge counts;
WS5's layout bindings changed how subscribe/sources are populated). Do NOT do a naive
field-move: state/height become Cell, subscribers/sources/owned become per-node
RefCell<IdVec>, so the SlotMap only needs borrow_mut at insert/remove. Land 20.1 (owned)
as a standalone proof, then 20.2, then the coupled 20.3. Gate every stage on
benches/allocations.rs (no regression) + benches/reactivity. Coordinate with WS9b
(Decision 2) so the read path is not churned twice.
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
| A18 | Interactive-widget internal-structure refactor — `Select`'s options storage (`Rc<MaybeReactive<Vec<SelectOption>>>`, the WS4.1 `Rc` wrap at `select.rs:99`) + the state/selected wiring are awkward (maintainer: "looks horrible"). Revisit once the core reactive/layout/ctx refactors (WS4/WS5/WS7) stabilize (PR #11 review, 2026-07-09) | BACKLOG (post-core)  |
| A19 | **AA primitive rasterizers — rewrite or replace (added 2026-08-01).** `eg/primitives/{arc,sector,rounded_rect}.rs` iterate their full bounding box and rely on the clip inside `pixel_alpha` → `draw_pixels` to reject writes, so clipping is a **write filter, not a loop bound** — a clipped pass costs the same as an unclipped one. `arc.rs` additionally evaluates `atan2f` per pixel for the sweep test (removable outright: two cross-product sign tests against the boundary rays). Maintainer's position (2026-08-01): these are acknowledged stubs to be rewritten from scratch or replaced with an external lib — **no design decision may be centered on their current cost**, and WS6.4's cost model deliberately excludes them. Two consequences worth keeping: intersecting primitive bounds with the active clip before the loop wins on Scrollables/`ClipPath::InnerRect` **today**, single-pass, and is a hard prerequisite for any multi-pass mode; tiny-skia is already tile-friendly by comparison (scanline rasterizer clips spans, so per-tile cost is O(segments) flatten + O(spans in tile) fill) | BACKLOG              |
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

## Issue inbox (local "GitHub issues" — ideas · RFCs · bugs · cleanups)

Opened 2026-08-01 (maintainer). Things found **while developing the plan** that are worth recording but do **not** block any plan iteration: design ideas/RFCs, bugs, cleanups, ergonomics gripes. Deliberately kept here rather than in GitHub Issues — this repo's process lives in this file, and a second tracker would split it.

**Convention.** One block per item, ID `ISSUE-<n>`, append-only — never renumber, and a closed item keeps its number. Each block carries Filed / Kind / Status / Area / Relates, then the submission, then what the code says today (grounded, `file:line`), then a verdict with promotion criteria. Statuses: `OPEN` (recorded) · `ACCEPTED → WS<x>` (promoted — the charter moves to that WS and this block becomes a pointer) · `CLOSED — done` (with commit) · `CLOSED — rejected` (with the reason; also mirror into the Parked/rejected register if it is the kind of thing that gets re-proposed). An item promoted to a workstream is executed **there**, never from here. Items here carry **no commitment** and no sequence position — the plan's order is unaffected by their existence.

### ISSUE-1 — Probe "ghosting": a probe with no reactive sources degenerates into an inline callback

| | |
| --- | --- |
| **Filed** | 2026-08-01 (maintainer) |
| **Kind** | idea / optimization (RAM + per-poll CPU) |
| **Status** | **OPEN** — recorded, not scheduled; gated on evidence (see verdict) |
| **Area** | `rsact-reactive` (`probe.rs`, `runtime.rs`) · `rsact-ui` (`el/render.rs`, `el/state.rs`) |
| **Relates** | **WS4.4** (same target, owner-side mechanism — investigation `docs/plans/2026-07-09-ws4.4-zero-source-probes-investigation.md`) · **WS4.3** (relocated to WS5: `force_redraw` demotion — a hard precondition) · **WS20** (AoS fold changes the size of the prize) · invariant "no core registry for owner-held handles" |

**The submission.** A probe whose callback sourced no reactive value need not exist in the runtime at all — its `poll` could simply call the closure inline. It cannot be decided once per probe lifetime, because a conditional path may start reading a signal later; so `Probe` would have to be an **enum switching between the two modes**. Maintainer's own read: _"a pretty awkward idea, maybe still it has its place as possible future optimization if proved so."_

**Relation to WS4.4 — same target, different location, not a duplicate.** WS4.4 investigated this win from the **owner** side: `ElState`'s part entry becomes `enum PartGate { Static, Probe(Probe) }`. Recorded there: (a) author-declared opt-in static — recommended, sound, no core change; (b) demote-after-N-clean-runs — **rejected as unsound** (the conditional-first-run hazard); (c) lazy re-promotion via a sentinel observer — sound and automatic, held behind its own decision gate because it costs a branch on the hot `track()` path. **This submission is (c) relocated from the owner into the primitive**: the mode enum lives in `Probe` itself, so every probe user — including an out-of-tree render engine, which the probe module docs explicitly invite — inherits it, not just `rsact-ui` parts. That generality is the real delta; 4.4's hazard analysis applies to it unchanged.

**What the code says today** (five facts that decide this):

1. ~~**The ghost population is empty today — a precondition, not a detail.**~~ **✓ RESOLVED 2026-08-04 by WS6.4.0(iv) (PR #34) — this fact is now stale, and the precondition it names is satisfied.** It read: `render_part` calls `self.shared.force_redraw.track()` _inside_ every probe poll body, so every part probe has ≥1 source forever by construction, and nothing can ghost until that page-wide edge dies. That edge is **gone**: `force_redraw` is a plain `bool` carried down the walk and OR-ed into the part gate (`el/render.rs`), not a signal each part subscribes to. Consequence for this issue: criterion (i) below is met, so the idea is now **measurable** — which was the one thing blocking evidence. It is still gated on (ii)–(iv). Note the population is not automatically non-empty: a part that reads any reactive value still has sources, and 4.4's warning stands that `ui_labels`' label part reads a content memo.
2. **Inline ≠ untracked-by-nobody.** Running the closure with no observer installed does not merely skip tracking — reads inside it attach to whatever observer is **ambient** (`Runtime::subscribe` reads the current-observer cell; `run_probe` installs one via `with_observer`, `rsact-reactive/src/runtime.rs:752`). A ghost polled inside a parent observer would silently donate its dependencies to the parent, which then re-runs on changes it cannot attribute — precisely the page-wide fan-out WS2/WS6 removed. So a ghost must still install **something** as current observer for the duration of the call: 4.4(c)'s slot-less sentinel. **The enum handle does not avoid the sentinel; it only relocates where the two modes are stored.** This is the honest core of the "awkward" feeling — the saving is in the runtime, but the mechanism still has to touch the read path.
3. **Promotion must be retroactive within the same poll.** It fires on the first `track()` _during_ the run, so the node must be minted **and subscribed inside that very call**, recording the triggering read (mechanically fine: `track()` → `subscribe` happens at read time, so mint-then-subscribe is in-band). If promotion only took effect from the _next_ poll, the first reactive frame loses its edges — and a branch that reads once and then stops re-reading loses them permanently: stale UI, the same failure mode that got (b) rejected.
4. **The owner already holds the handle mutably — but `Probe` is `Copy`.** In-place promotion needs no side table in rsact-ui: `RenderCtx.part_probes: &'a mut TinyVec<[(&'static str, Probe); 2]>` (`el/render.rs:87`), and `render_part` already lazily creates and pushes probes (`el/render.rs:275-291`). That matters because WS2 deliberately **deleted** the hash-keyed registry — a promotion channel must not resurrect a core-side ghost→node map (invariant: ownership maps for reactive handles live with their owner, never in a core registry). But `Probe` is `#[derive(Clone, Copy, …)]` with `poll(&self, …)` (`rsact-reactive/src/probe.rs:80,96`): the moment a ghost handle is copied, promoting one copy leaves the others as ghosts that silently track nothing. So ghosting needs either `poll(&mut self)` + dropping `Copy` (rippling to the `TinyVec` `Default`-padding use the module docs call out, to `PartialEq`, and to every owner storing handles by value) or a **by-construction** "a ghost is never copied" rule — much harder to guarantee than to state.
5. **What one ghost actually buys.** One `SlotMap` slot + its `Rc<RefCell<dyn Any>>` payload + its entries in the three edge maps (~150 B/node class per the baselines table; **WS20's AoS fold changes that number and may shrink the prize**), plus per-poll `is_alive` + `subscribe` + `maybe_update` dispatch (`runtime.rs:737-742`). Population: genuinely static parts — a `Label` over `"literal".inert()`, fixed dividers, constant icons. Heed 4.4's warning: the canonical `ui_labels` scenario's label part reads a reactive content memo, so it would **not** ghost. The win must be measured with a static-part counter in the 0.4 scenario set before anyone believes in it.

**Verdict (recorded, not scheduled).** Keep as a candidate, and keep it **behind WS4.4(a)**: (a) is sound, owner-side, needs no core change, and costs nothing on the `track()` path. Ghosting buys **automaticity** over (a) and pays for it inside the primitive every reactive read goes through. Promote only when all four hold:

- (i) `force_redraw`'s per-part track edge is gone (WS4.3-in-WS5 / WS6 force-path rework) — else there is nothing to ghost;
- (ii) a measured static-part count on a real page is a material fraction (add the counter to the 0.4 scenarios);
- (iii) the sentinel branch measures negligible in `benches/reactivity` — the whole graph would pay it to speed up a few static parts;
- (iv) `Copy`-loss (or a mechanically-enforced no-copy rule) has an accepted shape, and the promotion path adds no core-side registry.

Failing (i) it cannot be measured; failing (iii) it is a net loss. The evidence that would most cleanly promote it: (a) lands, and measurement then shows most static parts are _not_ author-annotatable in practice.

### ISSUE-2 — A text change bypasses the arena dirty set, so every one is a blanket relayout + whole-viewport flush (with `incremental-layout` ON too)

| | |
| --- | --- |
| **Filed** | 2026-08-04 (found by WS6.4a's measurement harness, not by inspection) |
| **Kind** | bug / gap — invalidation channel mismatch (CPU + flush bytes on the commonest dynamic change there is) |
| **Status** | **OPEN** — recorded, not scheduled. Blocks nothing in flight; changes what 6.4d can promise. |
| **Area** | `rsact-ui` (`widget/label.rs`, `layout/*` `ContentLayout::text`, `el/build.rs` `bind_layout`, `page/mod.rs` `compute_layout`) |
| **Relates** | **WS5.2** (incremental relayout — its precondition is the dirty set) · **WS6.1** (targeted repaint roots — never reached) · **WS6.4a** (the measurement) · **WS6.4d** (the interactive-frame promise) · **WS5.4** (text-measure cache — same read path, different problem) |

**What was measured.** `tile_damage_240.txt`, on a 240×240 page (header label · checkbox · signal-bound label · footer label), using rsact's **own** damage rects rather than an invented partition:

| frame | regions | coverage | required ops |
| --- | --- | --- | --- |
| cold (whole viewport) | 1 | 1.00 | 480 |
| paint-only (`checked.set(true)`) | 1 | **0.00** (16×16) | **3** |
| text (`caption.set("value 1")`) | 1 | **1.00** | **478** |

A checkbox toggle damages exactly its own rect — the WS6 damage machinery working as designed. A one-character text change on a same-width string damages the **whole viewport**, i.e. costs a cold frame. **Identical with `incremental-layout` enabled** (the golden matches byte-for-byte under both feature sets), which is the part worth filing: the two workstreams built to prevent exactly this do not engage.

**Why — a channel mismatch, not a threshold.** There are two ways a layout input can change, and only one of them is recorded:

1. **Marked channel.** `BuildCtx::bind_layout`'s effect writes the prop and calls `arena.mark_dirty(id)` (`el/build.rs:151`); `set_children`/`set_single_child` call `mark_full_relayout`. This is the channel `relayout_if_needed` reads (`arena.is_layout_dirty()`) and the *only* one WS5.2/WS6.1 can act on.
2. **Tracked-read channel.** A `Label` hands its content to its layout — `ContentLayout::text(content.clone())` (`widget/label.rs:60`, holding a `MaybeReactive<String>`, `layout/mod.rs:130`) — and the string is read during **measurement** by tracking `content.with(…)` calls (`layout/mod.rs:118`, `:151`, `:188`), which run inside `model_layout`, which runs inside the layout probe's poll closure. So the text is a *tracked source of the layout probe*: a write dirties the probe and the next `poll` recomputes, with **nothing marked in the arena**. Confirmed from both ends — `widget/label.rs` never calls `bind_layout` at all, and the measured behaviour is identical with `incremental-layout` on, which only an empty dirty set explains.

Follow `compute_layout` with an empty dirty set: the incremental branch requires `!_dirty.is_empty()`, so it is skipped → full `model_layout` → `targeted` stays false → `blanket = true` → `Page::force_redraw()` → `full_flush` → damage = whole viewport. Both feature paths land there, one by omission and one by construction. Note the mechanism is *correct* (the frame does repaint, and the probe subscription is what wakes it) — it is only maximally pessimistic.

**Why it matters beyond one label.** Text is the commonest reactive content in a UI, so this is the default case, not an edge: a value readout, a clock, a progress percentage each repaint and re-flush the entire screen. Under WS6.4d the price rises rather than falls — a cold frame under tiling costs the region multiplier (6.4a: ×1.42–2.17 culled, ×N uncalled), so "interactive frames +0%" holds only for paint-only changes until this is fixed. It also interacts with **WS5.4**: the text-measure cache would make the recompute cheaper without making it targeted, so the two are complements, not substitutes.

**Candidate directions (not a decision).** (a) Route the tracked read into the marked channel: have `ContentLayout::text` (or `Label`'s builder) `bind_layout` a memo of the *measured size*, so a text change marks its own node dirty and WS5.2/WS6.1 engage unchanged — smallest change, and it makes the content→layout edge explicit rather than implicit. (b) Let `relayout_if_needed` learn *which* probe sources changed and derive the dirty set from them — more general, needs a reactive-side capability rsact-reactive does not have, and 6.4c(2) is on record that 6.4 needs no reactive changes. (c) Accept it and rely on damage-driven flush alone — rejected by the measurement: the damage *is* the whole viewport, so there is nothing left to narrow. Direction (a) is the obvious first probe; the question it must answer is whether measurement inside `bind_layout`'s effect can see the font context it needs.

**Verdict (recorded, not scheduled).** Do not fold into 6.4b/6.4c — it is an invalidation-channel question, not a rendering one, and either WS5 or a WS6.1 follow-up owns it. Promote when: (i) 6.4b/6.4c land, so the tiled interactive frame is the next thing measured; or (ii) any hardware profile shows text-driven pages dominated by flush bytes (WS17). The regression test already exists: the `text-change` row of `tile_damage_240.txt` is exactly the number a fix must move, and it is asserted-by-golden today, so a fix cannot land silently.

### ISSUE-3 — Widgets paint outside `layout.outer`, so damage under-reports and both culls can crack a tile

| | |
| --- | --- |
| **Filed** | 2026-08-05 (maintainer asked for the audit while settling 6.4c; found by reading every widget's draw calls, not by a failing test) |
| **Kind** | bug — invariant violation. Every geometry decision in WS6 assumes "a widget never draws outside `layout.outer`"; nothing checks it and it is already false. |
| **Status** | **OPEN** — audit complete, mechanism designed (6.4c(G)), computation deliberately postponed. Maintainer: "This is a problem and I don't want to turn a blind eye to it … We don't need to calculate it now btw. It can be postponed, but must be documented." |
| **Area** | `rsact-render` (`primitives/block.rs`, `style/block.rs`) · `rsact-ui` (`el/render.rs` focus outline + damage push, `widget/{checkbox,slider,image,select}.rs`) |
| **Relates** | **6.4c(G)** (`paint_bounds` / `ext_draw`, the resolution path) · **6.4b** (the per-node cull tests `outer`) · **6.2** (the damage push records `outer`) · **6.4a** (its tile-invariance check is the only defence) · **6.4d** (where a 1 px under-report becomes a visible crack) |

**The audit — every widget's draw calls, verified against the source.**

| widget / path | outside `outer`? | mechanism |
| --- | --- | --- |
| `render_focus_outline` (`el/render.rs:141`) | **YES — structural, 1 px on all four sides** | `OutlineStyle::base()` is `offset: 0, width: 0` (`style/block.rs:181-188`); the call sets `.width(1)`, so `Block::render` takes `outline_rect = rect` and strokes it `StrokeAlignment::Outside` at width 1 (`primitives/block.rs:51-63`). Live on checkbox, button, slider, knob, select, scrollable whenever focused. |
| any `BlockStyle` with `outline.width > 0` | **YES — `outline_offset + outline_width` per side** | same path; theme-settable on every container widget, so this is a *style*, not an exotic case. |
| checkbox checkmark (`widget/checkbox.rs:98-106`) | **possible** | `CHECKBOX_ICON_POINTS = [(3,8),(7,12),(13,4)]` are fixed constants placed at `inner.top_left`, never scaled to the box, stroked at width 2 → spans ≈ x∈[2,14], y∈[3,13] at *any* checkbox size. Escapes `inner` below ≈15×14, and `outer` when padding does not absorb it. |
| slider thumb (`widget/slider.rs`) | **possible, style-dependent** | the thumb is centred on the track (`thumb_pos = start + canon(…, −half_thumb_size)`, `start` on `inner`'s cross-axis centre), so it escapes the cross axis whenever `style.thumb_size > inner.size.cross(axis)`. |
| image (`widget/image.rs:56`) | **possible** | eg `Image::new(&self.data, inner.top_left).draw(…)` paints the data's *intrinsic* size, unclipped — any layout rect smaller than the image escapes. |
| select selected-highlight (`widget/select.rs`) | **possible, latent** | drawn at `child.outer.translate(options_offset)` inside an unclipped part. Dead today (GAP 2: options are never arena children), so latent rather than live. |
| knob | **no** — *checked and refuted* | `sector(inner.top_left, inner.size.max_square().width, …)`, and `max_square()` is `min(w, h)` (`geometry/size.rs:85-89`) — the *inscribed* square, not the bounding one. |
| bar | **no** | `inner.resized_axis(…)` ⊆ `inner`. |
| scrollable's own scrollbar | **no** — arithmetic is exactly bounded | track sits at `inner`'s edge, offset inward by `scrollbar_width/2`; and with `thumb_offset = (s/c)·offset`, `thumb_len = s²/c`, `offset ≤ c−s` gives `thumb_offset + thumb_len ≤ s` exactly. |
| canvas | **no** | `clip_inner` confines the user closure. |
| edge / button / container | **no**, apart from the outline row above | `Block` on `layout.outer`. |
| icon | draws nothing | `TODO(unimplemented)`. |
| scrollable *content* | escapes — but this is the known intended-broken widget, not a paint-bounds bug; 6.4c(F)'s widget-declared clip is what fixes it. |

**Two consequences, both real today and both bounded by the outline width — which is why neither has been noticed.**

1. **Damage under-reports.** `render_part` pushes `layout.outer` (`el/render.rs:394`) as the flush region, so the outline's ring of pixels is painted into the framebuffer and then *not flushed*. On a partial-flush frame a focus outline appears only once something forces a full flush.
2. **Both culls can skip a part that would have drawn.** 6.4b's gate is `!layout.outer.intersects(&clip)`; a widget sitting 1 px outside a region whose outline reaches into it is skipped, leaving a 1 px crack. Harmless on a persistent framebuffer (last frame's pixels are still there); a visible crack under 6.4d, where the tile has no history.

**Resolution path — designed now, computed later (6.4c(G)).** One extensible function `paint_bounds(layout, ext) -> Rect` returning `layout.outer.outset(ext)`, with `ext == Padding::zero()` at every call site today, fed by a `Widget::ext_draw()` method defaulting to zero. All four consumers (the per-node gate, the damage push, the traversal prune, the seam's `debug_assert`) must route through it so that filling in `ext_draw` later is a one-function change. LVGL's `ext_draw_size` is the reference; the deliberate differences are that rsact **computes** it on demand rather than caching it on the object (maintainer: "not to store 'real area' but to calculate it when looking for the widget affected area"), and that it is a `Padding` rather than a scalar because shadows are offset as well as spread. This is also the groundwork the maintainer wants for **absolute positioning, box shadows and tooltips**.

**Independently cheap fixes, not blocked on the mechanism:** scale `CHECKBOX_ICON_POINTS` to `inner` (or clip the mark), and clip the image draw to its layout rect. Both are widget-local and would remove two of the three "possible" rows above.

## Parting notes — operating wisdom (2026-07-08)

Lessons distilled from running this plan across ~30 sessions — written down so they survive any single collaborator.

1. **Measure before you optimize — every time.** This plan's biggest course-corrections all came from measurement contradicting intuition: idle frames were already free (the "optimize static_observers" premise was wrong); `W: WidgetCtx` cost zero flash (one instantiation per firmware — the collapse is an API play); 1.3b's "obviously correct" hardening doubled allocations and was reverted on bench evidence. The metrics tool is the referee; the tripwire pattern (bench from day one + a pre-approved fallback) beats preemptive cleverness.
2. **Adversarial verification is not optional.** The original audit refuted 8 of 94 findings on re-read; the plan-consistency sweep found 13 seams between decided gates and item text; the WS0 review caught a landed feature (micromath) that was dead-on-arrival through the dependency graph. A plausible claim without a repro is a rumor. Review every workstream's output the way WS0 was reviewed.
3. **The plan is shared mutable state — treat it like contended state.** A session's stale-copy save clobbered item 0.9 once (recovered from git history). Rule: re-read the section immediately before the final checkbox-marking write; if something looks lost, `git log -p docs/plans/` has it.
4. **Known-fails must have owners.** `static_wrapper` is red _on purpose_ — it is WS4's acceptance test. A red test with an owner is a specification; a red test without one is rot. Never fix an owned known-fail out of turn; never leave a new red test unowned.
5. **Decisions in gates, execution late, both in writing.** G4 (WidgetCtx shape) is still open — no session may improvise it. Gates are what let core work proceed for weeks without building on doomed shapes. When a decision reverses (G5 did), sweep the plan for its consequences — stale text from a reversed decision is how sessions get misled.
6. **Report, don't silently fix — and record reversals.** 1.3b's revert note, WS2's "observe() removed outright" deviation, the icons scope-cut — each recorded with _why_, which is what makes the next session trustworthy. An execution note costs a paragraph; a silent deviation costs a debugging session.
7. **Deep-first held under pressure — keep holding it.** Every time exterior polish tempted (naming, examples, README), the core moved underneath within days. WS11 is last _by design_; the moment traits stabilize (WS7) is when polish stops churning.
8. **Ops gotchas that will bite again:** never run cargo commands concurrently with a `cargo hack` powerset (transient Cargo.toml rewrite); never workspace-wide `cargo fmt` (per-file `rustfmt` only); parallel sessions need separate git worktrees; rsact-crate tests stay serial; post-0.7a every no-default-features build must name a math backend.
9. **The one-sentence identity to protect: _you pay for what you wire._** Zero-cost Inert, pay-per-binding layout, demoted probes, sanctioned feature axes — all serve that sentence. Evaluate every future proposal against it. The v2-bloat failure mode this project explicitly fears begins the day something costs RAM or flash that nobody wired.
10. **WS17 is marketing as engineering.** The published LVGL comparison with honest methodology is the adoption lever; README numbers come from hardware, never estimates. Performance transparency (the public Pages dashboard) builds the kind of trust rustc's perf tracking built.

## Ideas & horizons — where rsact can grow (2026-07-08, non-commitments)

Strategic growth vectors beyond the plan and backlog. None are scheduled; each deserves a design session when its moment comes. Ordered roughly by leverage-per-effort.

| ID  | Idea                                                                                                                                                                       | Why it's worth it                                                                                                                                                                              |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| H1  | **Own the e-paper niche completely** — ghosting management (full refresh every N partials), waveform selection, region-refresh scheduling on WS6's regions API              | Nobody does e-paper well; change-gated rendering is _natively_ the right architecture. A category rsact can win outright, in a growing battery-product market.                                   |
| H2  | **Power-aware scheduling as a first-class API** — the graph knows the next animation deadline + every wake source; let the framework own the sleep loop (`run_low_power`)   | A measured µA story is purchasing-decision material; no Rust UI framework has one. Pairs with H1. Evolves backlog A14.                                                                           |
| H3  | **Snapshot testing as a user-facing feature** — generalize WS6.9's golden harness into `assert_screen!` for user UIs in CI                                                  | UI regression testing on embedded is unsolved pain; LVGL has nothing comparable. Cheap once 6.9 exists.                                                                                          |
| H4  | **Record/replay + time-travel debugging** — record signal writes with timestamps, replay field sessions in the simulator, step backward via `what_changed`                  | Field-bug reproduction is embedded's most expensive activity; the deterministic runtime + DevTools already provide the foundations.                                                              |
| H5  | **Themes as data** — styler registry (G9) + serde: themes as TOML/JSON token files, compiled in at build; the C4 playground exports them                                     | Designers ship themes without Rust; brand variants without recompiling logic. The registry made it possible — this cashes it in.                                                                 |
| H6  | **Headless rsact / remote surfaces** — widget tree + layout on a host/gateway, WS16's `DrawCommand` IR streamed to dumb display clients over the WS14 protocol               | One codebase drives fleet displays, secondary MCUs, debug mirrors. The IR + protocol were built for this even if neither says so yet.                                                            |
| H7  | **Optional declarative macro layer** — `view! { Col { gap: 4, Label(text) } }` compiling to the builder API                                                                  | Keeps the no-codegen builder as ground truth (the differentiator vs Slint) while offering DSL ergonomics as pure sugar. Only after WS7 + WS13 stabilize what it compiles to.                     |
| H8  | **The executable reactive spec** — grow WS1.5a's oracle/property tests into a conformance corpus documenting exact engine semantics                                          | Makes engine rewrites (WS18's slab and beyond) safe to attempt; the document serious adopters read before trusting a reactivity engine.                                                          |
| H9  | **WASM simulator in the docs** — tiny-skia to canvas; docs-site pages (VitePress, per WS19) embed live, editable examples                                                                             | Zero-install onboarding: try rsact in the browser before installing a toolchain. Highest-leverage docs investment after the guide itself.                                                        |
| H10 | **Agent-legible framework** — machine-readable widget/props/styles catalog (the registry + View traits make it introspectable), llms.txt, prompt-friendly docs               | This framework was _evolved_ by agent sessions — lean in. "The embedded UI framework AI assistants are best at" is a real, unclaimed position.                                                   |
| H11 | **Industrial HMI vocabulary as an ecosystem crate** — gauges, sparklines, seven-segment, tank levels, alarm banners in `rsact-widgets-hmi`, third-party-themed via G9         | Serves the actual industrial market while keeping core widget-minimal; also the first real proof that third-party widget authorship works. After WS12 publishes.                                 |
| H12 | **Multi-surface compositing** — after WS16's IR: one reactive graph driving N surfaces (main TFT + status OLED) with per-surface damage                                      | Real products have two displays more often than admitted; G4's one-renderer-per-binary stays intact (surfaces share one renderer type per target).                                               |

## Cross-cutting invariants (any WS must respect)

- **I1–I7 (render gating, from D5):** identity aliasing impossible by construction; records live/die with their element; nothing consumes a part's dirtiness except executing it; deps re-tracked per execution; parent redraw ⇒ child overdraw, child dirty ⇒ page dirty, O(1) idle gate survives; 0 idle allocs / O(1) change allocs; observer-cell restore stays panic-safe.
- **Layout stop rule (from D3):** upward propagation may stop at a node only if BOTH resolved `outer_size` AND `min_size` are unchanged under unchanged inputs (min_size feeds parent wrap decisions and min-clamps over max in fluid children).
- **Arena↔layout structural parallelism** is load-bearing (positional zip in event+render passes) until WS5.1 makes identity explicit; divergence must degrade (log), never panic (WS3.5).
- **"UI must never panic"** (EVOLUTION.md): every new code path logs and degrades; no new `unwrap` on render/event/nav paths; prefer exposing `try_*`/`Option` to the user where possible (WS1.8).
- **Crate encapsulation (maintainer, 2026-07-06):** rsact-reactive stays UI-vocabulary-free — no `ElId`/part/page knowledge in the core; ownership maps for reactive handles live with their owner (rsact-ui), never in a core registry.
- **No feature-flag sprawl:** the only sanctioned axes are storage backend (std | single-thread | unsafe-single-thread; `fixed-capacity` joins this axis if WS18 runs), math backend (libm | micromath — WS0.1), render backend, font provider, extras (simulator/anim/tiny-icons/debug-info), dev-only `test-utils` (never in a production graph — enabled via dev-dependencies), plus `incremental-layout`/`layout-counters` while maturing. Anything else must be architectural (pay-per-use by construction).
- **`Note:`/`TODO:` comments are never deleted** unless the referenced work is 100% done; EVOLUTION.md checklist protocol applies on every pass.
- **Serial tests for the rsact crates:** `-- --test-threads=1` (shared thread-local runtime); host tests need `--features std`. Exception: metrics-probe is parallel-safe since 0.7d (`TEST_LOCK`).

## Baselines & verification commands (ACTUALIZED 2026-07-07 — WS0 complete through 0.9; CI runs these same checks via `.github/workflows/ci.yml` + the runnable scripts it wraps)

```sh
cargo test -p rsact-reactive --features std -- --test-threads=1   # 76 pass / 0 fail (since WS4.1):
#   (was 75/1 pre-WS4; WS4.1 flipped the last known-fail `maybe::tests::static_wrapper` to
#   passing by inlining `Inert`. The older second known-fail
#   observe_recreates_disposed_child_observer was rewritten per G2 in WS2 as
#   child_observer_reruns_only_on_own_dep_change (passing).) Doctests: 4 pre-existing
#   maybe-module failures, unrelated — do not chase.
cargo test -p rsact-ui --lib --features "std,embedded-graphics" -- --test-threads=1   # 60 / 0
#   (was 53 pre-WS3; +7 WS3 lifecycle/repro tests)
#   (--lib is required: examples lack required-features; a font provider feature is required)
cargo test -p rsact-render --features "std,embedded-graphics,tiny-skia" -- --test-threads=1  # 6 / 0
cargo test -p metrics-probe --features layout-counters              # 3 / 0 (parallel-safe)
cargo bench -p rsact-reactive --features std                        # criterion + allocations harness
cargo run -p metrics-probe -- record            # Layer-1 snapshot (+ --sizes for thumb sections)
# feature powersets are PER-CRATE with two mutual-exclusion groups (backends + libm/micromath);
# a single --all powerset cannot be green by design — exact commands: docs/features.md
# NOTE since 0.7a: --no-default-features drops libm — thumb/minimal commands must name a math
# backend explicitly (e.g. ...,libm). Automation: post-commit hook records snapshots locally;
# ci.yml (tests/powersets/thumbv7m) + metrics.yml (metrics-data branch — LIVE on origin, Pages,
# PR deltas) on GitHub.
# OPS GOTCHAS (from WS2 execution): never run other cargo commands concurrently with a
# cargo-hack powerset (it rewrites Cargo.toml transiently — breaks the test-utils self-dev-dep);
# do NOT run workspace-wide `cargo fmt` — use per-file rustfmt.
```

Measured reference numbers (2026-07-05 audit + WS0's in-repo size-probe; the 0.3 snapshot tool is now the living source — these rows are the frozen baseline):

| Metric                                   | Value                                                                                                                                      |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Reactive core, thumbv6m opt-z fat-LTO    | `.text` 16.8 KiB (audit's minimal probe; total 24.9), statics ~56 B, 0 steady-state allocs                                                 |
| In-repo size-probe (0.3 L2) — re-recorded 2026-07-07 post-WS9a.1 (fixed probe) | `.text`: reactive **16.58 K** thumbv7m / **15.67 K** thumbv6m · ui **75.9 K** thumbv7m / **80.5 K** thumbv6m (`.data` 32 B; `.bss` 1064 B reactive / 1068 B ui = probe heap + cortex-m-rt statics; `.text/.rodata` = flash signal). ⚠ Supersedes the stale **27.4/27.5 K reactive · 79/84 K ui** row: the reactive probe called the WS2-deleted `observe()`, so `--sizes` silently skipped the reactive bin from WS2 until `dfaf2b5`. Reactive drop 27.4 → 16.58 K = WS2 registry/ahash removal never re-measured (~4 K; 23.36 K on master with the fixed probe) **+ WS9a.1 BTreeSet/sort −6.78 K** (23.36 → 16.58). ui −3.1/−3.5 K is 9a.1's shared reactive code. |
| Engine allocs/op                         | create_signal 1/144 B · create_memo 3/319 B · create_effect 3/702 B · write w/1 effect ~~2/112 B~~ → **0/0 (WS9a.1 landed)**; effect_rerun_100_subs 18.7/2931 B → **0/0**    |
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
