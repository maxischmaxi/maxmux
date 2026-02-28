/// Command palette overlay with fuzzy search over registered commands.
///
/// Provides a centered modal dialog where users can type a query to
/// filter through available commands and execute the selected one.
use std::fmt::Write as FmtWrite;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandPaletteAction {
    None,
    Execute(String), // command ID
    Close,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct CommandPalette {
    pub query: String,
    pub selected_index: usize,
    pub commands: Vec<CommandEntry>,
    pub filtered: Vec<usize>, // indices into commands
    pub visible: bool,
}

impl CommandPalette {
    pub fn new(commands: Vec<CommandEntry>) -> Self {
        let filtered: Vec<usize> = (0..commands.len()).collect();
        Self {
            query: String::new(),
            selected_index: 0,
            commands,
            filtered,
            visible: true,
        }
    }

    // -- key dispatch -------------------------------------------------------

    pub fn handle_key(&mut self, key: &str) -> CommandPaletteAction {
        match key {
            "Escape" => CommandPaletteAction::Close,
            "Enter" => {
                if let Some(&idx) = self.filtered.get(self.selected_index) {
                    CommandPaletteAction::Execute(self.commands[idx].id.clone())
                } else {
                    CommandPaletteAction::None
                }
            }
            "Up" => self.move_selection(-1),
            "Down" => self.move_selection(1),
            "Backspace" => {
                self.query.pop();
                self.filter();
                CommandPaletteAction::None
            }
            ch => {
                // Accept single printable characters
                if ch.len() == 1 {
                    let c = ch.chars().next().unwrap();
                    if !c.is_control() {
                        self.query.push(c);
                        self.filter();
                    }
                }
                CommandPaletteAction::None
            }
        }
    }

    // -- filtering ----------------------------------------------------------

    fn filter(&mut self) {
        let query_lower = self.query.to_lowercase();
        self.filtered = self
            .commands
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                if query_lower.is_empty() {
                    return true;
                }
                let haystack = format!("{} {}", entry.id, entry.description).to_lowercase();
                haystack.contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect();

        // Clamp selected index to the new filtered list length
        if self.filtered.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered.len() {
            self.selected_index = self.filtered.len() - 1;
        }
    }

    // -- selection movement -------------------------------------------------

    fn move_selection(&mut self, delta: i32) -> CommandPaletteAction {
        if self.filtered.is_empty() {
            return CommandPaletteAction::None;
        }
        let len = self.filtered.len() as i32;
        let new_idx = (self.selected_index as i32 + delta).rem_euclid(len);
        self.selected_index = new_idx as usize;
        CommandPaletteAction::None
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
    pub const SELECTED_BG: (u8, u8, u8) = (49, 50, 68); // #313244
    pub const SELECTED_FG: (u8, u8, u8) = (137, 180, 250); // #89b4fa
    pub const CMD_ID: (u8, u8, u8) = (203, 166, 247); // #cba6f7 (purple)
    pub const DESC: (u8, u8, u8) = (166, 173, 200); // #a6adc8 (dim)
}

impl CommandPalette {
    /// Render the command palette as a centered modal overlay.
    ///
    /// Returns an ANSI escape sequence string that draws the palette
    /// at the center of the given terminal dimensions.
    pub fn render(&self, cols: u16, rows: u16) -> String {
        let cols = cols as usize;
        let rows = rows as usize;
        if cols < 10 || rows < 6 {
            return String::new();
        }

        // Modal dimensions: width = min(60, cols-4), height depends on items
        let modal_width = 60.min(cols.saturating_sub(4));
        let max_visible_items = 10.min(rows.saturating_sub(8));
        // Fixed chrome: top border + title + query + separator + bottom border = 5
        let chrome_lines = 5;
        let item_count = self.filtered.len().min(max_visible_items);
        let modal_height = chrome_lines + item_count.max(1); // at least 1 row for "no results"

        // Center the modal
        let start_col = (cols.saturating_sub(modal_width)) / 2 + 1;
        let start_row = (rows.saturating_sub(modal_height)) / 2 + 1;

        let inner_width = modal_width.saturating_sub(2); // inside the left/right borders

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
        let sel_bg = format!(
            "\x1b[48;2;{};{};{}m",
            colors::SELECTED_BG.0,
            colors::SELECTED_BG.1,
            colors::SELECTED_BG.2
        );
        let sel_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::SELECTED_FG.0,
            colors::SELECTED_FG.1,
            colors::SELECTED_FG.2
        );
        let cmd_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::CMD_ID.0,
            colors::CMD_ID.1,
            colors::CMD_ID.2
        );
        let desc_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::DESC.0,
            colors::DESC.1,
            colors::DESC.2
        );
        let reset = "\x1b[0m";

        // Helper: move cursor to (row, col) -- 1-based
        let goto = |r: usize, c: usize| -> String { format!("\x1b[{};{}H", r, c) };

        // -- top border: rounded corners ------------------------------------
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}",
            goto(start_row, start_col),
            bg,
            border_fg,
            "\u{256d}", // rounded top-left
            "\u{2500}".repeat(inner_width),
            "\u{256e}", // rounded top-right
            reset,
        );

        // -- title line -----------------------------------------------------
        let title = "Command Palette";
        let title_padding = inner_width.saturating_sub(title.len());
        let title_left_pad = title_padding / 2;
        let title_right_pad = title_padding - title_left_pad;
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}{}{}",
            goto(start_row + 1, start_col),
            bg,
            border_fg,
            "\u{2502}", // left border
            title_fg,
            " ".repeat(title_left_pad),
            title,
            " ".repeat(title_right_pad),
            reset,
        );
        // right border
        let _ = write!(out, "{}{}{}{}{}", bg, border_fg, "\u{2502}", reset, "",);

        // -- query line: "> {query}_" ----------------------------------------
        let cursor_char = "_";
        let query_display = format!("> {}{}", self.query, cursor_char);
        let query_visible: String = if query_display.len() > inner_width {
            query_display[..inner_width].to_string()
        } else {
            let padding = inner_width - query_display.len();
            format!("{}{}", query_display, " ".repeat(padding))
        };
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}{}",
            goto(start_row + 2, start_col),
            bg,
            border_fg,
            "\u{2502}",
            text_fg,
            query_visible,
            border_fg,
            "\u{2502}",
        );
        let _ = write!(out, "{}", reset);

        // -- separator -------------------------------------------------------
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}",
            goto(start_row + 3, start_col),
            bg,
            border_fg,
            "\u{251c}", // left tee
            "\u{2500}".repeat(inner_width),
            "\u{2524}", // right tee
            reset,
        );

        // -- command list ----------------------------------------------------
        let list_start_row = start_row + 4;
        if self.filtered.is_empty() {
            // "No results" placeholder
            let msg = "No matching commands";
            let msg_visible: String = if msg.len() > inner_width {
                msg[..inner_width].to_string()
            } else {
                let padding = inner_width - msg.len();
                format!("{}{}", msg, " ".repeat(padding))
            };
            let _ = write!(
                out,
                "{}{}{}{}{}{}{}{}{}",
                goto(list_start_row, start_col),
                bg,
                border_fg,
                "\u{2502}",
                desc_fg,
                msg_visible,
                border_fg,
                "\u{2502}",
                reset,
            );
        } else {
            for (vis_i, &cmd_idx) in self.filtered.iter().take(max_visible_items).enumerate() {
                let entry = &self.commands[cmd_idx];
                let is_selected = vis_i == self.selected_index;

                let row_bg = if is_selected { &sel_bg } else { &bg };
                let id_color = if is_selected { &sel_fg } else { &cmd_fg };
                let desc_color = if is_selected { &sel_fg } else { &desc_fg };

                // Format: "  {id}  {description}"
                let prefix = if is_selected { "> " } else { "  " };
                let spacer = "  ";
                let prefix_len = prefix.len();
                let id_len = entry.id.len();
                let spacer_len = spacer.len();
                let avail_for_desc = inner_width.saturating_sub(prefix_len + id_len + spacer_len);
                let desc_truncated: String = if entry.description.len() > avail_for_desc {
                    entry.description[..avail_for_desc].to_string()
                } else {
                    entry.description.clone()
                };
                let trailing_pad = inner_width
                    .saturating_sub(prefix_len + id_len + spacer_len + desc_truncated.len());

                let _ = write!(
                    out,
                    "{}{}{}{}{}{}{}{}{}{}{}{}",
                    goto(list_start_row + vis_i, start_col),
                    row_bg,
                    border_fg,
                    "\u{2502}",
                    id_color,
                    prefix,
                    &entry.id,
                    desc_color,
                    spacer,
                    desc_truncated,
                    " ".repeat(trailing_pad),
                    reset,
                );
                // right border
                let _ = write!(out, "{}{}{}{}", row_bg, border_fg, "\u{2502}", reset,);
            }
        }

        // -- bottom border ---------------------------------------------------
        let bottom_row = list_start_row + item_count.max(1);
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}",
            goto(bottom_row, start_col),
            bg,
            border_fg,
            "\u{2570}", // rounded bottom-left
            "\u{2500}".repeat(inner_width),
            "\u{256f}", // rounded bottom-right
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

    fn sample_commands() -> Vec<CommandEntry> {
        vec![
            CommandEntry {
                id: "pane:split-horizontal".into(),
                description: "Split the current pane horizontally".into(),
            },
            CommandEntry {
                id: "pane:split-vertical".into(),
                description: "Split the current pane vertically".into(),
            },
            CommandEntry {
                id: "window:create".into(),
                description: "Create a new window".into(),
            },
            CommandEntry {
                id: "window:close".into(),
                description: "Close the current window".into(),
            },
            CommandEntry {
                id: "session:detach".into(),
                description: "Detach from the current session".into(),
            },
        ]
    }

    // 1. New palette with commands - all commands visible and filtered
    #[test]
    fn test_new_palette_with_commands() {
        let cmds = sample_commands();
        let palette = CommandPalette::new(cmds.clone());

        assert!(palette.visible);
        assert_eq!(palette.query, "");
        assert_eq!(palette.selected_index, 0);
        assert_eq!(palette.commands.len(), 5);
        assert_eq!(palette.filtered.len(), 5);
        // All indices present in order
        for (i, &idx) in palette.filtered.iter().enumerate() {
            assert_eq!(i, idx);
        }
    }

    // 2. Filter narrows results
    #[test]
    fn test_filter_narrows_results() {
        let mut palette = CommandPalette::new(sample_commands());
        assert_eq!(palette.filtered.len(), 5);

        // Type "split" -- should match the two pane:split-* commands
        for ch in "split".chars() {
            palette.handle_key(&ch.to_string());
        }
        assert_eq!(palette.query, "split");
        assert_eq!(palette.filtered.len(), 2);
        // Both should be pane:split-* entries (indices 0 and 1)
        assert!(palette.filtered.contains(&0));
        assert!(palette.filtered.contains(&1));
    }

    // 3. Clear filter shows all commands
    #[test]
    fn test_clear_filter_shows_all() {
        let mut palette = CommandPalette::new(sample_commands());

        // Type and then delete all chars
        for ch in "xyz".chars() {
            palette.handle_key(&ch.to_string());
        }
        assert_eq!(palette.query, "xyz");
        assert_eq!(palette.filtered.len(), 0); // no match

        // Backspace three times
        palette.handle_key("Backspace");
        palette.handle_key("Backspace");
        palette.handle_key("Backspace");
        assert_eq!(palette.query, "");
        assert_eq!(palette.filtered.len(), 5); // all restored
    }

    // 4. Select (Enter) returns command ID
    #[test]
    fn test_select_returns_command_id() {
        let mut palette = CommandPalette::new(sample_commands());

        // Default selection is index 0 -> "pane:split-horizontal"
        let action = palette.handle_key("Enter");
        assert_eq!(
            action,
            CommandPaletteAction::Execute("pane:split-horizontal".into())
        );
    }

    // 5. Move selection wraps around
    #[test]
    fn test_move_selection_wraps() {
        let mut palette = CommandPalette::new(sample_commands());
        assert_eq!(palette.selected_index, 0);

        // Down four times -> index 4
        for _ in 0..4 {
            palette.handle_key("Down");
        }
        assert_eq!(palette.selected_index, 4);

        // Down once more -> wraps to 0
        palette.handle_key("Down");
        assert_eq!(palette.selected_index, 0);

        // Up from 0 -> wraps to 4
        palette.handle_key("Up");
        assert_eq!(palette.selected_index, 4);
    }

    // 6. Escape returns Close
    #[test]
    fn test_escape_returns_close() {
        let mut palette = CommandPalette::new(sample_commands());
        let action = palette.handle_key("Escape");
        assert_eq!(action, CommandPaletteAction::Close);
    }

    // 7. Backspace removes last character
    #[test]
    fn test_backspace_removes_char() {
        let mut palette = CommandPalette::new(sample_commands());

        palette.handle_key("a");
        palette.handle_key("b");
        palette.handle_key("c");
        assert_eq!(palette.query, "abc");

        palette.handle_key("Backspace");
        assert_eq!(palette.query, "ab");

        palette.handle_key("Backspace");
        assert_eq!(palette.query, "a");

        palette.handle_key("Backspace");
        assert_eq!(palette.query, "");

        // Backspace on empty query is harmless
        palette.handle_key("Backspace");
        assert_eq!(palette.query, "");
    }

    // 8. Render produces non-empty output
    #[test]
    fn test_render_produces_output() {
        let palette = CommandPalette::new(sample_commands());
        let output = palette.render(80, 24);
        assert!(!output.is_empty());
        // Should contain the title
        assert!(output.contains("Command Palette"));
        // Should contain the query prompt
        assert!(output.contains("> "));
        // Should contain at least one command id
        assert!(output.contains("pane:split-horizontal"));
    }

    // -- Additional tests ---------------------------------------------------

    // 9. Filter is case-insensitive
    #[test]
    fn test_filter_case_insensitive() {
        let mut palette = CommandPalette::new(sample_commands());

        for ch in "WINDOW".chars() {
            palette.handle_key(&ch.to_string());
        }
        assert_eq!(palette.filtered.len(), 2); // window:create and window:close
    }

    // 10. Enter with empty filtered list returns None
    #[test]
    fn test_enter_empty_filtered_returns_none() {
        let mut palette = CommandPalette::new(sample_commands());

        // Type something that matches nothing
        for ch in "zzzzz".chars() {
            palette.handle_key(&ch.to_string());
        }
        assert!(palette.filtered.is_empty());

        let action = palette.handle_key("Enter");
        assert_eq!(action, CommandPaletteAction::None);
    }

    // 11. Selected index clamped after filter narrows
    #[test]
    fn test_selected_index_clamped_after_filter() {
        let mut palette = CommandPalette::new(sample_commands());

        // Move to index 4
        for _ in 0..4 {
            palette.handle_key("Down");
        }
        assert_eq!(palette.selected_index, 4);

        // Filter to only 2 results -> index should clamp to 1
        for ch in "split".chars() {
            palette.handle_key(&ch.to_string());
        }
        assert_eq!(palette.filtered.len(), 2);
        assert!(palette.selected_index < palette.filtered.len());
    }

    // 12. Move selection on empty list does nothing
    #[test]
    fn test_move_selection_empty_list() {
        let mut palette = CommandPalette::new(sample_commands());

        // Filter to nothing
        for ch in "zzz".chars() {
            palette.handle_key(&ch.to_string());
        }
        assert!(palette.filtered.is_empty());

        let action = palette.handle_key("Down");
        assert_eq!(action, CommandPaletteAction::None);

        let action = palette.handle_key("Up");
        assert_eq!(action, CommandPaletteAction::None);
    }

    // 13. Filter matches description text
    #[test]
    fn test_filter_matches_description() {
        let mut palette = CommandPalette::new(sample_commands());

        // "detach" appears in the description of session:detach
        for ch in "detach".chars() {
            palette.handle_key(&ch.to_string());
        }
        assert_eq!(palette.filtered.len(), 1);
        assert_eq!(palette.commands[palette.filtered[0]].id, "session:detach");
    }

    // 14. Render with small terminal returns empty
    #[test]
    fn test_render_small_terminal_returns_empty() {
        let palette = CommandPalette::new(sample_commands());
        let output = palette.render(5, 3);
        assert!(output.is_empty());
    }

    // 15. Down then Enter selects correct command
    #[test]
    fn test_navigate_and_select() {
        let mut palette = CommandPalette::new(sample_commands());

        // Move down twice -> index 2 -> "window:create"
        palette.handle_key("Down");
        palette.handle_key("Down");
        assert_eq!(palette.selected_index, 2);

        let action = palette.handle_key("Enter");
        assert_eq!(
            action,
            CommandPaletteAction::Execute("window:create".into())
        );
    }

    // 16. New palette with empty commands list
    #[test]
    fn test_new_palette_empty_commands() {
        let palette = CommandPalette::new(vec![]);
        assert!(palette.visible);
        assert!(palette.commands.is_empty());
        assert!(palette.filtered.is_empty());
        assert_eq!(palette.selected_index, 0);

        // Render should still produce output (with "No matching commands")
        let output = palette.render(80, 24);
        assert!(output.contains("No matching commands"));
    }

    // 17. Control characters are not appended to query
    #[test]
    fn test_control_chars_ignored() {
        let mut palette = CommandPalette::new(sample_commands());
        // Multi-char key strings are not single printable chars and should be ignored
        palette.handle_key("Ctrl+C");
        assert_eq!(palette.query, "");
    }

    // 18. Render includes selected indicator
    #[test]
    fn test_render_selected_indicator() {
        let mut palette = CommandPalette::new(sample_commands());
        // Move down to select "pane:split-vertical"
        palette.handle_key("Down");

        let output = palette.render(80, 24);
        // The selected line should have the "> " prefix
        assert!(output.contains("> pane:split-vertical"));
    }
}
