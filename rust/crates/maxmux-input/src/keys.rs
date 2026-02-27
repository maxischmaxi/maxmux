#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    Char(char),
    Ctrl(char),     // C-a through C-z (0x01-0x1a)
    Alt(char),      // ESC + char
    AltCtrl(char),  // ESC + 0x01-0x1a (rare but possible)
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Backspace, // 0x7f
    Tab,       // 0x09
    Enter,     // 0x0d
    Escape,    // lone ESC (not followed by [ within a reasonable parse)
    F(u8),     // F1-F12
    Unknown(Vec<u8>),
}

/// Parse one key from the front of a byte buffer.
/// Returns (Key, bytes_consumed).
pub fn parse_key(data: &[u8]) -> (Key, usize) {
    if data.is_empty() {
        return (Key::Unknown(vec![]), 0);
    }

    match data[0] {
        0x1b => {
            // ESC
            if data.len() == 1 {
                return (Key::Escape, 1);
            }
            match data[1] {
                b'[' => parse_csi(&data[2..]), // CSI sequence
                b'O' => parse_ss3(&data[2..]), // SS3 (function keys)
                c if (0x01..=0x1a).contains(&c) => {
                    (Key::AltCtrl((c - 1 + b'a') as char), 2)
                }
                c => (Key::Alt(c as char), 2), // Alt + printable
            }
        }
        0x01..=0x1a if data[0] != 0x09 && data[0] != 0x0d => {
            // C-a=0x01, C-b=0x02, etc.
            (Key::Ctrl((data[0] - 1 + b'a') as char), 1)
        }
        0x09 => (Key::Tab, 1),
        0x0d => (Key::Enter, 1),
        0x7f => (Key::Backspace, 1),
        c if c >= 0x20 && c < 0x7f => (Key::Char(c as char), 1),
        // UTF-8 multi-byte: read full codepoint
        _ => parse_utf8(data),
    }
}

/// Parse CSI sequences: ESC [ ...
/// `data` starts AFTER the ESC[ prefix.
/// Returns (Key, total_bytes_consumed) including the ESC[ prefix (so +2).
fn parse_csi(data: &[u8]) -> (Key, usize) {
    if data.is_empty() {
        return (Key::Unknown(vec![0x1b, b'[']), 2);
    }

    // Check for simple single-char terminators first
    match data[0] {
        b'A' => return (Key::Up, 3),
        b'B' => return (Key::Down, 3),
        b'C' => return (Key::Right, 3),
        b'D' => return (Key::Left, 3),
        b'H' => return (Key::Home, 3),
        b'F' => return (Key::End, 3),
        _ => {}
    }

    // Parse numeric parameter sequences: ESC [ <number> ~
    // Collect digits and look for ~ terminator
    let mut i = 0;
    let mut num: u16 = 0;
    while i < data.len() && data[i].is_ascii_digit() {
        num = num * 10 + (data[i] - b'0') as u16;
        i += 1;
    }

    if i < data.len() && data[i] == b'~' {
        let consumed = 2 + i + 1; // ESC[ + digits + ~
        let key = match num {
            1 => Key::Home,
            2 => Key::Insert,
            3 => Key::Delete,
            4 => Key::End,
            5 => Key::PageUp,
            6 => Key::PageDown,
            15 => Key::F(5),
            17 => Key::F(6),
            18 => Key::F(7),
            19 => Key::F(8),
            20 => Key::F(9),
            21 => Key::F(10),
            23 => Key::F(11),
            24 => Key::F(12),
            _ => Key::Unknown(data[..=i].to_vec()),
        };
        return (key, consumed);
    }

    // Unknown CSI sequence - consume what we can
    // Find the terminating byte (0x40-0x7e)
    let mut end = 0;
    while end < data.len() {
        if data[end] >= 0x40 && data[end] <= 0x7e {
            return (
                Key::Unknown(
                    [&[0x1b, b'['], &data[..=end]].concat(),
                ),
                2 + end + 1,
            );
        }
        end += 1;
    }

    // No terminator found, consume what we have
    (
        Key::Unknown([&[0x1b, b'['], data].concat()),
        2 + data.len(),
    )
}

/// Parse SS3 sequences: ESC O ...
/// `data` starts AFTER the ESC O prefix.
/// Returns (Key, total_bytes_consumed) including the ESC O prefix (so +2).
fn parse_ss3(data: &[u8]) -> (Key, usize) {
    if data.is_empty() {
        return (Key::Unknown(vec![0x1b, b'O']), 2);
    }

    let key = match data[0] {
        b'P' => Key::F(1),
        b'Q' => Key::F(2),
        b'R' => Key::F(3),
        b'S' => Key::F(4),
        b'A' => Key::Up,
        b'B' => Key::Down,
        b'C' => Key::Right,
        b'D' => Key::Left,
        b'H' => Key::Home,
        b'F' => Key::End,
        _ => Key::Unknown(vec![0x1b, b'O', data[0]]),
    };
    (key, 3)
}

/// Parse a UTF-8 multi-byte character from the front of data.
fn parse_utf8(data: &[u8]) -> (Key, usize) {
    if data.is_empty() {
        return (Key::Unknown(vec![]), 0);
    }

    let first = data[0];
    let expected_len = if first & 0b1110_0000 == 0b1100_0000 {
        2
    } else if first & 0b1111_0000 == 0b1110_0000 {
        3
    } else if first & 0b1111_1000 == 0b1111_0000 {
        4
    } else {
        // Not a valid UTF-8 start byte
        return (Key::Unknown(vec![first]), 1);
    };

    if data.len() < expected_len {
        return (Key::Unknown(data.to_vec()), data.len());
    }

    match std::str::from_utf8(&data[..expected_len]) {
        Ok(s) => {
            let c = s.chars().next().unwrap();
            (Key::Char(c), expected_len)
        }
        Err(_) => (Key::Unknown(data[..expected_len].to_vec()), expected_len),
    }
}

