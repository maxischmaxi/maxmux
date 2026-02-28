/// Note editor overlay for creating and editing notes.
///
/// Provides a centered modal with a text editing area that supports
/// basic cursor movement, insertion, deletion, and line splitting.
use std::fmt::Write as FmtWrite;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum NoteEditorAction {
    None,
    Save {
        id: Option<String>,
        title: String,
        content: String,
    },
    Close,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct NoteEditor {
    pub note_id: Option<String>,
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_offset: usize,
    pub visible: bool,
}

impl NoteEditor {
    /// Create a new editor for a fresh note.
    pub fn new() -> Self {
        Self {
            note_id: None,
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            visible: true,
        }
    }

    /// Create an editor pre-populated with existing content.
    pub fn with_content(id: String, content: &str) -> Self {
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(|l| l.to_string()).collect()
        };
        // Ensure at least one line
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        Self {
            note_id: Some(id),
            lines,
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            visible: true,
        }
    }

    // -- title derivation ---------------------------------------------------

    /// Derive the note title from the first non-empty line, or "Untitled Note".
    pub fn title(&self) -> String {
        for line in &self.lines {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        "Untitled Note".to_string()
    }

    /// Join all lines into a single content string.
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    // -- key dispatch -------------------------------------------------------

    pub fn handle_key(&mut self, key: &str, visible_rows: usize) -> NoteEditorAction {
        match key {
            "Ctrl+S" | "C-s" => self.save_action(),
            "Escape" => self.save_action(),

            "Enter" => {
                self.split_line();
                self.ensure_cursor_visible(visible_rows);
                NoteEditorAction::None
            }
            "Backspace" => {
                self.backspace();
                self.ensure_cursor_visible(visible_rows);
                NoteEditorAction::None
            }
            "Delete" => {
                self.delete_char();
                NoteEditorAction::None
            }
            "Left" => {
                self.move_left();
                self.ensure_cursor_visible(visible_rows);
                NoteEditorAction::None
            }
            "Right" => {
                self.move_right();
                self.ensure_cursor_visible(visible_rows);
                NoteEditorAction::None
            }
            "Up" => {
                self.move_up();
                self.ensure_cursor_visible(visible_rows);
                NoteEditorAction::None
            }
            "Down" => {
                self.move_down();
                self.ensure_cursor_visible(visible_rows);
                NoteEditorAction::None
            }
            "Home" | "0" => {
                self.cursor_col = 0;
                NoteEditorAction::None
            }
            "End" | "$" => {
                self.cursor_col = self.current_line_len();
                NoteEditorAction::None
            }

            _ => {
                // Accept single printable characters
                if key.len() == 1 {
                    let c = key.chars().next().unwrap();
                    if !c.is_control() {
                        self.insert_char(c);
                        self.ensure_cursor_visible(visible_rows);
                    }
                }
                NoteEditorAction::None
            }
        }
    }

    // -- editing operations -------------------------------------------------

    fn insert_char(&mut self, c: char) {
        let line = &mut self.lines[self.cursor_row];
        let col = self.cursor_col.min(line.len());
        line.insert(col, c);
        self.cursor_col = col + 1;
    }

    fn split_line(&mut self) {
        let line = &self.lines[self.cursor_row];
        let col = self.cursor_col.min(line.len());
        let rest = line[col..].to_string();
        self.lines[self.cursor_row] = line[..col].to_string();
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, rest);
        self.cursor_col = 0;
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let col = self.cursor_col.min(self.lines[self.cursor_row].len());
            self.lines[self.cursor_row].remove(col - 1);
            self.cursor_col = col - 1;
        } else if self.cursor_row > 0 {
            // Join with previous line
            let current_line = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
            self.lines[self.cursor_row].push_str(&current_line);
        }
    }

    fn delete_char(&mut self) {
        let line_len = self.lines[self.cursor_row].len();
        let col = self.cursor_col.min(line_len);
        if col < line_len {
            self.lines[self.cursor_row].remove(col);
        } else if self.cursor_row + 1 < self.lines.len() {
            // Join with next line
            let next_line = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next_line);
        }
    }

    // -- cursor movement ----------------------------------------------------

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
        }
    }

    fn move_right(&mut self) {
        let line_len = self.current_line_len();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
        }
    }

    fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
        }
    }

    // -- helpers ------------------------------------------------------------

    fn current_line_len(&self) -> usize {
        self.lines[self.cursor_row].len()
    }

    fn save_action(&self) -> NoteEditorAction {
        NoteEditorAction::Save {
            id: self.note_id.clone(),
            title: self.title(),
            content: self.content(),
        }
    }

    fn ensure_cursor_visible(&mut self, visible_rows: usize) {
        if visible_rows == 0 {
            return;
        }
        if self.cursor_row < self.scroll_offset {
            self.scroll_offset = self.cursor_row;
        } else if self.cursor_row >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.cursor_row - visible_rows + 1;
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
    pub const HINT: (u8, u8, u8) = (166, 173, 200); // #a6adc8 (dim)
    pub const CURSOR_BG: (u8, u8, u8) = (137, 180, 250); // #89b4fa
    pub const CURSOR_FG: (u8, u8, u8) = (30, 30, 46); // #1e1e2e
}

