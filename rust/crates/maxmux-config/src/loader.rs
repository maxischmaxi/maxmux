use std::path::{Path, PathBuf};

use crate::schema::{default_keybindings, MaxmuxConfig};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ConfigLoadError {
    #[error("failed to read config file {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("config validation failed: {0}")]
    Validation(#[from] crate::schema::ConfigValidationError),
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Search for a config file in the standard locations.
///
/// Order:
/// 1. `./maxmux.toml` (current working directory)
/// 2. `~/.config/maxmux/config.toml`
///
/// Returns `None` if no config file is found (defaults will be used).
pub fn find_config_file() -> Option<PathBuf> {
    // 1. CWD
    let cwd_path = Path::new("maxmux.toml");
    if cwd_path.exists() {
        return Some(cwd_path.to_path_buf());
    }

    // 2. XDG / home config dir
    if let Some(config_dir) = dirs::config_dir() {
        let home_path = config_dir.join("maxmux").join("config.toml");
        if home_path.exists() {
            return Some(home_path);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Load config from a specific TOML file path.
///
/// Parses the file, merges keybindings with defaults (user overrides win),
/// and validates the result.
pub fn load_config_from_path(path: &Path) -> Result<MaxmuxConfig, ConfigLoadError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigLoadError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    load_config_from_str(&content, path)
}

/// Parse a TOML string into a validated `MaxmuxConfig`.
///
/// This is the core parsing function, also useful for testing.
pub fn load_config_from_str(content: &str, path: &Path) -> Result<MaxmuxConfig, ConfigLoadError> {
    let mut config: MaxmuxConfig =
        toml::from_str(content).map_err(|e| ConfigLoadError::Parse {
            path: path.to_path_buf(),
            source: e,
        })?;

    // Merge keybindings: defaults first, then overlay user bindings.
    // If the TOML file specified keybindings, serde replaced the whole field
    // with only the user's entries. We merge defaults underneath.
    let user_keybindings = config.keybindings.clone();
    let mut merged = default_keybindings();
    merged.extend(user_keybindings);
    config.keybindings = merged;

    // global_keybindings: no defaults to merge, user's value is final.

    config.validate()?;

    tracing::debug!(?path, "loaded config");
    Ok(config)
}

/// Discover and load config, falling back to defaults if no file is found.
pub fn load_config() -> Result<MaxmuxConfig, ConfigLoadError> {
    match find_config_file() {
        Some(path) => {
            tracing::info!(?path, "found config file");
            load_config_from_path(&path)
        }
        None => {
            tracing::info!("no config file found, using defaults");
            Ok(MaxmuxConfig::default())
        }
    }
}
