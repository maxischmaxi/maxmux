// Screen compositor: combines pane contents, borders, and status bar into final
// terminal output.
//
// The compositor reads cell data from VirtualTerminal grids (backed by
// alacritty_terminal), writes them into the ScreenBuffer at the correct
// positions, renders borders between panes, and produces the complete ANSI
// escape sequence string to send to the client terminal.

use crate::ansi;
use crate::border::{self, BorderStyle};
use crate::screen::{ScreenBuffer, ScreenCell};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};
use maxmux_core::layout::Rect;
use maxmux_core::session::PaneId;
use maxmux_core::terminal::VirtualTerminal;
use std::collections::HashMap;

/// Configuration for how pane borders are drawn.
pub struct BorderConfig {
    pub style: BorderStyle,
    pub fg: (u8, u8, u8),
    pub active_fg: (u8, u8, u8),
}

/// Describes the cursor state after compositing so the caller can position
/// and show/hide the hardware cursor.
pub struct CursorState {
    pub x: u16,
    pub y: u16,
    pub visible: bool,
    /// DECSCUSR value (0=default, 1=block blink, 2=block steady,
    /// 3=underline blink, 4=underline steady, 5=bar blink, 6=bar steady).
    pub shape: u8,
}

/// The compositor owns a double-buffered ScreenBuffer and orchestrates the
/// rendering of all panes, borders, and status bar into a single ANSI output
/// string each frame.
pub struct Compositor {
    screen: ScreenBuffer,
    cols: u16,
    rows: u16,
}

