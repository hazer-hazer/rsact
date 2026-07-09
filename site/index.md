---
layout: home
hero:
  name: rsact
  text: Reactive Rust GUI for embedded
  tagline: You pay for what you wire. Fine-grained reactivity, no_std by construction.
  actions:
    - theme: brand
      text: Get started
      link: /docs/
    - theme: alt
      text: Live metrics
      link: /metrics/
    - theme: alt
      text: GitHub
      link: https://github.com/hazer-hazer/rsact
features:
  - title: Fine-grained reactivity
    details: Signals, memos, effects and probes. Change detection at the consumer and glitch-free, topologically-ordered effect flushing — only what actually changed recomputes.
  - title: no_std by default
    details: Builds for thumbv7m (Cortex-M3, e.g. an STM32 "Blue Pill") and up. std is an opt-in feature; a heap allocator is the only hard requirement.
  - title: You pay for what you wire
    details: Pay-per-use by construction. Reactive-graph cost scales with the UI you actually build — and it's measured every commit, not promised.
  - title: Multiple render backends
    details: One UI, many targets — embedded-graphics for MCUs and tiny-skia for desktop/anti-aliased output — through the rsact-render abstraction.
  - title: Transparent performance
    details: Node counts, heap bytes, allocation counts and flash sizes are recorded in CI for every commit and charted live on the metrics page.
  - title: Honest comparisons
    details: The LVGL / Slint comparison and on-device numbers land with hardware validation (WS17). Until then they are marked placeholders — never estimates.
---

## A taste

A signal drives a widget directly — no manual wiring, no diffing:

```rust
use rsact_ui::prelude::*;

// A signal is a Copy handle into the reactive runtime.
let selected = create_signal(0);

// One UI; pick a renderer (embedded-graphics here). The page is a closure, so
// navigating (re)builds its tree — you capture Copy signal handles, not widgets.
let mut ui = UI::new(Theme::default(), EGRenderer::new(viewport))
    .with_page(SinglePage, move || {
        // Widgets take reactive values directly.
        let select = Select::vertical(selected, vec![0, 1, 2, 3].inert());
        Flex::col([select.el()]).center().fill()
    });
```

## Numbers you can check

rsact publishes its own footprint. Every commit records reactive node counts,
heap usage, per-frame allocations and (on release builds) Cortex-M flash/RAM
section sizes into a durable store, charted on the [metrics page](/metrics/).

The headline embedded comparison against LVGL and Slint — and measured
on-device frame rates — arrive with hardware validation (roadmap **WS17**) and
are intentionally **not** quoted here as estimates.
