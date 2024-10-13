use bitvec::{order::Msb0, vec::BitVec};
use convert_case::Casing;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use resvg::tiny_skia::Pixmap;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    str::FromStr,
};
use syn::Ident;
use usvg::{Transform, Tree};

mod common;
pub mod icon_set;

use common::CommonIcon;

// const ALPHA_CUTOFF: u8 = 0x60;
const BASE_SIZE: f32 = 24.0;
const OUTPUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/rendered");

const BYTE_ORDER_EG: &str = "BigEndian";
type Bits = BitVec<u8, Msb0>;

// struct RenderOptions<S: IconSet> {
//     size: u32,
//     alpha_cutoff: u8,
//     modifiers: Option<fn(S) -> Modifiers>,
// }

// impl<S: IconSet> RenderOptions<S> {
//     const fn new(size: u32, alpha_cutoff: u8) -> Self {
//         Self { size, alpha_cutoff, modifiers: None }
//     }

//     const fn modify(mut self, modifiers: fn(S) -> Modifiers) -> Self {
//         self.modifiers = Some(modifiers);
//         self
//     }

//     fn display(&self, icon: &Icon<S>) -> String {
//         format!(
//             "size: {}, alpha_cutoff: {:#x}, scale: {:?}",
//             self.size,
//             self.alpha_cutoff(icon),
//             self.scale(icon)
//         )
//     }

//     fn alpha_cutoff(&self, icon: &Icon<S>) -> u8 {
//         self.modifiers
//             .map(|modifiers| modifiers(icon.kind).alpha_cutoff)
//             .flatten()
//             .or(icon.modifiers.alpha_cutoff)
//             .unwrap_or(self.alpha_cutoff)
//     }

//     fn scale(&self, icon: &Icon<S>) -> (f32, f32) {
//         // let default_scale = (self.size as f32 + 2.0) / BASE_SIZE;
//         let default_scale = 1.0;
//         let modified =
//             self.modifiers.map(|modifiers| modifiers(icon.kind).scale);

//         (
//             modified
//                 .map(|m| m.0)
//                 .flatten()
//                 .or(icon.modifiers.scale.0)
//                 .unwrap_or(default_scale),
//             modified
//                 .map(|m| m.1)
//                 .flatten()
//                 .or(icon.modifiers.scale.1)
//                 .unwrap_or(default_scale),
//         )
//     }
// }

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
}

impl<S: IconSet> Icon<S> {
    fn const_name(&self) -> Ident {
        format_ident!("{}", self.name.to_case(convert_case::Case::UpperSnake))
    }
}

pub trait IconSet: Sized + Copy + 'static {
    fn name() -> &'static str;
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

    // Add 2 pixels to size to remove material icon margins
    // `- size_f / BASE_SIZE` is an offset of 1 pixel in this size
    // let one_pixel = size_f / BASE_SIZE;
    // let translate = (
    //     // (one_pixel - scale.0) - one_pixel,
    //     // (one_pixel - scale.1) - one_pixel,
    //     -one_pixel, -one_pixel,
    // );

    let auto_scale = (size_f + 2.0) / BASE_SIZE;

    // println!("{ratio_to_base} translate: {translate:?}");
    // let pre_scale = size as f32 / BASE_SIZE;
    // let pre_translate = (
    //     (-1.0 * scale.0),
    //     (-1.0 * scale.1),
    // );

    // println!("Translate {pre_translate:?}");

    let scale = (scale.0, scale.1);

    resvg::render(
        &Tree::from_str(
            &icon.data,
            &usvg::Options {
                shape_rendering: usvg::ShapeRendering::GeometricPrecision,
                // image_rendering: usvg::ImageRendering::OptimizeQuality,
                // default_size: usvg::Size::from_wh(size_f, size_f).unwrap(),
                ..Default::default()
            },
        )
        .unwrap(),
        Transform::default()
            // .pre_translate(translate.0.round(), translate.1.round())
            .pre_translate(-12.0, -12.0)
            // .pre_translate(pre_translate.0, pre_translate.1)
            .pre_scale(auto_scale, auto_scale)
            .pre_scale(scale.0, scale.1)
            .post_translate(-scale.0, -scale.1)
            .post_translate(12.0, 12.0),
        // .pre_translate(scale.0, scale.1),
        &mut pixmap.as_mut(),
    );

    pixmap
        .data()
        .iter()
        .enumerate()
        .filter(|(a, _)| a % 4 == 3 /* select alpha channel */)
        .map(|(_, b)| *b) // discard index
        .map(|alpha| alpha > mods.alpha_cutoff)
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

    // TODO: Print modifiers
    let info =
        format!(" name: {}; filename: {};", icon.name, icon.source_filename);

    let const_name = icon.const_name();

    let bo = format_ident!("{BYTE_ORDER_EG}");
    let tokens = quote! {
        #[doc = #info]
        #[doc = #painting]
        const #const_name: crate::IconRaw<embedded_graphics::pixelcolor::raw::#bo> = crate::IconRaw::new(&[#(#bytes),*], #size);
    };

    (tokens, memory)
}

