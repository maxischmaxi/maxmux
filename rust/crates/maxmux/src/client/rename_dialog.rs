/// Rename dialog overlay for renaming sessions and windows.
///
/// Provides a small centered modal with a text input field,
/// cursor navigation, and confirm/cancel actions.
use std::fmt::Write as FmtWrite;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum RenameAction {
    None,
    Confirm(String),
    Cancel,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct RenameDialog {
    pub title: String,
    pub value: String,
    pub cursor_pos: usize,
    pub visible: bool,
}

impl RenameDialog {
    pub fn new(title: impl Into<String>, initial_value: impl Into<String>) -> Self {
        let value: String = initial_value.into();
        let cursor_pos = value.len();
        Self {
            title: title.into(),
            value,
            cursor_pos,
            visible: true,
        }
    }

    // -- key dispatch -------------------------------------------------------

    pub fn handle_key(&mut self, key: &str) -> RenameAction {
        match key {
            "Escape" => RenameAction::Cancel,

            "Enter" => RenameAction::Confirm(self.value.clone()),

            "Backspace" => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.value.remove(self.cursor_pos);
                }
                RenameAction::None
            }

            "Delete" => {
                if self.cursor_pos < self.value.len() {
                    self.value.remove(self.cursor_pos);
                }
                RenameAction::None
            }

            "Left" => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
                RenameAction::None
            }

            "Right" => {
                if self.cursor_pos < self.value.len() {
                    self.cursor_pos += 1;
                }
                RenameAction::None
            }

            "Home" => {
                self.cursor_pos = 0;
                RenameAction::None
            }

            "End" => {
                self.cursor_pos = self.value.len();
                RenameAction::None
            }

            ch => {
                // Accept single printable characters
                if ch.len() == 1 {
                    let c = ch.chars().next().unwrap();
                    if !c.is_control() {
                        self.value.insert(self.cursor_pos, c);
                        self.cursor_pos += 1;
                    }
                }
                RenameAction::None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Catppuccin Mocha palette constants (RGB).
mod colors {
    pub const BORDER: (u8, u8, u8) = (88, 91, 112); // #585b70
    pub const TITLE: (u8, u8, u8) = (137, 180, 250); // #89b4fa
    pub const BG: (u8, u8, u8) = (30, 30, 46); // #1e1e2e
    pub const TEXT: (u8, u8, u8) = (205, 214, 244); // #cdd6f4
    pub const CURSOR_BG: (u8, u8, u8) = (205, 214, 244); // #cdd6f4
    pub const CURSOR_FG: (u8, u8, u8) = (30, 30, 46); // #1e1e2e
    pub const HINT: (u8, u8, u8) = (166, 173, 200); // #a6adc8
}

impl RenameDialog {
    /// Render the rename dialog as a centered modal overlay.
    ///
    /// Returns an ANSI escape sequence string.
    pub fn render(&self, cols: u16, rows: u16) -> String {
        let cols = cols as usize;
        let rows = rows as usize;
        if cols < 14 || rows < 7 {
            return String::new();
        }

        let modal_width = 44.min(cols.saturating_sub(4));
        let modal_height = 5; // top border, title, input, hint, bottom border

        let start_col = (cols.saturating_sub(modal_width)) / 2 + 1;
        let start_row = (rows.saturating_sub(modal_height)) / 2 + 1;

        let inner_width = modal_width.saturating_sub(2);

        let mut out = String::with_capacity(modal_height * modal_width * 4);

        let bg = format!(
            "\x1b[48;2;{};{};{}m",
            colors::BG.0,
            colors::BG.1,
            colors::BG.2
        );
        let border_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::BORDER.0,
            colors::BORDER.1,
            colors::BORDER.2
        );
        let title_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::TITLE.0,
            colors::TITLE.1,
            colors::TITLE.2
        );
        let text_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::TEXT.0,
            colors::TEXT.1,
            colors::TEXT.2
        );
        let cursor_style = format!(
            "\x1b[48;2;{};{};{}m\x1b[38;2;{};{};{}m",
            colors::CURSOR_BG.0,
            colors::CURSOR_BG.1,
            colors::CURSOR_BG.2,
            colors::CURSOR_FG.0,
            colors::CURSOR_FG.1,
            colors::CURSOR_FG.2,
        );
        let hint_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::HINT.0,
            colors::HINT.1,
            colors::HINT.2
        );
        let reset = "\x1b[0m";

        let goto = |r: usize, c: usize| -> String { format!("\x1b[{};{}H", r, c) };

        // -- top border -------------------------------------------------------
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}",
            goto(start_row, start_col),
            bg,
            border_fg,
            "\u{256d}",
            "\u{2500}".repeat(inner_width),
            "\u{256e}",
            reset,
        );

        // -- title line -------------------------------------------------------
        let title = &self.title;
        let title_padding = inner_width.saturating_sub(title.len());
        let title_left_pad = title_padding / 2;
        let title_right_pad = title_padding - title_left_pad;
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}{}{}{}",
            goto(start_row + 1, start_col),
            bg,
            border_fg,
            "\u{2502}",
            title_fg,
            " ".repeat(title_left_pad),
            title,
            " ".repeat(title_right_pad),
            border_fg,
            "\u{2502}",
        );
        let _ = write!(out, "{}", reset);

        // -- input line: "> {value}" with cursor ------------------------------
        let prompt = "> ";
        let prompt_len = prompt.len();
        // Build value with embedded cursor
        let before_cursor = &self.value[..self.cursor_pos];
        let cursor_char = if self.cursor_pos < self.value.len() {
            &self.value[self.cursor_pos..self.cursor_pos + 1]
        } else {
            " "
        };
        let after_cursor = if self.cursor_pos < self.value.len() {
            &self.value[self.cursor_pos + 1..]
        } else {
            ""
        };

        // Calculate total visible content length
        let content_len = prompt_len + before_cursor.len() + 1 + after_cursor.len();
        let trailing_pad = if content_len < inner_width {
            inner_width - content_len
        } else {
            0
        };

        let _ = write!(
            out,
            "{}{}{}{}{}{}{}{}{}{}{}{}{}",
            goto(start_row + 2, start_col),
            bg,
            border_fg,
            "\u{2502}",
            text_fg,
            prompt,
            before_cursor,
            cursor_style,
            cursor_char,
            reset,
            bg,
            text_fg,
            after_cursor,
        );
        // Fill remaining space and right border
        let _ = write!(
            out,
            "{}{}{}{}",
            " ".repeat(trailing_pad),
            border_fg,
            "\u{2502}",
            reset,
        );

        // -- hint line --------------------------------------------------------
        let hint = "Enter: confirm  Esc: cancel";
        let hint_padding = inner_width.saturating_sub(hint.len());
        let hint_left_pad = hint_padding / 2;
        let hint_right_pad = hint_padding - hint_left_pad;
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}{}{}{}",
            goto(start_row + 3, start_col),
            bg,
            border_fg,
            "\u{2502}",
            hint_fg,
            " ".repeat(hint_left_pad),
            hint,
            " ".repeat(hint_right_pad),
            border_fg,
            "\u{2502}",
        );
        let _ = write!(out, "{}", reset);

        // -- bottom border ----------------------------------------------------
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}",
            goto(start_row + 4, start_col),
            bg,
            border_fg,
            "\u{2570}",
            "\u{2500}".repeat(inner_width),
            "\u{256f}",
            reset,
        );

        out
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // 1. Type adds chars
    #[test]
    fn test_type_adds_chars() {
        let mut dialog = RenameDialog::new("Rename Window", "");
        dialog.handle_key("h");
        dialog.handle_key("e");
        dialog.handle_key("l");
        dialog.handle_key("l");
        dialog.handle_key("o");
        assert_eq!(dialog.value, "hello");
        assert_eq!(dialog.cursor_pos, 5);
    }

    // 2. Enter confirms with current value
    #[test]
    fn test_enter_confirms() {
        let mut dialog = RenameDialog::new("Rename Window", "my-window");
        let action = dialog.handle_key("Enter");
        assert_eq!(action, RenameAction::Confirm("my-window".into()));
    }

    // 3. Escape cancels
    #[test]
    fn test_escape_cancels() {
        let mut dialog = RenameDialog::new("Rename Window", "");
        let action = dialog.handle_key("Escape");
        assert_eq!(action, RenameAction::Cancel);
    }

    // 4. Backspace removes char
    #[test]
    fn test_backspace_removes_char() {
        let mut dialog = RenameDialog::new("Rename", "abc");
        assert_eq!(dialog.cursor_pos, 3);

        dialog.handle_key("Backspace");
        assert_eq!(dialog.value, "ab");
        assert_eq!(dialog.cursor_pos, 2);

        dialog.handle_key("Backspace");
        assert_eq!(dialog.value, "a");
        assert_eq!(dialog.cursor_pos, 1);
    }

    // 5. Backspace at start does nothing
    #[test]
    fn test_backspace_at_start() {
        let mut dialog = RenameDialog::new("Rename", "");
        dialog.handle_key("Backspace");
        assert_eq!(dialog.value, "");
        assert_eq!(dialog.cursor_pos, 0);
    }

    // 6. Delete removes char at cursor
    #[test]
    fn test_delete_removes_char() {
        let mut dialog = RenameDialog::new("Rename", "abc");
        dialog.cursor_pos = 1; // cursor on 'b'
        dialog.handle_key("Delete");
        assert_eq!(dialog.value, "ac");
        assert_eq!(dialog.cursor_pos, 1);
    }

    // 7. Delete at end does nothing
    #[test]
    fn test_delete_at_end() {
        let mut dialog = RenameDialog::new("Rename", "abc");
        assert_eq!(dialog.cursor_pos, 3); // at end
        dialog.handle_key("Delete");
        assert_eq!(dialog.value, "abc");
    }

    // 8. Left/Right cursor movement
    #[test]
    fn test_cursor_left_right() {
        let mut dialog = RenameDialog::new("Rename", "abc");
        assert_eq!(dialog.cursor_pos, 3);

        dialog.handle_key("Left");
        assert_eq!(dialog.cursor_pos, 2);

        dialog.handle_key("Left");
        assert_eq!(dialog.cursor_pos, 1);

        dialog.handle_key("Right");
        assert_eq!(dialog.cursor_pos, 2);

        // Right past end stays at end
        dialog.handle_key("Right");
        dialog.handle_key("Right");
        dialog.handle_key("Right");
        assert_eq!(dialog.cursor_pos, 3);

        // Left past start stays at 0
        dialog.handle_key("Left");
        dialog.handle_key("Left");
        dialog.handle_key("Left");
        dialog.handle_key("Left");
        assert_eq!(dialog.cursor_pos, 0);
    }

    // 9. Home and End
    #[test]
    fn test_home_end() {
        let mut dialog = RenameDialog::new("Rename", "hello");
        assert_eq!(dialog.cursor_pos, 5);

        dialog.handle_key("Home");
        assert_eq!(dialog.cursor_pos, 0);

        dialog.handle_key("End");
        assert_eq!(dialog.cursor_pos, 5);
    }

    // 10. Insert at cursor position (middle of string)
    #[test]
    fn test_insert_at_middle() {
        let mut dialog = RenameDialog::new("Rename", "ac");
        dialog.cursor_pos = 1; // between 'a' and 'c'
        dialog.handle_key("b");
        assert_eq!(dialog.value, "abc");
        assert_eq!(dialog.cursor_pos, 2);
    }

    // 11. Render produces output
    #[test]
    fn test_render_produces_output() {
        let dialog = RenameDialog::new("Rename Window", "my-window");
        let output = dialog.render(80, 24);
        assert!(!output.is_empty());
        assert!(output.contains("Rename Window"));
        assert!(output.contains("> "));
        assert!(output.contains("my-window"));
        assert!(output.contains("Enter: confirm"));
        assert!(output.contains("Esc: cancel"));
    }

    // 12. Render with small terminal returns empty
    #[test]
    fn test_render_small_terminal() {
        let dialog = RenameDialog::new("Rename", "test");
        let output = dialog.render(5, 3);
        assert!(output.is_empty());
    }

    // 13. New dialog starts visible with cursor at end
    #[test]
    fn test_new_dialog_state() {
        let dialog = RenameDialog::new("Rename Session", "dev");
        assert!(dialog.visible);
        assert_eq!(dialog.title, "Rename Session");
        assert_eq!(dialog.value, "dev");
        assert_eq!(dialog.cursor_pos, 3);
    }

    // 14. Enter with empty value confirms empty string
    #[test]
    fn test_enter_empty_value() {
        let mut dialog = RenameDialog::new("Rename", "");
        let action = dialog.handle_key("Enter");
        assert_eq!(action, RenameAction::Confirm("".into()));
    }

    // 15. Control characters ignored
    #[test]
    fn test_control_chars_ignored() {
        let mut dialog = RenameDialog::new("Rename", "");
        dialog.handle_key("Ctrl+C");
        assert_eq!(dialog.value, "");
    }
}
