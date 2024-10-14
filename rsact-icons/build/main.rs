use bitvec::{order::Msb0, vec::BitVec};
use convert_case::Casing;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use resvg::tiny_skia::Pixmap;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    str::FromStr,
};
use syn::Ident;
use system::SystemIcon;
use usvg::{Transform, Tree};

mod common;
pub mod icon_set;
pub mod system;

pub use common::CommonIcon;

macro_rules! build_sizes {
    ($($size: literal: $feature: literal),* $(,)?) => [
        &[$(
            #[cfg(feature = $feature)]
            $size
        ),*]
    ];
}

const BUILD_SIZES: &[u32] = build_sizes![
    5: "5px",
    6: "6px",
    7: "7px",
    8: "8px",
    9: "9px",
    10: "10px",
    11: "11px",
    12: "12px",
    13: "13px",
    14: "14px",
    15: "15px",
    16: "16px",
    17: "17px",
    18: "18px",
    19: "19px",
    20: "20px",
    21: "21px",
    22: "22px",
    23: "23px",
    24: "24px",
];
// const ALPHA_CUTOFF: u8 = 0x60;
const BASE_SIZE: f32 = 24.0;
const OUTPUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/rendered");

const BYTE_ORDER_EG: &str = "BigEndian";
type Bits = BitVec<u8, Msb0>;

fn byte_order() -> TokenStream {
    let bo = format_ident!("{BYTE_ORDER_EG}");
    quote! {embedded_graphics::pixelcolor::raw::#bo}
}

fn icon_type() -> TokenStream {
    let bo = byte_order();
    quote! {crate::IconRaw<#bo>}
}

pub struct Modifiers {
    alpha_cutoff: u8,
    scale: (f32, f32),
}

impl Modifiers {
    const fn base(alpha_cutoff: u8) -> Self {
        Self { alpha_cutoff, scale: (1.0, 1.0) }
    }

    const fn alpha_cutoff(mut self, alpha_cutoff: u8) -> Self {
        self.alpha_cutoff = alpha_cutoff;
        self
    }

    const fn ac(mut self, alpha_cutoff: u8) -> Self {
        self.alpha_cutoff = alpha_cutoff;
        self
    }

    const fn scale_x(mut self, scale: f32) -> Self {
        self.scale.0 = scale;
        self
    }

    const fn scale_y(mut self, scale: f32) -> Self {
        self.scale.1 = scale;
        self
    }
}

pub struct Icon<S: IconSet> {
    source_filename: &'static str,
    data: &'static str,
    name: &'static str,
    kind: S,
    aliases: &'static [&'static str],
}

impl<S: IconSet> Icon<S> {
    fn const_name(&self) -> Ident {
        format_ident!("{}", self.name.to_case(convert_case::Case::UpperSnake))
    }

    fn aliases_consts(&self) -> impl Iterator<Item = Ident> {
        self.aliases.iter().map(|alias| {
            format_ident!("{}", alias.to_case(convert_case::Case::UpperSnake))
        })
    }

    fn aliases_variants(&self) -> impl Iterator<Item = Ident> {
        self.aliases.iter().map(|alias| format_ident!("{alias}"))
    }
}

pub trait IconSet: Sized + Copy + 'static {
    fn mod_name() -> &'static str;
    fn ident() -> Ident;
    fn sizes() -> &'static [u32];
    fn icons() -> &'static [Icon<Self>];
    fn modifiers(size: u32, kind: Self) -> Modifiers;
}

