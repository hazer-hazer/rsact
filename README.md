<div align="center">

    # rsact

    Rust UI framework made for embedded systems usage in mind.

</div>

> rsact is at a such early stage where everything is clumsy and messy, there's a lot of work to do, refactor, re-imagine and document. Though I hope the core idea works and will grow into "something".

`rsact` is a GUI framework targeting embedded systems in Rust. The framework is based on fine-grained reactivity system, hence the name.

The project consist of these parts:

- [`rsact_reactive`](./rsact-reactive/README.md) fine-grained reactivity framework.
- [`rsact_ui`](./rsact-ui/README.md) the core of UI framework.
- [`rsact_icons`](./rsact-icons/README.md) tuned pre-rendered icons targeting tiny sizes.
- [`rsact_macros`](./rsact-macros/README.md) proc macros used both for `rsact_ui` and `rsact_reactive`.
- [`rsact_encoder`](./rsact-encoder/README.md) (planned) widgets specific for platforms with encoder+button control.
- [`rsact_widgets`](./rsact-widgets/README.md) (planned) high-level widget kinds such as drop-down list, menus, etc.
