// Status bar renderer.
//
// Composes left and right segments into a single ANSI-escaped line,
// with separators between segments, bar-colored fill in the middle,
// and optional prefix-active color override.

use maxmux_renderer::ansi;

use crate::separators::get_separator_chars;
use crate::types::{ResolvedTheme, Segment};

/// Compute the display width of a string, counting only visible characters.
/// This is a simplified version that counts Unicode scalar values, which is
/// sufficient for the single-width characters we use in the status bar.
fn display_width(s: &str) -> usize {
    // Count characters, ignoring ANSI escape sequences.
    let mut width = 0;
    let mut in_escape = false;
    for ch in s.chars() {
        if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
            continue;
        }
        if ch == '\x1b' {
            in_escape = true;
            continue;
        }
        width += 1;
    }
    width
}

/// Render a single segment's content (text with style attributes) as ANSI.
fn render_segment_content(seg: &Segment, prefix_active: bool, theme: &ResolvedTheme) -> String {
    let mut out = String::new();

    let (fg, bg) = if prefix_active {
        (&theme.modules.prefix.fg, &theme.modules.prefix.bg)
    } else {
        (&seg.fg, &seg.bg)
    };

    out.push_str(&ansi::fg_hex(fg));
    out.push_str(&ansi::bg_hex(bg));

    if seg.bold || prefix_active {
        out.push_str(ansi::bold());
    }
    if seg.italic {
        out.push_str(ansi::italic());
    }
    if seg.dim {
        out.push_str(ansi::dim());
    }

    out.push_str(&seg.text);
    out.push_str(ansi::reset_style());
    out
}

/// Render a left separator between two segments (or between a segment and the bar).
///
/// Left separators: fg = current segment bg, bg = next segment bg (or bar bg).
fn render_left_separator(
    sep_char: &str,
    current_bg: &str,
    next_bg: &str,
    prefix_active: bool,
    theme: &ResolvedTheme,
) -> String {
    let mut out = String::new();
    let (fg, bg): (&str, &str) = if prefix_active {
        (&theme.modules.prefix.bg, next_bg)
    } else {
        (current_bg, next_bg)
    };
    out.push_str(&ansi::fg_hex(fg));
    out.push_str(&ansi::bg_hex(bg));
    out.push_str(sep_char);
    out.push_str(ansi::reset_style());
    out
}

/// Render a right separator before a segment (transition from bar/previous to segment).
///
/// Right separators: fg = next segment bg, bg = current/previous bg (or bar bg).
fn render_right_separator(
    sep_char: &str,
    prev_bg: &str,
    segment_bg: &str,
    prefix_active: bool,
    theme: &ResolvedTheme,
) -> String {
    let mut out = String::new();
    let (fg, bg): (&str, &str) = if prefix_active {
        (&theme.modules.prefix.bg, prev_bg)
    } else {
        (segment_bg, prev_bg)
    };
    out.push_str(&ansi::fg_hex(fg));
    out.push_str(&ansi::bg_hex(bg));
    out.push_str(sep_char);
    out.push_str(ansi::reset_style());
    out
}

/// Render left-aligned segments with separators between them.
///
/// Returns `(ansi_string, total_display_width)`.
fn render_left_segments(
    segments: &[Segment],
    theme: &ResolvedTheme,
    left_sep: &str,
    prefix_active: bool,
) -> (String, usize) {
    let mut out = String::new();
    let mut width: usize = 0;

    for (i, seg) in segments.iter().enumerate() {
        // Render the segment content.
        let content = render_segment_content(seg, prefix_active, theme);
        let content_width = display_width(&seg.text);
        out.push_str(&content);
        width += content_width;

        // Render the separator after this segment.
        let next_bg = if i + 1 < segments.len() {
            if prefix_active {
                &theme.modules.prefix.bg
            } else {
                &segments[i + 1].bg
            }
        } else {
            &theme.bar.bg
        };

        let current_bg = if prefix_active {
            &theme.modules.prefix.bg
        } else {
            &seg.bg
        };

        let sep = render_left_separator(left_sep, current_bg, next_bg, prefix_active, theme);
        let sep_width = display_width(left_sep);
        out.push_str(&sep);
        width += sep_width;
    }

    (out, width)
}

/// Render right-aligned segments with separators between them.
///
/// Returns `(ansi_string, total_display_width)`.
fn render_right_segments(
    segments: &[Segment],
    theme: &ResolvedTheme,
    right_sep: &str,
    prefix_active: bool,
) -> (String, usize) {
    let mut out = String::new();
    let mut width: usize = 0;

    for (i, seg) in segments.iter().enumerate() {
        // Render the separator before this segment.
        let prev_bg = if i == 0 {
            &theme.bar.bg
        } else if prefix_active {
            &theme.modules.prefix.bg
        } else {
            &segments[i - 1].bg
        };

        let seg_bg = if prefix_active {
            &theme.modules.prefix.bg
        } else {
            &seg.bg
        };

        let sep = render_right_separator(right_sep, prev_bg, seg_bg, prefix_active, theme);
        let sep_width = display_width(right_sep);
        out.push_str(&sep);
        width += sep_width;

        // Render the segment content.
        let content = render_segment_content(seg, prefix_active, theme);
        let content_width = display_width(&seg.text);
        out.push_str(&content);
        width += content_width;
    }

    (out, width)
}

