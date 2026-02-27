// Border rendering for pane separators using box-drawing Unicode characters.
//
// Draws lines between adjacent panes, highlighting borders adjacent to the
// active pane in a distinct color.

use crate::screen::{ScreenBuffer, ScreenCell};
use maxmux_core::layout::Rect;
use maxmux_core::session::PaneId;
use std::collections::{HashMap, HashSet};

/// The set of box-drawing characters used to render pane borders.
pub struct BorderChars {
    pub horizontal: char,
    pub vertical: char,
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub tee_left: char,
    pub tee_right: char,
    pub tee_top: char,
    pub tee_bottom: char,
    pub cross: char,
}

pub const ROUNDED: BorderChars = BorderChars {
    horizontal: '\u{2500}',   // ─
    vertical: '\u{2502}',     // │
    top_left: '\u{256D}',     // ╭
    top_right: '\u{256E}',    // ╮
    bottom_left: '\u{2570}',  // ╰
    bottom_right: '\u{256F}', // ╯
    tee_left: '\u{251C}',     // ├
    tee_right: '\u{2524}',    // ┤
    tee_top: '\u{252C}',      // ┬
    tee_bottom: '\u{2534}',   // ┴
    cross: '\u{253C}',        // ┼
};

pub const SHARP: BorderChars = BorderChars {
    horizontal: '\u{2500}',   // ─
    vertical: '\u{2502}',     // │
    top_left: '\u{250C}',     // ┌
    top_right: '\u{2510}',    // ┐
    bottom_left: '\u{2514}',  // └
    bottom_right: '\u{2518}', // ┘
    tee_left: '\u{251C}',     // ├
    tee_right: '\u{2524}',    // ┤
    tee_top: '\u{252C}',      // ┬
    tee_bottom: '\u{2534}',   // ┴
    cross: '\u{253C}',        // ┼
};

pub const DOUBLE: BorderChars = BorderChars {
    horizontal: '\u{2550}',   // ═
    vertical: '\u{2551}',     // ║
    top_left: '\u{2554}',     // ╔
    top_right: '\u{2557}',    // ╗
    bottom_left: '\u{255A}',  // ╚
    bottom_right: '\u{255D}', // ╝
    tee_left: '\u{2560}',     // ╠
    tee_right: '\u{2563}',    // ╣
    tee_top: '\u{2566}',      // ╦
    tee_bottom: '\u{2569}',   // ╩
    cross: '\u{256C}',        // ╬
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorderStyle {
    Rounded,
    Sharp,
    Double,
    None,
}

impl BorderStyle {
    pub fn chars(&self) -> Option<&'static BorderChars> {
        match self {
            Self::Rounded => Some(&ROUNDED),
            Self::Sharp => Some(&SHARP),
            Self::Double => Some(&DOUBLE),
            Self::None => None,
        }
    }
}

/// Returns true if the point (x, y) lies inside `rect`.
fn point_in_rect(x: u16, y: u16, rect: &Rect) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

/// Returns true if (x, y) is inside any pane rect.
fn in_any_pane(x: u16, y: u16, pane_rects: &HashMap<PaneId, Rect>) -> bool {
    pane_rects.values().any(|r| point_in_rect(x, y, r))
}

/// Check whether any pane adjacent to the border cell at (bx, by) is the
/// active pane.  "Adjacent" means the pane rect touches the border cell on
/// any side (left, right, above, below).
fn is_adjacent_to_active(
    bx: u16,
    by: u16,
    pane_rects: &HashMap<PaneId, Rect>,
    active_pane: &str,
) -> bool {
    if let Some(rect) = pane_rects.get(active_pane) {
        // Check if the active pane is immediately to the left (pane's right edge == bx)
        if rect.x.saturating_add(rect.width) == bx
            && by >= rect.y
            && by < rect.y.saturating_add(rect.height)
        {
            return true;
        }
        // Immediately to the right
        if bx.saturating_add(1) == rect.x
            && by >= rect.y
            && by < rect.y.saturating_add(rect.height)
        {
            return true;
        }
        // Immediately above
        if rect.y.saturating_add(rect.height) == by
            && bx >= rect.x
            && bx < rect.x.saturating_add(rect.width)
        {
            return true;
        }
        // Immediately below
        if by.saturating_add(1) == rect.y
            && bx >= rect.x
            && bx < rect.x.saturating_add(rect.width)
        {
            return true;
        }
    }
    false
}

