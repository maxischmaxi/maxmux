// Mouse text selection – tracks click-drag selection state across pane cells.
//
// The client uses this to highlight selected text and eventually copy it to
// the system clipboard.  Selection coordinates are in *screen* space (the
// pane's content grid, not the global terminal grid).

/// The current phase of a mouse selection gesture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionState {
    /// No selection active.
    None,
    /// The user is currently dragging (mouse button held).
    Selecting {
        start_row: u16,
        start_col: u16,
        end_row: u16,
        end_col: u16,
    },
    /// The drag finished and a region is selected.
    Selected {
        start_row: u16,
        start_col: u16,
        end_row: u16,
        end_col: u16,
    },
}

/// Manages a single mouse text selection within one pane.
pub struct MouseSelection {
    pub state: SelectionState,
    pub pane_id: Option<String>,
}

impl MouseSelection {
    pub fn new() -> Self {
        Self {
            state: SelectionState::None,
            pane_id: None,
        }
    }

    /// Begin a new selection at (`row`, `col`) inside the given pane.
    pub fn start(&mut self, pane_id: &str, row: u16, col: u16) {
        self.pane_id = Some(pane_id.to_string());
        self.state = SelectionState::Selecting {
            start_row: row,
            start_col: col,
            end_row: row,
            end_col: col,
        };
    }

    /// Extend the selection as the mouse moves to (`row`, `col`).
    pub fn update(&mut self, row: u16, col: u16) {
        if let SelectionState::Selecting {
            start_row,
            start_col,
            ..
        } = self.state
        {
            self.state = SelectionState::Selecting {
                start_row,
                start_col,
                end_row: row,
                end_col: col,
            };
        }
    }

    /// Finalize the selection on mouse release.
    ///
    /// If the start and end positions are identical (a simple click with no
    /// drag) the selection is cleared instead of leaving a zero-width region.
    pub fn finish(&mut self) {
        if let SelectionState::Selecting {
            start_row,
            start_col,
            end_row,
            end_col,
        } = self.state
        {
            if start_row == end_row && start_col == end_col {
                self.clear(); // click with no drag
            } else {
                self.state = SelectionState::Selected {
                    start_row,
                    start_col,
                    end_row,
                    end_col,
                };
            }
        }
    }

    /// Clear the selection entirely.
    pub fn clear(&mut self) {
        self.state = SelectionState::None;
        self.pane_id = None;
    }

    /// Get the normalized selection bounds where start <= end.
    ///
    /// Returns `(start_row, start_col, end_row, end_col)` with the guarantee
    /// that start is before end in reading order.
    pub fn get_range(&self) -> Option<(u16, u16, u16, u16)> {
        match &self.state {
            SelectionState::Selecting {
                start_row,
                start_col,
                end_row,
                end_col,
            }
            | SelectionState::Selected {
                start_row,
                start_col,
                end_row,
                end_col,
            } => {
                if *start_row < *end_row
                    || (*start_row == *end_row && *start_col <= *end_col)
                {
                    Some((*start_row, *start_col, *end_row, *end_col))
                } else {
                    Some((*end_row, *end_col, *start_row, *start_col))
                }
            }
            SelectionState::None => None,
        }
    }

    /// Check whether a cell at (`row`, `col`) falls within the current selection.
    pub fn is_selected(&self, row: u16, col: u16) -> bool {
        if let Some((sr, sc, er, ec)) = self.get_range() {
            if row < sr || row > er {
                return false;
            }
            if row == sr && row == er {
                return col >= sc && col <= ec;
            }
            if row == sr {
                return col >= sc;
            }
            if row == er {
                return col <= ec;
            }
            // Middle rows are fully selected.
            true
        } else {
            false
        }
    }
}

impl Default for MouseSelection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_creates_selecting_state() {
        let mut sel = MouseSelection::new();
        sel.start("pane-1", 5, 10);
        assert_eq!(
            sel.state,
            SelectionState::Selecting {
                start_row: 5,
                start_col: 10,
                end_row: 5,
                end_col: 10,
            }
        );
        assert_eq!(sel.pane_id, Some("pane-1".to_string()));
    }

    #[test]
    fn test_update_changes_end_position() {
        let mut sel = MouseSelection::new();
        sel.start("pane-1", 5, 10);
        sel.update(8, 20);
        assert_eq!(
            sel.state,
            SelectionState::Selecting {
                start_row: 5,
                start_col: 10,
                end_row: 8,
                end_col: 20,
            }
        );
    }

    #[test]
    fn test_finish_finalizes_selection() {
        let mut sel = MouseSelection::new();
        sel.start("pane-1", 2, 3);
        sel.update(4, 15);
        sel.finish();
        assert_eq!(
            sel.state,
            SelectionState::Selected {
                start_row: 2,
                start_col: 3,
                end_row: 4,
                end_col: 15,
            }
        );
    }

    #[test]
    fn test_click_no_drag_clears_selection() {
        let mut sel = MouseSelection::new();
        sel.start("pane-1", 5, 10);
        // No update call – start and end are the same.
        sel.finish();
        assert_eq!(sel.state, SelectionState::None);
        assert_eq!(sel.pane_id, None);
    }

    #[test]
    fn test_is_selected_checks_range_correctly() {
        let mut sel = MouseSelection::new();
        sel.start("pane-1", 2, 5);
        sel.update(4, 10);
        sel.finish();

        // Row 2 (start row): col 5 and beyond should be selected
        assert!(sel.is_selected(2, 5));
        assert!(sel.is_selected(2, 8));
        assert!(!sel.is_selected(2, 4));

        // Row 3 (middle row): entire row selected
        assert!(sel.is_selected(3, 0));
        assert!(sel.is_selected(3, 100));

        // Row 4 (end row): up to col 10
        assert!(sel.is_selected(4, 0));
        assert!(sel.is_selected(4, 10));
        assert!(!sel.is_selected(4, 11));

        // Outside the selection
        assert!(!sel.is_selected(1, 5));
        assert!(!sel.is_selected(5, 0));
    }

    #[test]
    fn test_is_selected_single_row_selection() {
        let mut sel = MouseSelection::new();
        sel.start("pane-1", 3, 5);
        sel.update(3, 15);
        sel.finish();

        assert!(sel.is_selected(3, 5));
        assert!(sel.is_selected(3, 10));
        assert!(sel.is_selected(3, 15));
        assert!(!sel.is_selected(3, 4));
        assert!(!sel.is_selected(3, 16));
    }

    #[test]
    fn test_get_range_normalizes_reverse_selection() {
        let mut sel = MouseSelection::new();
        // Start below/right, drag up/left (reverse selection)
        sel.start("pane-1", 10, 20);
        sel.update(5, 3);
        let range = sel.get_range();
        assert_eq!(range, Some((5, 3, 10, 20)));
    }

    #[test]
    fn test_clear_resets_state() {
        let mut sel = MouseSelection::new();
        sel.start("pane-1", 1, 1);
        sel.update(5, 5);
        sel.clear();
        assert_eq!(sel.state, SelectionState::None);
        assert_eq!(sel.pane_id, None);
        assert_eq!(sel.get_range(), None);
    }
}
