# Getting started

rsact is an early-stage, reactive Rust GUI framework for embedded systems. It is
`no_std` by default and built around a fine-grained reactivity system
(signals / memos / effects).

## Add the dependency

```toml
[dependencies]
rsact-ui = { git = "https://github.com/hazer-hazer/rsact" }
```

## Pick your backends

rsact has no default reactive-storage or render backend baked in — you choose
per target. On an MCU:

```sh
cargo build -p rsact-ui --no-default-features \
  --features "unsafe-single-thread,embedded-graphics,libm" \
  --target thumbv7m-none-eabi
```

- **Reactive storage** (exactly one): `std` (host), `single-thread` (needs a
  `critical-section` impl), or `unsafe-single-thread` (single execution context).
- **Render backend:** `embedded-graphics` (MCUs) and/or `tiny-skia` (desktop / AA).
- **Math:** `libm` (default) or `micromath`.

`--no-default-features` drops the default `libm`, so re-add `libm` or
`micromath` explicitly. See the [feature matrix](/docs/features).

## A minimal UI

```rust
use rsact_ui::prelude::*;

let selected = create_signal(0);
let select = Select::vertical(selected, vec![0, 1, 2, 3].inert());
let page = row![col![select]].center().fill();

let mut ui = UI::new(Theme::default(), EGRenderer::new(viewport))
    .with_page(SinglePage, page.el());

// In your app loop:
// ui.tick(events);
// ui.render(&mut display);
```

You need a heap — [`embedded-alloc`](https://crates.io/crates/embedded-alloc)
works in `no_std`. Heap usage scales with screen size and with the data you put
into `Signal`s and `Memo`s.

## Next

- [Feature matrix](/docs/features) — the sanctioned feature axes.
- [Architecture](/docs/architecture) — how the pieces fit.
- [Metrics](/metrics/) — the framework's measured footprint, per commit.