fn draw(bit_vec: &Bits, size: usize) -> String {
    let lines = bit_vec
        .chunks(size)
        .map(|line| {
            format!(
                "┃{}┃",
                line.iter()
                    .map(|bit| {
                        match *bit {
                            true => "█",
                            false => "🗍",
                        }
                    })
                    .collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let top_bottom_border = "━".repeat(size);
    format!("\n┏{top_bottom_border}┓\n{lines}\n┗{top_bottom_border}┛\n")
}

fn render<S: IconSet>(size: u32, icon: &Icon<S>) -> Bits {
    let mods = S::modifiers(size, icon.kind);
    let scale = mods.scale;

    let mut pixmap = Pixmap::new(size, size).unwrap();

    let size_f = size as f32;
    let auto_scale = (size_f + 2.0) / BASE_SIZE;
    let scale = (scale.0, scale.1);

    resvg::render(
        &Tree::from_str(
            &icon.data,
            &usvg::Options {
                shape_rendering: usvg::ShapeRendering::GeometricPrecision,
                ..Default::default()
            },
        )
        .unwrap(),
        Transform::default()
            .pre_translate(-12.0, -12.0)
            .pre_scale(auto_scale, auto_scale)
            .pre_scale(scale.0, scale.1)
            .post_translate(-scale.0, -scale.1)
            .post_translate(12.0, 12.0),
        &mut pixmap.as_mut(),
    );

    pixmap
        .data()
        .iter()
        .enumerate()
        .filter(|(a, _)| a % 4 == 3)
        .map(|(_, alpha)| *alpha > mods.alpha_cutoff)
        .collect()
}

fn pretty(tokens: TokenStream) -> String {
    let syntax_tree = syn::parse2(tokens).unwrap();
    prettyplease::unparse(&syntax_tree)
}

fn gen_icon<S: IconSet>(size: u32, icon: &Icon<S>) -> (TokenStream, usize) {
    let rendered = render(size, icon);

    let painting = draw(&rendered, size as usize);

    let bytes = rendered.into_vec();
    let bytes = bytes
        .iter()
        .map(|bytes| {
            TokenStream::from_str(&format!("0b{:08b}", bytes)).unwrap()
        })
        .collect::<Vec<_>>();
    let memory = bytes.len();

    let aliases = icon.aliases_consts();
    let const_name = icon.const_name();

    // TODO: Display modifiers
    let info =
        format!(" Kind: {}; filename: {};", icon.name, icon.source_filename);
    let alias_info = format!(" Alias to [`{}`]", const_name);

    let icon_type = icon_type();
    let tokens = quote! {
        #[doc = #info]
        #[doc = #painting]
        pub const #const_name: #icon_type = crate::IconRaw::new(&[#(#bytes),*], #size);

        #(
            #[doc = #alias_info]
            pub const #aliases: #icon_type = #const_name;
        )*
    };

    (tokens, memory)
}

fn gen_mod<S: IconSet>(size: u32, icons: &[Icon<S>]) -> (TokenStream, usize) {
    let generated_icons =
        icons.iter().map(|icon| gen_icon(size, icon)).collect::<Vec<_>>();

    let mod_memory = generated_icons.iter().map(|(_, memory)| memory).sum();
    let icon_tokens = generated_icons.into_iter().map(|i| i.0);
    let icons_data = icons
        .iter()
        .map(|icon| {
            (
                format_ident!("{}", icon.name),
                icon.const_name(),
                icon.aliases_variants().collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    let icon_kinds = icons_data.iter().map(|(kind, ..)| kind);
    let icon_const_names = icons_data.iter().map(|(_, name, _)| name);
    let (icon_aliases, icon_aliases_targets): (Vec<_>, Vec<_>) = icons_data
        .iter()
        .map(|(_, name, aliases)| aliases.iter().zip(core::iter::repeat(name)))
        .flatten()
        .unzip();

    let set_mod = format_ident!("{}", S::mod_name());
    let set = S::ident();
    let set_path = quote! {crate::#set_mod::#set};

    let info = format!(
        " Icons {} {size}x{size}; total size of mod: {}",
        S::ident(),
        mod_memory
    );
    let icon_type = icon_type();
    let tokens = quote! {
        #![doc = #info]

        #(#icon_tokens)*

        pub fn get_icon(kind: #set_path) -> #icon_type {
            match kind {
                #(#set_path::#icon_kinds => #icon_const_names,)*
                #(#set_path::#icon_aliases => #icon_aliases_targets,)*
            }
        }
    };

    (tokens, mod_memory)
}

fn gen_set<S: IconSet>() {
    let mut set_memory = 0;

    let dirpath = &Path::new(OUTPUT_DIR).join(S::mod_name());
    fs::create_dir_all(dirpath).unwrap();

    let mods = S::sizes()
        .iter()
        .copied()
        .filter(|size| BUILD_SIZES.contains(size))
        .map(|size| {
            let (code, memory) = gen_mod(size, S::icons());

            set_memory += memory;

            let mod_name = format!("icons_{}", size);

            let code = pretty(code);

            let filepath =
                &dirpath.join(Path::new(&mod_name).with_extension("rs"));

            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(filepath)
                .expect(&format!(
                    "Failed to write to file {}",
                    filepath.to_str().unwrap()
                ));

            writeln!(&mut file, "{}", code).expect(&format!(
                "Failed to write to file {}",
                filepath.to_str().unwrap()
            ));

            (format_ident!("{mod_name}"), size, format!("{size}px"))
        })
        .collect::<Vec<_>>();
    let mod_names = mods.iter().map(|(name, ..)| name);
    let mod_names1 = mods.iter().map(|(name, ..)| name).clone();
    let size_features = mods.iter().map(|(_, _, feature)| feature);
    let sizes = mods.iter().map(|(_, size, _)| size);
    let sizes1 = sizes.clone();
    let largest_size_mod = sizes
        .clone()
        .zip(mod_names.clone())
        .max_by_key(|(size, _)| *size)
        .map(|(_, mod_name)| mod_name)
        .into_iter();

    let set_name = S::ident();
    let icon_names =
        S::icons().iter().map(|icon| format_ident!("{}", icon.name));
    let aliases = S::icons().iter().map(|icon| {
        let alias_info = format!(" Alias to [`{set_name}::{}`]", icon.name);
        let aliases = icon.aliases_variants();
        quote! {
            #(
                #[doc = #alias_info]
                #aliases,
            )*
        }
    });

    let info = format!(
        "{set_name} icon set, {:#.2}KB ({set_memory}B) total",
        set_memory as f32 / 1024.0
    );

    let bo = byte_order();
    let icon_type = icon_type();
    let code = quote! {
        #(
            #[cfg(feature = #size_features)]
            pub mod #mod_names;
        )*

        #[doc = #info]
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub enum #set_name {
            #(#icon_names,)*
            #(#aliases)*
        }

        impl crate::IconSet<#bo> for #set_name {
            const SIZES: &[u32] = &[
                #(#sizes1),*
            ];

            fn size(&self, size: u32) -> #icon_type {
                match size {
                    #(..#sizes => self::#mod_names1::get_icon(*self),)*

                    #(_ => self::#largest_size_mod::get_icon(*self))*
                }
            }
        }
    };

    let mod_filepath = &dirpath.join("mod").with_extension("rs");
    if fs::exists(mod_filepath).unwrap() {
        fs::remove_file(mod_filepath).unwrap();
    }

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(mod_filepath)
        .unwrap();

    let code = pretty(code);

    writeln!(&mut file, "{}", code).expect("Failed to write mod.rs");
}

fn main() {
    if !fs::exists(OUTPUT_DIR).unwrap() {
        fs::create_dir(OUTPUT_DIR).unwrap();
    }

    let sets_names = [
        #[cfg(feature = "system")]
        SystemIcon::mod_name(),
        #[cfg(feature = "common")]
        CommonIcon::mod_name(),
    ];
    let sets_mods = sets_names.iter().map(|name| format_ident!("{name}"));
    let rendered_mod = quote! {
        #(
            #[cfg(feature = #sets_names)]
            pub mod #sets_mods;
        )*
    };

    let mut rendered_mod_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(Path::new(OUTPUT_DIR).join("mod").with_extension("rs"))
        .expect("Failed to write to file rendered/mod.rs");

    writeln!(&mut rendered_mod_file, "{}", pretty(rendered_mod)).unwrap();

    #[cfg(feature = "system")]
    gen_set::<SystemIcon>();

    #[cfg(feature = "common")]
    gen_set::<CommonIcon>();
}
