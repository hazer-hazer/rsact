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
- [] Remove embedded-graphics dependencies from rsact-icons like endianness. Remove feature flag for rsact-icons
- [] Remove embedded-graphics dependencies from Image widget.
