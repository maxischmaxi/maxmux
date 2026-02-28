use std::path::{Path, PathBuf};

use crate::lua::LuaRuntime;

/// Result of loading a single plugin file.
pub struct PluginLoadResult {
    /// Path to the plugin file that was loaded.
    pub path: PathBuf,
    /// `None` if the plugin loaded successfully, `Some(message)` if it failed.
    pub error: Option<String>,
}

impl PluginLoadResult {
    /// Create a successful load result.
    pub fn ok(path: PathBuf) -> Self {
        Self { path, error: None }
    }

    /// Create a failed load result.
    pub fn err(path: PathBuf, error: impl std::fmt::Display) -> Self {
        Self {
            path,
            error: Some(error.to_string()),
        }
    }

    /// Returns `true` if the plugin loaded successfully.
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

/// Discovers and loads Lua plugin files from a directory.
pub struct PluginLoader {
    plugin_dir: PathBuf,
}

impl PluginLoader {
    /// Create a new plugin loader targeting the given directory.
    pub fn new(plugin_dir: PathBuf) -> Self {
        Self { plugin_dir }
    }

    /// Return the default plugin directory (`$XDG_CONFIG_HOME/maxmux/plugins`
    /// or `~/.config/maxmux/plugins`).
    pub fn default_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_default()
            .join("maxmux")
            .join("plugins")
    }

    /// Return the configured plugin directory.
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// Discover `.lua` files in the plugin directory.
    ///
    /// Returns an empty `Vec` if the directory does not exist. Files are sorted
    /// by name to ensure deterministic load order.
    pub fn discover(&self) -> Vec<PathBuf> {
        if !self.plugin_dir.exists() {
            return Vec::new();
        }

        let mut files: Vec<PathBuf> = match std::fs::read_dir(&self.plugin_dir) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "lua"))
                .collect(),
            Err(_) => Vec::new(),
        };

        files.sort();
        files
    }

    /// Load all discovered plugins into the given Lua runtime.
    ///
    /// Returns a `PluginLoadResult` for each file, in load order.
    pub fn load_all(&self, runtime: &LuaRuntime) -> Vec<PluginLoadResult> {
        let files = self.discover();
        let mut results = Vec::new();

        for file in files {
            match runtime.exec_file(&file) {
                Ok(()) => {
                    tracing::info!("loaded plugin: {}", file.display());
                    results.push(PluginLoadResult::ok(file));
                }
                Err(e) => {
                    tracing::warn!("failed to load plugin {}: {}", file.display(), e);
                    results.push(PluginLoadResult::err(file, e));
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn discover_returns_empty_for_missing_directory() {
        let loader = PluginLoader::new(PathBuf::from("/nonexistent/path/to/plugins"));
        let files = loader.discover();
        assert!(files.is_empty());
    }

    #[test]
    fn discover_finds_lua_files_sorted() {
        let dir = TempDir::new().unwrap();

        // Create files in non-alphabetical order.
        fs::write(dir.path().join("c_plugin.lua"), "-- c").unwrap();
        fs::write(dir.path().join("a_plugin.lua"), "-- a").unwrap();
        fs::write(dir.path().join("b_plugin.lua"), "-- b").unwrap();
        fs::write(dir.path().join("not_a_plugin.txt"), "nope").unwrap();

        let loader = PluginLoader::new(dir.path().to_path_buf());
        let files = loader.discover();

        assert_eq!(files.len(), 3);
        assert!(files[0].ends_with("a_plugin.lua"));
        assert!(files[1].ends_with("b_plugin.lua"));
        assert!(files[2].ends_with("c_plugin.lua"));
    }

    #[test]
    fn load_all_loads_files_from_directory() {
        let dir = TempDir::new().unwrap();

        fs::write(dir.path().join("good.lua"), "x = 42").unwrap();
        fs::write(dir.path().join("also_good.lua"), "y = 7").unwrap();

        let loader = PluginLoader::new(dir.path().to_path_buf());
        let runtime = LuaRuntime::new().expect("failed to create runtime");

        let results = loader.load_all(&runtime);

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn load_all_reports_errors_for_bad_scripts() {
        let dir = TempDir::new().unwrap();

        fs::write(dir.path().join("good.lua"), "x = 1").unwrap();
        fs::write(dir.path().join("bad.lua"), "this is not valid lua !!!").unwrap();

        let loader = PluginLoader::new(dir.path().to_path_buf());
        let runtime = LuaRuntime::new().expect("failed to create runtime");

        let results = loader.load_all(&runtime);

        assert_eq!(results.len(), 2);
        // bad.lua sorts before good.lua
        assert!(results[0].error.is_some()); // bad.lua failed
        assert!(results[1].error.is_none()); // good.lua succeeded
    }
}
