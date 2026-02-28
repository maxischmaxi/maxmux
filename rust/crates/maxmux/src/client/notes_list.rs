/// Notes list overlay for browsing, filtering, and managing notes.
///
/// Presents a centered modal with a query input, a filterable list
/// of notes, and options to open, delete, or create notes.
use std::fmt::Write as FmtWrite;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NotesListEntry {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotesListAction {
    None,
    Open(String),   // note ID to open
    Delete(String), // note ID to delete
    Create,         // create new note
    Close,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct NotesList {
    pub query: String,
    pub selected_index: usize,
    pub entries: Vec<NotesListEntry>,
    pub filtered: Vec<usize>, // indices into entries
    pub confirm_delete: bool,
    #[allow(dead_code)]
    pub visible: bool,
}

impl NotesList {
    pub fn new(entries: Vec<NotesListEntry>) -> Self {
        let filtered: Vec<usize> = (0..entries.len()).collect();
        Self {
            query: String::new(),
            selected_index: 0,
            entries,
            filtered,
            confirm_delete: false,
            visible: true,
        }
    }

    // -- key dispatch -------------------------------------------------------

    pub fn handle_key(&mut self, key: &str) -> NotesListAction {
        if self.confirm_delete {
            return self.handle_confirm_delete_key(key);
        }
        self.handle_normal_key(key)
    }

    // -- confirm delete mode ------------------------------------------------

    fn handle_confirm_delete_key(&mut self, key: &str) -> NotesListAction {
        match key {
            "y" => {
                self.confirm_delete = false;
                if let Some(&idx) = self.filtered.get(self.selected_index) {
                    let id = self.entries[idx].id.clone();
                    NotesListAction::Delete(id)
                } else {
                    NotesListAction::None
                }
            }
            "n" | "Escape" => {
                self.confirm_delete = false;
                NotesListAction::None
            }
            _ => NotesListAction::None,
        }
    }

    // -- normal mode --------------------------------------------------------

    fn handle_normal_key(&mut self, key: &str) -> NotesListAction {
        match key {
            "Escape" => NotesListAction::Close,

            "Enter" => {
                if let Some(&idx) = self.filtered.get(self.selected_index) {
                    NotesListAction::Open(self.entries[idx].id.clone())
                } else {
                    NotesListAction::None
                }
            }

            "Up" | "k" => self.move_selection(-1),
            "Down" | "j" => self.move_selection(1),

            "d" => {
                if !self.filtered.is_empty() {
                    self.confirm_delete = true;
                }
                NotesListAction::None
            }

            "Ctrl+N" | "C-n" => NotesListAction::Create,

            "Backspace" => {
                self.query.pop();
                self.filter();
                NotesListAction::None
            }

            _ => {
                // "n" creates a new note only when query is empty (dedicated shortcut);
                // otherwise treat as query character
                if key == "n" && self.query.is_empty() {
                    return NotesListAction::Create;
                }

                // Accept single printable characters as query input
                if key.len() == 1 {
                    let c = key.chars().next().unwrap();
                    if !c.is_control() {
                        self.query.push(c);
                        self.filter();
                    }
                }
                NotesListAction::None
            }
        }
    }

    // -- filtering ----------------------------------------------------------

    fn filter(&mut self) {
        let query_lower = self.query.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                if query_lower.is_empty() {
                    return true;
                }
                entry.title.to_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect();

        // Clamp selected index
        if self.filtered.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered.len() {
            self.selected_index = self.filtered.len() - 1;
        }
    }

    // -- selection movement -------------------------------------------------

    fn move_selection(&mut self, delta: i32) -> NotesListAction {
        if self.filtered.is_empty() {
            return NotesListAction::None;
        }
        let len = self.filtered.len() as i32;
        let new_idx = (self.selected_index as i32 + delta).rem_euclid(len);
        self.selected_index = new_idx as usize;
        NotesListAction::None
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
    pub const HINT: (u8, u8, u8) = (166, 173, 200); // #a6adc8 (dim)
    pub const DATE: (u8, u8, u8) = (166, 173, 200); // #a6adc8
    pub const DELETE_FG: (u8, u8, u8) = (243, 139, 168); // #f38ba8 (red)
}

impl NotesList {
    /// Format a timestamp as a human-readable date string.
    fn format_date(timestamp: u64) -> String {
        // Simple formatting: show as relative or absolute
        // For simplicity, show as "YYYY-MM-DD HH:MM" derived from epoch
        let secs = timestamp;
        let days = secs / 86400;
        let years = 1970 + days / 365;
        let remaining_days = days % 365;
        let months = remaining_days / 30 + 1;
        let day = remaining_days % 30 + 1;
        let hours = (secs % 86400) / 3600;
        let minutes = (secs % 3600) / 60;
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            years, months, day, hours, minutes
        )
    }

    /// Render the notes list as a centered modal overlay.
    ///
    /// Returns an ANSI escape sequence string.
    pub fn render(&self, cols: u16, rows: u16) -> String {
        let cols = cols as usize;
        let rows = rows as usize;
        if cols < 12 || rows < 8 {
            return String::new();
        }

        // Modal dimensions
        let modal_width = 60.min(cols.saturating_sub(4));
        let max_visible_items = 10.min(rows.saturating_sub(8));
        // Chrome: top border + title + query + separator + [items] + separator + hint + bottom border = 7
        let chrome_lines = 7;
        let item_count = self.filtered.len().min(max_visible_items);
        let display_rows = item_count.max(1); // at least 1 row for "No notes"
        let modal_height = chrome_lines + display_rows;

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
        let hint_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::HINT.0,
            colors::HINT.1,
            colors::HINT.2
        );
        let date_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::DATE.0,
            colors::DATE.1,
            colors::DATE.2
        );
        let delete_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::DELETE_FG.0,
            colors::DELETE_FG.1,
            colors::DELETE_FG.2
        );
        let reset = "\x1b[0m";

        let goto = |r: usize, c: usize| -> String { format!("\x1b[{};{}H", r, c) };

        // -- top border -----------------------------------------------------
        let _ = write!(
            out,
            "{}{}{}\u{256d}{}\u{256e}{}",
            goto(start_row, start_col),
            bg,
            border_fg,
            "\u{2500}".repeat(inner_width),
            reset,
        );

        // -- title line: "Notes" --------------------------------------------
        let title = "Notes";
        let title_padding = inner_width.saturating_sub(title.len());
        let title_left_pad = title_padding / 2;
        let title_right_pad = title_padding - title_left_pad;
        let _ = write!(
            out,
            "{}{}{}\u{2502}{}{}{}{}{}\u{2502}",
            goto(start_row + 1, start_col),
            bg,
            border_fg,
            title_fg,
            " ".repeat(title_left_pad),
            title,
            " ".repeat(title_right_pad),
            border_fg,
        );
        let _ = write!(out, "{}", reset);

        // -- query line: "> {query}_" ---------------------------------------
        let query_display = format!("> {}_", self.query);
        let query_visible: String = if query_display.len() > inner_width {
            query_display[..inner_width].to_string()
        } else {
            let padding = inner_width - query_display.len();
            format!("{}{}", query_display, " ".repeat(padding))
        };
        let _ = write!(
            out,
            "{}{}{}\u{2502}{}{}{}\u{2502}",
            goto(start_row + 2, start_col),
            bg,
            border_fg,
            text_fg,
            query_visible,
            border_fg,
        );
        let _ = write!(out, "{}", reset);

        // -- separator below query ------------------------------------------
        let _ = write!(
            out,
            "{}{}{}\u{251c}{}\u{2524}{}",
            goto(start_row + 3, start_col),
            bg,
            border_fg,
            "\u{2500}".repeat(inner_width),
            reset,
        );

        // -- notes list -----------------------------------------------------
        let list_start_row = start_row + 4;
        if self.filtered.is_empty() {
            let msg = if self.entries.is_empty() {
                "No notes yet"
            } else {
                "No matching notes"
            };
            let msg_padding = inner_width.saturating_sub(msg.len());
            let msg_left = msg_padding / 2;
            let msg_right = msg_padding - msg_left;
            let _ = write!(
                out,
                "{}{}{}\u{2502}{}{}{}{}{}\u{2502}",
                goto(list_start_row, start_col),
                bg,
                border_fg,
                hint_fg,
                " ".repeat(msg_left),
                msg,
                " ".repeat(msg_right),
                border_fg,
            );
            let _ = write!(out, "{}", reset);
        } else {
            for (vis_i, &entry_idx) in self.filtered.iter().take(max_visible_items).enumerate() {
                let entry = &self.entries[entry_idx];
                let is_selected = vis_i == self.selected_index;

                let row_bg = if is_selected { &sel_bg } else { &bg };
                let row_fg = if is_selected { &sel_fg } else { &text_fg };
                let row_date_fg = if is_selected { &sel_fg } else { &date_fg };

                // Format date
                let date_str = Self::format_date(entry.updated_at);
                let date_display_len = date_str.len();

                // Format: "  {title}  {date}"
                let prefix = if is_selected { "> " } else { "  " };
                let spacer = "  ";
                let avail_for_title =
                    inner_width.saturating_sub(prefix.len() + spacer.len() + date_display_len);
                let title_truncated = if entry.title.len() > avail_for_title {
                    if avail_for_title > 3 {
                        format!("{}...", &entry.title[..avail_for_title - 3])
                    } else {
                        entry.title[..avail_for_title].to_string()
                    }
                } else {
                    entry.title.clone()
                };
                let trailing_pad = avail_for_title.saturating_sub(title_truncated.len());

                let _ = write!(
                    out,
                    "{}{}{}\u{2502}{}{}{}{}{}{}{}",
                    goto(list_start_row + vis_i, start_col),
                    row_bg,
                    border_fg,
                    row_fg,
                    prefix,
                    title_truncated,
                    " ".repeat(trailing_pad),
                    row_date_fg,
                    spacer,
                    date_str,
                );
                // right border
                let _ = write!(out, "{}{}\u{2502}{}", row_bg, border_fg, reset,);
            }
        }

        // -- separator above hint -------------------------------------------
        let hint_sep_row = list_start_row + display_rows;
        let _ = write!(
            out,
            "{}{}{}\u{251c}{}\u{2524}{}",
            goto(hint_sep_row, start_col),
            bg,
            border_fg,
            "\u{2500}".repeat(inner_width),
            reset,
        );

        // -- hint / confirm delete line ------------------------------------
        let hint_row = hint_sep_row + 1;
        if self.confirm_delete {
            let msg = "Delete this note? y/n";
            let msg_padding = inner_width.saturating_sub(msg.len());
            let msg_left = msg_padding / 2;
            let msg_right = msg_padding - msg_left;
            let _ = write!(
                out,
                "{}{}{}\u{2502}{}{}{}{}{}\u{2502}",
                goto(hint_row, start_col),
                bg,
                border_fg,
                delete_fg,
                " ".repeat(msg_left),
                msg,
                " ".repeat(msg_right),
                border_fg,
            );
            let _ = write!(out, "{}", reset);
        } else {
            let hint = "Enter:open  d:delete  n:new  Esc:close";
            let hint_display = if hint.len() > inner_width {
                hint[..inner_width].to_string()
            } else {
                hint.to_string()
            };
            let hint_padding = inner_width.saturating_sub(hint_display.len());
            let hint_left = hint_padding / 2;
            let hint_right = hint_padding - hint_left;
            let _ = write!(
                out,
                "{}{}{}\u{2502}{}{}{}{}{}\u{2502}",
                goto(hint_row, start_col),
                bg,
                border_fg,
                hint_fg,
                " ".repeat(hint_left),
                hint_display,
                " ".repeat(hint_right),
                border_fg,
            );
            let _ = write!(out, "{}", reset);
        }

        // -- bottom border --------------------------------------------------
        let _ = write!(
            out,
            "{}{}{}\u{2570}{}\u{256f}{}",
            goto(hint_row + 1, start_col),
            bg,
            border_fg,
            "\u{2500}".repeat(inner_width),
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

    fn sample_entries() -> Vec<NotesListEntry> {
        vec![
            NotesListEntry {
                id: "note-1".into(),
                title: "Meeting Notes".into(),
                updated_at: 1700000000,
            },
            NotesListEntry {
                id: "note-2".into(),
                title: "Shopping List".into(),
                updated_at: 1700100000,
            },
            NotesListEntry {
                id: "note-3".into(),
                title: "Project Ideas".into(),
                updated_at: 1700200000,
            },
            NotesListEntry {
                id: "note-4".into(),
                title: "Meeting Agenda".into(),
                updated_at: 1700300000,
            },
        ]
    }

    // 1. Filter narrows results
    #[test]
    fn test_filter_narrows_results() {
        let mut list = NotesList::new(sample_entries());
        assert_eq!(list.filtered.len(), 4);

        // Type "meeting" -> should match "Meeting Notes" and "Meeting Agenda"
        for ch in "meeting".chars() {
            list.handle_key(&ch.to_string());
        }
        assert_eq!(list.query, "meeting");
        assert_eq!(list.filtered.len(), 2);

        let matched_titles: Vec<&str> = list
            .filtered
            .iter()
            .map(|&i| list.entries[i].title.as_str())
            .collect();
        assert!(matched_titles.contains(&"Meeting Notes"));
        assert!(matched_titles.contains(&"Meeting Agenda"));
    }

    // 2. Select returns Open with correct ID
    #[test]
    fn test_select_returns_open() {
        let mut list = NotesList::new(sample_entries());

        // Move to second entry
        list.handle_key("Down");
        let action = list.handle_key("Enter");
        assert_eq!(action, NotesListAction::Open("note-2".into()));
    }

    // 3. Delete confirmation flow (d -> y)
    #[test]
    fn test_delete_confirmation_flow() {
        let mut list = NotesList::new(sample_entries());

        // Press d to enter delete confirmation
        let action = list.handle_key("d");
        assert_eq!(action, NotesListAction::None);
        assert!(list.confirm_delete);

        // Press y to confirm
        let action = list.handle_key("y");
        assert_eq!(action, NotesListAction::Delete("note-1".into()));
        assert!(!list.confirm_delete);
    }

    // 4. Delete cancel (d -> n)
    #[test]
    fn test_delete_cancel() {
        let mut list = NotesList::new(sample_entries());

        // Press d to enter delete confirmation
        list.handle_key("d");
        assert!(list.confirm_delete);

        // Press n to cancel
        let action = list.handle_key("n");
        assert_eq!(action, NotesListAction::None);
        assert!(!list.confirm_delete);
    }

    // 5. Move selection
    #[test]
    fn test_move_selection() {
        let mut list = NotesList::new(sample_entries());
        assert_eq!(list.selected_index, 0);

        list.handle_key("Down");
        assert_eq!(list.selected_index, 1);

        list.handle_key("Down");
        assert_eq!(list.selected_index, 2);

        list.handle_key("Up");
        assert_eq!(list.selected_index, 1);

        // Wraps at bottom
        list.handle_key("Down");
        list.handle_key("Down");
        assert_eq!(list.selected_index, 3);
        list.handle_key("Down");
        assert_eq!(list.selected_index, 0); // wraps

        // Wraps at top
        list.handle_key("Up");
        assert_eq!(list.selected_index, 3); // wraps
    }

    // 6. Create new note
    #[test]
    fn test_create_new_note() {
        let mut list = NotesList::new(sample_entries());

        // n with empty query creates new note
        let action = list.handle_key("n");
        assert_eq!(action, NotesListAction::Create);
    }

    // 7. Ctrl+N always creates new note
    #[test]
    fn test_ctrl_n_creates_new_note() {
        let mut list = NotesList::new(sample_entries());
        let action = list.handle_key("Ctrl+N");
        assert_eq!(action, NotesListAction::Create);
    }

    // 8. Escape returns Close
    #[test]
    fn test_escape_returns_close() {
        let mut list = NotesList::new(sample_entries());
        let action = list.handle_key("Escape");
        assert_eq!(action, NotesListAction::Close);
    }

    // 9. Backspace removes query char
    #[test]
    fn test_backspace_removes_query_char() {
        let mut list = NotesList::new(sample_entries());

        for ch in "abc".chars() {
            list.handle_key(&ch.to_string());
        }
        assert_eq!(list.query, "abc");

        list.handle_key("Backspace");
        assert_eq!(list.query, "ab");

        list.handle_key("Backspace");
        assert_eq!(list.query, "a");

        list.handle_key("Backspace");
        assert_eq!(list.query, "");

        // Backspace on empty is harmless
        list.handle_key("Backspace");
        assert_eq!(list.query, "");
    }

    // 10. j/k navigation
    #[test]
    fn test_j_k_navigation() {
        let mut list = NotesList::new(sample_entries());
        assert_eq!(list.selected_index, 0);

        list.handle_key("j");
        assert_eq!(list.selected_index, 1);

        list.handle_key("k");
        assert_eq!(list.selected_index, 0);
    }

    // 11. Delete cancel with Escape
    #[test]
    fn test_delete_cancel_with_escape() {
        let mut list = NotesList::new(sample_entries());
        list.handle_key("d");
        assert!(list.confirm_delete);

        let action = list.handle_key("Escape");
        assert_eq!(action, NotesListAction::None);
        assert!(!list.confirm_delete);
    }

    // 12. Selected index clamped after filter
    #[test]
    fn test_selected_index_clamped_after_filter() {
        let mut list = NotesList::new(sample_entries());

        // Move to last entry
        list.handle_key("Down");
        list.handle_key("Down");
        list.handle_key("Down");
        assert_eq!(list.selected_index, 3);

        // Filter to only 1 result
        for ch in "shopping".chars() {
            list.handle_key(&ch.to_string());
        }
        assert_eq!(list.filtered.len(), 1);
        assert!(list.selected_index < list.filtered.len());
    }

    // 13. Render produces non-empty output
    #[test]
    fn test_render_produces_output() {
        let list = NotesList::new(sample_entries());
        let output = list.render(80, 24);
        assert!(!output.is_empty());
        assert!(output.contains("Notes"));
        assert!(output.contains("Meeting Notes"));
        assert!(output.contains("Enter:open"));
    }

    // 14. Render with small terminal returns empty
    #[test]
    fn test_render_small_terminal() {
        let list = NotesList::new(sample_entries());
        let output = list.render(5, 3);
        assert!(output.is_empty());
    }

    // 15. Render shows delete confirmation
    #[test]
    fn test_render_shows_delete_confirmation() {
        let mut list = NotesList::new(sample_entries());
        list.handle_key("d");

        let output = list.render(80, 24);
        assert!(output.contains("Delete this note? y/n"));
    }

    // 16. Empty entries list
    #[test]
    fn test_empty_entries() {
        let list = NotesList::new(vec![]);
        assert!(list.filtered.is_empty());
        assert_eq!(list.selected_index, 0);

        let output = list.render(80, 24);
        assert!(output.contains("No notes yet"));
    }

    // 17. Enter on empty filtered returns None
    #[test]
    fn test_enter_empty_filtered() {
        let mut list = NotesList::new(sample_entries());
        for ch in "zzzzz".chars() {
            list.handle_key(&ch.to_string());
        }
        assert!(list.filtered.is_empty());

        let action = list.handle_key("Enter");
        assert_eq!(action, NotesListAction::None);
    }

    // 18. d on empty list does not enable confirm_delete
    #[test]
    fn test_d_on_empty_list() {
        let mut list = NotesList::new(vec![]);
        list.handle_key("d");
        assert!(!list.confirm_delete);
    }

    // 19. Filter is case-insensitive
    #[test]
    fn test_filter_case_insensitive() {
        let mut list = NotesList::new(sample_entries());
        for ch in "MEETING".chars() {
            list.handle_key(&ch.to_string());
        }
        assert_eq!(list.filtered.len(), 2);
    }

    // 20. Clear filter restores all
    #[test]
    fn test_clear_filter_restores_all() {
        let mut list = NotesList::new(sample_entries());
        for ch in "xyz".chars() {
            list.handle_key(&ch.to_string());
        }
        assert_eq!(list.filtered.len(), 0);

        list.handle_key("Backspace");
        list.handle_key("Backspace");
        list.handle_key("Backspace");
        assert_eq!(list.query, "");
        assert_eq!(list.filtered.len(), 4);
    }
}