impl NoteEditor {
    /// Render the note editor as a centered modal overlay.
    ///
    /// Returns an ANSI escape sequence string that draws the editor
    /// at the center of the given terminal dimensions.
    pub fn render(&self, cols: u16, rows: u16) -> String {
        let cols = cols as usize;
        let rows = rows as usize;
        if cols < 12 || rows < 8 {
            return String::new();
        }

        // Modal dimensions
        let modal_width = 70.min(cols.saturating_sub(4));
        let max_content_rows = rows.saturating_sub(8).max(3);
        // Chrome: top border + title line + separator + [content] + separator + hint + bottom border = 6 chrome lines
        let chrome_lines = 6;
        let content_rows = max_content_rows.min(self.lines.len().max(1));
        let modal_height = chrome_lines + content_rows;

        // Center the modal
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
        let hint_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::HINT.0,
            colors::HINT.1,
            colors::HINT.2
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
        let reset = "\x1b[0m";

        let goto = |r: usize, c: usize| -> String { format!("\x1b[{};{}H", r, c) };

        // -- top border -----------------------------------------------------
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

        // -- title line -----------------------------------------------------
        let title = self.title();
        let title_display: String = if title.len() > inner_width {
            format!("{}...", &title[..inner_width.saturating_sub(3)])
        } else {
            title.clone()
        };
        let title_padding = inner_width.saturating_sub(title_display.len());
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
            title_display,
            " ".repeat(title_right_pad),
            border_fg,
            "\u{2502}",
        );
        let _ = write!(out, "{}", reset);

