//! Font-agnostic text measurement math for fixed-advance (monospace) fonts.
//!
//! These are pure integer functions: width in pixels of a run of `n`
//! monospace characters is `n*char_w + (n-1)*spacing`, where `char_w` is the
//! per-character cell width and `spacing` the inter-character gap. Wrapping is
//! computed in *character columns* (the grid a monospace renderer such as
//! `embedded_text` wraps on), which avoids pixel-rounding mismatch with the
//! draw pass.
//!
//! Proportional fonts (future `fontdue`) do not use this module — they
//! implement `FontHandler::text_height_for_width` directly via glyph metrics.

use super::{TextIntrinsics, TextOverflow};

/// Pixel width of a single monospace line containing `char_count` characters.
pub fn line_px(char_count: u32, char_w: u32, spacing: u32) -> u32 {
    // Saturate: pathological text (huge char_count) could overflow u32.
    char_count
        .saturating_mul(char_w)
        .saturating_add(char_count.saturating_sub(1).saturating_mul(spacing))
}

/// Maximum number of monospace characters that fit in `width` pixels.
/// At least 1 (a column always fits at least one character).
pub fn mono_cols(width: u32, char_w: u32, spacing: u32) -> u32 {
    let advance = char_w + spacing;
    if advance == 0 {
        // Degenerate zero-advance font: everything fits on one line.
        return u32::MAX;
    }
    // n chars span `n*char_w + (n-1)*spacing = n*advance - spacing`, so the
    // largest n with `n*advance - spacing <= width` is `(width + spacing) / advance`.
    ((width + spacing) / advance).max(1)
}

/// Number of visual lines a single hard line (no `'\n'`) wraps into, given
/// `cols` available character columns. Greedy word wrap on ASCII whitespace; a
/// word longer than `cols` is character-broken across `ceil(len/cols)` lines.
/// An empty line still occupies one visual line.
pub fn wrap_line_cols(line: &str, cols: u32) -> u32 {
    let cols = cols.max(1);
    let mut lines = 1u32;
    let mut col = 0u32; // columns used on the current visual line
    for word in line.split_whitespace() {
        let wlen = word.chars().count() as u32;
        if wlen == 0 {
            continue;
        }
        // If the word (plus a separating space) doesn't fit, move to a new line.
        if col != 0 && col + 1 + wlen > cols {
            lines += 1;
            col = 0;
        }
        if col == 0 {
            if wlen <= cols {
                col = wlen;
            } else {
                // Character-break an over-long word across ceil(wlen/cols) lines.
                let extra = (wlen - 1) / cols;
                lines += extra;
                col = wlen - extra * cols;
            }
        } else {
            col += 1 + wlen;
        }
    }
    lines
}

/// Count of hard (`'\n'`-separated) lines; at least 1.
pub fn hard_line_count(content: &str) -> u32 {
    content.split('\n').count().max(1) as u32
}

/// Width of the longest hard line with no soft wrapping (max-content width).
pub fn mono_max_content(content: &str, char_w: u32, spacing: u32) -> u32 {
    content
        .split('\n')
        .map(|line| line_px(line.chars().count() as u32, char_w, spacing))
        .max()
        .unwrap_or(0)
}

/// Width of the widest unbreakable word (min-content width for wrapping).
pub fn mono_min_content(content: &str, char_w: u32, spacing: u32) -> u32 {
    content
        .split_whitespace()
        .map(|word| line_px(word.chars().count() as u32, char_w, spacing))
        .max()
        .unwrap_or(0)
}

/// Total height of `content` laid out into `width` pixels under `overflow`.
pub fn mono_height_for_width(
    content: &str,
    char_w: u32,
    spacing: u32,
    line_height: u32,
    width: u32,
    overflow: TextOverflow,
) -> u32 {
    let visual_lines = match overflow {
        // Clip/Ellipsis keep one visual line per hard line.
        TextOverflow::Clip | TextOverflow::Ellipsis => hard_line_count(content),
        TextOverflow::Wrap => {
            let cols = mono_cols(width, char_w, spacing);
            content
                .split('\n')
                .map(|line| wrap_line_cols(line, cols))
                .sum()
        },
    };
    visual_lines.max(1) * line_height
}

