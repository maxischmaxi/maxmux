pub mod loader;
pub mod schema;
pub mod watcher;

#[cfg(test)]
mod tests;

// Re-export the main types for convenience.
pub use schema::{
    BorderStyle, BorderThemeConfig, KeybindingValue, LineStyle, MaxmuxConfig, Position,
    SeparatorConfig, SeparatorStyle, SessionListConfig, SessionListMode, SessionsConfig,
    StatusBarConfig, StatusBarThemeColors, ThemeConfig,
};

pub use loader::{ConfigLoadError, find_config_file, load_config, load_config_from_path};

pub use watcher::{ConfigWatchError, ConfigWatcher};