impl Compositor {
    /// Create a new compositor with the given terminal dimensions.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            screen: ScreenBuffer::new(cols, rows),
            cols,
            rows,
        }
    }

    /// Resize the compositor (and its internal screen buffer) to new dimensions.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.screen.resize(cols, rows);
    }

    /// Number of columns.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Number of rows.
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Compose the full screen and return (ANSI output string, cursor state).
    ///
    /// This is the main entry point called once per render frame. It:
    /// 1. Snapshots the previous frame for dirty tracking.
    /// 2. Clears the screen buffer.
    /// 3. Renders all pane contents (or a single zoomed pane).
    /// 4. Renders borders between panes (unless zoomed).
    /// 5. Flushes the screen buffer to produce diff-based ANSI output.
    /// 6. Appends the status bar (raw ANSI, bypasses the buffer).
    /// 7. Positions/shows/hides the cursor for the active pane.
    pub fn compose(
        &mut self,
        terminals: &HashMap<PaneId, &VirtualTerminal>,
        pane_rects: &HashMap<PaneId, Rect>,
        active_pane: &str,
        status_bar_line: Option<&str>,
        border_config: &BorderConfig,
        zoomed_pane: Option<&str>,
    ) -> (String, Option<CursorState>) {
        self.screen.snapshot();
        self.screen.clear();

        let status_bar_rows = if status_bar_line.is_some() { 1 } else { 0 };
        let content_rows = self.rows.saturating_sub(status_bar_rows);

        if let Some(zoomed_id) = zoomed_pane {
            // Zoomed mode: single pane fills entire content area.
            if let Some(vt) = terminals.get(zoomed_id) {
                self.render_pane(
                    vt,
                    &Rect {
                        x: 0,
                        y: 0,
                        width: self.cols,
                        height: content_rows,
                    },
                );
            }
        } else {
            // Normal mode: render all panes at their positions.
            for (pane_id, rect) in pane_rects {
                if let Some(vt) = terminals.get(pane_id) {
                    self.render_pane(vt, rect);
                }
            }
            // Render borders between panes.
            border::render_borders(
                &mut self.screen,
                pane_rects,
                active_pane,
                border_config.style,
                border_config.fg,
                border_config.active_fg,
            );
        }

        // Calculate cursor state from active pane.
        let cursor = self.calculate_cursor(terminals, pane_rects, active_pane, zoomed_pane);

        // Flush screen buffer (produces diff-based ANSI output).
        let mut output = self.screen.flush();

        // Append status bar raw output (bypasses the screen buffer).
        if let Some(bar) = status_bar_line {
            output.push_str(&ansi::move_to(0, self.rows - 1));
            output.push_str(bar);
        }

        // Append cursor positioning and visibility.
        if let Some(ref c) = cursor {
            if c.visible {
                output.push_str(&ansi::move_to(c.x, c.y));
                output.push_str(ansi::show_cursor());
                output.push_str(&ansi::set_cursor_style(c.shape));
            } else {
                output.push_str(ansi::hide_cursor());
            }
        } else {
            output.push_str(ansi::hide_cursor());
        }

        (output, cursor)
    }

    // --- Private helpers ---

    /// Render a VirtualTerminal's content into the screen buffer at the given
    /// rect.
    ///
    /// For each cell in the pane's visible area, reads the alacritty_terminal
    /// Cell, converts it to a ScreenCell, and writes it to the screen buffer at
    /// the corresponding screen position.
    fn render_pane(&mut self, vt: &VirtualTerminal, rect: &Rect) {
        let pane_cols = rect.width.min(vt.cols());
        let pane_rows = rect.height.min(vt.rows());

        for row in 0..pane_rows {
            for col in 0..pane_cols {
                let cell = vt.cell(col, row);

                // Skip wide-char spacers -- the preceding wide char already
                // occupies this column visually.
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER)
                    || cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }

                let screen_cell = cell_to_screen_cell(cell);
                self.screen.set(rect.x + col, rect.y + row, screen_cell);
            }
        }
    }

    /// Calculate cursor position in screen coordinates for the active (or
    /// zoomed) pane.
    fn calculate_cursor(
        &self,
        terminals: &HashMap<PaneId, &VirtualTerminal>,
        pane_rects: &HashMap<PaneId, Rect>,
        active_pane: &str,
        zoomed_pane: Option<&str>,
    ) -> Option<CursorState> {
        let pane_id = zoomed_pane.unwrap_or(active_pane);
        let vt = terminals.get(pane_id)?;
        let rect = if zoomed_pane.is_some() {
            Rect {
                x: 0,
                y: 0,
                width: self.cols,
                height: self.rows,
            }
        } else {
            *pane_rects.get(pane_id)?
        };

        let (cx, cy) = vt.cursor_position();

        Some(CursorState {
            x: rect.x + cx,
            y: rect.y + cy,
            visible: vt.is_cursor_visible(),
            shape: cursor_shape_to_decscusr(vt.cursor_shape()),
        })
    }
}

// ---------------------------------------------------------------------------
// Color and cell conversion helpers
// ---------------------------------------------------------------------------

/// Convert an alacritty_terminal Color to an optional RGB triple.
///
/// Returns `None` for "default terminal color" (foreground/background), which
/// tells the screen buffer not to emit any color escape sequence.
fn color_to_rgb(color: &Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Spec(rgb) => Some((rgb.r, rgb.g, rgb.b)),
        Color::Named(name) => named_color_to_rgb(name),
        Color::Indexed(idx) => indexed_color_to_rgb(*idx),
    }
}