fn gen_mod<S: IconSet>(size: u32, icons: &[Icon<S>]) -> (TokenStream, usize) {
    let generated_icons =
        icons.iter().map(|icon| gen_icon(size, icon)).collect::<Vec<_>>();

    let mod_memory = generated_icons.iter().map(|(_, memory)| memory).sum();
    let icon_tokens = generated_icons.into_iter().map(|i| i.0);
    let (icon_kinds, icon_const_names): (Vec<_>, Vec<_>) =
        icons.iter().map(|icon| (icon.name, icon.const_name())).unzip();

    let set_name = S::name();

    let tokens = quote! {
        #(#icon_tokens)*

        pub fn get_icon(kind: crate::#set_name) -> crate::IconRaw {
            match kind {
                #(crate::#set_name::#icon_kinds => #icon_const_names),*
            }
        }
    };

    (tokens, mod_memory)
}

fn gen_set<S: IconSet>() -> usize {
    let mut set_memory = 0;

    let (mod_names, size_features): (Vec<_>, Vec<_>) = S::sizes()
        .iter()
        .copied()
        .map(|size| {
            let (icons, memory) = gen_mod(size, S::icons());

            set_memory += memory;

            let mod_name = format!("icons_{}", size);

            let doc =
                format!("Icons {size}x{size}, the whole pack takes {memory}B");
            let code = quote! {
                #[doc = #doc]

                #icons
            };
            let code = pretty(code);

            let filepath = &Path::new(OUTPUT_DIR)
                .join(Path::new(&mod_name).with_extension("rs"));

            // if fs::exists(filepath).unwrap() {
            //     fs::rename(
            //         filepath,
            //         filepath.with_file_name(format!(
            //             "{}-old-{}",
            //             mod_name,
            //             SystemTime::now()
            //                 .duration_since(UNIX_EPOCH)
            //                 .unwrap()
            //                 .as_millis()
            //         )),
            //     )
            //     .unwrap();
            // }

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

            (format_ident!("{mod_name}"), format!("{size}px"))
        })
        .unzip();

    let mod_tokens = quote! {
        #(
            #[cfg(feature = #size_features)]
            pub mod #mod_names;
        )*
    };

    let mod_filepath = &Path::new(OUTPUT_DIR).join("mod").with_extension("rs");
    if fs::exists(mod_filepath).unwrap() {
        fs::remove_file(mod_filepath).unwrap();
    }

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(mod_filepath)
        .unwrap();

    writeln!(&mut file, "{}", pretty(mod_tokens))
        .expect("Failed to write mod.rs");

    set_memory
}

