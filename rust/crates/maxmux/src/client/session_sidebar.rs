/// Session sidebar overlay showing a vertical list of sessions with
/// nested windows.
///
/// Can be positioned on the left or right side of the terminal.
/// Supports keyboard navigation and session switching.
use std::fmt::Write as FmtWrite;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SidebarWindow {
    pub id: String,
    pub name: String,
    pub index: usize,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct SidebarSession {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub is_attached: bool,
    pub windows: Vec<SidebarWindow>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SidebarPosition {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SidebarAction {
    None,
    SwitchSession(String), // session ID
    Close,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct SessionSidebar {
    pub sessions: Vec<SidebarSession>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub width: u16,
    pub position: SidebarPosition,
    pub visible: bool,
}

impl SessionSidebar {
    pub fn new(sessions: Vec<SidebarSession>, position: SidebarPosition) -> Self {
        Self {
            sessions,
            selected_index: 0,
            scroll_offset: 0,
            width: 30,
            position,
            visible: true,
        }
    }

    // -- key dispatch -------------------------------------------------------

    pub fn handle_key(&mut self, key: &str) -> SidebarAction {
        match key {
            "Escape" | "q" => SidebarAction::Close,

            "Enter" => {
                if let Some(session) = self.sessions.get(self.selected_index) {
                    SidebarAction::SwitchSession(session.id.clone())
                } else {
                    SidebarAction::None
                }
            }

            "j" | "Down" => {
                self.select_next();
                SidebarAction::None
            }

            "k" | "Up" => {
                self.select_prev();
                SidebarAction::None
            }

            _ => SidebarAction::None,
        }
    }

    fn select_next(&mut self) {
        if !self.sessions.is_empty() && self.selected_index < self.sessions.len() - 1 {
            self.selected_index += 1;
        }
    }

    fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Total number of display rows needed (sessions + windows of selected).
    fn total_display_rows(&self) -> usize {
        let mut count = self.sessions.len();
        if let Some(session) = self.sessions.get(self.selected_index) {
            count += session.windows.len();
        }
        count
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
    pub const ACTIVE: (u8, u8, u8) = (166, 227, 161); // #a6e3a1 (green)
    pub const ATTACHED: (u8, u8, u8) = (250, 179, 135); // #fab387 (peach)
    pub const HINT: (u8, u8, u8) = (166, 173, 200); // #a6adc8
    pub const WINDOW_FG: (u8, u8, u8) = (166, 173, 200); // #a6adc8 (dim)
}

impl SessionSidebar {
    /// Render the session sidebar as a vertical panel.
    ///
    /// Returns an ANSI escape sequence string that draws the sidebar
    /// on the left or right side of the terminal.
    pub fn render(&self, cols: u16, rows: u16) -> String {
        let cols = cols as usize;
        let rows = rows as usize;
        let sidebar_width = (self.width as usize).min(cols.saturating_sub(2));

        if sidebar_width < 8 || rows < 5 {
            return String::new();
        }

        let inner_width = sidebar_width.saturating_sub(1); // 1 col for border line

        // Determine starting column based on position
        let (content_start_col, border_col) = match self.position {
            SidebarPosition::Left => (1, sidebar_width),
            SidebarPosition::Right => {
                let start = cols.saturating_sub(sidebar_width) + 1;
                (start + 1, start) // border is the leftmost column
            }
        };

        let mut out = String::with_capacity(rows * sidebar_width * 4);

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
            "\x1b[38;2;{};{};{}m\x1b[1m",
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
        let active_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::ACTIVE.0,
            colors::ACTIVE.1,
            colors::ACTIVE.2
        );
        let attached_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::ATTACHED.0,
            colors::ATTACHED.1,
            colors::ATTACHED.2
        );
        let hint_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::HINT.0,
            colors::HINT.1,
            colors::HINT.2
        );
        let window_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::WINDOW_FG.0,
            colors::WINDOW_FG.1,
            colors::WINDOW_FG.2
        );
        let reset = "\x1b[0m";

        let goto = |r: usize, c: usize| -> String { format!("\x1b[{};{}H", r, c) };

        // Reserve rows: 1 title + 1 separator + N content + 1 hint
        let content_rows = rows.saturating_sub(3);

        // -- Row 0: Title -------------------------------------------------------
        let title = " Sessions ";
        let title_padding = inner_width.saturating_sub(title.len());
        let title_left_pad = title_padding / 2;
        let title_right_pad = title_padding - title_left_pad;
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}",
            goto(1, content_start_col),
            bg,
            title_fg,
            " ".repeat(title_left_pad),
            title,
            " ".repeat(title_right_pad),
            reset,
        );

        // -- Row 1: Separator ---------------------------------------------------
        let _ = write!(
            out,
            "{}{}{}{}{}",
            goto(2, content_start_col),
            bg,
            border_fg,
            "\u{2500}".repeat(inner_width),
            reset,
        );

        // -- Content rows: Sessions and windows ---------------------------------
        let mut current_row = 3usize; // 1-based terminal row
        let max_row = 2 + content_rows; // last usable row for content

        for (sess_i, session) in self.sessions.iter().enumerate() {
            if current_row > max_row {
                break;
            }

            let is_selected = sess_i == self.selected_index;
            let row_bg = if is_selected { &sel_bg } else { &bg };
            let row_fg = if is_selected { &sel_fg } else { &text_fg };

            // Session row: "▸ name" or "  name" + markers
            let prefix = if is_selected { "\u{25b8} " } else { "  " };
            let mut markers = String::new();
            if session.is_active {
                markers.push_str(" *");
            }
            if session.is_attached {
                markers.push_str(" \u{25cf}");
            }

            let name_avail =
                inner_width.saturating_sub(prefix.chars().count() + markers.chars().count());
            let name_display = if session.name.len() > name_avail {
                format!("{}...", &session.name[..name_avail.saturating_sub(3)])
            } else {
                session.name.clone()
            };
            let content_len = prefix.chars().count() + name_display.len() + markers.chars().count();
            let trailing_pad = inner_width.saturating_sub(content_len);

            let _ = write!(
                out,
                "{}{}{}{}{}",
                goto(current_row, content_start_col),
                row_bg,
                row_fg,
                prefix,
                name_display,
            );

            // Markers with special colors
            if session.is_active {
                let _ = write!(out, "{} *", active_fg);
            }
            if session.is_attached {
                let _ = write!(out, "{} \u{25cf}", attached_fg);
            }

            let _ = write!(out, "{}{}", " ".repeat(trailing_pad), reset);

            current_row += 1;

            // Show windows under selected session
            if is_selected && !session.windows.is_empty() {
                let win_count = session.windows.len();
                for (win_i, window) in session.windows.iter().enumerate() {
                    if current_row > max_row {
                        break;
                    }

                    let is_last = win_i == win_count - 1;
                    let tree_char = if is_last { "\u{2514}" } else { "\u{251c}" }; // └ or ├
                    let win_label = format!("  {} {}:{}", tree_char, window.index, window.name);

                    let active_marker = if window.is_active { " *" } else { "" };
                    let win_content_len = win_label.len() + active_marker.len();
                    let win_trailing_pad = inner_width.saturating_sub(win_content_len);

                    let _ = write!(
                        out,
                        "{}{}{}{}",
                        goto(current_row, content_start_col),
                        bg,
                        window_fg,
                        win_label,
                    );

                    if window.is_active {
                        let _ = write!(out, "{}{}", active_fg, active_marker);
                    }

                    let _ = write!(out, "{}{}", " ".repeat(win_trailing_pad), reset);

                    current_row += 1;
                }
            }
        }

        // Fill remaining content rows with blank
        while current_row <= max_row {
            let _ = write!(
                out,
                "{}{}{}{}",
                goto(current_row, content_start_col),
                bg,
                " ".repeat(inner_width),
                reset,
            );
            current_row += 1;
        }

        // -- Hint row -----------------------------------------------------------
        let hint = "j/k:nav  Enter:switch  Esc:close";
        let hint_truncated = if hint.len() > inner_width {
            &hint[..inner_width]
        } else {
            hint
        };
        let hint_padding = inner_width.saturating_sub(hint_truncated.len());
        let _ = write!(
            out,
            "{}{}{}{}{}{}",
            goto(current_row, content_start_col),
            bg,
            hint_fg,
            hint_truncated,
            " ".repeat(hint_padding),
            reset,
        );

        // -- Vertical border line -----------------------------------------------
        for row in 1..=rows {
            let _ = write!(
                out,
                "{}{}{}{}{}",
                goto(row, border_col),
                bg,
                border_fg,
                "\u{2502}",
                reset,
            );
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sessions() -> Vec<SidebarSession> {
        vec![
            SidebarSession {
                id: "s1".into(),
                name: "dev-server".into(),
                is_active: true,
                is_attached: true,
                windows: vec![
                    SidebarWindow {
                        id: "w1".into(),
                        name: "editor".into(),
                        index: 0,
                        is_active: true,
                    },
                    SidebarWindow {
                        id: "w2".into(),
                        name: "shell".into(),
                        index: 1,
                        is_active: false,
                    },
                    SidebarWindow {
                        id: "w3".into(),
                        name: "logs".into(),
                        index: 2,
                        is_active: false,
                    },
                ],
            },
            SidebarSession {
                id: "s2".into(),
                name: "database".into(),
                is_active: false,
                is_attached: false,
                windows: vec![SidebarWindow {
                    id: "w4".into(),
                    name: "psql".into(),
                    index: 0,
                    is_active: true,
                }],
            },
            SidebarSession {
                id: "s3".into(),
                name: "monitoring".into(),
                is_active: false,
                is_attached: true,
                windows: vec![
                    SidebarWindow {
                        id: "w5".into(),
                        name: "htop".into(),
                        index: 0,
                        is_active: true,
                    },
                    SidebarWindow {
                        id: "w6".into(),
                        name: "grafana".into(),
                        index: 1,
                        is_active: false,
                    },
                ],
            },
        ]
    }

    // 1. Navigate down
    #[test]
    fn test_navigate_down() {
        let mut sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        assert_eq!(sidebar.selected_index, 0);

        sidebar.handle_key("j");
        assert_eq!(sidebar.selected_index, 1);

        sidebar.handle_key("Down");
        assert_eq!(sidebar.selected_index, 2);
    }

    // 2. Navigate up
    #[test]
    fn test_navigate_up() {
        let mut sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        sidebar.selected_index = 2;

        sidebar.handle_key("k");
        assert_eq!(sidebar.selected_index, 1);

        sidebar.handle_key("Up");
        assert_eq!(sidebar.selected_index, 0);
    }

    // 3. Navigate up at zero stays at zero
    #[test]
    fn test_navigate_up_at_zero() {
        let mut sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        assert_eq!(sidebar.selected_index, 0);

        sidebar.handle_key("Up");
        assert_eq!(sidebar.selected_index, 0);
    }

    // 4. Navigate down clamps at last
    #[test]
    fn test_navigate_down_clamps() {
        let mut sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        for _ in 0..10 {
            sidebar.handle_key("Down");
        }
        assert_eq!(sidebar.selected_index, 2); // last session index
    }

    // 5. Select returns session ID
    #[test]
    fn test_select_returns_session_id() {
        let mut sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        sidebar.handle_key("Down"); // select "database"
        let action = sidebar.handle_key("Enter");
        assert_eq!(action, SidebarAction::SwitchSession("s2".into()));
    }

    // 6. Enter on first session returns correct ID
    #[test]
    fn test_enter_first_session() {
        let mut sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        let action = sidebar.handle_key("Enter");
        assert_eq!(action, SidebarAction::SwitchSession("s1".into()));
    }

    // 7. Escape closes
    #[test]
    fn test_escape_closes() {
        let mut sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        let action = sidebar.handle_key("Escape");
        assert_eq!(action, SidebarAction::Close);
    }

    // 8. q closes
    #[test]
    fn test_q_closes() {
        let mut sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        let action = sidebar.handle_key("q");
        assert_eq!(action, SidebarAction::Close);
    }

    // 9. Selected session shows windows in total_display_rows
    #[test]
    fn test_selected_session_shows_windows() {
        let sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        // First session is selected, it has 3 windows
        // Total: 3 sessions + 3 windows = 6 display rows
        assert_eq!(sidebar.total_display_rows(), 6);
    }

    // 10. Second session selected shows its windows
    #[test]
    fn test_second_session_windows() {
        let mut sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        sidebar.selected_index = 1; // database: 1 window
        // Total: 3 sessions + 1 window = 4 display rows
        assert_eq!(sidebar.total_display_rows(), 4);
    }

    // 11. Render produces output
    #[test]
    fn test_render_produces_output() {
        let sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        let output = sidebar.render(80, 24);
        assert!(!output.is_empty());
        // Should contain the title
        assert!(output.contains("Sessions"));
        // Should contain session names
        assert!(output.contains("dev-server"));
        assert!(output.contains("database"));
        assert!(output.contains("monitoring"));
        // Should contain the hint
        assert!(output.contains("j/k:nav"));
    }

    // 12. Render contains windows of selected session
    #[test]
    fn test_render_contains_windows() {
        let sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        let output = sidebar.render(80, 24);
        // First session is selected, should show its windows
        assert!(output.contains("editor"));
        assert!(output.contains("shell"));
        assert!(output.contains("logs"));
    }

    // 13. Render with right position works
    #[test]
    fn test_render_right_position() {
        let sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Right);
        let output = sidebar.render(80, 24);
        assert!(!output.is_empty());
        assert!(output.contains("Sessions"));
    }

    // 14. Render with small terminal returns empty
    #[test]
    fn test_render_small_terminal() {
        let sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        let output = sidebar.render(5, 3);
        assert!(output.is_empty());
    }

    // 15. New sidebar starts visible
    #[test]
    fn test_new_starts_visible() {
        let sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        assert!(sidebar.visible);
        assert_eq!(sidebar.selected_index, 0);
        assert_eq!(sidebar.scroll_offset, 0);
        assert_eq!(sidebar.width, 30);
    }

    // 16. Empty sessions list
    #[test]
    fn test_empty_sessions() {
        let mut sidebar = SessionSidebar::new(vec![], SidebarPosition::Left);
        assert_eq!(sidebar.selected_index, 0);
        assert_eq!(sidebar.total_display_rows(), 0);

        // Enter on empty list returns None
        let action = sidebar.handle_key("Enter");
        assert_eq!(action, SidebarAction::None);
    }

    // 17. Render contains tree characters
    #[test]
    fn test_render_contains_tree_chars() {
        let sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        let output = sidebar.render(80, 24);
        // Should contain tree drawing characters (├ and └)
        assert!(output.contains('\u{251c}')); // ├
        assert!(output.contains('\u{2514}')); // └
    }

    // 18. Render contains active and attached markers
    #[test]
    fn test_render_contains_markers() {
        let sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        let output = sidebar.render(80, 24);
        // Active marker: *
        assert!(output.contains('*'));
        // Attached marker: ●
        assert!(output.contains('\u{25cf}'));
    }

    // 19. Unrecognized key returns None
    #[test]
    fn test_unrecognized_key_returns_none() {
        let mut sidebar = SessionSidebar::new(sample_sessions(), SidebarPosition::Left);
        let action = sidebar.handle_key("x");
        assert_eq!(action, SidebarAction::None);
    }

    // 20. Position enum equality
    #[test]
    fn test_position_equality() {
        assert_eq!(SidebarPosition::Left, SidebarPosition::Left);
        assert_ne!(SidebarPosition::Left, SidebarPosition::Right);
    }
}
