# WS13 — Analysis: "Views as widget builders" (builder/retained split), and whether it must precede WS5

**Date:** 2026-07-09 · **Author:** WS5-prep session (analysis for a maintainer sequencing decision) · **Feeds:** WS5.1 (Layout off-graph), WS13 (B2 — the EVOLUTION RFC) · **Status:** scoping deliverable — go/no-go + recommended sequencing, not yet a design spec.

This is a WS4.0-style written scoping pass. It answers: **(1)** what the "Views as widget builders" split actually is against the current code; **(2)** whether it is a genuine *prerequisite* for a clean WS5.1 (off-graph, ElId-identified, node-free, `Rc`-free layouts) or merely desirable; **(3)** the full blast radius; **(4)** the candidate split shapes with a recommendation; **(5)** go/no-go + how to sequence it relative to WS5.

All claims are `file:line`-cited from two adversarial code sweeps over `rsact-ui` + `rsact-macros`.

---

## 0. The RFC, and what the code looks like today

The RFC (`EVOLUTION.md:64-68`, "Views as widget builders" → roadmap **B2 → WS13**, currently scheduled *after* WS12): *"avoid storing widget's properties that [are] needed only for building while unused in lifecycle passes … splitting each widget into a Builder and actual Widget."* The section trails off mid-sentence — it was never fleshed out.

Current shape:

- A widget is **one struct** that is simultaneously (i) the **builder** — it carries the builder methods (`.width`/`.padding`/`.gap`/`.font_*`) via the trait family `LayoutWidget`/`SizedWidget`/`BlockModelWidget`/`FontSettingWidget` (`widget/mod.rs:74-246`), each routing through `layout_mut()`; and (ii) the **retained runtime state** — it is boxed `Box<dyn Widget<W>>` (`el/mod.rs:70`) and lives in the arena for the element's whole lifetime, driving `layout`/`on_event`/`render`/`update`.
- `View<W>` (`el/view.rs:23`) is today a **thin erasure conversion** only: `into_el(self) -> El<W>` = `Widget::el(self)`. `#[derive(View)]` (`rsact-macros/src/lib.rs:45-74`) injects `'static` on type params and emits just the `View` + `SingleViewMarker` impls; it inspects no fields and emits no `Copy`/`Clone`. It presumes a hand-written `Widget` impl. **`View` is not yet a builder abstraction — it is a one-line forwarder.**

---

## 1. The pivotal fact: today there is *no* build-only prop inflation — the node absorbs it

The census (all `rsact-ui/src/widget/*.rs`) shows the retained widget is already lean, for two reasons:

1. **Layout props are never stored on the widget.** `.width(sig)` immediately does `layout_mut().setter(...)` (`widget/mod.rs:119`), which for a reactive source calls `Layout::now_reactive()` (`layout/node.rs:125`) and creates the binding **effect right there at builder time**, targeting the `Layout` **runtime node**. Static props are baked straight into the node's `LayoutData`. So `width`/`gap`/`padding`/`font_*` leave **zero residue** on the widget struct — the node is their home.
2. **Child widgets are already not retained.** `arena.add` does `core::mem::replace(el, El::Stored { id, layout })` (`el/arena.rs:247-260`): the real child `Box<dyn Widget>` is *moved into the arena*, leaving a `{id, layout}` **husk** in the parent's `content`/`children` field.

So the only build-only data actually retained on widgets today is:

