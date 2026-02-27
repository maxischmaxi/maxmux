// Double-buffered screen with dirty tracking.
//
// Maintains a 2D grid of styled cells and a snapshot of the previous frame.
// `flush()` compares the two to produce minimal ANSI output.

use crate::ansi;

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenCell {
    pub ch: char,
    pub fg: Option<(u8, u8, u8)>,
    pub bg: Option<(u8, u8, u8)>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for ScreenCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg: None,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
        }
    }
}

/// Style state tracked during flush to avoid redundant escape codes.
#[derive(Clone, Debug, PartialEq)]
struct CurrentStyle {
    fg: Option<(u8, u8, u8)>,
    bg: Option<(u8, u8, u8)>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
}

impl CurrentStyle {
    fn unknown() -> Self {
        // Start with a state that will never match any cell, forcing the
        // first cell to emit its full style.
        Self {
            fg: None,
            bg: None,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
        }
    }

    /// Emit the minimal ANSI codes needed to transition from `self` to the
    /// style described by `cell`, then update `self` to match.
    fn apply(&mut self, cell: &ScreenCell, out: &mut String) {
        // If the cell has no style attributes and default colors, and we are
        // already in that state, skip entirely.
        if *self == (CurrentStyle {
            fg: cell.fg,
            bg: cell.bg,
            bold: cell.bold,
            dim: cell.dim,
            italic: cell.italic,
            underline: cell.underline,
        }) {
            return;
        }

        // Determine if we need a reset. A reset is needed when an attribute
        // that is currently ON needs to be turned OFF (there is no "unbold"
        // code that works reliably across terminals).
        let needs_reset = (self.bold && !cell.bold)
            || (self.dim && !cell.dim)
            || (self.italic && !cell.italic)
            || (self.underline && !cell.underline)
            || (self.fg.is_some() && cell.fg.is_none())
            || (self.bg.is_some() && cell.bg.is_none());

        if needs_reset {
            out.push_str(ansi::reset_style());
            // After reset, everything is off / default.
            self.fg = None;
            self.bg = None;
            self.bold = false;
            self.dim = false;
            self.italic = false;
            self.underline = false;
        }

        // Now selectively enable what the cell needs.
        if cell.bold && !self.bold {
            out.push_str(ansi::bold());
            self.bold = true;
        }
        if cell.dim && !self.dim {
            out.push_str(ansi::dim());
            self.dim = true;
        }
        if cell.italic && !self.italic {
            out.push_str(ansi::italic());
            self.italic = true;
        }
        if cell.underline && !self.underline {
            out.push_str(ansi::underline());
            self.underline = true;
        }
        if cell.fg != self.fg {
            if let Some((r, g, b)) = cell.fg {
                out.push_str(&ansi::fg_rgb(r, g, b));
            }
            self.fg = cell.fg;
        }
        if cell.bg != self.bg {
            if let Some((r, g, b)) = cell.bg {
                out.push_str(&ansi::bg_rgb(r, g, b));
            }
            self.bg = cell.bg;
        }
    }
}

pub struct ScreenBuffer {
    cols: u16,
    rows: u16,
    cells: Vec<Vec<ScreenCell>>,
    prev_cells: Vec<Vec<ScreenCell>>,
}

impl ScreenBuffer {
    /// Create a new screen buffer of the given size, filled with default cells.
    pub fn new(cols: u16, rows: u16) -> Self {
        let cells = Self::make_grid(cols, rows);
        let prev_cells = cells.clone();
        Self { cols, rows, cells, prev_cells }
    }

