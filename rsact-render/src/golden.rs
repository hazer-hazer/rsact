//! Golden-file test harness (WS6.9) — the blessed-reference machinery behind
//! the render tests.
//!
//! A *golden* is a committed reference file. A test produces some current
//! output (a [`format_ops`](crate::record::format_ops) draw-call log, or an
//! encoded PNG) and asserts it equals the golden. When the output legitimately
//! changes, you **bless** the new output by re-running with `UPDATE_GOLDENS=1`,
//! which rewrites the golden instead of failing — so a render change is
//! reviewed as a diff of the golden file, exactly what WS6's damage work and
//! WS6.10's renderer-parity audit need.
//!
//! This module is `std`-gated (file I/O) and lives here — not in a `#[cfg(test)]`
//! block — so it is reusable from *other* crates' tests: `rsact-ui` asserts
//! page-render goldens, and `rsact-render` itself will assert EG-vs-tiny-skia
//! PNG parity goldens. Each caller passes its own `env!("CARGO_MANIFEST_DIR")`
//! so goldens live under that crate's `tests/goldens/`.

use alloc::string::String;
use std::{fs, path::PathBuf};

/// `<manifest_dir>/tests/goldens/<name>`.
fn golden_path(manifest_dir: &str, name: &str) -> PathBuf {
    let mut path = PathBuf::from(manifest_dir);
    path.push("tests");
    path.push("goldens");
    path.push(name);
    path
}

/// Whether we are in bless mode (`UPDATE_GOLDENS=1`).
fn blessing() -> bool {
    std::env::var("UPDATE_GOLDENS").ok().as_deref() == Some("1")
}

/// Write `bytes` to `path`, creating `tests/goldens/` if needed. Used by the
/// bless path of both asserts.
fn bless(path: &PathBuf, bytes: &[u8]) {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).unwrap_or_else(|e| {
            panic!("failed to create golden dir {}: {e}", dir.display())
        });
    }
    fs::write(path, bytes).unwrap_or_else(|e| {
        panic!("failed to write golden {}: {e}", path.display())
    });
}

/// Assert `actual` matches the text golden at
/// `<manifest_dir>/tests/goldens/<name>`.
///
/// - Match → pass.
/// - Mismatch → panic with a line diff (unless blessing, which rewrites it).
/// - Missing golden → panic asking you to bless (unless blessing, which creates
///   it).
///
/// Pass `env!("CARGO_MANIFEST_DIR")` as `manifest_dir`.
pub fn assert_text_golden(manifest_dir: &str, name: &str, actual: &str) {
    let path = golden_path(manifest_dir, name);

    if blessing() {
        bless(&path, actual.as_bytes());
        return;
    }

    match fs::read_to_string(&path) {
        Ok(expected) if expected == actual => {},
        Ok(expected) => panic!(
            "golden mismatch: {}\n{}\n\
             run with UPDATE_GOLDENS=1 to bless the new output",
            path.display(),
            line_diff(&expected, actual),
        ),
        Err(_) => panic!(
            "golden missing: {} — run with UPDATE_GOLDENS=1 to create it",
            path.display(),
        ),
    }
}

/// Assert `actual` matches the binary golden at
/// `<manifest_dir>/tests/goldens/<name>` (e.g. an encoded PNG). Same bless
/// workflow as [`assert_text_golden`]; the mismatch message reports byte
/// lengths rather than a diff.
pub fn assert_bytes_golden(manifest_dir: &str, name: &str, actual: &[u8]) {
    let path = golden_path(manifest_dir, name);

    if blessing() {
        bless(&path, actual);
        return;
    }

    match fs::read(&path) {
        Ok(expected) if expected == actual => {},
        Ok(expected) => panic!(
            "golden byte mismatch: {} (golden {} bytes, actual {} bytes) — \
             run with UPDATE_GOLDENS=1 to bless",
            path.display(),
            expected.len(),
            actual.len(),
        ),
        Err(_) => panic!(
            "golden missing: {} — run with UPDATE_GOLDENS=1 to create it",
            path.display(),
        ),
    }
}

/// A minimal positional line diff for the mismatch message — `-` lines are the
/// golden, `+` lines are the current output. Not an LCS diff (a shift shows the
/// whole tail as changed), which is plenty for spotting what moved in a small
/// draw-op log.
fn line_diff(expected: &str, actual: &str) -> String {
    use core::fmt::Write as _;

    let expected: alloc::vec::Vec<&str> = expected.lines().collect();
    let actual: alloc::vec::Vec<&str> = actual.lines().collect();
    let mut out = String::from("--- golden\n+++ actual\n");
    for i in 0..expected.len().max(actual.len()) {
        match (expected.get(i), actual.get(i)) {
            (Some(e), Some(a)) if e == a => {
                let _ = writeln!(out, " {e}");
            },
            (Some(e), Some(a)) => {
                let _ = writeln!(out, "-{e}\n+{a}");
            },
            (Some(e), None) => {
                let _ = writeln!(out, "-{e}");
            },
            (None, Some(a)) => {
                let _ = writeln!(out, "+{a}");
            },
            (None, None) => {},
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_diff_marks_changed_added_and_removed_lines() {
        let diff = line_diff("a\nb\nc", "a\nB\nc\nd");
        assert_eq!(diff, "--- golden\n+++ actual\n a\n-b\n+B\n c\n+d\n");
    }

    #[test]
    fn blessing_reads_env() {
        // Documents the toggle the harness keys off — no env is set in the
        // normal test run, so this must be false.
        assert!(!blessing());
    }
}
