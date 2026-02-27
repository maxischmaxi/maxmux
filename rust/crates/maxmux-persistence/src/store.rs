use std::fs;
use std::io::Write;
use std::path::PathBuf;

use maxmux_core::session::Session;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Persists sessions to a JSON file on disk.
///
/// Uses atomic writes (write to temp file, then rename) to prevent
/// corruption from partial writes or crashes.
pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    /// Create a new `SessionStore` that reads/writes the given path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Return the default path for the session file:
    /// `~/.maxmux/sessions/sessions.json`
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".maxmux")
            .join("sessions")
            .join("sessions.json")
    }

    /// Save sessions to disk using an atomic write.
    ///
    /// 1. Serializes sessions to pretty-printed JSON.
    /// 2. Creates parent directories if they don't exist.
    /// 3. Writes to a temporary file in the same directory.
    /// 4. Renames the temporary file to the target path (atomic on POSIX).
    pub fn save(&self, sessions: &[Session]) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(sessions)?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write to a temp file in the same directory so rename is atomic.
        let tmp_path = self.path.with_extension("json.tmp");
        {
            let mut file = fs::File::create(&tmp_path)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        }

        fs::rename(&tmp_path, &self.path)?;

        tracing::debug!(path = %self.path.display(), count = sessions.len(), "saved sessions");

        Ok(())
    }

    /// Load sessions from disk.
    ///
    /// Returns an empty `Vec` if the file does not exist.
    /// Returns an error if the file exists but contains invalid JSON.
    pub fn load(&self) -> Result<Vec<Session>, StoreError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let data = fs::read_to_string(&self.path)?;
        let sessions: Vec<Session> = serde_json::from_str(&data)?;

        tracing::debug!(path = %self.path.display(), count = sessions.len(), "loaded sessions");

        Ok(sessions)
    }

    /// Check whether the session file exists on disk.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Return the path this store reads from / writes to.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maxmux_core::session::{LayoutNode, Pane, SplitDirection, Window};
    use tempfile::TempDir;

    /// Helper: create a realistic `Session` for testing.
    fn sample_session(name: &str) -> Session {
        Session {
            id: format!("sid-{name}"),
            name: name.to_string(),
            windows: vec![Window {
                id: "w1".into(),
                name: "shell".into(),
                panes: vec![Pane {
                    id: "p1".into(),
                    pid: Some(4242),
                    cwd: "/home/user".into(),
                    command: "zsh".into(),
                    title: "zsh".into(),
                }],
                layout: LayoutNode::Split {
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
                active_pane: "p1".into(),
            }],
            active_window: "w1".into(),
            created_at: 1700000000,
            attached_clients: vec!["client-1".into()],
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions.json");
        let store = SessionStore::new(path);

        let sessions = vec![sample_session("alpha"), sample_session("beta")];
        store.save(&sessions).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "sid-alpha");
        assert_eq!(loaded[0].name, "alpha");
        assert_eq!(loaded[1].id, "sid-beta");
        assert_eq!(loaded[1].name, "beta");

        // Verify window / pane data survived
        assert_eq!(loaded[0].windows.len(), 1);
        assert_eq!(loaded[0].windows[0].panes.len(), 1);
        assert_eq!(loaded[0].windows[0].panes[0].command, "zsh");
        assert_eq!(loaded[0].active_window, "w1");
        assert_eq!(loaded[0].attached_clients, vec!["client-1"]);
    }

    #[test]
    fn load_returns_empty_vec_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let store = SessionStore::new(path);

        let loaded = store.load().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a").join("b").join("c").join("sessions.json");
        let store = SessionStore::new(path.clone());

        store.save(&[sample_session("deep")]).unwrap();

        assert!(path.exists());
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "deep");
    }

    #[test]
    fn load_returns_error_on_corrupt_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions.json");

        fs::write(&path, "this is not json {{{").unwrap();

        let store = SessionStore::new(path);
        let result = store.load();
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, StoreError::Serialize(_)));
    }

    #[test]
    fn default_path_is_correct() {
        let path = SessionStore::default_path();
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with(".maxmux/sessions/sessions.json"),
            "unexpected default path: {path_str}"
        );
    }

    #[test]
    fn exists_reflects_file_state() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions.json");
        let store = SessionStore::new(path);

        assert!(!store.exists());

        store.save(&[]).unwrap();
        assert!(store.exists());
    }

    #[test]
    fn save_overwrites_previous_data() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions.json");
        let store = SessionStore::new(path);

        store.save(&[sample_session("first")]).unwrap();
        assert_eq!(store.load().unwrap().len(), 1);

        store
            .save(&[sample_session("a"), sample_session("b"), sample_session("c")])
            .unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].name, "a");
    }

    #[test]
    fn save_empty_sessions() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions.json");
        let store = SessionStore::new(path);

        store.save(&[]).unwrap();
        let loaded = store.load().unwrap();
        assert!(loaded.is_empty());
    }
}
