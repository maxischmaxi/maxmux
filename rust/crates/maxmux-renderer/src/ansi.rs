// ANSI escape code utilities.
//
// All functions are pure string builders. No state, no side effects.

// --- Cursor movement ---

/// Move cursor to position (x, y). ANSI coordinates are 1-based.
pub fn move_to(x: u16, y: u16) -> String {
    format!("\x1b[{};{}H", y + 1, x + 1)
}

/// Hide the cursor.
pub fn hide_cursor() -> &'static str {
    "\x1b[?25l"
}

/// Show the cursor.
pub fn show_cursor() -> &'static str {
    "\x1b[?25h"
}

/// Set cursor style (0=default, 1=block, 3=underline, 5=bar).
pub fn set_cursor_style(style: u8) -> String {
    format!("\x1b[{style} q")
}

// --- Style attributes ---

/// Reset all style attributes.
pub fn reset_style() -> &'static str {
    "\x1b[0m"
}

/// Bold text.
pub fn bold() -> &'static str {
    "\x1b[1m"
}

/// Dim text.
pub fn dim() -> &'static str {
    "\x1b[2m"
}

/// Italic text.
pub fn italic() -> &'static str {
    "\x1b[3m"
}

/// Underlined text.
pub fn underline() -> &'static str {
    "\x1b[4m"
}

/// Inverse (swap foreground/background).
pub fn inverse() -> &'static str {
    "\x1b[7m"
}

// --- Colors (RGB) ---

/// Set foreground color using RGB values.
pub fn fg_rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

/// Set background color using RGB values.
pub fn bg_rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[48;2;{r};{g};{b}m")
}

// --- Colors (Hex) ---

/// Set foreground color using a hex string (e.g. "#RRGGBB" or "RRGGBB").
pub fn fg_hex(hex: &str) -> String {
    let (r, g, b) = hex_to_rgb(hex);
    fg_rgb(r, g, b)
}

/// Set background color using a hex string (e.g. "#RRGGBB" or "RRGGBB").
pub fn bg_hex(hex: &str) -> String {
    let (r, g, b) = hex_to_rgb(hex);
    bg_rgb(r, g, b)
}

// --- Screen ---

/// Clear the entire screen.
pub fn clear_screen() -> &'static str {
    "\x1b[2J"
}

/// Clear the current line.
pub fn clear_line() -> &'static str {
    "\x1b[2K"
}

/// Clear from cursor to end of line.
pub fn clear_to_end() -> &'static str {
    "\x1b[K"
}

/// Enter the alternate screen buffer.
pub fn enter_alt_screen() -> &'static str {
    "\x1b[?1049h"
}

/// Exit the alternate screen buffer.
pub fn exit_alt_screen() -> &'static str {
    "\x1b[?1049l"
}

// --- Mouse ---

/// Enable mouse tracking (basic, button motion, SGR extended).
pub fn enable_mouse() -> &'static str {
    "\x1b[?1000h\x1b[?1002h\x1b[?1006h"
}

/// Disable mouse tracking.
pub fn disable_mouse() -> &'static str {
    "\x1b[?1000l\x1b[?1002l\x1b[?1006l"
}

// --- Bracketed paste ---

/// Enable bracketed paste mode.
pub fn enable_bracketed_paste() -> &'static str {
    "\x1b[?2004h"
}

/// Disable bracketed paste mode.
pub fn disable_bracketed_paste() -> &'static str {
    "\x1b[?2004l"
}

// --- Helper ---

/// Parse a hex color string (with or without leading '#') into (R, G, B).
fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    (r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_to() {
        assert_eq!(move_to(0, 0), "\x1b[1;1H");
        assert_eq!(move_to(10, 5), "\x1b[6;11H");
    }

    #[test]
    fn test_fg_rgb() {
        assert_eq!(fg_rgb(255, 0, 128), "\x1b[38;2;255;0;128m");
    }

    #[test]
    fn test_bg_rgb() {
        assert_eq!(bg_rgb(0, 255, 0), "\x1b[48;2;0;255;0m");
    }

    #[test]
    fn test_fg_hex() {
        assert_eq!(fg_hex("#ff0080"), "\x1b[38;2;255;0;128m");
    }

    #[test]
    fn test_fg_hex_without_hash() {
        assert_eq!(fg_hex("ff0080"), "\x1b[38;2;255;0;128m");
    }

    #[test]
    fn test_cursor_style() {
        assert_eq!(set_cursor_style(1), "\x1b[1 q");
    }

    #[test]
    fn test_static_strings() {
        assert_eq!(hide_cursor(), "\x1b[?25l");
        assert_eq!(show_cursor(), "\x1b[?25h");
        assert_eq!(reset_style(), "\x1b[0m");
        assert_eq!(bold(), "\x1b[1m");
        assert_eq!(enter_alt_screen(), "\x1b[?1049h");
        assert_eq!(exit_alt_screen(), "\x1b[?1049l");
    }
}
