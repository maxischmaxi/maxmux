use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum BorderStyle {
    #[default]
    Rounded,
    Sharp,
    Double,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Position {
    Top,
    #[default]
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SeparatorStyle {
    #[default]
    Powerline,
    Rounded,
    Flat,
    Arrow,
    Slant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SessionListMode {
    #[default]
    Sidebar,
    Overlay,
}

// ---------------------------------------------------------------------------
// KeybindingValue — either a plain command string or { command, unless }
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeybindingValue {
    Command(String),
    Conditional {
        command: String,
        #[serde(default)]
        unless: Vec<String>,
    },
}

// ---------------------------------------------------------------------------
// Theme structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusBarThemeColors {
    pub bg: String,
    pub fg: String,
    pub active: String,
}

impl Default for StatusBarThemeColors {
    fn default() -> Self {
        Self {
            bg: "#1e1e2e".into(),
            fg: "#cdd6f4".into(),
            active: "#89b4fa".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BorderThemeConfig {
    pub style: BorderStyle,
    pub line_style: LineStyle,
    pub fg: String,
    pub active_fg: String,
}

impl Default for BorderThemeConfig {
    fn default() -> Self {
        Self {
            style: BorderStyle::Rounded,
            line_style: LineStyle::Solid,
            fg: "#585b70".into(),
            active_fg: "#89b4fa".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct ThemeConfig {
    pub status_bar: StatusBarThemeColors,
    pub border: BorderThemeConfig,
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeparatorConfig {
    pub style: SeparatorStyle,
    pub left: Option<String>,
    pub right: Option<String>,
}

impl Default for SeparatorConfig {
    fn default() -> Self {
        Self {
            style: SeparatorStyle::Powerline,
            left: None,
            right: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusBarConfig {
    pub enabled: bool,
    pub position: Position,
    pub theme: String,
    pub separator: SeparatorConfig,
    pub icons: bool,
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub modules: HashMap<String, toml::Value>,
    pub refresh_interval: u64,
    pub metrics_interval: u64,
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            position: Position::Bottom,
            theme: "catppuccin-mocha".into(),
            separator: SeparatorConfig::default(),
            icons: true,
            left: vec!["session".into(), "windows".into()],
            right: vec!["git".into(), "cwd".into(), "datetime".into()],
            modules: HashMap::new(),
            refresh_interval: 1000,
            metrics_interval: 5000,
        }
    }
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionsConfig {
    pub auto_save: bool,
    pub auto_save_interval: u64,
    pub auto_restore: bool,
    pub save_path: String,
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            auto_save: true,
            auto_save_interval: 30_000,
            auto_restore: true,
            save_path: "~/.maxmux/sessions/".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Session list
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionListConfig {
    pub mode: SessionListMode,
    pub sidebar_position: String,
    pub sidebar_width: u16,
}

impl Default for SessionListConfig {
    fn default() -> Self {
        Self {
            mode: SessionListMode::Sidebar,
            sidebar_position: "left".into(),
            sidebar_width: 30,
        }
    }
}

// ---------------------------------------------------------------------------
// Root config
// ---------------------------------------------------------------------------

fn default_shell() -> String {
    env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into())
}

/// Returns the full set of default prefix-mode keybindings.
pub fn default_keybindings() -> HashMap<String, KeybindingValue> {
    let bindings: &[(&str, &str)] = &[
        ("c", "window:create"),
        ("n", "window:next"),
        ("p", "window:previous"),
        ("&", "window:close"),
        (",", "window:rename"),
        ("|", "pane:split-horizontal"),
        ("-", "pane:split-vertical"),
        ("o", "pane:next"),
        ("x", "pane:close"),
        ("z", "pane:zoom"),
        ("Up", "pane:focus-up"),
        ("Down", "pane:focus-down"),
        ("Left", "pane:focus-left"),
        ("Right", "pane:focus-right"),
        ("d", "session:detach"),
        ("$", "session:rename"),
        ("s", "session:find"),
        (":", "command-palette"),
        ("?", "keybindings:show"),
        ("[", "copy-mode:enter"),
        ("N", "notes:create"),
        ("L", "notes:list"),
    ];
    bindings
        .iter()
        .map(|(k, v)| ((*k).to_string(), KeybindingValue::Command((*v).to_string())))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaxmuxConfig {
    pub prefix_key: String,
    pub prefix_timeout: u64,
    pub history_limit: u32,
    pub shell: String,
    pub new_pane_cwd: String,
    pub switch_to_new_window: bool,
    pub automatic_rename: bool,
    pub automatic_rename_interval: u64,
    pub mouse: bool,
    pub show_prefix_help: bool,
    pub debug: bool,
    pub theme: ThemeConfig,
    pub keybindings: HashMap<String, KeybindingValue>,
    pub global_keybindings: HashMap<String, KeybindingValue>,
    pub status_bar: StatusBarConfig,
    pub sessions: SessionsConfig,
    pub session_list: SessionListConfig,
    pub plugins: Vec<toml::Value>,
}

impl Default for MaxmuxConfig {
    fn default() -> Self {
        Self {
            prefix_key: "C-a".into(),
            prefix_timeout: 0,
            history_limit: 10_000,
            shell: default_shell(),
            new_pane_cwd: "inherit".into(),
            switch_to_new_window: true,
            automatic_rename: true,
            automatic_rename_interval: 2000,
            mouse: true,
            show_prefix_help: true,
            debug: false,
            theme: ThemeConfig::default(),
            keybindings: default_keybindings(),
            global_keybindings: HashMap::new(),
            status_bar: StatusBarConfig::default(),
            sessions: SessionsConfig::default(),
            session_list: SessionListConfig::default(),
            plugins: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("history_limit must be between 0 and 100000, got {0}")]
    HistoryLimitOutOfRange(u32),
    #[error("sidebar_width must be between 20 and 80, got {0}")]
    SidebarWidthOutOfRange(u16),
}

impl MaxmuxConfig {
    /// Validate config values that cannot be expressed via serde alone.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.history_limit > 100_000 {
            return Err(ConfigValidationError::HistoryLimitOutOfRange(
                self.history_limit,
            ));
        }
        if self.session_list.sidebar_width < 20 || self.session_list.sidebar_width > 80 {
            return Err(ConfigValidationError::SidebarWidthOutOfRange(
                self.session_list.sidebar_width,
            ));
        }
        Ok(())
    }
}