/// Map well-known named ANSI colors to reasonable RGB defaults.
///
/// `Foreground` and `Background` return `None` so the terminal's own default
/// colors are used.
fn named_color_to_rgb(name: &NamedColor) -> Option<(u8, u8, u8)> {
    match name {
        NamedColor::Black => Some((0, 0, 0)),
        NamedColor::Red => Some((205, 49, 49)),
        NamedColor::Green => Some((13, 188, 121)),
        NamedColor::Yellow => Some((229, 229, 16)),
        NamedColor::Blue => Some((36, 114, 200)),
        NamedColor::Magenta => Some((188, 63, 188)),
        NamedColor::Cyan => Some((17, 168, 205)),
        NamedColor::White => Some((229, 229, 229)),
        NamedColor::BrightBlack => Some((102, 102, 102)),
        NamedColor::BrightRed => Some((241, 76, 76)),
        NamedColor::BrightGreen => Some((35, 209, 139)),
        NamedColor::BrightYellow => Some((245, 245, 67)),
        NamedColor::BrightBlue => Some((59, 142, 234)),
        NamedColor::BrightMagenta => Some((214, 112, 214)),
        NamedColor::BrightCyan => Some((41, 184, 219)),
        NamedColor::BrightWhite => Some((242, 242, 242)),
        NamedColor::DimBlack => Some((0, 0, 0)),
        NamedColor::DimRed => Some((154, 37, 37)),
        NamedColor::DimGreen => Some((10, 141, 91)),
        NamedColor::DimYellow => Some((172, 172, 12)),
        NamedColor::DimBlue => Some((27, 86, 150)),
        NamedColor::DimMagenta => Some((141, 47, 141)),
        NamedColor::DimCyan => Some((13, 126, 154)),
        NamedColor::DimWhite => Some((172, 172, 172)),
        // Foreground, Background, Cursor, etc. => use terminal default.
        NamedColor::Foreground
        | NamedColor::Background
        | NamedColor::Cursor
        | NamedColor::BrightForeground
        | NamedColor::DimForeground => None,
    }
}

/// Map a 256-color palette index to an RGB triple.
///
/// Indices 0-15 are mapped to the standard ANSI colors, 16-231 are the 6x6x6
/// color cube, and 232-255 are the grayscale ramp.
fn indexed_color_to_rgb(idx: u8) -> Option<(u8, u8, u8)> {
    match idx {
        // Standard 16 colors (same mapping as named).
        0 => Some((0, 0, 0)),
        1 => Some((205, 49, 49)),
        2 => Some((13, 188, 121)),
        3 => Some((229, 229, 16)),
        4 => Some((36, 114, 200)),
        5 => Some((188, 63, 188)),
        6 => Some((17, 168, 205)),
        7 => Some((229, 229, 229)),
        8 => Some((102, 102, 102)),
        9 => Some((241, 76, 76)),
        10 => Some((35, 209, 139)),
        11 => Some((245, 245, 67)),
        12 => Some((59, 142, 234)),
        13 => Some((214, 112, 214)),
        14 => Some((41, 184, 219)),
        15 => Some((242, 242, 242)),
        // 6x6x6 color cube (indices 16-231).
        16..=231 => {
            let idx = idx - 16;
            let r_idx = idx / 36;
            let g_idx = (idx % 36) / 6;
            let b_idx = idx % 6;
            let to_value = |i: u8| if i == 0 { 0 } else { 55 + 40 * i };
            Some((to_value(r_idx), to_value(g_idx), to_value(b_idx)))
        }
        // Grayscale ramp (indices 232-255).
        232..=255 => {
            let v = 8 + 10 * (idx - 232);
            Some((v, v, v))
        }
    }
}

/// Convert an alacritty_terminal Cell to our ScreenCell.
fn cell_to_screen_cell(cell: &Cell) -> ScreenCell {
    let ch = cell.c;
    let fg = color_to_rgb(&cell.fg);
    let bg = color_to_rgb(&cell.bg);

    let bold = cell.flags.contains(Flags::BOLD);
    let dim = cell.flags.contains(Flags::DIM);
    let italic = cell.flags.contains(Flags::ITALIC);
    let underline = cell.flags.intersects(Flags::ALL_UNDERLINES);

    ScreenCell {
        ch,
        fg,
        bg,
        bold,
        dim,
        italic,
        underline,
    }
}