        // -- separator below title ------------------------------------------
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}",
            goto(start_row + 2, start_col),
            bg,
            border_fg,
            "\u{251c}",
            "\u{2500}".repeat(inner_width),
            "\u{2524}",
            reset,
        );

        // -- content area ---------------------------------------------------
        let content_start_row = start_row + 3;
        for vis_i in 0..content_rows {
            let line_idx = self.scroll_offset + vis_i;
            let line_content = if line_idx < self.lines.len() {
                &self.lines[line_idx]
            } else {
                ""
            };

            let _ = write!(
                out,
                "{}{}{}{}",
                goto(content_start_row + vis_i, start_col),
                bg,
                border_fg,
                "\u{2502}",
            );

            // Render the line content character by character (for cursor highlight)
            let is_cursor_line = line_idx == self.cursor_row;
            let mut col_written = 0;

            if is_cursor_line {
                let line_chars: Vec<char> = line_content.chars().collect();
                let cursor_col = self.cursor_col.min(line_chars.len());

                // Characters before cursor
                if cursor_col > 0 && col_written < inner_width {
                    let before: String = line_chars[..cursor_col.min(inner_width)].iter().collect();
                    let _ = write!(out, "{}{}", text_fg, before);
                    col_written += before.len();
                }

                // Cursor character
                if col_written < inner_width {
                    let cursor_ch = if cursor_col < line_chars.len() {
                        line_chars[cursor_col]
                    } else {
                        ' '
                    };
                    let _ = write!(out, "{}{}{}", cursor_style, cursor_ch, reset);
                    let _ = write!(out, "{}{}", bg, text_fg);
                    col_written += 1;
                }

                // Characters after cursor
                if cursor_col < line_chars.len() && col_written < inner_width {
                    let after_start = cursor_col + 1;
                    let after_end = line_chars
                        .len()
                        .min(after_start + inner_width - col_written);
                    let after: String = line_chars[after_start..after_end].iter().collect();
                    let _ = write!(out, "{}", after);
                    col_written += after.len();
                }
            } else {
                // Non-cursor line: just render text
                let display: String = if line_content.len() > inner_width {
                    line_content[..inner_width].to_string()
                } else {
                    line_content.to_string()
                };
                let _ = write!(out, "{}{}", text_fg, display);
                col_written = display.len();
            }

            // Pad remaining width
            if col_written < inner_width {
                let _ = write!(out, "{}", " ".repeat(inner_width - col_written));
            }

            // Right border
            let _ = write!(out, "{}{}{}", border_fg, "\u{2502}", reset);
        }

        // -- separator above hint -------------------------------------------
        let hint_sep_row = content_start_row + content_rows;
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}",
            goto(hint_sep_row, start_col),
            bg,
            border_fg,
            "\u{251c}",
            "\u{2500}".repeat(inner_width),
            "\u{2524}",
            reset,
        );

        // -- hint line ------------------------------------------------------
        let hint = "Ctrl+S: save  Esc: close";
        let hint_display: String = if hint.len() > inner_width {
            hint[..inner_width].to_string()
        } else {
            hint.to_string()
        };
        let hint_padding = inner_width.saturating_sub(hint_display.len());
        let hint_left_pad = hint_padding / 2;
        let hint_right_pad = hint_padding - hint_left_pad;
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}{}{}{}",
            goto(hint_sep_row + 1, start_col),
            bg,
            border_fg,
            "\u{2502}",
            hint_fg,
            " ".repeat(hint_left_pad),
            hint_display,
            " ".repeat(hint_right_pad),
            border_fg,
            "\u{2502}",
        );
        let _ = write!(out, "{}", reset);

        // -- bottom border --------------------------------------------------
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}",
            goto(hint_sep_row + 2, start_col),
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

    // 1. New editor has one empty line
    #[test]
    fn test_new_editor_has_one_empty_line() {
        let editor = NoteEditor::new();
        assert_eq!(editor.lines.len(), 1);
        assert_eq!(editor.lines[0], "");
        assert_eq!(editor.cursor_row, 0);
        assert_eq!(editor.cursor_col, 0);
        assert!(editor.visible);
        assert!(editor.note_id.is_none());
    }

    // 2. Insert character
    #[test]
    fn test_insert_character() {
        let mut editor = NoteEditor::new();
        editor.handle_key("H", 10);
        editor.handle_key("i", 10);
        assert_eq!(editor.lines[0], "Hi");
        assert_eq!(editor.cursor_col, 2);
    }

    // 3. Enter creates new line
    #[test]
    fn test_enter_creates_new_line() {
        let mut editor = NoteEditor::new();
        // Type "Hello"
        for ch in "Hello".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        assert_eq!(editor.lines[0], "Hello");

        // Press Enter
        editor.handle_key("Enter", 10);
        assert_eq!(editor.lines.len(), 2);
        assert_eq!(editor.lines[0], "Hello");
        assert_eq!(editor.lines[1], "");
        assert_eq!(editor.cursor_row, 1);
        assert_eq!(editor.cursor_col, 0);
    }

    // 4. Backspace at col 0 joins lines
    #[test]
    fn test_backspace_at_col_0_joins_lines() {
        let mut editor = NoteEditor::new();
        // Type "Hello", Enter, "World"
        for ch in "Hello".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        editor.handle_key("Enter", 10);
        for ch in "World".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        assert_eq!(editor.lines.len(), 2);
        assert_eq!(editor.lines[0], "Hello");
        assert_eq!(editor.lines[1], "World");

        // Move to beginning of line 1
        editor.cursor_col = 0;

        // Backspace should join with previous line
        editor.handle_key("Backspace", 10);
        assert_eq!(editor.lines.len(), 1);
        assert_eq!(editor.lines[0], "HelloWorld");
        assert_eq!(editor.cursor_row, 0);
        assert_eq!(editor.cursor_col, 5); // after "Hello"
    }

    // 5. Ctrl+S returns Save action
    #[test]
    fn test_ctrl_s_returns_save() {
        let mut editor = NoteEditor::new();
        for ch in "My Note".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        let action = editor.handle_key("Ctrl+S", 10);
        match action {
            NoteEditorAction::Save { id, title, content } => {
                assert!(id.is_none());
                assert_eq!(title, "My Note");
                assert_eq!(content, "My Note");
            }
            _ => panic!("Expected Save action"),
        }
    }

    // 6. Escape returns Save action
    #[test]
    fn test_escape_returns_save() {
        let mut editor = NoteEditor::new();
        for ch in "Test".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        let action = editor.handle_key("Escape", 10);
        match action {
            NoteEditorAction::Save { id, title, content } => {
                assert!(id.is_none());
                assert_eq!(title, "Test");
                assert_eq!(content, "Test");
            }
            _ => panic!("Expected Save action"),
        }
    }

    // 7. Title derivation - first non-empty line
    #[test]
    fn test_title_derivation() {
        let mut editor = NoteEditor::new();
        // First line empty, second has content
        editor.handle_key("Enter", 10);
        for ch in "Real Title".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        assert_eq!(editor.title(), "Real Title");
    }

    // 8. Title derivation - untitled when all empty
    #[test]
    fn test_title_untitled_when_empty() {
        let editor = NoteEditor::new();
        assert_eq!(editor.title(), "Untitled Note");
    }

    // 9. Enter splits line at cursor
    #[test]
    fn test_enter_splits_line_at_cursor() {
        let mut editor = NoteEditor::new();
        for ch in "HelloWorld".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        // Move cursor to position 5 (between Hello and World)
        editor.cursor_col = 5;
        editor.handle_key("Enter", 10);
        assert_eq!(editor.lines.len(), 2);
        assert_eq!(editor.lines[0], "Hello");
        assert_eq!(editor.lines[1], "World");
        assert_eq!(editor.cursor_row, 1);
        assert_eq!(editor.cursor_col, 0);
    }

    // 10. Delete joins with next line at end
    #[test]
    fn test_delete_joins_next_line() {
        let mut editor = NoteEditor::new();
        for ch in "Hello".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        editor.handle_key("Enter", 10);
        for ch in "World".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        // Go to end of first line
        editor.cursor_row = 0;
        editor.cursor_col = 5;

        editor.handle_key("Delete", 10);
        assert_eq!(editor.lines.len(), 1);
        assert_eq!(editor.lines[0], "HelloWorld");
    }

    // 11. Up/Down navigation
    #[test]
    fn test_up_down_navigation() {
        let mut editor = NoteEditor::new();
        for ch in "Line 1".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        editor.handle_key("Enter", 10);
        for ch in "Line 2".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        editor.handle_key("Enter", 10);
        for ch in "Line 3".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        assert_eq!(editor.cursor_row, 2);

        editor.handle_key("Up", 10);
        assert_eq!(editor.cursor_row, 1);

        editor.handle_key("Up", 10);
        assert_eq!(editor.cursor_row, 0);

        // Up at top stays at top
        editor.handle_key("Up", 10);
        assert_eq!(editor.cursor_row, 0);

        editor.handle_key("Down", 10);
        assert_eq!(editor.cursor_row, 1);
    }

    // 12. Left/Right navigation wraps
    #[test]
    fn test_left_right_wraps() {
        let mut editor = NoteEditor::new();
        for ch in "AB".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        editor.handle_key("Enter", 10);
        for ch in "CD".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        // cursor_row = 1, cursor_col = 2

        // Move left to beginning of line 1
        editor.handle_key("Left", 10);
        editor.handle_key("Left", 10);
        assert_eq!(editor.cursor_row, 1);
        assert_eq!(editor.cursor_col, 0);

        // Left again wraps to end of line 0
        editor.handle_key("Left", 10);
        assert_eq!(editor.cursor_row, 0);
        assert_eq!(editor.cursor_col, 2);

        // Move right past end of line 0 wraps to beginning of line 1
        editor.handle_key("Right", 10);
        assert_eq!(editor.cursor_row, 1);
        assert_eq!(editor.cursor_col, 0);
    }

    // 13. Home and End keys
    #[test]
    fn test_home_end_keys() {
        let mut editor = NoteEditor::new();
        for ch in "Hello".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        assert_eq!(editor.cursor_col, 5);

        editor.handle_key("Home", 10);
        assert_eq!(editor.cursor_col, 0);

        editor.handle_key("End", 10);
        assert_eq!(editor.cursor_col, 5);
    }

    // 14. With_content pre-populates
    #[test]
    fn test_with_content() {
        let editor = NoteEditor::with_content("note-1".into(), "Hello\nWorld");
        assert_eq!(editor.note_id, Some("note-1".into()));
        assert_eq!(editor.lines.len(), 2);
        assert_eq!(editor.lines[0], "Hello");
        assert_eq!(editor.lines[1], "World");
    }

    // 15. Backspace deletes character within line
    #[test]
    fn test_backspace_within_line() {
        let mut editor = NoteEditor::new();
        for ch in "ABC".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        assert_eq!(editor.lines[0], "ABC");

        editor.handle_key("Backspace", 10);
        assert_eq!(editor.lines[0], "AB");
        assert_eq!(editor.cursor_col, 2);
    }

    // 16. Render produces non-empty output
    #[test]
    fn test_render_produces_output() {
        let mut editor = NoteEditor::new();
        for ch in "Hello World".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        let output = editor.render(80, 24);
        assert!(!output.is_empty());
        assert!(output.contains("Hello World"));
        assert!(output.contains("Ctrl+S: save"));
    }

    // 17. Render with small terminal returns empty
    #[test]
    fn test_render_small_terminal() {
        let editor = NoteEditor::new();
        let output = editor.render(5, 3);
        assert!(output.is_empty());
    }

    // 18. Scroll offset adjusts for cursor visibility
    #[test]
    fn test_scroll_offset_adjusts() {
        let mut editor = NoteEditor::new();
        // Create many lines
        for i in 0..20 {
            for ch in format!("Line {}", i).chars() {
                editor.handle_key(&ch.to_string(), 5);
            }
            if i < 19 {
                editor.handle_key("Enter", 5);
            }
        }
        // cursor_row should be 19, scroll_offset should keep cursor visible
        assert_eq!(editor.cursor_row, 19);
        assert!(editor.scroll_offset > 0);
        // Cursor should be within visible area
        assert!(editor.cursor_row >= editor.scroll_offset);
        assert!(editor.cursor_row < editor.scroll_offset + 5);
    }

    // 19. Save action with existing note_id
    #[test]
    fn test_save_with_existing_id() {
        let mut editor = NoteEditor::with_content("existing-id".into(), "Content");
        let action = editor.handle_key("Ctrl+S", 10);
        match action {
            NoteEditorAction::Save { id, title, content } => {
                assert_eq!(id, Some("existing-id".into()));
                assert_eq!(title, "Content");
                assert_eq!(content, "Content");
            }
            _ => panic!("Expected Save action"),
        }
    }

    // 20. Delete character within line
    #[test]
    fn test_delete_within_line() {
        let mut editor = NoteEditor::new();
        for ch in "ABC".chars() {
            editor.handle_key(&ch.to_string(), 10);
        }
        editor.cursor_col = 1;

        editor.handle_key("Delete", 10);
        assert_eq!(editor.lines[0], "AC");
    }
}