/// Intrinsic width range + single-line height for `content` under `overflow`.
pub fn mono_intrinsics(
    content: &str,
    char_w: u32,
    spacing: u32,
    line_height: u32,
    overflow: TextOverflow,
) -> TextIntrinsics {
    let min_content_width = match overflow {
        // Clip/Ellipsis can be squeezed below the widest word.
        TextOverflow::Clip | TextOverflow::Ellipsis => 0,
        TextOverflow::Wrap => mono_min_content(content, char_w, spacing),
    };
    TextIntrinsics {
        min_content_width,
        max_content_width: mono_max_content(content, char_w, spacing),
        line_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_px_counts_chars_and_inter_char_spacing() {
        assert_eq!(line_px(0, 6, 1), 0);
        assert_eq!(line_px(1, 6, 1), 6);
        assert_eq!(line_px(3, 6, 1), 3 * 6 + 2 * 1);
        assert_eq!(line_px(3, 6, 0), 18);
    }

    #[test]
    fn mono_cols_is_chars_that_fit_at_least_one() {
        assert_eq!(mono_cols(20, 6, 1), 3); // (20+1)/7 = 3
        assert_eq!(mono_cols(6, 6, 1), 1); // (6+1)/7 = 1
        assert_eq!(mono_cols(0, 6, 1), 1); // clamped to 1
        assert_eq!(mono_cols(36, 6, 0), 6);
    }

    #[test]
    fn wrap_line_cols_packs_words_greedily() {
        assert_eq!(wrap_line_cols("hello world", 11), 1); // exact fit incl. space
        assert_eq!(wrap_line_cols("hello world", 10), 2); // space pushes over
        assert_eq!(wrap_line_cols("a b c", 1), 3); // each word own line
        assert_eq!(wrap_line_cols("", 5), 1); // empty still one line
    }

    #[test]
    fn wrap_line_cols_breaks_overlong_words() {
        assert_eq!(wrap_line_cols("abcdefgh", 3), 3); // ceil(8/3)
        assert_eq!(wrap_line_cols("abcdef", 3), 2); // exact 2
    }

    #[test]
    fn hard_line_count_splits_on_newline() {
        assert_eq!(hard_line_count(""), 1);
        assert_eq!(hard_line_count("a"), 1);
        assert_eq!(hard_line_count("a\nb"), 2);
        assert_eq!(hard_line_count("a\nb\nc"), 3);
    }

    #[test]
    fn max_content_is_longest_line() {
        assert_eq!(mono_max_content("ab\ncde", 6, 0), 18); // "cde"
        assert_eq!(mono_max_content("ab cde", 6, 0), 36); // single line, incl. space
        assert_eq!(mono_max_content("", 6, 0), 0);
    }

    #[test]
    fn min_content_is_widest_word() {
        assert_eq!(mono_min_content("ab cde\nf", 6, 0), 18); // "cde"
        assert_eq!(mono_min_content("", 6, 0), 0);
        assert_eq!(mono_min_content("   ", 6, 0), 0);
    }

    #[test]
    fn height_for_width_wraps_when_narrow() {
        // width 36 px, char 6 px, no spacing => 6 columns; "hello world" (11) => 2 lines
        assert_eq!(
            mono_height_for_width(
                "hello world",
                6,
                0,
                10,
                36,
                TextOverflow::Wrap
            ),
            20
        );
    }

    #[test]
    fn height_for_width_clip_ignores_wrapping() {
        assert_eq!(
            mono_height_for_width(
                "hello world",
                6,
                0,
                10,
                36,
                TextOverflow::Clip
            ),
            10
        );
        assert_eq!(
            mono_height_for_width("a\nb", 6, 0, 10, 6, TextOverflow::Clip),
            20
        );
    }

    #[test]
    fn height_for_width_counts_hard_lines_when_wide() {
        assert_eq!(
            mono_height_for_width("a\nb", 6, 0, 10, 1000, TextOverflow::Wrap),
            20
        );
    }

    #[test]
    fn intrinsics_wrap_reports_word_and_line_widths() {
        assert_eq!(
            mono_intrinsics("ab cde", 6, 0, 10, TextOverflow::Wrap),
            TextIntrinsics {
                min_content_width: 18,
                max_content_width: 36,
                line_height: 10
            }
        );
    }

    #[test]
    fn intrinsics_clip_is_squeezable() {
        assert_eq!(
            mono_intrinsics("ab cde", 6, 0, 10, TextOverflow::Clip),
            TextIntrinsics {
                min_content_width: 0,
                max_content_width: 36,
                line_height: 10
            }
        );
    }
}
