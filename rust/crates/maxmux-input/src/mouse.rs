#[derive(Debug, Clone, PartialEq)]
pub struct MouseEvent {
    pub button: u8,
    pub x: u16, // 0-based
    pub y: u16, // 0-based
    pub is_release: bool,
}

pub const MOUSE_LEFT: u8 = 0;
pub const MOUSE_MIDDLE: u8 = 1;
pub const MOUSE_RIGHT: u8 = 2;
pub const SCROLL_UP: u8 = 64;
pub const SCROLL_DOWN: u8 = 65;

/// Parse SGR mouse event from bytes.
/// Format: ESC[<Cb;Cx;CyM (press) or ESC[<Cb;Cx;Cym (release)
/// Returns (MouseEvent, total bytes consumed INCLUDING the ESC[< prefix)
pub fn parse_sgr_mouse(data: &[u8]) -> Option<(MouseEvent, usize)> {
    // Look for ESC[< prefix
    if data.len() < 3 || data[0] != 0x1b || data[1] != b'[' || data[2] != b'<' {
        return None;
    }

    let rest = &data[3..];

    // Parse Cb;Cx;Cy terminated by M or m
    // Find terminator M or m
    let term_pos = rest.iter().position(|&b| b == b'M' || b == b'm')?;

    let params_str = std::str::from_utf8(&rest[..term_pos]).ok()?;
    let parts: Vec<&str> = params_str.split(';').collect();
    if parts.len() != 3 {
        return None;
    }

    let button: u8 = parts[0].parse().ok()?;
    let x: u16 = parts[1].parse().ok()?;
    let y: u16 = parts[2].parse().ok()?;
    let is_release = rest[term_pos] == b'm';

    // Coords are 1-based in protocol, convert to 0-based
    let event = MouseEvent {
        button,
        x: x.saturating_sub(1),
        y: y.saturating_sub(1),
        is_release,
    };

    // Total consumed: 3 (ESC[<) + params + 1 (M or m)
    let consumed = 3 + term_pos + 1;
    Some((event, consumed))
}

/// Encode a mouse event to SGR format.
/// Coords are 0-based, will be converted to 1-based for the protocol.
pub fn encode_sgr_mouse(button: u8, x: u16, y: u16, is_release: bool) -> String {
    format!(
        "\x1b[<{};{};{}{}",
        button,
        x + 1,
        y + 1,
        if is_release { 'm' } else { 'M' }
    )
}

/// Extract the base button number (0=left, 1=middle, 2=right).
pub fn base_button(button: u8) -> u8 {
    button & 0b11
}

/// Check if this is a scroll event.
pub fn is_scroll(button: u8) -> bool {
    button & 64 != 0
}

/// Check if this is a motion event.
pub fn is_motion(button: u8) -> bool {
    button & 32 != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_left_click() {
        let data = b"\x1b[<0;10;5M";
        let (event, consumed) = parse_sgr_mouse(data).unwrap();
        assert_eq!(event.button, 0);
        assert_eq!(event.x, 9); // 10-1
        assert_eq!(event.y, 4); // 5-1
        assert!(!event.is_release);
        assert_eq!(consumed, data.len());
    }

    #[test]
    fn test_parse_release() {
        let (event, _) = parse_sgr_mouse(b"\x1b[<0;10;5m").unwrap();
        assert!(event.is_release);
    }

    #[test]
    fn test_encode_roundtrip() {
        let encoded = encode_sgr_mouse(0, 9, 4, false);
        assert_eq!(encoded, "\x1b[<0;10;5M");
    }

    #[test]
    fn test_scroll_detection() {
        assert!(is_scroll(64));
        assert!(!is_scroll(0));
    }

    #[test]
    fn test_base_button() {
        assert_eq!(base_button(MOUSE_LEFT), 0);
        assert_eq!(base_button(MOUSE_RIGHT), 2);
        assert_eq!(base_button(MOUSE_MIDDLE), 1);
    }

    #[test]
    fn test_motion_detection() {
        assert!(is_motion(32));
        assert!(!is_motion(0));
    }

    #[test]
    fn test_parse_invalid_prefix() {
        assert!(parse_sgr_mouse(b"hello").is_none());
    }

    #[test]
    fn test_parse_incomplete() {
        assert!(parse_sgr_mouse(b"\x1b[<0;10;5").is_none());
    }

    #[test]
    fn test_encode_release() {
        let encoded = encode_sgr_mouse(0, 9, 4, true);
        assert_eq!(encoded, "\x1b[<0;10;5m");
    }

    #[test]
    fn test_parse_right_click() {
        let data = b"\x1b[<2;1;1M";
        let (event, consumed) = parse_sgr_mouse(data).unwrap();
        assert_eq!(event.button, 2);
        assert_eq!(event.x, 0);
        assert_eq!(event.y, 0);
        assert!(!event.is_release);
        assert_eq!(consumed, data.len());
    }
}
