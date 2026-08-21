# Coding rules for this project

## Reactivity best practices

1. For cases with a copy-type struct when only a single field needed, use `struct.with(|struct| struct.field)` instead of `struct.get().field` to avoid useless copy of bigger struct.

## General coding rules

Repetitive logic must be carried out to some wrapper function.
For example I asked to replace fonts inheritance in mount context with layout font_props value usage in render. Agent added this lines to each Widget:

```rust
let mut fp = self.layout.with(|l| l.font_props().unwrap_or_default());
fp.inherit(&ctx.font_props);
ctx.with_font_props(fp, |ctx| ctx.render_child(&self.content))
```

Instead of this model must have had add layout to `render_child` method parameters and inherit font_props inside it.

## File layout

1. **`#[cfg(test)] mod tests` always goes at the END of the file.** Never insert a
   test module between `impl` blocks or next to the code it tests, even when the
   item under test is defined mid-file — a reader scrolling for production code
   should never have to page through tests to reach the next `impl`, and diffs
   stay readable when tests grow. Same for helper `fn`s that exist only for
   tests: they belong inside that trailing `mod tests`.

## Spelling

1. **American English in code and comments: `color`, not `colour`.** The public
   API is already `Color`/`PackedColor`/`ColorStyle`, so British spellings in
   prose put two words for one concept into the same file and break a reader's
   `grep`. Same for the rest of the family — `behavior`, `initialize`,
   `normalize`, `center`. (Prose in `docs/plans/` is the maintainer's and is not
   covered by this.)

## Never-do restrictions

1. Never delete any `Note:` or `TODO:` comments until it is 100% done `TODO` or `Note` to a deleted code part.
