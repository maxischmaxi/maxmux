// Separator style definitions for the status bar.
//
// Each style defines a left and right separator character. The separator
// is drawn between adjacent segments with carefully chosen fg/bg colors
// to create the visual transition effect.

/// All available built-in separator style names.
pub const SEPARATOR_STYLES: &[&str] = &["powerline", "rounded", "flat", "arrow", "slant"];

/// The default separator style.
pub const DEFAULT_SEPARATOR_STYLE: &str = "powerline";

/// Powerline left separator: U+E0B0 (right-pointing triangle).
pub const POWERLINE_LEFT: &str = "\u{e0b0}";
/// Powerline right separator: U+E0B2 (left-pointing triangle).
pub const POWERLINE_RIGHT: &str = "\u{e0b2}";

/// Rounded left separator: U+E0B4.
pub const ROUNDED_LEFT: &str = "\u{e0b4}";
/// Rounded right separator: U+E0B6.
pub const ROUNDED_RIGHT: &str = "\u{e0b6}";

/// Flat separator: just a space.
pub const FLAT_LEFT: &str = " ";
/// Flat separator: just a space.
pub const FLAT_RIGHT: &str = " ";

/// Arrow left separator.
pub const ARROW_LEFT: &str = ">";
/// Arrow right separator.
pub const ARROW_RIGHT: &str = "<";

/// Slant left separator: U+E0B8.
pub const SLANT_LEFT: &str = "\u{e0b8}";
/// Slant right separator: U+E0BA.
pub const SLANT_RIGHT: &str = "\u{e0ba}";

/// Get the left and right separator characters for a given style.
///
/// If `custom_left` or `custom_right` are provided, they override the
/// built-in characters for that side.
///
/// Returns `(left_char, right_char)`.
pub fn get_separator_chars(
    style: &str,
    custom_left: Option<&str>,
    custom_right: Option<&str>,
) -> (String, String) {
    let (default_left, default_right) = match style {
        "powerline" => (POWERLINE_LEFT, POWERLINE_RIGHT),
        "rounded" => (ROUNDED_LEFT, ROUNDED_RIGHT),
        "flat" => (FLAT_LEFT, FLAT_RIGHT),
        "arrow" => (ARROW_LEFT, ARROW_RIGHT),
        "slant" => (SLANT_LEFT, SLANT_RIGHT),
        // Fall back to powerline for unknown styles.
        _ => (POWERLINE_LEFT, POWERLINE_RIGHT),
    };

    let left = custom_left.unwrap_or(default_left).to_string();
    let right = custom_right.unwrap_or(default_right).to_string();
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_powerline_chars() {
        let (l, r) = get_separator_chars("powerline", None, None);
        assert_eq!(l, "\u{e0b0}");
        assert_eq!(r, "\u{e0b2}");
    }

    #[test]
    fn test_rounded_chars() {
        let (l, r) = get_separator_chars("rounded", None, None);
        assert_eq!(l, "\u{e0b4}");
        assert_eq!(r, "\u{e0b6}");
    }

    #[test]
    fn test_flat_chars() {
        let (l, r) = get_separator_chars("flat", None, None);
        assert_eq!(l, " ");
        assert_eq!(r, " ");
    }

    #[test]
    fn test_arrow_chars() {
        let (l, r) = get_separator_chars("arrow", None, None);
        assert_eq!(l, ">");
        assert_eq!(r, "<");
    }

    #[test]
    fn test_slant_chars() {
        let (l, r) = get_separator_chars("slant", None, None);
        assert_eq!(l, "\u{e0b8}");
        assert_eq!(r, "\u{e0ba}");
    }

    #[test]
    fn test_custom_overrides() {
        let (l, r) = get_separator_chars("powerline", Some("|"), Some("|"));
        assert_eq!(l, "|");
        assert_eq!(r, "|");
    }

    #[test]
    fn test_partial_custom_override() {
        let (l, r) = get_separator_chars("powerline", Some("X"), None);
        assert_eq!(l, "X");
        assert_eq!(r, "\u{e0b2}");
    }

    #[test]
    fn test_unknown_style_defaults_to_powerline() {
        let (l, r) = get_separator_chars("unknown", None, None);
        assert_eq!(l, "\u{e0b0}");
        assert_eq!(r, "\u{e0b2}");
    }
}
