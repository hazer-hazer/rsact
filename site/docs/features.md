# Feature matrix

The only sanctioned feature axes are: reactive storage backend, render backend,
font provider, math backend, and a small set of extras. Anything else must be
architectural (pay-per-use by construction), not a flag. Features propagate
top-down: `rsact` → `rsact-ui` → `rsact-render` → `rsact-reactive`.

## Mutually-exclusive axes (choose exactly one)

| Axis                | Crate            | Options                                          | Default |
| ------------------- | ---------------- | ------------------------------------------------ | ------- |
| Reactive storage    | `rsact-reactive` | `std` · `single-thread` · `unsafe-single-thread` | none¹   |
| no_std f32 math     | `rsact-render`   | `libm` · `micromath`                             | `libm`  |

¹ No default backend: a downstream crate (or the host test invocation) must
pick one. `std` is host-only; `single-thread` needs a `critical-section` impl;
`unsafe-single-thread` is the no-impl, single-execution-context backend.
Enabling two options on either axis is a `compile_error!`.

## Render / extras (opt-in)

- **Render backends:** `embedded-graphics`, `tiny-skia`.
- **Fonts:** `u8g2-fonts` (with `embedded-graphics`).
- **Reactive extras:** `debug-info`, `async`.

The canonical, always-current matrix (per-crate feature lists, propagation
rules, powerset CI commands) lives in the repo:
[`docs/features.md`](https://github.com/hazer-hazer/rsact/blob/master/docs/features.md).
