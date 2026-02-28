/// Vi-style copy mode overlay for navigating scrollback buffer,
/// selecting text, and copying to clipboard.
use std::fmt::Write as FmtWrite;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum CopyModePhase {
    Navigate,
    VisualChar,
    VisualLine,
    Search,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone)]
pub enum CopyModeAction {
    None,
    /// Text to copy to clipboard
    Yank(String),
    /// Copy mode should be exited
    Exit,
    /// Viewport changed, needs re-render
    ScrollChanged,
}

/// Inclusive range describing the current visual selection.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionRange {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
    pub is_line_mode: bool,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CopyModeState {
    pub active: bool,
    pub phase: CopyModePhase,
    pub pane_id: String,
    /// Absolute row position in buffer (0 = first line).
    pub cursor_row: usize,
    pub cursor_col: usize,
    /// Lines from bottom (0 = viewing bottom of buffer).
    pub scroll_offset: usize,
    /// Visual selection anchor (row).
    pub anchor_row: Option<usize>,
    /// Visual selection anchor (col).
    pub anchor_col: Option<usize>,
    pub search_query: String,
    pub search_direction: SearchDirection,
    pub search_matches: Vec<(usize, usize)>,
    pub current_match_index: Option<usize>,
    /// For multi-key commands like "gg".
    pub pending_key: Option<char>,
    /// Total lines in the scrollback buffer.
    pub buffer_lines: usize,
}

impl CopyModeState {
    pub fn new(pane_id: String, buffer_lines: usize, _viewport_height: usize) -> Self {
        let buffer_lines = buffer_lines.max(1);
        let cursor_row = buffer_lines.saturating_sub(1);
        Self {
            active: true,
            phase: CopyModePhase::Navigate,
            pane_id,
            cursor_row,
            cursor_col: 0,
            scroll_offset: 0,
            anchor_row: None,
            anchor_col: None,
            search_query: String::new(),
            search_direction: SearchDirection::Forward,
            search_matches: Vec::new(),
            current_match_index: None,
            pending_key: None,
            buffer_lines,
        }
    }

    // -- viewport helpers ---------------------------------------------------

    /// First visible row index (inclusive).
    pub fn first_visible_row(&self, viewport_height: usize) -> usize {
        self.buffer_lines
            .saturating_sub(viewport_height)
            .saturating_sub(self.scroll_offset)
    }

    /// Last visible row index (inclusive).
    pub fn last_visible_row(&self, viewport_height: usize) -> usize {
        let first = self.first_visible_row(viewport_height);
        (first + viewport_height)
            .min(self.buffer_lines)
            .saturating_sub(1)
    }

