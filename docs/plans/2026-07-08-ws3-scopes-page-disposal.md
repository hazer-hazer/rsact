# WS3 — Ownership & lifecycle: scopes, page disposal, one-shot — Implementation Plan

> **For agentic workers:** this is the WS3 execution plan expanded from
> `docs/plans/2026-07-05-rsact-evolution-roadmap.md` (WS3 + gate G11). TDD,
> one commit per landed item.

**Goal:** page-created reactive nodes become page-owned — a per-page scope
disposes everything a page built when the page drops, killing the disposed-arena
delayed panic and the navigation leak; subtree removal disposes widget nodes;
PageState never routes to freed ids; add a `render_once` one-shot; make the
arena↔layout positional zips degrade instead of abort.

**Architecture:** lean on WS1.1 (scope parent-chain) + WS2 (arena-owned probes).
A page owns a `ScopeHandle`; `load_page` builds the whole page (`init_page()` +
`Page::new`) with that scope current, then restores `current_scope` (the scope
outlives the build, so a non-lexical `leave` is needed — a new primitive).
`Page::drop` keeps WS2's explicit probe/arena disposal in its body; the `scope`
field drops after the body and disposes every build-time reactive node.

**Tech stack:** Rust, `no_std`, rsact-reactive runtime (thread-local slotmap graph).

## Global Constraints

- Serial tests: `-- --test-threads=1`; host needs `--features std`.
- Baselines to preserve: rsact-reactive **74 pass / 1 known-fail** (`static_wrapper` = WS4)
  after 3.0a; rsact-ui `--lib --features "std,embedded-graphics"` **53/0**; rsact-render **6/0**.
- "UI must never panic": new paths log + degrade; no new `unwrap` on render/event/nav.
- Encapsulation: rsact-reactive stays UI-vocabulary-free.
- Never delete `Note:`/`TODO:` comments unless 100% done.

---

## 3.0b Audit inventory (design record)

Every reactive-value creation site in rsact-ui, who creates it, and who *should*
own it under the scope model.

| Site | Creates | When (build phase) | Owner today | Owner after WS3 |
| --- | --- | --- | --- | --- |
| `Dynamic::new` (widget/dynamic.rs:31,33) | `Signal<Option<El>>` + **layout `Effect`** | `init_page()` (via `into_el`) | **nobody (leak → delayed panic)** | page scope |
| `Dynamic::build` (widget/dynamic.rs:57) | **build `Effect`** (sets child on arena) | `Page::new` → `BuildCtx::run` | **nobody (leak → delayed panic)** | page scope |
| widget ctors — slider/knob/icon/checkbox/select/show/scrollable | 1–2 signals/memos each | `init_page()` | nobody (leak) | page scope |
| `Page::new` — `force_redraw`, `layout` memo, `style` | 2 signals + 1 memo | `Page::new` | nobody (leak) | page scope |
| `Page::new` — `render_probe` (`untrack(create_probe)`) | Probe | `Page::new` | Page::drop (explicit, WS2) | Page::drop (explicit; scope skips via `is_alive`) |
| `load_page` — arena signal | `Signal<ElArena>` | `load_page` (before scope) | Page::drop (explicit, WS2) | Page::drop (explicit) — created **outside** scope |
| element `part_probes` | Probe per part | **render** (post-build) | remove_subtree / dispose_all_probes (WS2) | unchanged (WS2) |
| `UiQueue::new` (event/message.rs:40) | 2 signals + 1 memo | UI construction (before pages) | nobody | **UI-lifetime (correct to keep)** — created before any page scope |
| `Anim::handle` (anim/mod.rs:248,255) | signal + memo per anim | `init_page()` (user builds anims) | nobody (leak) | page scope |

**Discrepancies reported (roadmap protocol — not silently fixed):**
- 3.0b cites `DrawQueue::new` (canvas.rs:74-79) as an unowned site. **WS1b.1 deleted `DrawQueue`** — Canvas is now a single `Box<dyn Fn>` closure with no reactive nodes. Stale bullet.
- 3.0b's node-count baseline predates WS9a's added tests; the live reactive baseline is **74/1** post-3.0a.

**Key design consequence:** the scope must live in `load_page`, wrapping
`page_fn.init_page()` **and** `Page::new` — the two `Dynamic` effects are created
in those two phases respectively, and both must die with the page or the build
effect re-runs against a disposed arena (the delayed panic).

---

## Task 1: `ScopeHandle::leave` — non-lexical scope exit (rsact-reactive)

**Files:**
- Modify: `rsact-reactive/src/runtime.rs` (add `Runtime::exit_scope`)
- Modify: `rsact-reactive/src/scope.rs` (add `ScopeHandle::leave` + `id`; test)

**Interfaces:**
- Produces: `ScopeHandle::leave(&self)` — restores `current_scope` to this scope's
  parent iff this scope is current, WITHOUT disposing the scope. `Runtime::exit_scope(ScopeId)`.

- [ ] **Step 1 (RED):** in `scope.rs` tests, `leave_restores_current_to_parent`:
  create outer scope, create a page scope, `leave()` it, then a new value must be
  owned by outer (disposed when outer drops), not by the left page scope.
