// Virtual terminal wrapper module
//
// Wraps alacritty_terminal::Term to provide VT100/VT220 terminal emulation.
// Each pane in the multiplexer has a VirtualTerminal that processes escape
// sequences and maintains the screen grid.

use std::collections::HashMap;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{CursorShape, Processor};

/// Event proxy that discards all events from the terminal.
/// We poll terminal state directly rather than using event-driven updates.
#[derive(Clone)]
struct EventProxy;

impl EventListener for EventProxy {
    fn send_event(&self, _event: Event) {
        // We poll state directly, no event handling needed
    }
}

/// Dimensions type used to create and resize the terminal grid.
struct TermDimensions {
    columns: usize,
    screen_lines: usize,
}

impl Dimensions for TermDimensions {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

/// A virtual terminal emulator backed by alacritty_terminal.
///
/// This wraps `Term<EventProxy>` and provides a simplified API for the
/// multiplexer to write bytes, read cells, and query terminal state.
pub struct VirtualTerminal {
    term: Term<EventProxy>,
    parser: Processor,
    cols: u16,
    rows: u16,
}

impl VirtualTerminal {
    /// Create a new virtual terminal with the given dimensions and scrollback.
    pub fn new(cols: u16, rows: u16, scrollback: usize) -> Self {
        let dimensions = TermDimensions {
            columns: cols as usize,
            screen_lines: rows as usize,
        };

        let config = Config {
            scrolling_history: scrollback,
            ..Config::default()
        };

        let term = Term::new(config, &dimensions, EventProxy);
        let parser = Processor::new();

        Self {
            term,
            parser,
            cols,
            rows,
        }
    }

    /// Feed raw bytes through the VTE parser into the terminal.
    ///
    /// This processes escape sequences, control characters, and printable
    /// text, updating the terminal grid accordingly.
    pub fn write(&mut self, data: &[u8]) {
        self.parser.advance(&mut self.term, data);
    }

    /// Resize the terminal grid to new dimensions.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;

        let dimensions = TermDimensions {
            columns: cols as usize,
            screen_lines: rows as usize,
        };

        self.term.resize(dimensions);
    }

    /// Return the current cursor position as (column, row).
    ///
    /// Both values are zero-indexed.
    pub fn cursor_position(&self) -> (u16, u16) {
        let cursor = &self.term.grid().cursor;
        let col = cursor.point.column.0 as u16;
        let row = cursor.point.line.0 as u16;
        (col, row)
    }

    /// Return the current cursor shape.
    pub fn cursor_shape(&self) -> CursorShape {
        self.term.cursor_style().shape
    }

    /// Check if the cursor is visible (DECTCEM mode).
    pub fn is_cursor_visible(&self) -> bool {
        self.term.mode().contains(TermMode::SHOW_CURSOR)
    }

    /// Check if any mouse tracking mode is active.
    pub fn is_mouse_tracking_active(&self) -> bool {
        self.term.mode().intersects(TermMode::MOUSE_MODE)
    }

    /// Check if bracketed paste mode is active.
    pub fn is_bracketed_paste_active(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Access a cell in the visible grid at the given (col, row) position.
    ///
    /// Both col and row are zero-indexed. Row 0 is the topmost visible line.
    pub fn cell(&self, col: u16, row: u16) -> &Cell {
        let point = Point::new(Line(row as i32), Column(col as usize));
        &self.term.grid()[point]
    }

    /// Return the number of columns in the terminal.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Return the number of rows in the terminal.
    pub fn rows(&self) -> u16 {
        self.rows
    }
}

/// Manages a collection of virtual terminals keyed by string ID.
pub struct TerminalManager {
    terminals: HashMap<String, VirtualTerminal>,
}

impl TerminalManager {
    /// Create a new empty terminal manager.
    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
        }
    }

    /// Create a new virtual terminal with the given ID and dimensions.
    pub fn create(&mut self, id: String, cols: u16, rows: u16, scrollback: usize) {
        let vt = VirtualTerminal::new(cols, rows, scrollback);
        self.terminals.insert(id, vt);
    }

    /// Get an immutable reference to a terminal by ID.
    pub fn get(&self, id: &str) -> Option<&VirtualTerminal> {
        self.terminals.get(id)
    }

    /// Get a mutable reference to a terminal by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut VirtualTerminal> {
        self.terminals.get_mut(id)
    }

    /// Remove a terminal by ID, returning it if it existed.
    pub fn remove(&mut self, id: &str) -> Option<VirtualTerminal> {
        self.terminals.remove(id)
    }
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_and_cursor_position() {
        let mut vt = VirtualTerminal::new(80, 24, 1000);
        vt.write(b"Hello");
        let (col, row) = vt.cursor_position();
        assert_eq!(col, 5);
        assert_eq!(row, 0);
    }

    #[test]
    fn test_newline_moves_cursor() {
        let mut vt = VirtualTerminal::new(80, 24, 1000);
        vt.write(b"Hello\r\nWorld");
        let (col, row) = vt.cursor_position();
        assert_eq!(col, 5);
        assert_eq!(row, 1);
    }

    #[test]
    fn test_resize() {
        let mut vt = VirtualTerminal::new(80, 24, 1000);
        vt.resize(120, 40);
        assert_eq!(vt.cols(), 120);
        assert_eq!(vt.rows(), 40);
    }

    #[test]
    fn test_mouse_tracking_default_off() {
        let vt = VirtualTerminal::new(80, 24, 1000);
        assert!(!vt.is_mouse_tracking_active());
    }

    #[test]
    fn test_bracketed_paste_default_off() {
        let vt = VirtualTerminal::new(80, 24, 1000);
        assert!(!vt.is_bracketed_paste_active());
    }

    #[test]
    fn test_terminal_manager_crud() {
        let mut mgr = TerminalManager::new();
        mgr.create("p1".into(), 80, 24, 1000);
        assert!(mgr.get("p1").is_some());
        assert!(mgr.get("p2").is_none());
        mgr.remove("p1");
        assert!(mgr.get("p1").is_none());
    }
}