/// Choose the appropriate box-drawing character for a border cell at (bx, by)
/// based on which of its four neighbours are also border cells.
fn choose_border_char(
    bx: u16,
    by: u16,
    border_set: &HashSet<(u16, u16)>,
    chars: &BorderChars,
) -> char {
    let up = by > 0 && border_set.contains(&(bx, by - 1));
    let down = border_set.contains(&(bx, by + 1));
    let left = bx > 0 && border_set.contains(&(bx - 1, by));
    let right = border_set.contains(&(bx + 1, by));

    match (up, down, left, right) {
        // Four-way intersection
        (true, true, true, true) => chars.cross,
        // Tee variants (three connections)
        (true, true, false, true) => chars.tee_left,
        (true, true, true, false) => chars.tee_right,
        (false, true, true, true) => chars.tee_top,
        (true, false, true, true) => chars.tee_bottom,
        // Straight lines
        (true, true, false, false) => chars.vertical,
        (false, false, true, true) => chars.horizontal,
        // Corners (two connections, perpendicular)
        (false, true, false, true) => chars.top_left,
        (false, true, true, false) => chars.top_right,
        (true, false, false, true) => chars.bottom_left,
        (true, false, true, false) => chars.bottom_right,
        // Single connection or isolated cell: pick the most reasonable default
        (true, false, false, false) | (false, true, false, false) => chars.vertical,
        (false, false, true, false) | (false, false, false, true) => chars.horizontal,
        // Isolated border cell (shouldn't normally happen)
        (false, false, false, false) => chars.cross,
    }
}