/// Convert a CursorShape to a DECSCUSR value.
///
/// DECSCUSR: 0=default, 1=block blink, 2=block steady, 3=underline blink,
/// 4=underline steady, 5=bar blink, 6=bar steady.
fn cursor_shape_to_decscusr(shape: CursorShape) -> u8 {
    match shape {
        CursorShape::Block => 0,
        CursorShape::Underline => 3,
        CursorShape::Beam => 5,
        CursorShape::HollowBlock => 0,
        CursorShape::Hidden => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maxmux_core::layout::Rect;
    use maxmux_core::terminal::VirtualTerminal;

    #[test]
    fn test_new_compositor() {
        let comp = Compositor::new(80, 24);
        assert_eq!(comp.cols(), 80);
        assert_eq!(comp.rows(), 24);
    }

    #[test]
    fn test_resize() {
        let mut comp = Compositor::new(80, 24);
        comp.resize(120, 40);
        assert_eq!(comp.cols(), 120);
        assert_eq!(comp.rows(), 40);
    }

    #[test]
    fn test_compose_single_pane() {
        let mut comp = Compositor::new(80, 24);
        let mut vt = VirtualTerminal::new(80, 24, 100);
        vt.write(b"Hello, World!");

        let mut terminals: HashMap<PaneId, &VirtualTerminal> = HashMap::new();
        terminals.insert("p1".into(), &vt);
        let mut rects = HashMap::new();
        rects.insert(
            "p1".into(),
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );

        let border_config = BorderConfig {
            style: BorderStyle::Rounded,
            fg: (100, 100, 100),
            active_fg: (200, 200, 200),
        };

        let (output, cursor) = comp.compose(&terminals, &rects, "p1", None, &border_config, None);
        assert!(!output.is_empty());
        // Cursor should be visible at position after "Hello, World!"
        let c = cursor.unwrap();
        assert_eq!(c.x, 13);
        assert_eq!(c.y, 0);
        assert!(c.visible);
    }

    #[test]
    fn test_compose_zoomed_pane() {
        let mut comp = Compositor::new(80, 24);
        let mut vt1 = VirtualTerminal::new(40, 24, 100);
        vt1.write(b"Pane 1");
        let mut vt2 = VirtualTerminal::new(39, 24, 100);
        vt2.write(b"Pane 2");

        let mut terminals: HashMap<PaneId, &VirtualTerminal> = HashMap::new();
        terminals.insert("p1".into(), &vt1);
        terminals.insert("p2".into(), &vt2);
        let mut rects = HashMap::new();
        rects.insert(
            "p1".into(),
            Rect {
                x: 0,
                y: 0,
                width: 40,
                height: 24,
            },
        );
        rects.insert(
            "p2".into(),
            Rect {
                x: 41,
                y: 0,
                width: 39,
                height: 24,
            },
        );

        let border_config = BorderConfig {
            style: BorderStyle::Rounded,
            fg: (100, 100, 100),
            active_fg: (200, 200, 200),
        };

        // Zoom p1 - should fill entire screen
        let (_, cursor) = comp.compose(&terminals, &rects, "p1", None, &border_config, Some("p1"));
        let c = cursor.unwrap();
        // Zoomed cursor at (6, 0) since "Pane 1" is 6 chars
        assert_eq!(c.x, 6);
        assert_eq!(c.y, 0);
    }

    #[test]
    fn test_compose_hidden_cursor() {
        let mut comp = Compositor::new(80, 24);
        let mut vt = VirtualTerminal::new(80, 24, 100);
        vt.write(b"\x1b[?25l"); // Hide cursor (DECTCEM)

        let mut terminals: HashMap<PaneId, &VirtualTerminal> = HashMap::new();
        terminals.insert("p1".into(), &vt);
        let mut rects = HashMap::new();
        rects.insert(
            "p1".into(),
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );

        let border_config = BorderConfig {
            style: BorderStyle::Rounded,
            fg: (100, 100, 100),
            active_fg: (200, 200, 200),
        };

        let (output, cursor) = comp.compose(&terminals, &rects, "p1", None, &border_config, None);
        // Cursor should not be visible
        let c = cursor.unwrap();
        assert!(!c.visible);
        // Output should contain hide cursor sequence
        assert!(output.contains("\x1b[?25l"));
    }

    #[test]
    fn test_compose_with_status_bar() {
        let mut comp = Compositor::new(80, 24);
        let mut vt = VirtualTerminal::new(80, 23, 100);
        vt.write(b"Content");

        let mut terminals: HashMap<PaneId, &VirtualTerminal> = HashMap::new();
        terminals.insert("p1".into(), &vt);
        let mut rects = HashMap::new();
        rects.insert(
            "p1".into(),
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 23,
            },
        );

        let border_config = BorderConfig {
            style: BorderStyle::Rounded,
            fg: (100, 100, 100),
            active_fg: (200, 200, 200),
        };

        let status_bar = "\x1b[38;2;255;255;255m[session] 0:bash";
        let (output, _) = comp.compose(
            &terminals,
            &rects,
            "p1",
            Some(status_bar),
            &border_config,
            None,
        );
        // Status bar should be included in output
        assert!(output.contains("[session]"));
        // Should contain a move_to for the last row (row 23, ANSI 1-based: 24)
        assert!(output.contains(&ansi::move_to(0, 23)));
    }

    #[test]
    fn test_compose_empty_terminals() {
        let mut comp = Compositor::new(80, 24);
        let terminals: HashMap<PaneId, &VirtualTerminal> = HashMap::new();
        let rects = HashMap::new();

        let border_config = BorderConfig {
            style: BorderStyle::Rounded,
            fg: (100, 100, 100),
            active_fg: (200, 200, 200),
        };

        let (output, cursor) = comp.compose(&terminals, &rects, "p1", None, &border_config, None);
        // Should produce hide cursor since active pane is not found
        assert!(output.contains("\x1b[?25l"));
        assert!(cursor.is_none());
    }

    #[test]
    fn test_color_conversion_spec() {
        let color = Color::Spec(alacritty_terminal::vte::ansi::Rgb {
            r: 255,
            g: 128,
            b: 0,
        });
        assert_eq!(color_to_rgb(&color), Some((255, 128, 0)));
    }

    #[test]
    fn test_color_conversion_named_default() {
        let fg = Color::Named(NamedColor::Foreground);
        let bg = Color::Named(NamedColor::Background);
        assert_eq!(color_to_rgb(&fg), None);
        assert_eq!(color_to_rgb(&bg), None);
    }

    #[test]
    fn test_color_conversion_named_colors() {
        let red = Color::Named(NamedColor::Red);
        assert!(color_to_rgb(&red).is_some());

        let blue = Color::Named(NamedColor::Blue);
        assert!(color_to_rgb(&blue).is_some());
    }

    #[test]
    fn test_color_conversion_indexed() {
        // Index 0 is black
        assert_eq!(indexed_color_to_rgb(0), Some((0, 0, 0)));
        // Index 15 is bright white
        assert_eq!(indexed_color_to_rgb(15), Some((242, 242, 242)));
        // Index 232 is start of grayscale
        assert_eq!(indexed_color_to_rgb(232), Some((8, 8, 8)));
        // Index 255 is end of grayscale
        assert_eq!(indexed_color_to_rgb(255), Some((238, 238, 238)));
    }

    #[test]
    fn test_color_conversion_indexed_cube() {
        // Index 16 is the start of the 6x6x6 cube (should be (0,0,0))
        assert_eq!(indexed_color_to_rgb(16), Some((0, 0, 0)));
        // Index 196 = 16 + 180 = r=5, g=0, b=0 => (255, 0, 0)
        // 180 / 36 = 5, 180 % 36 = 0, 0 / 6 = 0, 0 % 6 = 0
        assert_eq!(indexed_color_to_rgb(196), Some((255, 0, 0)));
    }

    #[test]
    fn test_cursor_shape_conversion() {
        assert_eq!(cursor_shape_to_decscusr(CursorShape::Block), 0);
        assert_eq!(cursor_shape_to_decscusr(CursorShape::Underline), 3);
        assert_eq!(cursor_shape_to_decscusr(CursorShape::Beam), 5);
        assert_eq!(cursor_shape_to_decscusr(CursorShape::HollowBlock), 0);
        assert_eq!(cursor_shape_to_decscusr(CursorShape::Hidden), 0);
    }

    #[test]
    fn test_cell_to_screen_cell_default() {
        let cell = Cell::default();
        let sc = cell_to_screen_cell(&cell);
        assert_eq!(sc.ch, ' ');
        assert_eq!(sc.fg, None); // Named::Foreground -> None
        assert_eq!(sc.bg, None); // Named::Background -> None
        assert!(!sc.bold);
        assert!(!sc.dim);
        assert!(!sc.italic);
        assert!(!sc.underline);
    }

    #[test]
    fn test_compose_multiple_panes() {
        let mut comp = Compositor::new(80, 24);
        let mut vt1 = VirtualTerminal::new(40, 24, 100);
        vt1.write(b"Left");
        let mut vt2 = VirtualTerminal::new(39, 24, 100);
        vt2.write(b"Right");

        let mut terminals: HashMap<PaneId, &VirtualTerminal> = HashMap::new();
        terminals.insert("p1".into(), &vt1);
        terminals.insert("p2".into(), &vt2);
        let mut rects = HashMap::new();
        rects.insert(
            "p1".into(),
            Rect {
                x: 0,
                y: 0,
                width: 40,
                height: 24,
            },
        );
        rects.insert(
            "p2".into(),
            Rect {
                x: 41,
                y: 0,
                width: 39,
                height: 24,
            },
        );

        let border_config = BorderConfig {
            style: BorderStyle::Rounded,
            fg: (100, 100, 100),
            active_fg: (200, 200, 200),
        };

        let (output, cursor) = comp.compose(&terminals, &rects, "p1", None, &border_config, None);
        assert!(!output.is_empty());
        // Active pane is p1, cursor at col 4 (after "Left")
        let c = cursor.unwrap();
        assert_eq!(c.x, 4);
        assert_eq!(c.y, 0);
        assert!(c.visible);
    }

    #[test]
    fn test_compose_pane_with_colored_content() {
        let mut comp = Compositor::new(80, 24);
        let mut vt = VirtualTerminal::new(80, 24, 100);
        // Write red text: ESC[31m RED ESC[0m
        vt.write(b"\x1b[31mRED\x1b[0m");

        let mut terminals: HashMap<PaneId, &VirtualTerminal> = HashMap::new();
        terminals.insert("p1".into(), &vt);
        let mut rects = HashMap::new();
        rects.insert(
            "p1".into(),
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );

        let border_config = BorderConfig {
            style: BorderStyle::Rounded,
            fg: (100, 100, 100),
            active_fg: (200, 200, 200),
        };

        let (output, _) = comp.compose(&terminals, &rects, "p1", None, &border_config, None);
        // The output should contain the letter 'R' somewhere
        assert!(output.contains('R'));
        assert!(output.contains('E'));
        assert!(output.contains('D'));
    }

    #[test]
    fn test_compose_cursor_offset_by_pane_position() {
        let mut comp = Compositor::new(80, 24);
        let mut vt = VirtualTerminal::new(39, 24, 100);
        vt.write(b"Hi");

        let mut terminals: HashMap<PaneId, &VirtualTerminal> = HashMap::new();
        terminals.insert("p1".into(), &vt);
        let mut rects = HashMap::new();
        // Pane starts at x=41 (right side of a split)
        rects.insert(
            "p1".into(),
            Rect {
                x: 41,
                y: 0,
                width: 39,
                height: 24,
            },
        );

        let border_config = BorderConfig {
            style: BorderStyle::Rounded,
            fg: (100, 100, 100),
            active_fg: (200, 200, 200),
        };

        let (_, cursor) = comp.compose(&terminals, &rects, "p1", None, &border_config, None);
        let c = cursor.unwrap();
        // Cursor should be offset: pane x (41) + cursor col (2) = 43
        assert_eq!(c.x, 43);
        assert_eq!(c.y, 0);
    }
}