/// Render left and right segments into a complete status bar line.
///
/// The bar is positioned at the given `row` (0-based), fills `cols` columns,
/// and uses the theme's bar colors for the middle fill.
///
/// When `prefix_active` is true, all segment colors are overridden with the
/// theme's prefix colors and text is rendered bold.
pub fn render_segments(
    left_segments: &[Segment],
    right_segments: &[Segment],
    theme: &ResolvedTheme,
    separator_style: &str,
    cols: u16,
    row: u16,
    prefix_active: bool,
) -> String {
    let (left_sep, right_sep) = get_separator_chars(separator_style, None, None);

    let (left_ansi, left_width) =
        render_left_segments(left_segments, theme, &left_sep, prefix_active);
    let (right_ansi, right_width) =
        render_right_segments(right_segments, theme, &right_sep, prefix_active);

    let total_cols = cols as usize;
    let fill_width = total_cols.saturating_sub(left_width + right_width);

    // Build the fill (bar-colored spaces).
    let fill = format!(
        "{}{}{}{}",
        ansi::fg_hex(&theme.bar.fg),
        ansi::bg_hex(&theme.bar.bg),
        " ".repeat(fill_width),
        ansi::reset_style(),
    );

    // Compose the full line: move cursor, then left + fill + right.
    format!(
        "{}{}{}{}",
        ansi::move_to(0, row),
        left_ansi,
        fill,
        right_ansi,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::resolve_theme;

    #[test]
    fn test_display_width_plain() {
        assert_eq!(display_width("hello"), 5);
    }

    #[test]
    fn test_display_width_with_ansi() {
        let s = format!("{}hello{}", ansi::bold(), ansi::reset_style());
        assert_eq!(display_width(&s), 5);
    }

    #[test]
    fn test_render_simple_left_segment() {
        let theme = resolve_theme("catppuccin-mocha");
        let seg = Segment::new(" session ", "#1e1e2e", "#89b4fa");
        let output = render_segments(&[seg], &[], &theme, "powerline", 80, 0, false);
        // Should contain the segment text.
        assert!(output.contains(" session "));
        // Should contain the move_to sequence for row 0.
        assert!(output.contains("\x1b[1;1H"));
    }

    #[test]
    fn test_render_left_and_right_segments() {
        let theme = resolve_theme("dracula");
        let left = Segment::new(" L ", "#282a36", "#bd93f9");
        let right = Segment::new(" R ", "#282a36", "#50fa7b");
        let output = render_segments(&[left], &[right], &theme, "powerline", 80, 5, false);
        assert!(output.contains(" L "));
        assert!(output.contains(" R "));
        // Row 5 => ANSI row 6.
        assert!(output.contains("\x1b[6;1H"));
    }

    #[test]
    fn test_prefix_active_overrides_colors() {
        let theme = resolve_theme("nord");
        let seg = Segment::new(" test ", "#2e3440", "#88c0d0");
        let normal = render_segments(&[seg.clone()], &[], &theme, "flat", 40, 0, false);
        let prefix = render_segments(&[seg], &[], &theme, "flat", 40, 0, true);
        // They should differ because prefix mode overrides colors.
        assert_ne!(normal, prefix);
        // Prefix output should contain the prefix bg color (#bf616a for nord).
        assert!(prefix.contains("191;97;106")); // RGB for #bf616a
    }

    #[test]
    fn test_empty_segments_produce_bar_line() {
        let theme = resolve_theme("gruvbox");
        let output = render_segments(&[], &[], &theme, "powerline", 20, 0, false);
        // Should just be move_to + 20 spaces with bar colors.
        assert!(output.contains("\x1b[1;1H"));
        // Should contain bar bg color for gruvbox (#282828 = 40,40,40).
        assert!(output.contains("40;40;40"));
    }

    #[test]
    fn test_render_with_flat_separator() {
        let theme = resolve_theme("catppuccin-mocha");
        let seg = Segment::new(" A ", "#1e1e2e", "#89b4fa");
        let output = render_segments(&[seg], &[], &theme, "flat", 40, 0, false);
        // Flat separator is a space, no powerline characters.
        assert!(!output.contains('\u{e0b0}'));
    }

    #[test]
    fn test_render_bold_segment() {
        let theme = resolve_theme("catppuccin-mocha");
        let seg = Segment::new(" bold ", "#1e1e2e", "#89b4fa").with_bold(true);
        let output = render_segments(&[seg], &[], &theme, "flat", 40, 0, false);
        // Should contain bold escape.
        assert!(output.contains("\x1b[1m"));
    }

    #[test]
    fn test_render_italic_segment() {
        let theme = resolve_theme("catppuccin-mocha");
        let seg = Segment::new(" italic ", "#1e1e2e", "#89b4fa").with_italic(true);
        let output = render_segments(&[seg], &[], &theme, "flat", 40, 0, false);
        // Should contain italic escape.
        assert!(output.contains("\x1b[3m"));
    }

    #[test]
    fn test_multiple_left_segments() {
        let theme = resolve_theme("one-dark");
        let segs = vec![
            Segment::new(" A ", "#282c34", "#61afef"),
            Segment::new(" B ", "#282c34", "#98c379"),
            Segment::new(" C ", "#282c34", "#e5c07b"),
        ];
        let output = render_segments(&segs, &[], &theme, "powerline", 80, 0, false);
        assert!(output.contains(" A "));
        assert!(output.contains(" B "));
        assert!(output.contains(" C "));
        // Should contain powerline separators.
        assert!(output.contains('\u{e0b0}'));
    }

    #[test]
    fn test_multiple_right_segments() {
        let theme = resolve_theme("solarized");
        let segs = vec![
            Segment::new(" X ", "#002b36", "#268bd2"),
            Segment::new(" Y ", "#002b36", "#859900"),
        ];
        let output = render_segments(&[], &segs, &theme, "powerline", 60, 0, false);
        assert!(output.contains(" X "));
        assert!(output.contains(" Y "));
        // Should contain right powerline separator.
        assert!(output.contains('\u{e0b2}'));
    }
}
