## Reactivity best practices

- For cases with a copy-type struct when only a single field needed, use `struct.with(|struct| struct.field)` instead of `struct.get().field` to avoid useless copy of bigger struct.
