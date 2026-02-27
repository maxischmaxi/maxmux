// maxmux-statusbar: Status bar renderer and theme definitions for MaxMux.
//
// This crate provides:
// - 7 built-in color themes (Catppuccin Mocha, Dracula, Nord, Tokyo Night,
//   Gruvbox, One Dark, Solarized)
// - 5 separator styles (powerline, rounded, flat, arrow, slant)
// - A renderer that composes segments into an ANSI-escaped status bar line

pub mod renderer;
pub mod separators;
pub mod themes;
pub mod types;

// Re-export the most commonly used items.
pub use renderer::render_segments;
pub use separators::get_separator_chars;
pub use themes::resolve_theme;
pub use types::{ColorPair, ResolvedTheme, Segment, ThemeModuleColors};