/// Convert Key to config string format
pub fn key_name(key: &Key) -> String {
    match key {
        Key::Char(c) => c.to_string(),
        Key::Ctrl(c) => format!("C-{}", c),
        Key::Alt(c) => format!("M-{}", c),
        Key::AltCtrl(c) => format!("M-C-{}", c),
        Key::Up => "Up".into(),
        Key::Down => "Down".into(),
        Key::Left => "Left".into(),
        Key::Right => "Right".into(),
        Key::Home => "Home".into(),
        Key::End => "End".into(),
        Key::PageUp => "PageUp".into(),
        Key::PageDown => "PageDown".into(),
        Key::Backspace => "Backspace".into(),
        Key::Tab => "Tab".into(),
        Key::Enter => "Enter".into(),
        Key::Escape => "Escape".into(),
        Key::Insert => "Insert".into(),
        Key::Delete => "Delete".into(),
        Key::F(n) => format!("F{}", n),
        Key::Unknown(_) => "Unknown".into(),
    }
}

/// Parse a prefix key config string like "C-a" to its byte value
pub fn parse_prefix_key(name: &str) -> Option<u8> {
    if name.starts_with("C-") && name.len() == 3 {
        let c = name.as_bytes()[2].to_ascii_lowercase();
        if c >= b'a' && c <= b'z' {
            Some(c - b'a' + 1)
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_printable() {
        assert_eq!(parse_key(b"a"), (Key::Char('a'), 1));
    }

    #[test]
    fn test_parse_ctrl_a() {
        assert_eq!(parse_key(&[0x01]), (Key::Ctrl('a'), 1));
    }

    #[test]
    fn test_parse_ctrl_c() {
        assert_eq!(parse_key(&[0x03]), (Key::Ctrl('c'), 1));
    }

    #[test]
    fn test_parse_tab() {
        assert_eq!(parse_key(&[0x09]), (Key::Tab, 1));
    }

    #[test]
    fn test_parse_enter() {
        assert_eq!(parse_key(&[0x0d]), (Key::Enter, 1));
    }

    #[test]
    fn test_parse_backspace() {
        assert_eq!(parse_key(&[0x7f]), (Key::Backspace, 1));
    }

    #[test]
    fn test_parse_escape_alone() {
        assert_eq!(parse_key(&[0x1b]), (Key::Escape, 1));
    }

    #[test]
    fn test_parse_arrow_up() {
        assert_eq!(parse_key(b"\x1b[A"), (Key::Up, 3));
    }

    #[test]
    fn test_parse_arrow_down() {
        assert_eq!(parse_key(b"\x1b[B"), (Key::Down, 3));
    }

    #[test]
    fn test_parse_arrow_right() {
        assert_eq!(parse_key(b"\x1b[C"), (Key::Right, 3));
    }

    #[test]
    fn test_parse_arrow_left() {
        assert_eq!(parse_key(b"\x1b[D"), (Key::Left, 3));
    }

    #[test]
    fn test_parse_alt_x() {
        assert_eq!(parse_key(b"\x1bx"), (Key::Alt('x'), 2));
    }

    #[test]
    fn test_parse_home() {
        assert_eq!(parse_key(b"\x1b[H"), (Key::Home, 3));
    }

    #[test]
    fn test_parse_end() {
        assert_eq!(parse_key(b"\x1b[F"), (Key::End, 3));
    }

    #[test]
    fn test_key_name() {
        assert_eq!(key_name(&Key::Ctrl('a')), "C-a");
    }

    #[test]
    fn test_parse_prefix_key() {
        assert_eq!(parse_prefix_key("C-a"), Some(0x01));
    }

    #[test]
    fn test_parse_prefix_key_invalid() {
        assert_eq!(parse_prefix_key("X"), None);
    }

    #[test]
    fn test_parse_f1() {
        assert_eq!(parse_key(b"\x1bOP"), (Key::F(1), 3));
    }

    #[test]
    fn test_parse_f5() {
        assert_eq!(parse_key(b"\x1b[15~"), (Key::F(5), 5));
    }

    #[test]
    fn test_parse_delete() {
        assert_eq!(parse_key(b"\x1b[3~"), (Key::Delete, 4));
    }

    #[test]
    fn test_parse_insert() {
        assert_eq!(parse_key(b"\x1b[2~"), (Key::Insert, 4));
    }

    #[test]
    fn test_parse_page_up() {
        assert_eq!(parse_key(b"\x1b[5~"), (Key::PageUp, 4));
    }

    #[test]
    fn test_parse_page_down() {
        assert_eq!(parse_key(b"\x1b[6~"), (Key::PageDown, 4));
    }

    #[test]
    fn test_parse_utf8_2byte() {
        // U+00E9 = e-acute = 0xC3 0xA9
        let data = [0xC3, 0xA9];
        assert_eq!(parse_key(&data), (Key::Char('\u{00e9}'), 2));
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(parse_key(&[]), (Key::Unknown(vec![]), 0));
    }

    #[test]
    fn test_parse_alt_ctrl() {
        // ESC + 0x01 = Alt+Ctrl+a
        assert_eq!(parse_key(&[0x1b, 0x01]), (Key::AltCtrl('a'), 2));
    }
}
