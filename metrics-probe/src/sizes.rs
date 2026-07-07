//! Layer 2: `.text/.rodata/.bss` from the linked thumb `size-probe` binaries.
//!
//! Builds each (target, binary) via `cargo --manifest-path size-probe/Cargo.toml`
//! (the crate is excluded from the workspace) and reads section sizes from the
//! ELF with the `object` crate. `.bss` is dominated by the probe's fixed (tiny)
//! heap buffer + cortex-m-rt statics, not framework RAM (which is heap-resident)
//! — `.text`/`.rodata` are the meaningful flash-footprint signal. Missing
//! targets/toolchain degrade to a skipped entry (logged), never an abort.

use crate::snapshot::SectionSizes;
use object::{Object, ObjectSection};
use std::{path::PathBuf, process::Command};

/// Floor pair: Blue Pill (thumbv7m) + the thumbv6m compile-only baseline. Add
/// thumbv7em-none-eabihf (Black Pill) here when its budgets are wanted.
const TARGETS: &[&str] = &["thumbv7m-none-eabi", "thumbv6m-none-eabi"];
const BINS: &[&str] = &["reactive", "ui"];
const TARGET_DIR: &str = "target/size-probe";

pub fn measure_all() -> Vec<SectionSizes> {
    let mut out = Vec::new();
    for &target in TARGETS {
        for &bin in BINS {
            match build_and_read(target, bin) {
                Ok(s) => {
                    println!(
                        "  size {bin}/{target}: .text={} .rodata={} .bss={}",
                        s.text, s.rodata, s.bss
                    );
                    out.push(s);
                },
                Err(e) => eprintln!("  size {bin}/{target}: skipped ({e})"),
            }
        }
    }
    out
}

fn build_and_read(target: &str, bin: &str) -> Result<SectionSizes, String> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            "size-probe/Cargo.toml",
            "--release",
            "--target",
            target,
            "--bin",
            bin,
            "--target-dir",
            TARGET_DIR,
        ])
        .status()
        .map_err(|e| format!("spawn cargo: {e}"))?;
    if !status.success() {
        return Err("cargo build failed".into());
    }

    let elf = PathBuf::from(TARGET_DIR)
        .join(target)
        .join("release")
        .join(bin);
    let data = std::fs::read(&elf)
        .map_err(|e| format!("read {}: {e}", elf.display()))?;
    let obj =
        object::File::parse(&*data).map_err(|e| format!("parse ELF: {e}"))?;

    let (mut text, mut rodata, mut bss) = (0u64, 0u64, 0u64);
    for sec in obj.sections() {
        match sec.name().unwrap_or("") {
            ".text" => text += sec.size(),
            ".rodata" => rodata += sec.size(),
            ".bss" => bss += sec.size(),
            _ => {},
        }
    }

    Ok(SectionSizes {
        target: target.to_string(),
        binary: bin.to_string(),
        text,
        rodata,
        bss,
    })
}
