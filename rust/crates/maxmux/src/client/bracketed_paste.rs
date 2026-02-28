// Bracketed paste relay – detects bracketed-paste mode sequences emitted by
// inner applications and relays the mode change to the outer terminal.
//
// When a shell or editor inside a pane sends `\x1b[?2004h` (enable) or
// `\x1b[?2004l` (disable), we track the state per pane so that the client
// can propagate the correct mode to the real terminal.

use std::collections::HashSet;

/// Enable bracketed paste mode sequence.
const ENABLE_SEQ: &[u8] = b"\x1b[?2004h";
/// Disable bracketed paste mode sequence.
const DISABLE_SEQ: &[u8] = b"\x1b[?2004l";

/// Tracks which panes have bracketed paste mode enabled.
pub struct BracketedPasteTracker {
    enabled_panes: HashSet<String>,
}

impl BracketedPasteTracker {
    pub fn new() -> Self {
        Self {
            enabled_panes: HashSet::new(),
        }
    }

    /// Scan pane output for bracketed paste mode sequences.
    ///
    /// Returns `Some(true)` if enable was detected, `Some(false)` if disable
    /// was detected, or `None` if neither sequence appeared.
    ///
    /// When both sequences appear in the same chunk (e.g. a full-screen app
    /// toggling the mode), the *last* one wins.
    pub fn scan_output(&mut self, pane_id: &str, data: &[u8]) -> Option<bool> {
        let last_enable = data
            .windows(ENABLE_SEQ.len())
            .rposition(|w| w == ENABLE_SEQ);
        let last_disable = data
            .windows(DISABLE_SEQ.len())
            .rposition(|w| w == DISABLE_SEQ);

        match (last_enable, last_disable) {
            (Some(e), Some(d)) => {
                if e > d {
                    self.enabled_panes.insert(pane_id.to_string());
                    Some(true)
                } else {
                    self.enabled_panes.remove(pane_id);
                    Some(false)
                }
            }
            (Some(_), None) => {
                self.enabled_panes.insert(pane_id.to_string());
                Some(true)
            }
            (None, Some(_)) => {
                self.enabled_panes.remove(pane_id);
                Some(false)
            }
            (None, None) => None,
        }
    }

    /// Check whether bracketed paste mode is enabled for a given pane.
    pub fn is_enabled(&self, pane_id: &str) -> bool {
        self.enabled_panes.contains(pane_id)
    }

    /// Remove all state for a pane (e.g. when the pane exits).
    pub fn remove_pane(&mut self, pane_id: &str) {
        self.enabled_panes.remove(pane_id);
    }
}

impl Default for BracketedPasteTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_detects_enable_sequence() {
        let mut tracker = BracketedPasteTracker::new();
        let data = b"\x1b[?2004h";
        let result = tracker.scan_output("pane-1", data);
        assert_eq!(result, Some(true));
        assert!(tracker.is_enabled("pane-1"));
    }

    #[test]
    fn test_scan_detects_disable_sequence() {
        let mut tracker = BracketedPasteTracker::new();
        // First enable, then disable
        tracker.scan_output("pane-1", b"\x1b[?2004h");
        let result = tracker.scan_output("pane-1", b"\x1b[?2004l");
        assert_eq!(result, Some(false));
        assert!(!tracker.is_enabled("pane-1"));
    }

    #[test]
    fn test_no_sequence_returns_none() {
        let mut tracker = BracketedPasteTracker::new();
        let data = b"Hello, world! Some normal output.";
        let result = tracker.scan_output("pane-1", data);
        assert_eq!(result, None);
    }

    #[test]
    fn test_remove_pane_clears_state() {
        let mut tracker = BracketedPasteTracker::new();
        tracker.scan_output("pane-1", b"\x1b[?2004h");
        assert!(tracker.is_enabled("pane-1"));
        tracker.remove_pane("pane-1");
        assert!(!tracker.is_enabled("pane-1"));
    }

    #[test]
    fn test_both_sequences_last_one_wins() {
        let mut tracker = BracketedPasteTracker::new();
        // Enable then disable in same chunk -> disable wins (later position)
        let mut data = Vec::new();
        data.extend_from_slice(b"\x1b[?2004h");
        data.extend_from_slice(b"some output");
        data.extend_from_slice(b"\x1b[?2004l");
        let result = tracker.scan_output("pane-1", &data);
        assert_eq!(result, Some(false));
        assert!(!tracker.is_enabled("pane-1"));
    }

    #[test]
    fn test_multiple_panes_tracked_independently() {
        let mut tracker = BracketedPasteTracker::new();
        tracker.scan_output("pane-1", b"\x1b[?2004h");
        tracker.scan_output("pane-2", b"\x1b[?2004l");
        assert!(tracker.is_enabled("pane-1"));
        assert!(!tracker.is_enabled("pane-2"));
    }
}