| Build-only residue (today) | Widgets | Cost |
|---|---|---|
| `El<W>` / `Vec<El<W>>` child **husks** (`{id, layout}` after `mem::replace`) | `Button.content`, `Container.content`, `Scrollable.content`, `Flex.children` (`MaybeSignal<Vec<El>>`), `Dynamic.current`, `Show.el` | small per child; **redundant** with the arena's own `children: SecondaryMap<ElId, Vec<ElId>>` |
| `PhantomData` markers (`dir`/`ctx`/`is_reactive`) | Flex, Slider, Select, Bar, Scrollable, Space, Icon | ZST — zero bytes (but the *type param* must remain, it's used by lifecycle code) |
| constructor-consumed reactive config | `Show.show` (`Memo<bool>`, dead after `new`) | one handle |

**Conclusion:** the direct RAM win of the split *alone* is **modest** — the retained widgets are already `{layout, style, value/state, closures}`; the strippable residue is redundant child husks + a couple of handles + (zero-cost) PhantomData.

**The real point (and where your reasoning lands):** the inflation is not a problem the current design *has* — it is one the **pure-Arena WS5.1 would create**. Removing the `Layout` node (the fake-inert) removes the props' home, so bindings must be **deferred to build** (when `ElId` exists), which forces every layout prop onto the widget struct to bridge builder→build. The split is the mechanism that gives those props a **transient home (the builder)** so they never land in the retained widget. See §2.

---

## 2. Is the split a genuine prerequisite for a clean WS5.1? — **For the *node-free/`Rc`-free* form: YES.**

The constraint is hard: **any binding effect created at builder time that mutates shared `LayoutData` needs a residence that (a) exists at builder time and (b) survives the widget's move into the arena while remaining writable through a persisted reference.** The only things that qualify are *shared ownership*: an `Rc`, or a runtime node (which is `Rc<RefCell<dyn Any>>` internally, i.e. the current `Layout::Static`/`Reactive`). The widget itself cannot be the residence — an effect cannot hold `&mut self` across the move into the arena, and the `ElId` that would let it reach the arena slot does not exist until `arena.add` (`el/arena.rs:249`).

So the chain is airtight:

```
node-free & Rc-free layout  ⟹  no builder-time residence
                            ⟹  defer binding creation to build (ElId known — build(&mut self, ctx.id))
                            ⟹  hold props from construction to build
                            ⟹  (to hold them without inflating the RETAINED widget) a construction-only home = the builder/widget split
```

Two facts confirm the "wire at build" half is already viable, not speculative:

- `build(&mut self, ctx: BuildCtx<W>)` receives the ElId (`ctx.id`), minted in `arena.add`'s `insert_with_key` (`el/arena.rs:248`).
- **`Dynamic` already wires an effect at build capturing `ctx`** to rebuild children via `set_single_child` (`widget/dynamic.rs:55-64`). Routing layout-*prop* bindings through `build()` the same way is the identical move, extended to props.

**What is NOT prerequisite:** the incremental-layout *performance* goal (dirty-set, skip-clean-subtrees, O(N·D)→O(N)) does not need the split — it works on *any* residence (node, `Rc`, or arena slot). Only the *node-free/`Rc`-free storage* goal needs it. This is the crux of the sequencing decision (§5).

### The example you asked for (a maximally-propertied widget)

`Slider::new(value).width(w_sig).height(40).padding(pad_sig).gap(4).range(r_sig).step(2).on_change(cb)`

**Today** — `.width(w_sig)` resolves *now*, at builder time:
```rust
self.layout_mut().setter(w_sig.maybe_reactive(), |l, w| l.size.set_width(w.clone().into()));
// reactive arm: Layout::now_reactive() upgrades Static(ValueId)→Reactive(Signal); binding EFFECT created immediately.
```
`Slider` retains only `{ layout, value, range, step, state, style, dir }` — no build-only prop residue (the node holds the width binding).

**Pure-Arena WS5.1, no split** — no `ElId`/slot at builder time, so `.width(w_sig)` cannot bind; it must be recorded and wired at build:
```rust
// builder time: record, don't bind
self.pending.push(move |id, arena| create_effect(move || {
    arena.el_mut(id).layout.size.set_width(w_sig.get().into());
    page_dirty.mark(id);
}));
// build(ctx): drain `pending`, wire each with ctx.id
```
`Slider` now carries `pending` (build-only) into the arena → the inflation. **The split moves `pending` (and the props) onto a transient `SliderBuilder`, consumed at build, so the retained `Slider` is back to `{ layout, value, range, step, state, style, dir }`.**

---

## 3. Blast radius

| Surface | Extent | Notes |
|---|---|---|
| Widget structs to split | ~15 live (`label, flex, slider, knob, checkbox, select, button, icon, canvas, edge, space, show, dynamic, bar, container, scrollable`) + `Unit` | `image` is disabled (commented out `widget/mod.rs:12-13`, pre-`build` API); `icon` currently does not compile — **known** WS4.5 breakage (`tiny-icons` + needs `SignalOnWrite`), not introduced here. No `mono_text` widget exists. |
| Builder traits to re-home | `LayoutWidget` + `SizedWidget`/`BlockModelWidget`/`FontSettingWidget` (`widget/mod.rs:74-246`); inherent builders `Flex::gap` (`flex.rs:66`), `Container` aligns (`container.rs:41,55,69`) | Only `.width`/`.height` (+ `border_width`/`padding`/`font_*`) actually touch `layout_mut`; the rest delegate. `FontSettingWidget::font_props` (`mod.rs:199`) is the one builder that *reads* `layout()`. |
| Builder call sites (API churn) | ~50 in `src/`, ~130 in `examples/` | Size family dominates (`fill`×40, `gap`×33, `size`×16, `padding`×19 in examples). Font builders ~unused in examples. Churn is mechanical and mostly in examples. |
| `#[derive(View)]` | 1 macro, ~15 derives | Trivial to re-point (structural forwarder). A `#[derive(Builder)]`/extended `derive(View)` could generate the builder→widget plumbing to kill boilerplate. |
| `Widget::build` protocol | `build(&mut self, ctx)` mutates in place (`el/build.rs`) | A true split changes this to *transform* the type (builder→widget) — see §4. Biggest structural change. |
| `layout()` by-Copy reliance | 6 parent-collection sites (`flex.rs:46`, `container.rs:25`, `scrollable.rs:108`, `button.rs:29`, `select.rs:178`, `page/mod.rs:131`) + `arena.rs:249` husk-move + `dynamic.rs:39` + `model.rs:327,367` `*content` | This is the `Widget::layout → &Layout` / Copy-removal radius WS4.0 §105 pre-sized. The split + WS5.1 collapse it. |
| Regression guardrails | `page/mod.rs:1811-1863` (`reactive_*_setter_persists_and_reacts`) | Already pin the "upgrade must land on the owned layout, not a discarded copy" failure — the split must keep these green. |
| Three-way child encoding | arena `children` map + widget `children` husk Vec + `FlexLayout.children: MaybeReactive<Vec<Layout>>` | The split + WS5.1 collapse (2) and (3) into the arena — the real structural win (kills the WS3.5 positional-zip too). |

### Design-around cases (from the census)
- **`Show`** owns no `layout` field; `layout()` returns `self.el.layout()` (`show.rs:45`) — the child husk's handle. `Show.show` is dead after `new`. The split must handle a widget whose "layout" is a delegate.
- **Effects that must outlive build** — `Flex.children`/`Dynamic.current` are read only in `build` to wire a `set_children` effect (`flex.rs:184`, `dynamic.rs:56`); the effect re-fires from the runtime subscription, not the field. The builder can drop the field post-wire, but the effect (owned by the page scope) survives. Reactive *structure* stays on this build-time-effect path; only reactive *props* are newly routed through it.

---

## 4. Candidate split shapes

**(a) Explicit Builder type per widget (`LabelBuilder → Label`).** Builder holds all construction props + the builder-trait methods; `build`/`into_el` consumes it, wires prop effects with `ctx.id`, computes the initial owned `LayoutData`, returns the lean retained `Widget`. *Pros:* zero build-only residue on the retained type; type-safe; the clean end-state. *Cons:* ~2× type count, heavy boilerplate (mitigable by a derive macro), builder traits move to the Builder types, the ~180 call sites re-target the builder.

**(b) Generic deferred-binding queue on the widget (`pending: Vec<Box<dyn FnOnce(ElId)>>`).** Setters push closures at builder time; `build` drains them. *Pros:* minimal type churn, builder traits stay put, incremental. *Cons:* does **not** achieve the goal — the drained `Vec` header (+ a heap alloc) stays on the retained widget; construction concerns still live on the retained type. A bridge, not the split.

**(c) `build` transforms the type (Builder trait → Widget trait).** `View`/a `Builder` trait owns construction; `fn build(self, ctx) -> impl Widget` returns a *different, leaner* retained type, and the arena stores the product. This is the principled form of (a): the arena holds a builder pre-build and a widget post-build. *Pros:* the correct abstraction; `View` finally becomes the builder layer it was named for. *Cons:* the deepest protocol change (arena/`El::New`→build→retained, and `El::Stored` husk semantics).

**Recommendation:** **(a)/(c) are the same destination** — a transient builder consumed into a lean retained widget — and only they actually deliver "properties needed only for building are not retained." **(b) is a stepping stone at best** and leaves the smell in place. Recommend designing toward (a) realized via (c)'s transform, with a `derive` to absorb the boilerplate, in a dedicated spec.

---

## 5. Go / no-go + sequencing

**The split is real, adopted (B2/WS13), and is a genuine prerequisite for the *node-free, `Rc`-free* WS5.1 the maintainer wants** (§2). It is also a **large, cross-cutting refactor** (§3) whose *direct* RAM payoff is modest (§1) — its value is as the **enabler** for clean off-graph layouts, plus collapsing the three-way child encoding and retiring the fake-inert + `layout_mut` trap + the WS3.5 zip.

So this is a maintainer sequencing decision, and the honest framing is a genuine trade — not a slam-dunk:

- **Split-first (pull WS13 before WS5).** WS5.1 then lands directly on clean ElId-identified, node-free storage; identity/walk built **once**; the `Rc` you dislike is never introduced. Cost: a 2–3-session refactor with no immediate perf payoff, and WS5's perf win waits behind it.
- **Residence-now, split-later.** Do WS5.1 on a residence (either the existing `Signal` for reactive layouts — legitimately reactive, *not* fake-inert — or `Rc<RefCell>`), ship incremental layout sooner, then do WS13 at its scheduled slot. Cost: WS5.1's dirty-set identity + relayout walk get **reworked** when the split lands (the throwaway problem, one level up), and if `Rc` is used it's the shape you dislike.
- **Middle (bounded residence change, no split).** Change `Layout::Static(ValueId)` → owned `Static(LayoutData)`, keep `Reactive(Signal)` for genuinely-reactive layouts. Kills the fake-inert for *statics* (your primary objection) without the split and without `Rc`. But it still pays the full `Copy`-removal / `&Layout` blast radius (WS4.0 §105), keeps the two-tree structure and ValueId-ish identity, so it does **not** save meaningful total work vs. doing WS5.1 properly — a partial win.

**Recommendation:** because the split is already adopted and you dislike `Rc`, **split-first is the better long-run trade** — it avoids ever touching `Rc` and avoids reworking WS5.1's identity twice. Concretely: (1) resequence WS13 to before WS5 in the roadmap (maintainer-approved deviation, logged); (2) write a WS13 design spec targeting shape (a)/(c) with a boilerplate-killing derive; (3) then WS5.1 inherits clean node-free layouts.

If shipping WS5's perf sooner outweighs the double-work, residence-now (with `Signal`, not `Rc`, for reactive layouts) is the fallback — but it should be a conscious choice to defer the clean storage, not a default.

### Scope clarifications to flag (protocol: report, don't silently fix)
1. **`icon.rs` does not currently compile** (constructor/`IconValue` arity mismatch, old by-ref `render`) — the **known** WS4.5 icon-repair debt, not new. The split must not be blamed for it; the icon repair should land first or alongside.
2. **`image.rs` is disabled** (commented out of `mod.rs`, pre-`build` trait API). Either revive it under the new split or leave it excluded — decide explicitly.
3. WS13 is currently scheduled **after WS12**; pulling it before WS5 is a real reordering with downstream ripples (WS6 depends on WS5's changed-set). Record the rationale in the roadmap.
