# Evolution of rsact-tiny-icons

- [] Rename to rsact-tiny-icons as it is only about pre-rendered small icons of fixed size?
- [] Get rid of EG dependency
- [] Create a widget for these icons. Now the widget `Icon` is used for this in rsact-ui, but it requires `IconSet` and only works with rsact-tiny-icons, which is absolutely wrong. Better have a separate widget like `TinyIcon` in rsact-tiny-icons, and rsact-ui should have its own widget for a general icon, better SVG, not rasterized.
