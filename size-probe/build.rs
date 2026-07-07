//! Standard cortex-m link setup: put `memory.x` on the linker search path and
//! link cortex-m-rt's `link.x`. `rustc-link-arg-bins` scopes the link script to
//! this crate's binaries, so no workspace-wide `.cargo/config.toml` is needed.

use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out.join("memory.x"), include_bytes!("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
}
