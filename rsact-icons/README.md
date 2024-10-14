# rsact-icons

Collection of icons rendered from Material Design Icons (deprecated) on build time.
The main points of this library are:
- Support for super-small displays: `rsact-icons` minimum icon size is 5px!
- Include only what you need: rsact is a framework for embedded systems, so unused data should not take flash-memory space.
- Be generic: I tried to make this library as extensible as possible.

## How to use `rsact-icons`

Icons in this library are separated into three sets:
- `system` - icons that needed for `rsact-ui` framework and are rendered starting from `5px` size. The set is very small and include only "required" icons.
- `common` - common icons frequently found in the web.
- `extended` - either rarely used icons or such icons that impossible to distinguishably render on small sizes.

All sets are also feature flags: `system`, `common` and `extended`. `system` is enabled by default.
Sizes are also feature-gated, and you can render only needed ones.
Available sizes are `5..=24`.

> By default, feature `all-sizes` is enabled, to render `system` for all sizes, but `system` is a small set, whereas when using `common` or `extended`, much more memory will be used. So when using `common` or `extended`, set `default-features = false` and enable only required sizes.

Icons are stored as constant of type `IconRaw` containing size (square) and byte slices that should be interpreted as bit-slices, i.e. each byte contains 8 pixels.
For example:
```rs
// Get data for 10px magnet icon
let icon = rsact_icons::icons_10::MAGNET;
```

For your and debugging convenience, each icon has comment where it is rendered as NxN matrix.
So, to explore icons, better look into built source code.

Each set is an enum implementing `rsact_icons::IconSet` trait:
- `SystemIcon`
- `CommonIcon`
- `ExtendedIcon`
`CommonIcon::Magnet` is just a kind of icon, to get its data, use `IconSet::size` method:
```rs
let size: u32 = get_size_somewhere();
let icon = CommonIcon::Magnet.size(size);
```

Inside `IconSet::size`, `get_icon` methods are used: each generated `icon_N` module includes `get_icon` method to get icon data of size `N` by kind. For example:
```rs
let kind: CommonIcon = get_icon_kind();
let icon10px = rsact_icons::common::icons_10::get_icon(kind);
```

If your use of icons is limited by common symbols such as arrows, crosses, etc., you have high chance to find them in `system` set and don't need other sets. If you need some real-world symbols, try to find them in `common` or `extended`.

If you don't find an icon you need -- contributions are welcome 😸

### Aliases

Some icons have aliases to simplify searching for a specific kind of icon.
Aliases are both present as constants, e.g. `const ADD` for `const PLUS`, and kinds `SystemIcon::Add` for `SystemIcon::Plus`.

## How can I draw icons bigger than 24px?
Right now you can't. This is because this library is at first solving the problem with super-small icons.

It is planned to add bigger sizes but not in the way small sizes implemented. Instead of features flags I'll either use build-time rendering using [`tiny-skia`](https://github.com/RazrFalcon/tiny-skia) or render icons on some large size such as 48px and implement scaling for raster data, possibly with caching to avoid re-scaling.

## Roadmap

1. Move `get_icon` and `IconSet::size` implementations under feature flag (such as `match-icons`) to remove usage of all icons to be sure that compiler does not include all icon constants. Possibly it already should work without changes if user does not use `get_icon` or` IconSet::size`
2. Add `IconKind` enum containing icons from all generated icon sets.