- [ ] **Step 2:** run → fails to compile (`leave` missing) → add stub returning `()`, re-run → assertion fails.
- [ ] **Step 3 (GREEN):** implement `Runtime::exit_scope` (mirror `drop_scope`'s
  `current_scope` restore but without removing the scope) + `ScopeHandle::leave`.
- [ ] **Step 4:** run scope tests → pass; full reactive suite 74/1.
- [ ] **Step 5:** commit.

## Task 2: Per-page scope in `load_page` + repro test (rsact-ui) — item 3.1

**Files:**
- Modify: `rsact-ui/src/page/mod.rs` (add `scope: Option<ScopeHandle>` to `Page`; drop order)
- Modify: `rsact-ui/src/ui.rs` (`load_page` wraps build in a scope)
- Test: `rsact-ui/src/page/mod.rs` tests

**Interfaces:**
- Consumes: `new_scope`, `ScopeHandle::leave` (Task 1), `rsact_reactive::leak::*`.
- Produces: `Page` owns a page scope; dropping a page disposes its build-time nodes.

- [ ] **Step 1 (RED):** `disposed_page_effect_does_not_panic_on_app_signal` — app
  signal outside any page; a scoped page whose Dynamic root reads it; render;
  drop page; `app_signal.set(1)` must not panic and leak_report must be empty.
- [ ] **Step 2:** run → panics / leak survivors (build effect survived).
- [ ] **Step 3 (GREEN):** `load_page` = create arena (outside scope) → `let scope
  = new_scope()` → build `Page::new(page_fn.init_page(), ...)` → `scope.leave()` →
  store `Some(scope)` in the page. `Page.scope` field declared last so it drops
  after the `Drop` body (which keeps WS2's explicit probe/arena disposal).
- [ ] **Step 4:** run → pass; WS2 probe tests still pass; ui 53/0 + new tests.
- [ ] **Step 5:** commit.

- [ ] **Step 6 (RED):** `page_drop_disposes_build_time_nodes` — scoped page with
  widgets; snapshot before build; drop; `leak_report` empty.
- [ ] **Step 7–9:** already green from Step 3 (same mechanism) — verify, commit if separate.

## Task 3: Subtree disposal — item 3.2 (rsact-ui)

**Files:**
- Modify: `rsact-ui/src/el/arena.rs` (`remove_subtree` disposes widget nodes)
- Modify: widget build to register a per-subtree scope OR a dispose hook (design below)
- Test: `rsact-ui/src/page/mod.rs` or `el/arena.rs` tests

**Design:** `Dynamic`'s rebuild replaces its child subtree via `set_single_child`
→ `remove_subtree`. The old child's build-time reactive nodes (its own Dynamic
effects, widget signals) leak per rebuild. Give each built subtree a scope stored
in `ElState`/`ElData`, disposed in `remove_subtree` alongside `dispose_probes`.

- [ ] **Step 1 (RED):** `dynamic_rebuild_does_not_leak_reactive_nodes` — Dynamic
  child that creates a signal each build; 20 rebuilds; node count flat.
- [ ] **Step 2:** run → node count grows.
- [ ] **Step 3 (GREEN):** implement subtree scope + dispose in `remove_subtree`.
- [ ] **Step 4:** run → flat; `arena_rebuild_does_not_leak_subtree` still green.
- [ ] **Step 5:** commit.

## Task 4: PageState pruning — item 3.3 (rsact-ui)

**Files:**
- Modify: `rsact-ui/src/el/ctx.rs` (:97-99 TODO) + `remove_subtree` caller
- Test: page/ctx tests

- [ ] **Step 1 (RED):** `pagestate_forgets_removed_focused_id` — focus an id,
  remove its subtree, assert `focused`/`hovered`/`captured_by` no longer reference it.
- [ ] **Step 2–5:** invalidate matching ids on removal; run; commit.

## Task 5: `render_once` one-shot — item 3.4 (rsact-ui / root facade)

**Files:**
- Modify: `rsact-ui/src/ui.rs` or a new `rsact-ui/src/render_once.rs`
- Test: doc-test / example (e-paper sketch)

- [ ] **Step 1 (RED):** test builds → layout → renders → drops; heap ~baseline.
- [ ] **Step 2–5:** implement `render_once`; run; commit.

## Task 6: Checked zips — item 3.5 (rsact-ui)

**Files:**
- Modify: `rsact-ui/src/el/event.rs:89,126`, `rsact-ui/src/el/render.rs:499`
- Test: divergence degrades (logs), no panic.

- [ ] **Step 1 (RED):** feed mismatched arena/layout lengths; assert no panic + error logged.
- [ ] **Step 2–5:** replace `zip_eq` with checked zip + `error!`; run; commit.

---

## Self-review notes
- Spec coverage: 3.0a ✓ (committed), 3.0b ✓ (this inventory + tests in Tasks 2–3),
  3.1 = Task 2, 3.2 = Task 3, 3.3 = Task 4, 3.4 = Task 5, 3.5 = Task 6.
- Task 5/6 designs are refined against real code when reached (marked).
