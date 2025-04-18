<div align="center">
<img width=300 src="./rsact.png"></img>

__Reactive Rust GUI framework being built for embedded systems usage in mind__

<hr>

</div>

> rsact is at a such early stage where everything is clumsy and messy, there's a lot of work to do, refactor, re-imagine and document. Though I hope the core idea works and will grow into "something".

## Intro

`rsact` is a GUI framework targeting embedded systems. It is based on fine-grained reactivity system, hence the name.

The project consist of these parts:

- [`rsact_reactive`](./rsact-reactive/README.md) fine-grained reactivity framework.
- [`rsact_ui`](./rsact-ui/README.md) the core of UI framework.
- [`rsact_icons`](./rsact-icons/README.md) tuned pre-rendered icons targeting tiny sizes.
- [`rsact_macros`](./rsact-macros/README.md) proc macros used both for `rsact_ui` and `rsact_reactive`.
- [`rsact_encoder`](./rsact-encoder/README.md) (planned) widgets specific for platforms with encoder+button control.
- [`rsact_widgets`](./rsact-widgets/README.md) (planned) high-level widget kinds such as drop-down list, menus, etc.


### Setup

To use rsact you need a heap, [`embedded-alloc`](https://crates.io/crates/embedded-alloc) can be used in `no_std` environments. Heap usage depends on your screen size, `rsact-ui` needs at least a single buffer to render to, though [`embedded-graphics`](https://docs.rs/embedded-graphics/latest/embedded_graphics/) `BinaryColor` is optimized and packed by groups of 8 into single byte. It will possibly change in future if I add feature to pre-allocate buffer by users wherever they want. Heap usage also depends on amount of data stored in reactive system, so be carryful with what you put into `Signal`s and `Memo`s.
At the moment I have tested rsact only on `STM32F412RET6` running at 100MHz with SPI displays ST7789 240x135 and ST7735 160x80, got around 30FPS and 85FPS respectively.