    /// Adjust `scroll_offset` so the cursor is within the visible viewport.
    fn ensure_cursor_visible(&mut self, viewport_height: usize) {
        if viewport_height == 0 || self.buffer_lines == 0 {
            return;
        }

        let first = self.first_visible_row(viewport_height);
        let last = self.last_visible_row(viewport_height);

        if self.cursor_row < first {
            // Cursor is above viewport -- scroll up
            let deficit = first - self.cursor_row;
            self.scroll_offset = self.scroll_offset.saturating_add(deficit);
        } else if self.cursor_row > last {
            // Cursor is below viewport -- scroll down
            let excess = self.cursor_row - last;
            self.scroll_offset = self.scroll_offset.saturating_sub(excess);
        }

        // Clamp scroll_offset so we don't scroll past the top of the buffer
        let max_offset = self.buffer_lines.saturating_sub(viewport_height);
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    // -- selection helpers --------------------------------------------------

    /// Returns the normalised selection range (start <= end) if a visual
    /// selection is active.
    pub fn selection_range(&self) -> Option<SelectionRange> {
        let anchor_row = self.anchor_row?;
        let anchor_col = self.anchor_col.unwrap_or(0);
        let is_line_mode = self.phase == CopyModePhase::VisualLine;

        let (start_row, start_col, end_row, end_col) =
            if (anchor_row, anchor_col) <= (self.cursor_row, self.cursor_col) {
                (anchor_row, anchor_col, self.cursor_row, self.cursor_col)
            } else {
                (self.cursor_row, self.cursor_col, anchor_row, anchor_col)
            };

        Some(SelectionRange {
            start_row,
            start_col,
            end_row,
            end_col,
            is_line_mode,
        })
    }

    /// Build the selected text using buffer accessors.
    #[allow(dead_code)]
    pub fn yank_selection(
        &self,
        get_cell: impl Fn(usize, usize) -> char,
        get_line_length: impl Fn(usize) -> usize,
    ) -> String {
        let range = match self.selection_range() {
            Some(r) => r,
            None => return String::new(),
        };

        let mut result = String::new();

        if range.is_line_mode {
            for row in range.start_row..=range.end_row {
                let len = get_line_length(row);
                for col in 0..len {
                    result.push(get_cell(row, col));
                }
                if row < range.end_row {
                    result.push('\n');
                }
            }
        } else {
            for row in range.start_row..=range.end_row {
                let col_start = if row == range.start_row {
                    range.start_col
                } else {
                    0
                };
                let col_end = if row == range.end_row {
                    range.end_col
                } else {
                    get_line_length(row).saturating_sub(1)
                };
                for col in col_start..=col_end {
                    result.push(get_cell(row, col));
                }
                if row < range.end_row {
                    result.push('\n');
                }
            }
        }

        result
    }

    // -- key dispatch -------------------------------------------------------

    pub fn handle_key(&mut self, key: &str, viewport_height: usize) -> CopyModeAction {
        match self.phase {
            CopyModePhase::Search => self.handle_search_key(key),
            _ => self.handle_navigate_key(key, viewport_height),
        }
    }

    // -- search key handling ------------------------------------------------

    fn handle_search_key(&mut self, key: &str) -> CopyModeAction {
        match key {
            "Enter" => {
                // Execute search, return to Navigate
                self.phase = CopyModePhase::Navigate;
                // Search execution is deferred to the caller who has buffer access.
                // We just record the query and direction.
                CopyModeAction::ScrollChanged
            }
            "Escape" => {
                self.search_query.clear();
                self.phase = CopyModePhase::Navigate;
                CopyModeAction::ScrollChanged
            }
            "Backspace" => {
                self.search_query.pop();
                CopyModeAction::ScrollChanged
            }
            "Ctrl+U" | "C-u" => {
                self.search_query.clear();
                CopyModeAction::ScrollChanged
            }
            _ => {
                // Single printable character
                if key.len() == 1 {
                    let ch = key.chars().next().unwrap();
                    if !ch.is_control() {
                        self.search_query.push(ch);
                    }
                }
                CopyModeAction::ScrollChanged
            }
        }
    }

    // -- navigate / visual key handling -------------------------------------

    fn handle_navigate_key(&mut self, key: &str, viewport_height: usize) -> CopyModeAction {
        // Handle pending multi-key sequences
        if let Some(pending) = self.pending_key.take()
            && pending == 'g'
            && key == "g"
        {
            self.cursor_row = 0;
            self.cursor_col = 0;
            self.ensure_cursor_visible(viewport_height);
            return CopyModeAction::ScrollChanged;
        }
        // If the second key doesn't complete a sequence, fall through
        // to handle the new key normally.

        match key {
            // -- movement -----------------------------------------------
            "h" | "Left" => {
                self.cursor_col = self.cursor_col.saturating_sub(1);
                CopyModeAction::ScrollChanged
            }
            "j" | "Down" => {
                if self.cursor_row + 1 < self.buffer_lines {
                    self.cursor_row += 1;
                    self.ensure_cursor_visible(viewport_height);
                }
                CopyModeAction::ScrollChanged
            }
            "k" | "Up" => {
                self.cursor_row = self.cursor_row.saturating_sub(1);
                self.ensure_cursor_visible(viewport_height);
                CopyModeAction::ScrollChanged
            }
            "l" | "Right" => {
                self.cursor_col += 1;
                CopyModeAction::ScrollChanged
            }

            // -- line start / end ---------------------------------------
            "0" => {
                self.cursor_col = 0;
                CopyModeAction::ScrollChanged
            }
            "$" => {
                // Line end – the caller should clamp to actual line length.
                // We set to a large value; the renderer / caller will clamp.
                self.cursor_col = usize::MAX;
                CopyModeAction::ScrollChanged
            }

            // -- word motion (simplified) --------------------------------
            "w" => {
                // Forward word: just move cursor_col forward by a step.
                // Real word motion needs buffer access; provide a simple
                // placeholder that advances by 1 for the state machine.
                self.cursor_col += 1;
                CopyModeAction::ScrollChanged
            }
            "b" => {
                self.cursor_col = self.cursor_col.saturating_sub(1);
                CopyModeAction::ScrollChanged
            }

            // -- multi-key: gg ------------------------------------------
            "g" => {
                self.pending_key = Some('g');
                CopyModeAction::None
            }

            // -- buffer extremes ----------------------------------------
            "G" => {
                self.cursor_row = self.buffer_lines.saturating_sub(1);
                self.cursor_col = 0;
                self.scroll_offset = 0;
                CopyModeAction::ScrollChanged
            }

            // -- viewport-relative motion --------------------------------
            "H" => {
                self.cursor_row = self.first_visible_row(viewport_height);
                CopyModeAction::ScrollChanged
            }
            "M" => {
                let first = self.first_visible_row(viewport_height);
                let last = self.last_visible_row(viewport_height);
                self.cursor_row = first + (last - first) / 2;
                CopyModeAction::ScrollChanged
            }
            "L" => {
                self.cursor_row = self.last_visible_row(viewport_height);
                CopyModeAction::ScrollChanged
            }

            // -- scrolling -----------------------------------------------
            "Ctrl+U" | "C-u" => {
                let half = viewport_height / 2;
                self.scroll_offset = self.scroll_offset.saturating_add(half);
                let max_offset = self.buffer_lines.saturating_sub(viewport_height);
                self.scroll_offset = self.scroll_offset.min(max_offset);
                self.cursor_row = self.cursor_row.saturating_sub(half);
                self.ensure_cursor_visible(viewport_height);
                CopyModeAction::ScrollChanged
            }
            "Ctrl+D" | "C-d" => {
                let half = viewport_height / 2;
                self.scroll_offset = self.scroll_offset.saturating_sub(half);
                self.cursor_row = (self.cursor_row + half).min(self.buffer_lines.saturating_sub(1));
                self.ensure_cursor_visible(viewport_height);
                CopyModeAction::ScrollChanged
            }
            "Ctrl+B" | "C-b" => {
                self.scroll_offset = self.scroll_offset.saturating_add(viewport_height);
                let max_offset = self.buffer_lines.saturating_sub(viewport_height);
                self.scroll_offset = self.scroll_offset.min(max_offset);
                self.cursor_row = self.cursor_row.saturating_sub(viewport_height);
                self.ensure_cursor_visible(viewport_height);
                CopyModeAction::ScrollChanged
            }
            "Ctrl+F" | "C-f" => {
                self.scroll_offset = self.scroll_offset.saturating_sub(viewport_height);
                self.cursor_row =
                    (self.cursor_row + viewport_height).min(self.buffer_lines.saturating_sub(1));
                self.ensure_cursor_visible(viewport_height);
                CopyModeAction::ScrollChanged
            }

            // -- visual mode toggles -------------------------------------
            "v" => {
                if self.phase == CopyModePhase::VisualChar {
                    // Toggle off
                    self.phase = CopyModePhase::Navigate;
                    self.anchor_row = None;
                    self.anchor_col = None;
                } else {
                    self.phase = CopyModePhase::VisualChar;
                    self.anchor_row = Some(self.cursor_row);
                    self.anchor_col = Some(self.cursor_col);
                }
                CopyModeAction::ScrollChanged
            }
            "V" => {
                if self.phase == CopyModePhase::VisualLine {
                    self.phase = CopyModePhase::Navigate;
                    self.anchor_row = None;
                    self.anchor_col = None;
                } else {
                    self.phase = CopyModePhase::VisualLine;
                    self.anchor_row = Some(self.cursor_row);
                    self.anchor_col = Some(self.cursor_col);
                }
                CopyModeAction::ScrollChanged
            }

            // -- yank / copy ---------------------------------------------
            "y" => {
                if self.phase == CopyModePhase::VisualChar
                    || self.phase == CopyModePhase::VisualLine
                {
                    // Signal that yank is requested. The caller must call
                    // `yank_selection()` with buffer accessors to get the text.
                    CopyModeAction::Yank(String::new())
                } else {
                    CopyModeAction::None
                }
            }
            "Enter" => {
                // Yank and exit
                if self.phase == CopyModePhase::VisualChar
                    || self.phase == CopyModePhase::VisualLine
                {
                    self.active = false;
                    CopyModeAction::Yank(String::new())
                } else {
                    self.active = false;
                    CopyModeAction::Exit
                }
            }

            // -- exit / cancel -------------------------------------------
            "q" | "Escape" | "Ctrl+C" | "C-c" => {
                if self.phase == CopyModePhase::VisualChar
                    || self.phase == CopyModePhase::VisualLine
                {
                    // Cancel visual selection, return to navigate
                    self.phase = CopyModePhase::Navigate;
                    self.anchor_row = None;
                    self.anchor_col = None;
                    CopyModeAction::ScrollChanged
                } else {
                    self.active = false;
                    CopyModeAction::Exit
                }
            }

            // -- search entry --------------------------------------------
            "/" => {
                self.phase = CopyModePhase::Search;
                self.search_direction = SearchDirection::Forward;
                self.search_query.clear();
                CopyModeAction::ScrollChanged
            }
            "?" => {
                self.phase = CopyModePhase::Search;
                self.search_direction = SearchDirection::Backward;
                self.search_query.clear();
                CopyModeAction::ScrollChanged
            }

            // -- search navigation ---------------------------------------
            "n" => {
                if !self.search_matches.is_empty() {
                    let idx = match self.current_match_index {
                        Some(i) => (i + 1) % self.search_matches.len(),
                        None => 0,
                    };
                    self.current_match_index = Some(idx);
                    let (row, col) = self.search_matches[idx];
                    self.cursor_row = row;
                    self.cursor_col = col;
                    self.ensure_cursor_visible(viewport_height);
                }
                CopyModeAction::ScrollChanged
            }
            "N" => {
                if !self.search_matches.is_empty() {
                    let idx = match self.current_match_index {
                        Some(0) | None => self.search_matches.len() - 1,
                        Some(i) => i - 1,
                    };
                    self.current_match_index = Some(idx);
                    let (row, col) = self.search_matches[idx];
                    self.cursor_row = row;
                    self.cursor_col = col;
                    self.ensure_cursor_visible(viewport_height);
                }
                CopyModeAction::ScrollChanged
            }

            _ => CopyModeAction::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

pub struct CopyModeRenderer;

impl CopyModeRenderer {
    /// Render copy mode overlay for a pane area.
    ///
    /// Returns an ANSI string to write to the terminal.
    ///
    /// * `get_cell` – returns the character at (row, col) in the scrollback
    ///   buffer.
    /// * `get_line_length` – returns the number of columns in the given row.
    /// * `pane_x`, `pane_y` – top-left corner of the pane in the terminal.
    /// * `pane_width`, `pane_height` – dimensions of the pane.
    pub fn render(
        state: &CopyModeState,
        get_cell: impl Fn(usize, usize) -> char,
        get_line_length: impl Fn(usize) -> usize,
        pane_x: u16,
        pane_y: u16,
        pane_width: u16,
        pane_height: u16,
    ) -> String {
        let vh = pane_height as usize;
        let vw = pane_width as usize;
        if vh == 0 || vw == 0 {
            return String::new();
        }

        let mut output = String::with_capacity(vh * vw * 4);

        let first_row = state.first_visible_row(vh);
        let selection = state.selection_range();

        // Determine search-match set for quick lookup
        let search_set: std::collections::HashSet<(usize, usize)> =
            state.search_matches.iter().copied().collect();
        let current_match = state
            .current_match_index
            .and_then(|i| state.search_matches.get(i).copied());

        for screen_y in 0..vh {
            let buf_row = first_row + screen_y;

            // Position cursor at the start of this screen row
            let _ = write!(
                output,
                "\x1b[{};{}H",
                pane_y as usize + screen_y + 1,
                pane_x as usize + 1
            );

            if buf_row >= state.buffer_lines {
                // Past the end of the buffer -- render tilde line
                let _ = write!(output, "\x1b[90m~");
                let _ = write!(output, "{}\x1b[0m", " ".repeat(vw.saturating_sub(1)));
                continue;
            }

            let line_len = get_line_length(buf_row);

            for screen_x in 0..vw {
                let buf_col = screen_x;
                let ch = if buf_col < line_len {
                    get_cell(buf_row, buf_col)
                } else {
                    ' '
                };

                let is_cursor = buf_row == state.cursor_row && buf_col == state.cursor_col;
                let is_selected = Self::is_in_selection(buf_row, buf_col, &selection);
                let is_search = search_set.contains(&(buf_row, buf_col));
                let is_current_search = current_match == Some((buf_row, buf_col));

                // Build style
                if is_cursor {
                    // Cursor: inverse video
                    let _ = write!(output, "\x1b[7m{}\x1b[0m", ch);
                } else if is_current_search {
                    // Current search match: orange background
                    let _ = write!(output, "\x1b[48;5;208m\x1b[30m{}\x1b[0m", ch);
                } else if is_search {
                    // Other search matches: yellow background
                    let _ = write!(output, "\x1b[43m\x1b[30m{}\x1b[0m", ch);
                } else if is_selected {
                    // Visual selection: inverse video
                    let _ = write!(output, "\x1b[7m{}\x1b[0m", ch);
                } else {
                    let _ = write!(output, "{}", ch);
                }
            }
        }

        // 5. Position indicator top-right: [line/total]
        let indicator = format!("[{}/{}]", state.cursor_row + 1, state.buffer_lines);
        if indicator.len() < vw {
            let ind_x = pane_x as usize + vw - indicator.len();
            let _ = write!(
                output,
                "\x1b[{};{}H\x1b[1;33m{}\x1b[0m",
                pane_y as usize + 1,
                ind_x + 1,
                indicator
            );
        }

        // 6. Search bar at bottom (if in search phase)
        if state.phase == CopyModePhase::Search {
            let prompt = if state.search_direction == SearchDirection::Forward {
                "/"
            } else {
                "?"
            };
            let search_line = format!("{}{}", prompt, state.search_query);
            let display: String = if search_line.len() > vw {
                search_line[..vw].to_string()
            } else {
                let padding = " ".repeat(vw - search_line.len());
                format!("{}{}", search_line, padding)
            };
            let _ = write!(
                output,
                "\x1b[{};{}H\x1b[1;7m{}\x1b[0m",
                pane_y as usize + vh,
                pane_x as usize + 1,
                display,
            );
        }

        // Mode indicator at bottom-left (when not searching)
        if state.phase != CopyModePhase::Search {
            let mode_str = match state.phase {
                CopyModePhase::Navigate => "[COPY]",
                CopyModePhase::VisualChar => "[VISUAL]",
                CopyModePhase::VisualLine => "[V-LINE]",
                CopyModePhase::Search => unreachable!(),
            };
            let _ = write!(
                output,
                "\x1b[{};{}H\x1b[1;33m{}\x1b[0m",
                pane_y as usize + vh,
                pane_x as usize + 1,
                mode_str,
            );
        }

        output
    }

    fn is_in_selection(row: usize, col: usize, selection: &Option<SelectionRange>) -> bool {
        let sel = match selection {
            Some(s) => s,
            None => return false,
        };

        if sel.is_line_mode {
            row >= sel.start_row && row <= sel.end_row
        } else {
            if row < sel.start_row || row > sel.end_row {
                return false;
            }
            if sel.start_row == sel.end_row {
                col >= sel.start_col && col <= sel.end_col
            } else if row == sel.start_row {
                col >= sel.start_col
            } else if row == sel.end_row {
                col <= sel.end_col
            } else {
                true
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const VP: usize = 24; // viewport height for tests
    const BUF: usize = 100; // buffer lines for tests

    fn new_state() -> CopyModeState {
        CopyModeState::new("test-pane".into(), BUF, VP)
    }

    // 1. New state starts in Navigate phase
    #[test]
    fn test_new_state_navigate() {
        let s = new_state();
        assert!(s.active);
        assert_eq!(s.phase, CopyModePhase::Navigate);
        assert_eq!(s.cursor_row, BUF - 1);
        assert_eq!(s.cursor_col, 0);
        assert_eq!(s.scroll_offset, 0);
        assert!(s.anchor_row.is_none());
        assert!(s.anchor_col.is_none());
    }

    // 2. h/j/k/l movement
    #[test]
    fn test_hjkl_movement() {
        let mut s = new_state();
        // Start at row 99, col 0

        // l -> col 1
        s.handle_key("l", VP);
        assert_eq!(s.cursor_col, 1);

        // h -> col 0
        s.handle_key("h", VP);
        assert_eq!(s.cursor_col, 0);

        // h at 0 stays at 0
        s.handle_key("h", VP);
        assert_eq!(s.cursor_col, 0);

        // k -> row 98
        s.handle_key("k", VP);
        assert_eq!(s.cursor_row, 98);

        // j -> row 99
        s.handle_key("j", VP);
        assert_eq!(s.cursor_row, 99);

        // j at bottom stays at bottom
        s.handle_key("j", VP);
        assert_eq!(s.cursor_row, 99);
    }

    // 3. 0 and $ (line start/end)
    #[test]
    fn test_line_start_end() {
        let mut s = new_state();
        s.cursor_col = 10;

        s.handle_key("0", VP);
        assert_eq!(s.cursor_col, 0);

        s.handle_key("$", VP);
        assert_eq!(s.cursor_col, usize::MAX);
    }

    // 4. g+g goes to buffer start
    #[test]
    fn test_gg_goes_to_top() {
        let mut s = new_state();
        assert_eq!(s.cursor_row, 99);

        // First g sets pending
        let action = s.handle_key("g", VP);
        assert!(matches!(action, CopyModeAction::None));
        assert_eq!(s.pending_key, Some('g'));

        // Second g completes the sequence
        let action = s.handle_key("g", VP);
        assert!(matches!(action, CopyModeAction::ScrollChanged));
        assert_eq!(s.cursor_row, 0);
        assert_eq!(s.cursor_col, 0);
    }

    // 5. G goes to buffer end
    #[test]
    fn test_big_g_goes_to_bottom() {
        let mut s = new_state();
        s.cursor_row = 10;
        s.scroll_offset = 50;

        s.handle_key("G", VP);
        assert_eq!(s.cursor_row, 99);
        assert_eq!(s.scroll_offset, 0);
    }

    // 6. Ctrl+U/D scrolling
    #[test]
    fn test_ctrl_u_d_scrolling() {
        let mut s = new_state();
        // Start at bottom: row=99, scroll_offset=0
        let half = VP / 2; // 12

        // Ctrl+U: scroll up
        s.handle_key("C-u", VP);
        assert_eq!(s.scroll_offset, half);
        assert_eq!(s.cursor_row, 99 - half);

        // Ctrl+D: scroll back down
        s.handle_key("C-d", VP);
        assert_eq!(s.scroll_offset, 0);
        assert_eq!(s.cursor_row, 99);
    }

    // 7. v toggles visual char mode
    #[test]
    fn test_v_toggles_visual_char() {
        let mut s = new_state();
        assert_eq!(s.phase, CopyModePhase::Navigate);

        s.handle_key("v", VP);
        assert_eq!(s.phase, CopyModePhase::VisualChar);
        assert_eq!(s.anchor_row, Some(99));
        assert_eq!(s.anchor_col, Some(0));

        // Toggle off
        s.handle_key("v", VP);
        assert_eq!(s.phase, CopyModePhase::Navigate);
        assert!(s.anchor_row.is_none());
    }

    // 8. V toggles visual line mode
    #[test]
    fn test_big_v_toggles_visual_line() {
        let mut s = new_state();

        s.handle_key("V", VP);
        assert_eq!(s.phase, CopyModePhase::VisualLine);
        assert_eq!(s.anchor_row, Some(99));

        s.handle_key("V", VP);
        assert_eq!(s.phase, CopyModePhase::Navigate);
        assert!(s.anchor_row.is_none());
    }

    // 9. Escape exits copy mode
    #[test]
    fn test_escape_exits() {
        let mut s = new_state();
        let action = s.handle_key("Escape", VP);
        assert!(matches!(action, CopyModeAction::Exit));
        assert!(!s.active);
    }

    // 10. Escape in visual mode returns to Navigate (not exit)
    #[test]
    fn test_escape_in_visual_cancels() {
        let mut s = new_state();
        s.handle_key("v", VP);
        assert_eq!(s.phase, CopyModePhase::VisualChar);

        let action = s.handle_key("Escape", VP);
        assert!(matches!(action, CopyModeAction::ScrollChanged));
        assert!(s.active); // still in copy mode
        assert_eq!(s.phase, CopyModePhase::Navigate);
        assert!(s.anchor_row.is_none());
    }

    // 11. Search mode: typing adds to query
    #[test]
    fn test_search_typing() {
        let mut s = new_state();

        // Enter search forward
        s.handle_key("/", VP);
        assert_eq!(s.phase, CopyModePhase::Search);
        assert_eq!(s.search_direction, SearchDirection::Forward);

        // Type some characters
        s.handle_key("h", VP);
        s.handle_key("e", VP);
        s.handle_key("l", VP);
        s.handle_key("l", VP);
        s.handle_key("o", VP);
        assert_eq!(s.search_query, "hello");

        // Backspace removes last char
        s.handle_key("Backspace", VP);
        assert_eq!(s.search_query, "hell");
    }

    // 12. Search mode: Enter returns to Navigate
    #[test]
    fn test_search_enter_returns() {
        let mut s = new_state();

        s.handle_key("/", VP);
        s.handle_key("f", VP);
        s.handle_key("o", VP);
        s.handle_key("o", VP);
        assert_eq!(s.search_query, "foo");

        s.handle_key("Enter", VP);
        assert_eq!(s.phase, CopyModePhase::Navigate);
        assert_eq!(s.search_query, "foo"); // query preserved
    }

    // -- Additional tests ---------------------------------------------------

    // 13. Arrow keys work same as hjkl
    #[test]
    fn test_arrow_keys() {
        let mut s = new_state();
        s.cursor_col = 5;
        s.cursor_row = 50;

        s.handle_key("Left", VP);
        assert_eq!(s.cursor_col, 4);

        s.handle_key("Right", VP);
        assert_eq!(s.cursor_col, 5);

        s.handle_key("Up", VP);
        assert_eq!(s.cursor_row, 49);

        s.handle_key("Down", VP);
        assert_eq!(s.cursor_row, 50);
    }

    // 14. H/M/L viewport-relative
    #[test]
    fn test_h_m_l_viewport() {
        let mut s = new_state();
        // At bottom, scroll_offset=0. first_visible = 100-24 = 76, last = 99

        s.handle_key("H", VP);
        assert_eq!(s.cursor_row, 76);

        s.handle_key("L", VP);
        assert_eq!(s.cursor_row, 99);

        s.handle_key("M", VP);
        assert_eq!(s.cursor_row, 76 + (99 - 76) / 2); // 76 + 11 = 87
    }

    // 15. Ctrl+B / Ctrl+F full page
    #[test]
    fn test_ctrl_b_f_scrolling() {
        let mut s = new_state();

        s.handle_key("C-b", VP);
        assert_eq!(s.scroll_offset, VP);
        assert_eq!(s.cursor_row, 99 - VP);

        s.handle_key("C-f", VP);
        assert_eq!(s.scroll_offset, 0);
        assert_eq!(s.cursor_row, 99);
    }

    // 16. Search backward mode
    #[test]
    fn test_search_backward() {
        let mut s = new_state();
        s.handle_key("?", VP);
        assert_eq!(s.phase, CopyModePhase::Search);
        assert_eq!(s.search_direction, SearchDirection::Backward);
    }

    // 17. Search escape cancels
    #[test]
    fn test_search_escape_cancels() {
        let mut s = new_state();
        s.handle_key("/", VP);
        s.handle_key("a", VP);
        s.handle_key("b", VP);
        assert_eq!(s.search_query, "ab");

        s.handle_key("Escape", VP);
        assert_eq!(s.phase, CopyModePhase::Navigate);
        assert!(s.search_query.is_empty()); // cleared on escape
    }

    // 18. Yank in visual mode returns Yank action
    #[test]
    fn test_yank_in_visual() {
        let mut s = new_state();
        s.handle_key("v", VP);
        s.handle_key("l", VP);
        s.handle_key("l", VP);

        let action = s.handle_key("y", VP);
        assert!(matches!(action, CopyModeAction::Yank(_)));
    }

    // 19. Yank outside visual mode does nothing
    #[test]
    fn test_yank_outside_visual() {
        let mut s = new_state();
        let action = s.handle_key("y", VP);
        assert!(matches!(action, CopyModeAction::None));
    }

    // 20. q exits copy mode
    #[test]
    fn test_q_exits() {
        let mut s = new_state();
        let action = s.handle_key("q", VP);
        assert!(matches!(action, CopyModeAction::Exit));
        assert!(!s.active);
    }

    // 21. Selection range works correctly
    #[test]
    fn test_selection_range() {
        let mut s = new_state();

        // No selection in navigate
        assert!(s.selection_range().is_none());

        // Enter visual char, move cursor
        s.handle_key("v", VP);
        s.handle_key("l", VP);
        s.handle_key("l", VP);

        let range = s.selection_range().unwrap();
        assert_eq!(range.start_row, 99);
        assert_eq!(range.start_col, 0);
        assert_eq!(range.end_row, 99);
        assert_eq!(range.end_col, 2);
        assert!(!range.is_line_mode);
    }

    // 22. Visual line selection range
    #[test]
    fn test_visual_line_selection_range() {
        let mut s = new_state();
        s.cursor_row = 50;
        s.handle_key("V", VP);
        s.cursor_row = 52;

        let range = s.selection_range().unwrap();
        assert_eq!(range.start_row, 50);
        assert_eq!(range.end_row, 52);
        assert!(range.is_line_mode);
    }

    // 23. Render produces non-empty output
    #[test]
    fn test_render_non_empty() {
        let s = new_state();
        let output = CopyModeRenderer::render(&s, |_row, _col| ' ', |_row| 80, 0, 0, 80, 24);
        assert!(!output.is_empty());
        // Should contain the position indicator
        assert!(output.contains("[100/100]"));
    }

    // 24. Render search bar appears in search mode
    #[test]
    fn test_render_search_bar() {
        let mut s = new_state();
        s.handle_key("/", VP);
        s.handle_key("t", VP);
        s.handle_key("e", VP);
        s.handle_key("s", VP);
        s.handle_key("t", VP);

        let output = CopyModeRenderer::render(&s, |_row, _col| ' ', |_row| 80, 0, 0, 80, 24);
        assert!(output.contains("/test"));
    }

    // 25. n/N search navigation
    #[test]
    fn test_search_navigation() {
        let mut s = new_state();
        s.search_matches = vec![(10, 5), (20, 3), (30, 8)];

        // n goes to first match
        s.handle_key("n", VP);
        assert_eq!(s.current_match_index, Some(0));
        assert_eq!(s.cursor_row, 10);
        assert_eq!(s.cursor_col, 5);

        // n again goes to next
        s.handle_key("n", VP);
        assert_eq!(s.current_match_index, Some(1));
        assert_eq!(s.cursor_row, 20);

        // N goes back
        s.handle_key("N", VP);
        assert_eq!(s.current_match_index, Some(0));
        assert_eq!(s.cursor_row, 10);

        // N wraps to end
        s.handle_key("N", VP);
        assert_eq!(s.current_match_index, Some(2));
        assert_eq!(s.cursor_row, 30);
    }

    // 26. Search Ctrl+U clears query
    #[test]
    fn test_search_ctrl_u_clears() {
        let mut s = new_state();
        s.handle_key("/", VP);
        s.handle_key("a", VP);
        s.handle_key("b", VP);
        s.handle_key("c", VP);
        assert_eq!(s.search_query, "abc");

        s.handle_key("C-u", VP);
        assert!(s.search_query.is_empty());
    }

    // 27. Enter in navigate mode exits
    #[test]
    fn test_enter_in_navigate_exits() {
        let mut s = new_state();
        let action = s.handle_key("Enter", VP);
        assert!(matches!(action, CopyModeAction::Exit));
        assert!(!s.active);
    }

    // 28. Enter in visual mode yanks and exits
    #[test]
    fn test_enter_in_visual_yanks_and_exits() {
        let mut s = new_state();
        s.handle_key("v", VP);
        let action = s.handle_key("Enter", VP);
        assert!(matches!(action, CopyModeAction::Yank(_)));
        assert!(!s.active);
    }

    // 29. Pending key cleared on non-matching second key
    #[test]
    fn test_pending_key_cleared_on_mismatch() {
        let mut s = new_state();
        s.handle_key("g", VP);
        assert_eq!(s.pending_key, Some('g'));

        // Press something other than 'g' - pending cleared, key handled normally
        s.handle_key("j", VP);
        assert!(s.pending_key.is_none());
        // j should still have moved cursor down (but we're at bottom, so no change)
    }

    // 30. yank_selection extracts correct text
    #[test]
    fn test_yank_selection_text() {
        let mut s = CopyModeState::new("test".into(), 10, 10);
        s.cursor_row = 0;
        s.cursor_col = 0;
        s.handle_key("v", 10);
        // Move to col 4
        s.handle_key("l", 10);
        s.handle_key("l", 10);
        s.handle_key("l", 10);
        s.handle_key("l", 10);
        // anchor at (0,0), cursor at (0,4)

        let text = s.yank_selection(|_row, col| b"Hello World"[col] as char, |_row| 11);
        assert_eq!(text, "Hello");
    }
}
