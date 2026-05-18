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

## Never-do restrictions

1. Never delete any `Note:` or `TODO:` comments until it is 100% done `TODO` or `Note` to a deleted code part.