/// Render borders between panes onto the screen buffer.
///
/// The algorithm:
/// 1. Compute the bounding box of all pane rects.
/// 2. Identify every cell inside that bounding box that is not covered by any
///    pane rect -- these are border cells.
/// 3. For each border cell, determine the correct box-drawing character by
///    inspecting which of its four neighbours are also border cells.
/// 4. Apply `active_fg` colour to borders adjacent to the active pane,
///    otherwise use `fg`.
pub fn render_borders(
    screen: &mut ScreenBuffer,
    pane_rects: &HashMap<PaneId, Rect>,
    active_pane: &str,
    style: BorderStyle,
    fg: (u8, u8, u8),
    active_fg: (u8, u8, u8),
) {
    let chars = match style.chars() {
        Some(c) => c,
        None => return,
    };

    if pane_rects.is_empty() {
        return;
    }

    // Compute the bounding box that covers all pane rects.
    let mut min_x = u16::MAX;
    let mut min_y = u16::MAX;
    let mut max_x: u16 = 0;
    let mut max_y: u16 = 0;
    for rect in pane_rects.values() {
        min_x = min_x.min(rect.x);
        min_y = min_y.min(rect.y);
        max_x = max_x.max(rect.x.saturating_add(rect.width));
        max_y = max_y.max(rect.y.saturating_add(rect.height));
    }

    // Collect all border cell positions within the bounding box.
    let mut border_set: HashSet<(u16, u16)> = HashSet::new();
    for y in min_y..max_y {
        for x in min_x..max_x {
            if !in_any_pane(x, y, pane_rects) {
                border_set.insert((x, y));
            }
        }
    }

    // Render each border cell.
    for &(bx, by) in &border_set {
        let ch = choose_border_char(bx, by, &border_set, chars);
        let color = if is_adjacent_to_active(bx, by, pane_rects, active_pane) {
            active_fg
        } else {
            fg
        };
        screen.set(
            bx,
            by,
            ScreenCell {
                ch,
                fg: Some(color),
                bg: None,
                bold: false,
                dim: false,
                italic: false,
                underline: false,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::ScreenBuffer;
    use maxmux_core::layout::{calculate_layout, Rect};
    use maxmux_core::session::{LayoutNode, SplitDirection};

    #[test]
    fn test_no_borders_single_pane() {
        let mut screen = ScreenBuffer::new(80, 24);
        let mut rects = HashMap::new();
        rects.insert(
            "p1".to_string(),
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );
        render_borders(
            &mut screen,
            &rects,
            "p1",
            BorderStyle::Rounded,
            (100, 100, 100),
            (200, 200, 200),
        );
        // No borders should be drawn - screen should be all spaces
        assert_eq!(screen.get(0, 0).unwrap().ch, ' ');
    }

    #[test]
    fn test_vertical_border_horizontal_split() {
        let mut screen = ScreenBuffer::new(80, 24);
        let layout = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            children: Box::new((
                LayoutNode::Leaf {
                    pane_id: "p1".into(),
                },
                LayoutNode::Leaf {
                    pane_id: "p2".into(),
                },
            )),
        };
        let rects = calculate_layout(
            &layout,
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );
        render_borders(
            &mut screen,
            &rects,
            "p1",
            BorderStyle::Rounded,
            (100, 100, 100),
            (200, 200, 200),
        );
        // Border should be at x=40 (between p1 width=40 and p2 starting at x=41)
        let border_cell = screen.get(40, 0).unwrap();
        assert_eq!(border_cell.ch, '\u{2502}'); // │
    }

    #[test]
    fn test_horizontal_border_vertical_split() {
        let mut screen = ScreenBuffer::new(80, 24);
        let layout = LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            children: Box::new((
                LayoutNode::Leaf {
                    pane_id: "p1".into(),
                },
                LayoutNode::Leaf {
                    pane_id: "p2".into(),
                },
            )),
        };
        let rects = calculate_layout(
            &layout,
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );
        render_borders(
            &mut screen,
            &rects,
            "p1",
            BorderStyle::Rounded,
            (100, 100, 100),
            (200, 200, 200),
        );
        // Border should be at y=12 (between p1 height=12 and p2 starting at y=13)
        let border_cell = screen.get(0, 12).unwrap();
        assert_eq!(border_cell.ch, '\u{2500}'); // ─
    }

    #[test]
    fn test_none_style_no_borders() {
        let mut screen = ScreenBuffer::new(80, 24);
        let mut rects = HashMap::new();
        rects.insert(
            "p1".to_string(),
            Rect {
                x: 0,
                y: 0,
                width: 40,
                height: 24,
            },
        );
        rects.insert(
            "p2".to_string(),
            Rect {
                x: 41,
                y: 0,
                width: 39,
                height: 24,
            },
        );
        render_borders(
            &mut screen,
            &rects,
            "p1",
            BorderStyle::None,
            (100, 100, 100),
            (200, 200, 200),
        );
        // Nothing drawn
        assert_eq!(screen.get(40, 0).unwrap().ch, ' ');
    }

    #[test]
    fn test_active_border_color() {
        let mut screen = ScreenBuffer::new(80, 24);
        let layout = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            children: Box::new((
                LayoutNode::Leaf {
                    pane_id: "p1".into(),
                },
                LayoutNode::Leaf {
                    pane_id: "p2".into(),
                },
            )),
        };
        let rects = calculate_layout(
            &layout,
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );
        let active_fg = (200, 200, 200);
        render_borders(
            &mut screen,
            &rects,
            "p1",
            BorderStyle::Sharp,
            (100, 100, 100),
            active_fg,
        );
        // Border at x=40 should have active color since p1 is active and adjacent
        let border_cell = screen.get(40, 0).unwrap();
        assert_eq!(border_cell.fg, Some(active_fg));
    }

    #[test]
    fn test_cross_at_intersection() {
        // 4-pane grid: horizontal split inside a vertical split
        let layout = LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            children: Box::new((
                LayoutNode::Split {
                    direction: SplitDirection::Horizontal,
                    ratio: 0.5,
                    children: Box::new((
                        LayoutNode::Leaf {
                            pane_id: "p1".into(),
                        },
                        LayoutNode::Leaf {
                            pane_id: "p2".into(),
                        },
                    )),
                },
                LayoutNode::Split {
                    direction: SplitDirection::Horizontal,
                    ratio: 0.5,
                    children: Box::new((
                        LayoutNode::Leaf {
                            pane_id: "p3".into(),
                        },
                        LayoutNode::Leaf {
                            pane_id: "p4".into(),
                        },
                    )),
                },
            )),
        };
        let rects = calculate_layout(
            &layout,
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );
        let mut screen = ScreenBuffer::new(80, 24);
        render_borders(
            &mut screen,
            &rects,
            "p1",
            BorderStyle::Rounded,
            (100, 100, 100),
            (200, 200, 200),
        );
        // The intersection of the vertical border (x=40) and horizontal border (y=12)
        // should be a cross character
        let intersection = screen.get(40, 12).unwrap();
        assert_eq!(intersection.ch, '\u{253C}'); // ┼
    }

    #[test]
    fn test_tee_at_partial_intersection() {
        // 3-pane layout: p1 on the left, p2 top-right, p3 bottom-right
        let layout = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            children: Box::new((
                LayoutNode::Leaf {
                    pane_id: "p1".into(),
                },
                LayoutNode::Split {
                    direction: SplitDirection::Vertical,
                    ratio: 0.5,
                    children: Box::new((
                        LayoutNode::Leaf {
                            pane_id: "p2".into(),
                        },
                        LayoutNode::Leaf {
                            pane_id: "p3".into(),
                        },
                    )),
                },
            )),
        };
        let rects = calculate_layout(
            &layout,
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );
        let mut screen = ScreenBuffer::new(80, 24);
        render_borders(
            &mut screen,
            &rects,
            "p1",
            BorderStyle::Rounded,
            (100, 100, 100),
            (200, 200, 200),
        );

        // At (40, 12): vertical border goes up+down, horizontal border goes right only
        // This should be a tee_left (├) since connections are up, down, right
        let p2 = &rects["p2"];
        let border_y = p2.y + p2.height; // horizontal border y
        let border_x: u16 = 40; // vertical border x
        let tee_cell = screen.get(border_x, border_y).unwrap();
        assert_eq!(tee_cell.ch, '\u{251C}'); // ├
    }

    #[test]
    fn test_border_style_chars() {
        assert!(BorderStyle::Rounded.chars().is_some());
        assert!(BorderStyle::Sharp.chars().is_some());
        assert!(BorderStyle::Double.chars().is_some());
        assert!(BorderStyle::None.chars().is_none());
    }

    #[test]
    fn test_double_border_characters() {
        let mut screen = ScreenBuffer::new(80, 24);
        let layout = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            children: Box::new((
                LayoutNode::Leaf {
                    pane_id: "p1".into(),
                },
                LayoutNode::Leaf {
                    pane_id: "p2".into(),
                },
            )),
        };
        let rects = calculate_layout(
            &layout,
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );
        render_borders(
            &mut screen,
            &rects,
            "p1",
            BorderStyle::Double,
            (100, 100, 100),
            (200, 200, 200),
        );
        let border_cell = screen.get(40, 0).unwrap();
        assert_eq!(border_cell.ch, '\u{2551}'); // ║
    }

    #[test]
    fn test_inactive_border_color() {
        let mut screen = ScreenBuffer::new(80, 24);
        let mut rects = HashMap::new();
        // p1 on left, p2 in middle, p3 on right
        rects.insert(
            "p1".to_string(),
            Rect {
                x: 0,
                y: 0,
                width: 26,
                height: 24,
            },
        );
        rects.insert(
            "p2".to_string(),
            Rect {
                x: 27,
                y: 0,
                width: 26,
                height: 24,
            },
        );
        rects.insert(
            "p3".to_string(),
            Rect {
                x: 54,
                y: 0,
                width: 26,
                height: 24,
            },
        );
        let fg = (100, 100, 100);
        let active_fg = (200, 200, 200);
        render_borders(&mut screen, &rects, "p1", BorderStyle::Sharp, fg, active_fg);
        // Border at x=26 is adjacent to p1 (active) -> active_fg
        let cell_26 = screen.get(26, 0).unwrap();
        assert_eq!(cell_26.fg, Some(active_fg));
        // Border at x=53 is NOT adjacent to p1 -> fg
        let cell_53 = screen.get(53, 0).unwrap();
        assert_eq!(cell_53.fg, Some(fg));
    }

    #[test]
    fn test_entire_vertical_border_line() {
        let mut screen = ScreenBuffer::new(80, 24);
        let layout = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            children: Box::new((
                LayoutNode::Leaf {
                    pane_id: "p1".into(),
                },
                LayoutNode::Leaf {
                    pane_id: "p2".into(),
                },
            )),
        };
        let rects = calculate_layout(
            &layout,
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
        );
        render_borders(
            &mut screen,
            &rects,
            "p1",
            BorderStyle::Rounded,
            (100, 100, 100),
            (200, 200, 200),
        );
        // Every cell in column 40 should be a vertical border
        for y in 0..24u16 {
            let cell = screen.get(40, y).unwrap();
            assert_eq!(cell.ch, '\u{2502}', "Expected vertical bar at (40, {y})");
        }
        // No border in other columns
        assert_eq!(screen.get(0, 0).unwrap().ch, ' ');
        assert_eq!(screen.get(79, 0).unwrap().ch, ' ');
    }

    #[test]
    fn test_empty_pane_rects() {
        let mut screen = ScreenBuffer::new(80, 24);
        let rects = HashMap::new();
        // Should not panic with empty pane rects
        render_borders(
            &mut screen,
            &rects,
            "p1",
            BorderStyle::Rounded,
            (100, 100, 100),
            (200, 200, 200),
        );
        assert_eq!(screen.get(0, 0).unwrap().ch, ' ');
    }
}
