/// Fuzzy session finder overlay using nucleo for matching.
///
/// Presents a centered modal with a query input and a list of sessions
/// that can be filtered by typing.  Arrow keys (or Ctrl-j / Ctrl-k)
/// navigate the list, Enter selects, Escape closes.

use std::fmt::Write as FmtWrite;

use nucleo::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo::Matcher;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SessionFinderEntry {
    pub id: String,
    pub name: String,
    pub window_count: usize,
    pub attached: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionFinderAction {
    None,
    /// The user selected a session – contains the session id.
    Select(String),
    /// The user dismissed the finder.
    Close,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct SessionFinder {
    pub query: String,
    pub selected_index: usize,
    pub entries: Vec<SessionFinderEntry>,
    /// Indices into `entries` that match the current query, ordered by score
    /// (best first).
    pub filtered: Vec<usize>,
    pub visible: bool,
}

impl SessionFinder {
    pub fn new(entries: Vec<SessionFinderEntry>) -> Self {
        let filtered: Vec<usize> = (0..entries.len()).collect();
        Self {
            query: String::new(),
            selected_index: 0,
            entries,
            filtered,
            visible: true,
        }
    }

    // -- key handling -------------------------------------------------------

    pub fn handle_key(&mut self, key: &str) -> SessionFinderAction {
        match key {
            "Escape" => SessionFinderAction::Close,

            "Enter" => {
                if let Some(&idx) = self.filtered.get(self.selected_index) {
                    SessionFinderAction::Select(self.entries[idx].id.clone())
                } else {
                    SessionFinderAction::None
                }
            }

            // Navigation: Ctrl-k / Up  and  Ctrl-j / Down
            "Up" | "C-k" | "Ctrl+K" => self.move_selection(-1),
            "Down" | "C-j" | "Ctrl+J" => self.move_selection(1),

            "Backspace" => {
                self.query.pop();
                self.filter();
                SessionFinderAction::None
            }

            // Ctrl-U clears the query
            "C-u" | "Ctrl+U" => {
                self.query.clear();
                self.filter();
                SessionFinderAction::None
            }

            _ => {
                // Accept single printable characters.
                if key.len() == 1 {
                    let ch = key.chars().next().unwrap();
                    if !ch.is_control() {
                        self.query.push(ch);
                        self.filter();
                    }
                }
                SessionFinderAction::None
            }
        }
    }

    // -- filtering ----------------------------------------------------------

    fn filter(&mut self) {
        if self.query.is_empty() {
            // Show all entries in their original order.
            self.filtered = (0..self.entries.len()).collect();
        } else {
            // Use nucleo for fuzzy matching.
            let pattern = Pattern::new(
                &self.query,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );
            let mut matcher = Matcher::new(nucleo::Config::DEFAULT);

            // Collect (index, score) for entries that match, then sort by
            // score descending so the best match appears first.
            let mut scored: Vec<(usize, u32)> = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, entry)| {
                    let mut buf = Vec::new();
                    let haystack = nucleo::Utf32Str::new(&entry.name, &mut buf);
                    pattern.score(haystack, &mut matcher).map(|s| (i, s))
                })
                .collect();

            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }

        // Clamp selected_index.
        if self.filtered.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered.len() {
            self.selected_index = self.filtered.len() - 1;
        }
    }

    fn move_selection(&mut self, delta: i32) -> SessionFinderAction {
        if self.filtered.is_empty() {
            return SessionFinderAction::None;
        }
        let len = self.filtered.len() as i32;
        let mut new_idx = self.selected_index as i32 + delta;
        if new_idx < 0 {
            new_idx = 0;
        } else if new_idx >= len {
            new_idx = len - 1;
        }
        self.selected_index = new_idx as usize;
        SessionFinderAction::None
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Catppuccin Mocha palette constants (RGB).
mod colors {
    pub const BORDER: (u8, u8, u8) = (88, 91, 112);      // #585b70
    pub const TITLE: (u8, u8, u8) = (137, 180, 250);      // #89b4fa
    pub const BG: (u8, u8, u8) = (30, 30, 46);            // #1e1e2e
    pub const TEXT: (u8, u8, u8) = (205, 214, 244);        // #cdd6f4
    pub const SELECTED_BG: (u8, u8, u8) = (49, 50, 68);   // #313244
    pub const SELECTED_FG: (u8, u8, u8) = (137, 180, 250); // #89b4fa
    pub const ATTACHED: (u8, u8, u8) = (166, 227, 161);   // #a6e3a1
}

impl SessionFinder {
    /// Render the session finder as an ANSI-escaped string suitable for
    /// writing to a terminal.  The output is a centered modal box.
    pub fn render(&self, cols: u16, rows: u16) -> String {
        let width = 50u16.min(cols.saturating_sub(4)) as usize;
        let max_items = (rows as usize).saturating_sub(8);
        let item_count = self.filtered.len().min(max_items);
        // When there are no items we still show a "No matches" row.
        let display_rows = if item_count == 0 { 1 } else { item_count };
        // height: 1 top border + 1 query line + 1 separator + display_rows + 1 bottom border
        let height = display_rows + 4;

        if width < 10 || height < 5 {
            return String::new();
        }

        // Calculate top-left position for centering.
        let start_col = ((cols as usize).saturating_sub(width)) / 2 + 1; // 1-indexed
        let start_row = ((rows as usize).saturating_sub(height)) / 2 + 1;

        let mut output = String::with_capacity(height * width * 8);

        let bg = format!(
            "\x1b[48;2;{};{};{}m",
            colors::BG.0, colors::BG.1, colors::BG.2
        );
        let border_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::BORDER.0, colors::BORDER.1, colors::BORDER.2
        );
        let title_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::TITLE.0, colors::TITLE.1, colors::TITLE.2
        );
        let text_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::TEXT.0, colors::TEXT.1, colors::TEXT.2
        );
        let sel_bg = format!(
            "\x1b[48;2;{};{};{}m",
            colors::SELECTED_BG.0, colors::SELECTED_BG.1, colors::SELECTED_BG.2
        );
        let sel_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::SELECTED_FG.0, colors::SELECTED_FG.1, colors::SELECTED_FG.2
        );
        let attached_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::ATTACHED.0, colors::ATTACHED.1, colors::ATTACHED.2
        );

        let inner_width = width.saturating_sub(2); // inside the left/right borders

        // ---- Row 0: Top border with title ----
        {
            let title = "Find Session";
            let title_display_len = title.len();
            // Build: +-- Find Session --...--+
            let dashes_total = inner_width.saturating_sub(title_display_len + 2);
            let dashes_left = 2.min(dashes_total);
            let dashes_right = dashes_total.saturating_sub(dashes_left);

            let _ = write!(
                output,
                "\x1b[{};{}H{bg}{border_fg}\u{256d}{}\x1b[0m{bg}{title_fg}\x1b[1m {title} \x1b[0m{bg}{border_fg}{}\u{256e}\x1b[0m",
                start_row,
                start_col,
                "\u{2500}".repeat(dashes_left),
                "\u{2500}".repeat(dashes_right),
            );
        }

        // ---- Row 1: Query line ----
        {
            let prompt = "> ";
            let query_display: String = if self.query.len() + prompt.len() + 1 > inner_width {
                // Truncate query from the left if too long.
                let max = inner_width.saturating_sub(prompt.len() + 1);
                let start = self.query.len().saturating_sub(max);
                format!("{}{}", prompt, &self.query[start..])
            } else {
                format!("{}{}", prompt, &self.query)
            };
            let cursor = "_";
            let content_len = query_display.len() + cursor.len();
            let padding = inner_width.saturating_sub(content_len);

            let _ = write!(
                output,
                "\x1b[{};{}H{bg}{border_fg}\u{2502}\x1b[0m{bg}{text_fg}{query_display}{title_fg}{cursor}{}{border_fg}\u{2502}\x1b[0m",
                start_row + 1,
                start_col,
                " ".repeat(padding),
            );
        }

        // ---- Row 2: Separator ----
        {
            let _ = write!(
                output,
                "\x1b[{};{}H{bg}{border_fg}\u{251c}{}\u{2524}\x1b[0m",
                start_row + 2,
                start_col,
                "\u{2500}".repeat(inner_width),
            );
        }

        // ---- Rows 3..3+item_count: Filtered entries ----
        if self.filtered.is_empty() {
            // Show "No matches"
            let msg = "No matches";
            let padding_left = (inner_width.saturating_sub(msg.len())) / 2;
            let padding_right = inner_width.saturating_sub(msg.len() + padding_left);
            let _ = write!(
                output,
                "\x1b[{};{}H{bg}{border_fg}\u{2502}\x1b[0m{bg}{text_fg}{}{msg}{}{border_fg}\u{2502}\x1b[0m",
                start_row + 3,
                start_col,
                " ".repeat(padding_left),
                " ".repeat(padding_right),
            );
        } else {
            for (display_i, &entry_idx) in
                self.filtered.iter().take(max_items).enumerate()
            {
                let entry = &self.entries[entry_idx];
                let is_selected = display_i == self.selected_index;

                // Build content: "name - N win" + optional " *"
                let attached_marker = if entry.attached { " \u{25cf}" } else { "" };
                let label = format!(
                    "{} - {} win",
                    entry.name, entry.window_count
                );
                // Truncate label if needed (leave room for attached marker + padding).
                let max_label_len = inner_width.saturating_sub(attached_marker.len());
                let label_truncated = if label.len() > max_label_len {
                    format!("{}...", &label[..max_label_len.saturating_sub(3)])
                } else {
                    label.clone()
                };
                let content_len = label_truncated.len() + attached_marker.len();
                let padding = inner_width.saturating_sub(content_len);

                let row_bg = if is_selected { &sel_bg } else { &bg };
                let row_fg = if is_selected { &sel_fg } else { &text_fg };

                let _ = write!(
                    output,
                    "\x1b[{};{}H{bg}{border_fg}\u{2502}\x1b[0m{row_bg}{row_fg}{label_truncated}",
                    start_row + 3 + display_i,
                    start_col,
                );

                if entry.attached {
                    let _ = write!(
                        output,
                        "{row_bg}{attached_fg}{attached_marker}",
                    );
                }

                let _ = write!(
                    output,
                    "{}{bg}{border_fg}\u{2502}\x1b[0m",
                    " ".repeat(padding),
                );
            }
        }

        // ---- Bottom border ----
        {
            let bottom_row = if self.filtered.is_empty() {
                start_row + 4 // top + query + sep + "no matches" + bottom
            } else {
                start_row + 3 + item_count
            };
            let _ = write!(
                output,
                "\x1b[{};{}H{bg}{border_fg}\u{2570}{}\u{256f}\x1b[0m",
                bottom_row,
                start_col,
                "\u{2500}".repeat(inner_width),
            );
        }

        output
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<SessionFinderEntry> {
        vec![
            SessionFinderEntry {
                id: "s1".into(),
                name: "dev-server".into(),
                window_count: 3,
                attached: true,
            },
            SessionFinderEntry {
                id: "s2".into(),
                name: "database".into(),
                window_count: 1,
                attached: false,
            },
            SessionFinderEntry {
                id: "s3".into(),
                name: "monitoring".into(),
                window_count: 2,
                attached: false,
            },
            SessionFinderEntry {
                id: "s4".into(),
                name: "dev-tools".into(),
                window_count: 5,
                attached: true,
            },
        ]
    }

    // 1. New finder with entries – all visible, index at 0
    #[test]
    fn test_new_finder_with_entries() {
        let finder = SessionFinder::new(sample_entries());
        assert_eq!(finder.entries.len(), 4);
        assert_eq!(finder.filtered.len(), 4);
        assert_eq!(finder.selected_index, 0);
        assert!(finder.query.is_empty());
        assert!(finder.visible);
    }

    // 2. Filter narrows results
    #[test]
    fn test_filter_narrows_results() {
        let mut finder = SessionFinder::new(sample_entries());

        // Type "dev" – should match "dev-server" and "dev-tools"
        finder.handle_key("d");
        finder.handle_key("e");
        finder.handle_key("v");

        assert_eq!(finder.query, "dev");
        assert_eq!(finder.filtered.len(), 2);

        // Both matching entries should be dev-server (0) and dev-tools (3)
        let matched_names: Vec<&str> = finder
            .filtered
            .iter()
            .map(|&i| finder.entries[i].name.as_str())
            .collect();
        assert!(matched_names.contains(&"dev-server"));
        assert!(matched_names.contains(&"dev-tools"));
    }

    // 3. Clear filter shows all
    #[test]
    fn test_clear_filter_shows_all() {
        let mut finder = SessionFinder::new(sample_entries());

        // Type then clear
        finder.handle_key("d");
        finder.handle_key("e");
        finder.handle_key("v");
        assert_eq!(finder.filtered.len(), 2);

        // Clear with Ctrl-U
        finder.handle_key("C-u");
        assert!(finder.query.is_empty());
        assert_eq!(finder.filtered.len(), 4);
    }

    // 4. Move selection down and up
    #[test]
    fn test_move_selection_down_up() {
        let mut finder = SessionFinder::new(sample_entries());
        assert_eq!(finder.selected_index, 0);

        // Move down
        finder.handle_key("Down");
        assert_eq!(finder.selected_index, 1);

        finder.handle_key("Down");
        assert_eq!(finder.selected_index, 2);

        // Move up
        finder.handle_key("Up");
        assert_eq!(finder.selected_index, 1);

        // Move up past top – clamps to 0
        finder.handle_key("Up");
        assert_eq!(finder.selected_index, 0);
        finder.handle_key("Up");
        assert_eq!(finder.selected_index, 0);
    }

    // 5. Select returns correct session ID
    #[test]
    fn test_select_returns_correct_id() {
        let mut finder = SessionFinder::new(sample_entries());

        // Move to second entry
        finder.handle_key("Down");
        let action = finder.handle_key("Enter");
        assert_eq!(action, SessionFinderAction::Select("s2".into()));
    }

    // 6. Escape returns Close
    #[test]
    fn test_escape_returns_close() {
        let mut finder = SessionFinder::new(sample_entries());
        let action = finder.handle_key("Escape");
        assert_eq!(action, SessionFinderAction::Close);
    }

    // 7. Backspace removes last query char
    #[test]
    fn test_backspace_removes_query_char() {
        let mut finder = SessionFinder::new(sample_entries());

        finder.handle_key("a");
        finder.handle_key("b");
        finder.handle_key("c");
        assert_eq!(finder.query, "abc");

        finder.handle_key("Backspace");
        assert_eq!(finder.query, "ab");

        finder.handle_key("Backspace");
        assert_eq!(finder.query, "a");
    }

    // 8. Render produces output
    #[test]
    fn test_render_produces_output() {
        let finder = SessionFinder::new(sample_entries());
        let output = finder.render(80, 24);
        assert!(!output.is_empty());
        // Should contain the title
        assert!(output.contains("Find Session"));
        // Should contain entry names
        assert!(output.contains("dev-server"));
        assert!(output.contains("database"));
        assert!(output.contains("monitoring"));
        assert!(output.contains("dev-tools"));
    }

    // 9. Render shows "No matches" when filter matches nothing
    #[test]
    fn test_render_no_matches() {
        let mut finder = SessionFinder::new(sample_entries());
        // Type something that matches nothing
        finder.handle_key("z");
        finder.handle_key("z");
        finder.handle_key("z");

        let output = finder.render(80, 24);
        assert!(output.contains("No matches"));
    }

    // 10. Select on empty filtered list returns None
    #[test]
    fn test_select_on_empty_returns_none() {
        let mut finder = SessionFinder::new(sample_entries());
        finder.handle_key("z");
        finder.handle_key("z");
        finder.handle_key("z");
        assert!(finder.filtered.is_empty());

        let action = finder.handle_key("Enter");
        assert_eq!(action, SessionFinderAction::None);
    }

    // 11. Move selection clamps at bottom
    #[test]
    fn test_move_selection_clamps_at_bottom() {
        let mut finder = SessionFinder::new(sample_entries());
        // Move way past the end
        for _ in 0..20 {
            finder.handle_key("Down");
        }
        assert_eq!(finder.selected_index, 3); // last index
    }

    // 12. Ctrl-j / Ctrl-k navigation works
    #[test]
    fn test_ctrl_j_k_navigation() {
        let mut finder = SessionFinder::new(sample_entries());
        assert_eq!(finder.selected_index, 0);

        finder.handle_key("C-j");
        assert_eq!(finder.selected_index, 1);

        finder.handle_key("C-k");
        assert_eq!(finder.selected_index, 0);
    }

    // 13. Filter then select returns correct entry
    #[test]
    fn test_filter_then_select() {
        let mut finder = SessionFinder::new(sample_entries());

        // Filter to "mon" -> should match "monitoring"
        finder.handle_key("m");
        finder.handle_key("o");
        finder.handle_key("n");

        assert!(!finder.filtered.is_empty());
        let action = finder.handle_key("Enter");
        assert_eq!(action, SessionFinderAction::Select("s3".into()));
    }

    // 14. Attached marker appears in render output
    #[test]
    fn test_attached_marker_in_render() {
        let finder = SessionFinder::new(sample_entries());
        let output = finder.render(80, 24);
        // The bullet character used for attached sessions
        assert!(output.contains('\u{25cf}'));
    }

    // 15. Render with tiny terminal returns empty
    #[test]
    fn test_render_tiny_terminal() {
        let finder = SessionFinder::new(sample_entries());
        let output = finder.render(5, 5);
        assert!(output.is_empty());
    }

    // 16. Empty entries list
    #[test]
    fn test_empty_entries() {
        let finder = SessionFinder::new(vec![]);
        assert!(finder.filtered.is_empty());
        assert_eq!(finder.selected_index, 0);

        let output = finder.render(80, 24);
        assert!(output.contains("No matches"));
    }

    // 17. Selected index resets when filter shrinks past it
    #[test]
    fn test_selected_index_resets_on_filter_shrink() {
        let mut finder = SessionFinder::new(sample_entries());

        // Select the last item
        finder.handle_key("Down");
        finder.handle_key("Down");
        finder.handle_key("Down");
        assert_eq!(finder.selected_index, 3);

        // Now filter to only 2 results
        finder.handle_key("d");
        finder.handle_key("e");
        finder.handle_key("v");
        assert!(finder.selected_index < finder.filtered.len());
    }

    // 18. Backspace on empty query is no-op
    #[test]
    fn test_backspace_empty_query() {
        let mut finder = SessionFinder::new(sample_entries());
        assert!(finder.query.is_empty());

        let action = finder.handle_key("Backspace");
        assert_eq!(action, SessionFinderAction::None);
        assert!(finder.query.is_empty());
        assert_eq!(finder.filtered.len(), 4);
    }
}