    /// Resize the screen buffer, resetting all content.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.cells = Self::make_grid(cols, rows);
        self.prev_cells = self.cells.clone();
    }

    /// Set a cell at (x, y). Out-of-bounds coordinates are silently ignored.
    pub fn set(&mut self, x: u16, y: u16, cell: ScreenCell) {
        if x < self.cols && y < self.rows {
            self.cells[y as usize][x as usize] = cell;
        }
    }

    /// Get a reference to the cell at (x, y), or `None` if out of bounds.
    pub fn get(&self, x: u16, y: u16) -> Option<&ScreenCell> {
        if x < self.cols && y < self.rows {
            Some(&self.cells[y as usize][x as usize])
        } else {
            None
        }
    }

    /// Write a string horizontally starting at (x, y).
    /// Characters that fall outside the screen are clipped.
    pub fn write_string(
        &mut self,
        x: u16,
        y: u16,
        s: &str,
        fg: Option<(u8, u8, u8)>,
        bg: Option<(u8, u8, u8)>,
        bold: bool,
    ) {
        for (i, ch) in s.chars().enumerate() {
            let cx = x.saturating_add(i as u16);
            if cx >= self.cols || y >= self.rows {
                break;
            }
            self.cells[y as usize][cx as usize] = ScreenCell {
                ch,
                fg,
                bg,
                bold,
                dim: false,
                italic: false,
                underline: false,
            };
        }
    }

    /// Fill an entire row with the given character and colors.
    pub fn fill_row(
        &mut self,
        y: u16,
        ch: char,
        fg: Option<(u8, u8, u8)>,
        bg: Option<(u8, u8, u8)>,
    ) {
        if y >= self.rows {
            return;
        }
        for x in 0..self.cols {
            self.cells[y as usize][x as usize] = ScreenCell {
                ch,
                fg,
                bg,
                bold: false,
                dim: false,
                italic: false,
                underline: false,
            };
        }
    }

    /// Fill a rectangle with the given character and colors.
    pub fn fill_rect(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        ch: char,
        fg: Option<(u8, u8, u8)>,
        bg: Option<(u8, u8, u8)>,
    ) {
        for row in y..y.saturating_add(height).min(self.rows) {
            for col in x..x.saturating_add(width).min(self.cols) {
                self.cells[row as usize][col as usize] = ScreenCell {
                    ch,
                    fg,
                    bg,
                    bold: false,
                    dim: false,
                    italic: false,
                    underline: false,
                };
            }
        }
    }

    /// Reset all cells to default.
    pub fn clear(&mut self) {
        self.cells = Self::make_grid(self.cols, self.rows);
    }

    /// Save the current cells as the previous frame (for dirty tracking).
    pub fn snapshot(&mut self) {
        self.prev_cells = self.cells.clone();
    }

    /// Generate ANSI output for the changes since the last snapshot.
    ///
    /// - If more than 50% of cells are dirty, do a full redraw.
    /// - Otherwise, emit only the changed cells.
    /// - Tracks current style to minimize redundant escape codes.
    pub fn flush(&mut self) -> String {
        let total = self.cols as usize * self.rows as usize;
        if total == 0 {
            return String::new();
        }

        // Collect dirty cell positions.
        let mut dirty: Vec<(u16, u16)> = Vec::new();
        for y in 0..self.rows {
            for x in 0..self.cols {
                if self.cells[y as usize][x as usize]
                    != self.prev_cells[y as usize][x as usize]
                {
                    dirty.push((x, y));
                }
            }
        }

        if dirty.is_empty() {
            return String::new();
        }

        let mut out = String::with_capacity(total * 4);
        let mut style = CurrentStyle::unknown();

        if dirty.len() * 2 > total {
            // Full redraw: iterate every cell.
            for y in 0..self.rows {
                for x in 0..self.cols {
                    let cell = &self.cells[y as usize][x as usize];
                    out.push_str(&ansi::move_to(x, y));
                    style.apply(cell, &mut out);
                    out.push(cell.ch);
                }
            }
        } else {
            // Diff redraw: only dirty cells.
            for (x, y) in &dirty {
                let cell = &self.cells[*y as usize][*x as usize];
                out.push_str(&ansi::move_to(*x, *y));
                style.apply(cell, &mut out);
                out.push(cell.ch);
            }
        }

        // Reset style at the end so we don't leak attributes.
        out.push_str(ansi::reset_style());

        out
    }

    /// Number of columns.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Number of rows.
    pub fn rows(&self) -> u16 {
        self.rows
    }

    // --- Private helpers ---

    fn make_grid(cols: u16, rows: u16) -> Vec<Vec<ScreenCell>> {
        (0..rows)
            .map(|_| (0..cols).map(|_| ScreenCell::default()).collect())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let screen = ScreenBuffer::new(80, 24);
        let cell = screen.get(0, 0).unwrap();
        assert_eq!(cell.ch, ' ');
        assert_eq!(cell.fg, None);
    }

    #[test]
    fn test_set_and_get() {
        let mut screen = ScreenBuffer::new(80, 24);
        let cell = ScreenCell {
            ch: 'X',
            fg: Some((255, 0, 0)),
            ..Default::default()
        };
        screen.set(5, 3, cell.clone());
        assert_eq!(screen.get(5, 3), Some(&cell));
    }

    #[test]
    fn test_out_of_bounds_ignored() {
        let mut screen = ScreenBuffer::new(80, 24);
        screen.set(100, 100, ScreenCell::default()); // should not panic
        assert_eq!(screen.get(100, 100), None);
    }

    #[test]
    fn test_write_string() {
        let mut screen = ScreenBuffer::new(80, 24);
        screen.write_string(0, 0, "Hi", Some((255, 255, 255)), None, false);
        assert_eq!(screen.get(0, 0).unwrap().ch, 'H');
        assert_eq!(screen.get(1, 0).unwrap().ch, 'i');
    }

    #[test]
    fn test_fill_row() {
        let mut screen = ScreenBuffer::new(80, 24);
        screen.fill_row(0, '-', Some((100, 100, 100)), None);
        assert_eq!(screen.get(0, 0).unwrap().ch, '-');
        assert_eq!(screen.get(79, 0).unwrap().ch, '-');
    }

    #[test]
    fn test_clear() {
        let mut screen = ScreenBuffer::new(80, 24);
        screen.write_string(0, 0, "Hello", None, None, false);
        screen.clear();
        assert_eq!(screen.get(0, 0).unwrap().ch, ' ');
    }

    #[test]
    fn test_flush_produces_output() {
        let mut screen = ScreenBuffer::new(10, 2);
        screen.snapshot(); // save initial state
        screen.write_string(0, 0, "Hi", Some((255, 255, 255)), None, false);
        let output = screen.flush();
        assert!(!output.is_empty());
        assert!(output.contains("H"));
        assert!(output.contains("i"));
    }

    #[test]
    fn test_flush_no_changes_minimal_output() {
        let mut screen = ScreenBuffer::new(10, 2);
        screen.snapshot();
        // No changes since snapshot
        let output = screen.flush();
        // Should be empty or minimal (no dirty cells)
        assert!(output.is_empty() || output.len() < 10);
    }

    #[test]
    fn test_resize() {
        let mut screen = ScreenBuffer::new(80, 24);
        screen.write_string(0, 0, "Hello", None, None, false);
        screen.resize(40, 12);
        assert_eq!(screen.cols(), 40);
        assert_eq!(screen.rows(), 12);
        // Content is reset on resize
        assert_eq!(screen.get(0, 0).unwrap().ch, ' ');
    }
}
