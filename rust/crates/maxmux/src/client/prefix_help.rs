/// Prefix help overlay showing available keybindings after the prefix key.
///
/// Renders a centered modal listing all registered prefix bindings
/// with their human-readable descriptions and translated key names.

use std::fmt::Write as FmtWrite;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub key: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrefixHelpAction {
    None,
    Close,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct PrefixHelp {
    pub bindings: Vec<KeyBinding>,
    pub scroll_offset: usize,
    pub visible: bool,
}

impl PrefixHelp {
    pub fn new(bindings: Vec<KeyBinding>) -> Self {
        Self {
            bindings,
            scroll_offset: 0,
            visible: true,
        }
    }

    /// Build a default set of bindings from the hardcoded description map.
    ///
    /// Each tuple is `(key_string, command_id)`.  The command is looked up in
    /// the built-in description table; unknown commands are shown verbatim.
    pub fn from_command_bindings(bindings: &[(String, String)]) -> Self {
        let entries: Vec<KeyBinding> = bindings
            .iter()
            .map(|(key, cmd)| KeyBinding {
                key: translate_key_display(key),
                description: describe_command(cmd).to_string(),
            })
            .collect();
        Self::new(entries)
    }

    // -- key dispatch -------------------------------------------------------

    pub fn handle_key(&mut self, key: &str) -> PrefixHelpAction {
        match key {
            "Escape" | "q" => PrefixHelpAction::Close,
            "Up" | "k" => {
                self.scroll_up();
                PrefixHelpAction::None
            }
            "Down" | "j" => {
                self.scroll_down();
                PrefixHelpAction::None
            }
            _ => PrefixHelpAction::None,
        }
    }

    fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        if !self.bindings.is_empty() {
            let max = self.bindings.len().saturating_sub(1);
            if self.scroll_offset < max {
                self.scroll_offset += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Command description map
// ---------------------------------------------------------------------------

fn describe_command(cmd: &str) -> &str {
    match cmd {
        "window:create" => "Create new window",
        "window:next" => "Next window",
        "window:previous" => "Previous window",
        "window:close" => "Close window",
        "window:rename" => "Rename window",
        "pane:split-horizontal" => "Split pane horizontally",
        "pane:split-vertical" => "Split pane vertically",
        "pane:next" => "Next pane",
        "pane:close" => "Close pane",
        "pane:zoom" => "Toggle pane zoom",
        "pane:focus-up" => "Focus pane above",
        "pane:focus-down" => "Focus pane below",
        "pane:focus-left" => "Focus pane left",
        "pane:focus-right" => "Focus pane right",
        "session:detach" => "Detach from session",
        "session:create" => "Create new session",
        "session:list" => "List sessions",
        "session:find" => "Find session",
        "session:rename" => "Rename session",
        "server:kill" => "Kill server",
        "command-palette" => "Command palette",
        "keybindings:show" => "Show keybindings",
        "copy-mode:enter" => "Enter copy mode",
        "notes:create" => "Create note",
        "notes:list" => "List notes",
        _ => cmd,
    }
}

// ---------------------------------------------------------------------------
// Key display translation
// ---------------------------------------------------------------------------

fn translate_key_display(key: &str) -> String {
    match key {
        "Up" => "\u{2191}".to_string(),      // ↑
        "Down" => "\u{2193}".to_string(),    // ↓
        "Left" => "\u{2190}".to_string(),    // ←
        "Right" => "\u{2192}".to_string(),   // →
        other => other.to_string(),
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
    pub const KEY_FG: (u8, u8, u8) = (137, 180, 250);     // #89b4fa (blue/bold)
    pub const HINT: (u8, u8, u8) = (166, 173, 200);       // #a6adc8 (dim)
}

impl PrefixHelp {
    /// Render the prefix help as a centered modal overlay.
    ///
    /// Returns an ANSI escape sequence string that draws the help box
    /// at the center of the given terminal dimensions.
    pub fn render(&self, cols: u16, rows: u16) -> String {
        let cols = cols as usize;
        let rows = rows as usize;
        if cols < 12 || rows < 7 {
            return String::new();
        }

        // Modal dimensions
        let modal_width = 50.min(cols.saturating_sub(4));
        let max_visible_items = rows.saturating_sub(7); // top border + title + separator + footer hint + bottom border + margin
        let visible_bindings: Vec<&KeyBinding> = self
            .bindings
            .iter()
            .skip(self.scroll_offset)
            .take(max_visible_items)
            .collect();
        let item_count = visible_bindings.len().max(1); // at least 1 row

        // Chrome: top border + title + separator + items + hint + bottom border = items + 5
        let modal_height = item_count + 5;

        // Center the modal
        let start_col = (cols.saturating_sub(modal_width)) / 2 + 1;
        let start_row = (rows.saturating_sub(modal_height)) / 2 + 1;

        let inner_width = modal_width.saturating_sub(2);

        let mut out = String::with_capacity(modal_height * modal_width * 4);

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
        let key_fg = format!(
            "\x1b[38;2;{};{};{}m\x1b[1m",
            colors::KEY_FG.0, colors::KEY_FG.1, colors::KEY_FG.2
        );
        let hint_fg = format!(
            "\x1b[38;2;{};{};{}m",
            colors::HINT.0, colors::HINT.1, colors::HINT.2
        );
        let reset = "\x1b[0m";

        let goto = |r: usize, c: usize| -> String { format!("\x1b[{};{}H", r, c) };

        // Find max key width for alignment
        let max_key_len = visible_bindings
            .iter()
            .map(|b| b.key.chars().count())
            .max()
            .unwrap_or(1);

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
        let title = "Prefix Mode";
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

        // -- separator --------------------------------------------------------
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

        // -- keybinding rows --------------------------------------------------
        let list_start_row = start_row + 3;
        if visible_bindings.is_empty() {
            let msg = "No bindings";
            let msg_padding = inner_width.saturating_sub(msg.len());
            let _ = write!(
                out,
                "{}{}{}{}{}{}{}{}{}",
                goto(list_start_row, start_col),
                bg,
                border_fg,
                "\u{2502}",
                hint_fg,
                msg,
                " ".repeat(msg_padding),
                border_fg,
                "\u{2502}",
            );
            let _ = write!(out, "{}", reset);
        } else {
            for (vis_i, binding) in visible_bindings.iter().enumerate() {
                let key_display = &binding.key;
                let key_char_count = key_display.chars().count();
                let left_pad = max_key_len.saturating_sub(key_char_count);
                let spacer = "    "; // 4 spaces between key and description
                let desc = &binding.description;

                // Calculate available width for description
                let prefix_len = left_pad + key_char_count + spacer.len();
                let avail_for_desc = inner_width.saturating_sub(prefix_len);
                let desc_truncated: String = if desc.len() > avail_for_desc {
                    desc[..avail_for_desc].to_string()
                } else {
                    desc.clone()
                };
                let trailing_pad =
                    inner_width.saturating_sub(prefix_len + desc_truncated.len());

                let _ = write!(
                    out,
                    "{}{}{}{}{}{}{}{}{}{}{}{}{}",
                    goto(list_start_row + vis_i, start_col),
                    bg,
                    border_fg,
                    "\u{2502}",
                    key_fg,
                    " ".repeat(left_pad),
                    key_display,
                    text_fg,
                    spacer,
                    desc_truncated,
                    " ".repeat(trailing_pad),
                    border_fg,
                    "\u{2502}",
                );
                let _ = write!(out, "{}", reset);
            }
        }

        // -- hint line --------------------------------------------------------
        let hint_row = list_start_row + item_count;
        let hint = "Esc to close";
        let hint_padding = inner_width.saturating_sub(hint.len());
        let hint_left_pad = hint_padding / 2;
        let hint_right_pad = hint_padding - hint_left_pad;
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}{}{}{}",
            goto(hint_row, start_col),
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
        let bottom_row = hint_row + 1;
        let _ = write!(
            out,
            "{}{}{}{}{}{}{}",
            goto(bottom_row, start_col),
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

    fn sample_bindings() -> Vec<KeyBinding> {
        vec![
            KeyBinding {
                key: "c".into(),
                description: "Create new window".into(),
            },
            KeyBinding {
                key: "n".into(),
                description: "Next window".into(),
            },
            KeyBinding {
                key: "p".into(),
                description: "Previous window".into(),
            },
            KeyBinding {
                key: "%".into(),
                description: "Split pane horizontally".into(),
            },
            KeyBinding {
                key: "\"".into(),
                description: "Split pane vertically".into(),
            },
            KeyBinding {
                key: "d".into(),
                description: "Detach from session".into(),
            },
        ]
    }

    // 1. Escape closes
    #[test]
    fn test_escape_closes() {
        let mut help = PrefixHelp::new(sample_bindings());
        let action = help.handle_key("Escape");
        assert_eq!(action, PrefixHelpAction::Close);
    }

    // 2. q also closes
    #[test]
    fn test_q_closes() {
        let mut help = PrefixHelp::new(sample_bindings());
        let action = help.handle_key("q");
        assert_eq!(action, PrefixHelpAction::Close);
    }

    // 3. Scroll down works
    #[test]
    fn test_scroll_down() {
        let mut help = PrefixHelp::new(sample_bindings());
        assert_eq!(help.scroll_offset, 0);

        help.handle_key("Down");
        assert_eq!(help.scroll_offset, 1);

        help.handle_key("j");
        assert_eq!(help.scroll_offset, 2);
    }

    // 4. Scroll up works
    #[test]
    fn test_scroll_up() {
        let mut help = PrefixHelp::new(sample_bindings());
        help.scroll_offset = 3;

        help.handle_key("Up");
        assert_eq!(help.scroll_offset, 2);

        help.handle_key("k");
        assert_eq!(help.scroll_offset, 1);
    }

    // 5. Scroll up at zero stays at zero
    #[test]
    fn test_scroll_up_at_zero() {
        let mut help = PrefixHelp::new(sample_bindings());
        assert_eq!(help.scroll_offset, 0);

        help.handle_key("Up");
        assert_eq!(help.scroll_offset, 0);
    }

    // 6. Scroll down clamps at max
    #[test]
    fn test_scroll_down_clamps() {
        let mut help = PrefixHelp::new(sample_bindings());
        // 6 bindings, max offset = 5
        for _ in 0..20 {
            help.handle_key("Down");
        }
        assert_eq!(help.scroll_offset, 5);
    }

    // 7. Render produces output
    #[test]
    fn test_render_produces_output() {
        let help = PrefixHelp::new(sample_bindings());
        let output = help.render(80, 24);
        assert!(!output.is_empty());
        // Should contain the title
        assert!(output.contains("Prefix Mode"));
        // Should contain the footer hint
        assert!(output.contains("Esc to close"));
        // Should contain key descriptions
        assert!(output.contains("Create new window"));
    }

    // 8. Render with small terminal returns empty
    #[test]
    fn test_render_small_terminal() {
        let help = PrefixHelp::new(sample_bindings());
        let output = help.render(5, 3);
        assert!(output.is_empty());
    }

    // 9. Key display translation works
    #[test]
    fn test_key_display_translation() {
        assert_eq!(translate_key_display("Up"), "\u{2191}");
        assert_eq!(translate_key_display("Down"), "\u{2193}");
        assert_eq!(translate_key_display("Left"), "\u{2190}");
        assert_eq!(translate_key_display("Right"), "\u{2192}");
        assert_eq!(translate_key_display("c"), "c");
    }

    // 10. From command bindings works
    #[test]
    fn test_from_command_bindings() {
        let bindings = vec![
            ("c".to_string(), "window:create".to_string()),
            ("Up".to_string(), "pane:focus-up".to_string()),
        ];
        let help = PrefixHelp::from_command_bindings(&bindings);
        assert_eq!(help.bindings.len(), 2);
        assert_eq!(help.bindings[0].key, "c");
        assert_eq!(help.bindings[0].description, "Create new window");
        assert_eq!(help.bindings[1].key, "\u{2191}");
        assert_eq!(help.bindings[1].description, "Focus pane above");
    }

    // 11. Unknown command shown verbatim
    #[test]
    fn test_unknown_command_description() {
        let bindings = vec![("x".to_string(), "custom:thing".to_string())];
        let help = PrefixHelp::from_command_bindings(&bindings);
        assert_eq!(help.bindings[0].description, "custom:thing");
    }

    // 12. New help starts visible
    #[test]
    fn test_new_starts_visible() {
        let help = PrefixHelp::new(sample_bindings());
        assert!(help.visible);
        assert_eq!(help.scroll_offset, 0);
    }

    // 13. Unrecognized keys return None
    #[test]
    fn test_unrecognized_key_returns_none() {
        let mut help = PrefixHelp::new(sample_bindings());
        let action = help.handle_key("a");
        assert_eq!(action, PrefixHelpAction::None);
    }

    // 14. Empty bindings render correctly
    #[test]
    fn test_empty_bindings_render() {
        let help = PrefixHelp::new(vec![]);
        let output = help.render(80, 24);
        assert!(!output.is_empty());
        assert!(output.contains("Prefix Mode"));
        assert!(output.contains("No bindings"));
    }

    // 15. Render contains binding keys
    #[test]
    fn test_render_contains_keys() {
        let help = PrefixHelp::new(sample_bindings());
        let output = help.render(80, 24);
        assert!(output.contains("Next window"));
        assert!(output.contains("Previous window"));
        assert!(output.contains("Detach from session"));
    }
}
