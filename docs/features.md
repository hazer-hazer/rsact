# rsact feature matrix (WS0.6)

The only sanctioned feature axes (see the roadmap's cross-cutting invariants) are:
storage backend, render backend, font provider, math backend, and a small set of
extras. Anything else must be architectural (pay-per-use by construction), not a
flag.

Features **propagate top-down** through the crate tree (root `rsact` → `rsact-ui`
→ `rsact-render` → `rsact-reactive`); each crate's default is minimal and nothing
extra is enabled by default.

## Mutually-exclusive axes (exactly one)

| Axis                | Crate           | Options                                             | Default   |
| ------------------- | --------------- | --------------------------------------------------- | --------- |
| Reactive storage    | `rsact-reactive`| `std` · `single-thread` · `unsafe-single-thread`    | none¹     |
| no_std f32 math     | `rsact-render`  | `libm` · `micromath`                                | `libm`    |

¹ No default backend: a downstream crate (or the host test invocation) must pick
one. `std` is host-only; `single-thread` needs a `critical-section` impl (not
linkable in host test builds — use `check`, not `build`); `unsafe-single-thread`
is the no-impl, single-execution-context backend.

Enabling two options on either axis is a `compile_error!` (same pattern in
`rsact-reactive/src/thread_local.rs` and `rsact-render/src/lib.rs`).

## Per-crate features

### rsact-reactive
- `default = ["default-runtime"]`
- Backends (above): `std`, `single-thread` (→ `dep:critical-section`),
  `unsafe-single-thread`
- Extras: `debug-info`, `async`

### rsact-render
- `default = ["libm"]`
- Math (above): `libm` (→ `dep:num-traits`, `num-traits/libm`), `micromath`
- `std` (→ `rsact-reactive/std`, `log/std`)
- Render backends: `embedded-graphics` (→ `dep:embedded-graphics`,
  `dep:embedded-text`), `tiny-skia`
- `defmt` (→ `dep:defmt`, `embedded-graphics?/defmt`)

### rsact-ui
- `default = ["libm"]`
- Math (passthrough to rsact-render, mutually exclusive): `libm` (default),
  `micromath`. rsact-ui pulls rsact-render with `default-features = false` so
  the backend is chosen here, not forced — `--no-default-features` drops `libm`,
  so re-add `libm` or `micromath` explicitly.
- Backends forwarded: `std`, `single-thread`, `unsafe-single-thread`
- Render/font: `embedded-graphics`, `u8g2-fonts` (→ `embedded-graphics`),
  `tiny-skia`, `simulator` (→ `embedded-graphics`)
- Extras: `debug-info`, `defmt` (→ `embedded-graphics?/defmt`,
  `embedded-graphics-core?/defmt`), `tiny-icons`, `layout-counters`,
  `layout-diagnostics`
- A **font provider** (`embedded-graphics` or `u8g2-fonts`) is required or the
  build hits a `compile_error!` in `font/mod.rs`.

### rsact (root facade)
- `default = ["libm"]`; forwards `std`, `single-thread`, `unsafe-single-thread`,
  `simulator`, `defmt`, `libm`, `micromath` to `rsact-ui` (pulls rsact-ui with
  `default-features = false`). NOTE: it forwards no render-backend/font-provider
  features yet, so a standalone `-p rsact` build can't produce a working app —
  deferred (WS12.5).

### metrics-probe (host tool)
- `layout-counters` (→ `rsact-ui/layout-counters`) — include WS0.5 layout
  counters in the snapshot.

## Common invocations

```sh
# Host tests (a font provider is required for rsact-ui):
cargo test -p rsact-reactive --features std -- --test-threads=1
cargo test -p rsact-ui --lib --features "std,embedded-graphics" -- --test-threads=1
cargo test -p rsact-render --features "std,embedded-graphics,tiny-skia" -- --test-threads=1

# Embedded floor (Blue Pill, thumbv7m). `--no-default-features` drops the `libm`
# default, so name a math backend (libm | micromath) explicitly:
cargo build -p rsact-ui --no-default-features \
  --features "unsafe-single-thread,embedded-graphics,libm" --target thumbv7m-none-eabi
```

## Feature-powerset check

A single `--all` powerset cannot be green for this workspace, for reasons that
are design constraints, not leaks:

- `rsact-reactive` **requires** a storage backend, chosen downstream — so leaf
  crates that transitively depend on it but don't expose the choice
  (`rsact-tiny-icons`, `metrics-probe`) can't be checked standalone.
- `rsact-ui` **requires** a font provider (`compile_error!` in `font/mod.rs`).
- `rsact-ui`'s `tiny-icons` Icon widget is pre-existing WIP that does not
  compile (`icon.rs` has a `TODO(unimplemented)`); excluded until fixed.

So powerset the three axis-owning crates individually (each is green):

```sh
# reactive core — storage backend axis
cargo hack check --feature-powerset --no-dev-deps -p rsact-reactive \
  --mutually-exclusive-features std,single-thread,unsafe-single-thread \
  --at-least-one-of std,single-thread,unsafe-single-thread

# render — math axis (+ std forced, since it needs reactive's backend to check)
cargo hack check --feature-powerset --no-dev-deps -p rsact-render \
  --mutually-exclusive-features std,single-thread,unsafe-single-thread \
  --at-least-one-of std,single-thread,unsafe-single-thread \
  --mutually-exclusive-features libm,micromath \
  --at-least-one-of libm,micromath

# ui — backend + math backend + a required font provider; skip WIP tiny-icons
cargo hack check --feature-powerset --no-dev-deps -p rsact-ui \
  --exclude-features tiny-icons \
  --mutually-exclusive-features std,single-thread,unsafe-single-thread \
  --at-least-one-of std,single-thread,unsafe-single-thread \
  --mutually-exclusive-features libm,micromath \
  --at-least-one-of libm,micromath \
  --at-least-one-of embedded-graphics,u8g2-fonts
```

The earlier single `--all` command in CLAUDE.md predated `unsafe-single-thread`,
the `libm`/`micromath` axis, and the font-provider guard, so it could not have
been green.

## Removed / fixed leaks (WS0.6)

- `rsact-ui` `std` force-pulled optional `tiny-skia` via `tiny-skia/png-format`
  (missing `?`) → `tiny-skia?/png-format`.
- Workspace `embedded-graphics`/`embedded-graphics-core` forced `defmt` on every
  build (and pulled a second, 1.x, defmt) → removed; `defmt` is now opt-in and
  forwarded through each crate's `defmt` feature.
- Unused workspace `smallvec` dependency removed (the tree uses `tinyvec`).
