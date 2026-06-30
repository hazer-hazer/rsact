# Library evolution

I plan to deeply use LLMs to rethink this library.

Here are the contents written by me along points from models.

Use cargo-hack to check for all features to compile

```
cargo hack check --feature-powerset --no-dev-deps --all --mutually-exclusive-features std,single-thread --at-least-one-of std,single-thread
```

## Ideas

- rsact_reactive: support/pass custom comparator to memo
  I think a distinct function-constructor is better, like memo_with(...). Comparator can by any function `a, b -> bool`

- S.js library has `S.value` which is a signal by with memo behavior. It is interesting because controls change propagation from start point unlike my memos being controlled at the end point, i.e. `S.value` only fires change events if new value differs from previous, while my signals are always fired and it is the memos that check for value change. I find my memos more universal, because in most cases consumer tells what logic it needs, but such signal is interesting, because can optimize event propagation a lot, as in many cases signal is always wrapped in memos or expected to be so, thus we can avoid recomparison in each memo.
- Continuing the point of the previous idea, I think it is possible to optimize memos right now without `S.value`-like signal by storing signal comparison result at signal fire stage. ~~But I think some problems with graph can appear and it needs deep testing as value can possibly be changed by event dependent on it~~ (never mind, these are two distinct stages of reactivity process)

## TODOs

This are the actions to be done by me or LLM. When LLM completes one, it should mark it checked ([x] checkbox complete, [ ] - incomplete). LLM must not do checked todo item again, but must check all todo items each time to find if there's a conflict with other changes or todo item is incomplete or needs more investigation, in such cases it must give a feedback to the user. Items marked with "WIP" must be skipped by LLM because I didn't complete them to be ready for development.

- [] WIP: `S.js` has nice specifications and requirements for signals. Copy useful paragraphs from readme and tell LLM to write tests based on them.
- [] `SignalMapReactive` seems strange as it makes reactive values from `Inert`. This should be avoided as `Memo` from `Inert` leads to useless cloning. I think that it is okay to live with distinct cases where `MaybeReactive` or truly reactive values are expected.
- [] Go over the cases where something strange like `.inert().memo()` happens, most of the time this is incorrect.
- [x] `MaybeReactive` widget meta easily implementable through custom MaybeReactive tree of Meta. Look at MemoTree.
- [x] `MaybeReactive` layouts require reactive-on-write reactivity primitive that will turn into signal when user sets it from some reactive source
- [] For debugging purpose we can add `what_changed` function that will list values that are changed in current reactive observer telling why this observer recomputed
- [] ??? I think that now we can get rid of using MemoChain for styles in each widget. Let's replace them and make a perfect reactive dependency style inheritance in render pass.
- [] Add full mouse support. Start with simple traversal + maybe path cache for non-reactive element paths. Maybe move to more complex hit testing.
- [] Fully get rid of embedded_graphics as a required dependency and implement generic proxies for rendering.
- [] Remove embedded-graphics dependencies from rsact-tiny-icons like endianness. Remove feature flag for rsact-tiny-icons
- [] Remove embedded-graphics dependencies from Image widget.
- [] Move rendering to a separate crate. Split implementations for EG, tiny-skia and custom. rsact-ui and rsact-tiny-icons should depend on rsact-render. rsact-render should contain structures for images, primitives.
- [] Check that all primitives have common rendering behavior among all renderers.
  - Arc must start and sweep at the same points for EG and tiny-skia
- [] Learn more about kurbo library, it contains a lot of features to work with curves, maybe we can get some algorithms from there or even use it as a library adding interoperability with tiny-skia
- [] Think how to deal with a problem that we need radius for complex drawing still we targeting embedded where diameter is preferred because on small displays we often want precise size of an element, i.e. cannot express a circle element of size 5x5 pixels by its radius (because we use integers for the Size). It's okay to convert diameter to f32 radius for tiny-skia because it works with f32 anyway, but we should be correct here anyway.
- [] Rename rsact-icons to rsact-tiny-icons as it is only about super small icon sizes?
- [] If we plan to move from embedded graphics as a renderer and leave it only as a target, we need to implement a lot of rendering algorithms. I'm interested in effective algorithms with integer math to render everything tiny-skia can (or at least the most significant subset of it).
- [] Add support for `fontdue` crate
- [] Get rid of `unwrap`s, UI must never fail and should report errors via logs.
- [x] `FontProps` per-field reactivity (`Option<MaybeReactive<...>>`) was redundant with `Layout`'s reactive-on-write. Now stores plain data (`Option<Font>` etc.) and `.font()/.font_size()/.font_style()` route through the layout setter like `.width()/.padding()`. NOTE: the setters cannot use `self.layout()` — it returns `Layout` by value, so `now_reactive()`'s `Static→Reactive` upgrade lands on a discarded copy (disposes the inert id → panic on read). Added a `LayoutWidget` supertrait providing `layout_mut(&mut self) -> &mut Layout`; all three setter traits (`SizedWidget`/`BlockModelWidget`/`FontSettingWidget`) now use `self.layout_mut().setter(...)`, which also fixes the same latent reactive-panic bug for width/height/padding/border. Still subsumable by the `font/mod.rs` `TextStyle`-widget TODO.
- [ ] As we move to extension trait usage for containers (`ContainerExt`/`.container()`, `FlexExt`/`.row()`/`.col()`), should we have some common naming for these methods so it is clean that wrapping is done? I want some simple not verbose change `wrap_in_container`, at most `in_container` or `in_flex`. But with `in_container`/`in_flex` it is ambiguous because `(view1, view2, view3).in_flex()` is not actually "wrap in flex", it is an actual Flex widget creation. Maybe we can name them with Uppercase like `(view1, view2, view3).Flex()` and suppress the naming warning? But what to do with `.Row()` and `.Col()` which are not actual widgets but calls to `Flex` widget constructor? Maybe it is okay. This is needed because some long view declarations can appear, for example something like one below, where it is hard to see where the next widget nesting happens.

```rust
(view1, view2, view3)
.col()
.gap(5u32)
.fill()
.container()
.padding(5u32)
.width(Length::Shrink)
.height(Length::fill())
```

## Ideas/RFCs

### Views as widget builders

The idea is too avoid storing widget's properties that needed only for building while unused in lifecycle passes (such as `on_event`, `render`, etc.). This can be achieved by many ways, for example splitting each widget into a Builder and actual Widget.
Another path is to make
