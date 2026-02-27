// Type definitions for the status bar renderer.

use serde::{Deserialize, Serialize};

/// A foreground/background color pair, stored as hex strings (e.g. "#1e1e2e").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorPair {
    pub fg: String,
    pub bg: String,
}

impl ColorPair {
    pub fn new(fg: &str, bg: &str) -> Self {
        Self {
            fg: fg.to_string(),
            bg: bg.to_string(),
        }
    }
}

/// Per-module color assignments for a resolved theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeModuleColors {
    pub session: ColorPair,
    pub active_window: ColorPair,
    pub inactive_window: ColorPair,
    pub git_clean: ColorPair,
    pub git_dirty: ColorPair,
    pub cpu_low: ColorPair,
    pub cpu_med: ColorPair,
    pub cpu_high: ColorPair,
    pub battery_high: ColorPair,
    pub battery_med: ColorPair,
    pub battery_low: ColorPair,
    pub prefix: ColorPair,
    pub prefix_inactive: ColorPair,
}

/// A fully resolved theme with bar colors, accent palette, and module colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTheme {
    /// Bar background & foreground.
    pub bar: ColorPair,
    /// 8 accent hex colors for modules.
    pub accents: Vec<String>,
    /// Per-module color assignments.
    pub modules: ThemeModuleColors,
}

/// A single rendered segment in the status bar.
#[derive(Debug, Clone)]
pub struct Segment {
    pub text: String,
    pub fg: String,
    pub bg: String,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
}

impl Segment {
    /// Create a new segment with the given text and colors, no style attributes.
    pub fn new(text: &str, fg: &str, bg: &str) -> Self {
        Self {
            text: text.to_string(),
            fg: fg.to_string(),
            bg: bg.to_string(),
            bold: false,
            italic: false,
            dim: false,
        }
    }

    /// Builder: set bold.
    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    /// Builder: set italic.
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    /// Builder: set dim.
    pub fn with_dim(mut self, dim: bool) -> Self {
        self.dim = dim;
        self
    }
}