const OPTS: &[RenderOptions] = &[
    RenderOptions::new(6, 0x60).modify(|kind| match kind {
        CommonIcon::Add => Modifiers::none().alpha_cutoff(0x00),
        CommonIcon::ArrowExpand | CommonIcon::ArrowExpandAll => {
            Modifiers::none().alpha_cutoff(0xa0)
        },
        CommonIcon::AlertBox => Modifiers::none().alpha_cutoff(0xd0),
        CommonIcon::Archive => Modifiers::none().alpha_cutoff(0xbf),
        CommonIcon::Bars => Modifiers::none().scale_y(1.2),
        CommonIcon::Bell => Modifiers::none().scale_y(0.91),
        CommonIcon::Bolt =>
Modifiers::none().alpha_cutoff(0x40).scale_y(1.2),         CommonIcon::Clock
=> Modifiers::none().alpha_cutoff(0x3f).scale_x(1.01),
        CommonIcon::CloseCircle => Modifiers::none().alpha_cutoff(0xc0),
        CommonIcon::Comment => {
            Modifiers::none().alpha_cutoff(0xb0).scale_x(0.9)
        },
        CommonIcon::Commit => Modifiers::none().alpha_cutoff(0x30),
        CommonIcon::Cup => Modifiers::none().alpha_cutoff(0x30),
        // IconKind::Delete => Modifiers::none().alpha_cutoff(0x60),
        // IconKind::Eject =>
Modifiers::none().alpha_cutoff(0xa0).scale_y(2.0),
        CommonIcon::DotsHorizontal => {
            Modifiers::none().alpha_cutoff(0x80).scale_y(1.2).scale_x(0.9)
        },
        CommonIcon::DotsVertical => {
            Modifiers::none().scale_x(1.2).alpha_cutoff(0x8f).scale_y(0.9)
        },
        CommonIcon::Eye => Modifiers::none().alpha_cutoff(0x70),
        CommonIcon::Equal => Modifiers::none().alpha_cutoff(0x3f),
        CommonIcon::Function => {
            Modifiers::none().alpha_cutoff(0x20).scale_x(1.1)
        },
        CommonIcon::Heart => Modifiers::none().alpha_cutoff(0xc0),
        CommonIcon::Magnet => {
            Modifiers::none().alpha_cutoff(0xb0).scale_y(1.05)
        },
        // IconKind::Lock => Modifiers::none().alpha_cutoff(0x30),
        CommonIcon::Thermometer => Modifiers::none().alpha_cutoff(0x60),
        CommonIcon::Pencil => Modifiers::none().alpha_cutoff(0x60),
        CommonIcon::PlusMinus => Modifiers::none().alpha_cutoff(0x30),
        CommonIcon::Send => Modifiers::none().alpha_cutoff(0x80),
        CommonIcon::StepForward | CommonIcon::StepBackward => {
            Modifiers::none().alpha_cutoff(0xa0).scale_x(1.1)
        },
        CommonIcon::Terminal => Modifiers::none().alpha_cutoff(0x50),
        _ => Modifiers::none(),
    }),
    RenderOptions::new(7, 0x40).modify(|kind| match kind {
        CommonIcon::AlertBox => Modifiers::none().alpha_cutoff(0xba),
        CommonIcon::Archive => Modifiers::none().alpha_cutoff(0x80),
        CommonIcon::ArrowLeft | CommonIcon::ArrowRight => {
            Modifiers::none().alpha_cutoff(0x10)
        },
        CommonIcon::Bell => Modifiers::none().scale_y(0.9),
        CommonIcon::Delete => Modifiers::none().alpha_cutoff(0x7f),
        CommonIcon::Exclamation => Modifiers::none().alpha_cutoff(0x40),
        CommonIcon::Heart => Modifiers::none().alpha_cutoff(0xa0),
        CommonIcon::Hourglass => Modifiers::none().alpha_cutoff(0x7a),
        // IconKind::Lock => Modifiers::none().alpha_cutoff(0x30),
        CommonIcon::Thermometer => Modifiers::none().alpha_cutoff(0x30),
        _ => Modifiers::none(),
    }),
    RenderOptions::new(8, 0x3f).modify(|kind| match kind {
        CommonIcon::Archive => {
            Modifiers::none().alpha_cutoff(0xc0).scale_y(1.1)
        },
        CommonIcon::Delete => Modifiers::none().alpha_cutoff(0x70),
        CommonIcon::Link => Modifiers::none().alpha_cutoff(0x60),
        CommonIcon::Pencil => Modifiers::none().alpha_cutoff(0x70),
        CommonIcon::Pin => Modifiers::none().alpha_cutoff(0x30),
        CommonIcon::PlusMinus => {
            Modifiers::none().alpha_cutoff(0x60).scale_y(1.1)
        },
        CommonIcon::Share => Modifiers::none().alpha_cutoff(0x20),
        _ => Modifiers::none(),
    }),
    RenderOptions::new(9, 0x7f).modify(|kind| match kind {
        CommonIcon::AlertBox => Modifiers::none().alpha_cutoff(0x40),
        CommonIcon::Bookmark => Modifiers::none().alpha_cutoff(0xb0),
        CommonIcon::Droplet => Modifiers::none().alpha_cutoff(0x10),
        CommonIcon::Heart => Modifiers::none().alpha_cutoff(0x50),
        _ => Modifiers::none(),
    }),
    RenderOptions::new(24, 0x80),
];

fn main() {
    if !fs::exists(OUTPUT_DIR).unwrap() {
        fs::create_dir(OUTPUT_DIR).unwrap();
    }

    let total_memory = gen_set::<CommonIcon>();

    println!(
        "Total memory for all sets is {:#.2}KB",
        total_memory as f32 / 1024.0
    );
}
