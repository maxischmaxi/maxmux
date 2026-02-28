use std::path::PathBuf;
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ConfigWatchError {
    #[error("failed to initialize file watcher: {0}")]
    Init(#[from] notify::Error),
    #[error("config path has no parent directory: {0}")]
    NoParentDir(PathBuf),
}

// ---------------------------------------------------------------------------
// ConfigWatcher
// ---------------------------------------------------------------------------

/// Watches a config file for changes and sends a notification via an mpsc
/// channel. The watcher monitors the parent directory so it can detect file
/// creation (e.g. when the config file doesn't exist yet).
///
/// Events are debounced by a configurable duration (default 300 ms) to avoid
/// flooding consumers when editors perform multiple rapid writes.
pub struct ConfigWatcher {
    _watcher: RecommendedWatcher,
    // Hold the shutdown sender so the background debounce task stops when
    // ConfigWatcher is dropped.
    _shutdown_tx: mpsc::Sender<()>,
}

/// Default debounce duration (300 ms).
const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(300);

impl ConfigWatcher {
    /// Start watching `config_path` for changes.
    ///
    /// Returns `(ConfigWatcher, UnboundedReceiver<()>)`. The receiver yields
    /// `()` each time the config file is modified (after debouncing).
    ///
    /// Drop the `ConfigWatcher` to stop watching.
    pub fn new(
        config_path: PathBuf,
    ) -> Result<(Self, mpsc::UnboundedReceiver<()>), ConfigWatchError> {
        let (tx, rx) = mpsc::unbounded_channel();
        let watcher = Self::watch(config_path, tx, DEFAULT_DEBOUNCE)?;
        Ok((watcher, rx))
    }

    /// Start watching `config_path` and send change notifications to `tx`.
    ///
    /// `debounce` controls how long to wait after the last filesystem event
    /// before sending a notification.
    pub fn watch(
        config_path: PathBuf,
        tx: mpsc::UnboundedSender<()>,
        debounce: Duration,
    ) -> Result<Self, ConfigWatchError> {
        let watch_dir = config_path
            .parent()
            .ok_or_else(|| ConfigWatchError::NoParentDir(config_path.clone()))?
            .to_path_buf();

        let config_path_canon = normalize_path(&config_path);

        // Channel for raw notify events -> debounce task.
        let (raw_tx, mut raw_rx) = mpsc::unbounded_channel::<()>();

        // Shutdown channel: when _shutdown_tx is dropped the debounce task
        // will exit.
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        // Spawn the debounce task.
        tokio::spawn(async move {
            loop {
                // Wait for the first raw event (or shutdown).
                tokio::select! {
                    biased;
                    _ = shutdown_rx.recv() => break,
                    ev = raw_rx.recv() => {
                        if ev.is_none() {
                            break;
                        }
                    }
                }

                // Drain any additional events that arrive during the debounce
                // window.
                loop {
                    tokio::select! {
                        biased;
                        _ = shutdown_rx.recv() => return,
                        _ = raw_rx.recv() => {
                            // Another event arrived — restart the timer by
                            // continuing to drain.
                            continue;
                        }
                        _ = tokio::time::sleep(debounce) => break,
                    }
                }

                // Debounce window elapsed — notify consumer.
                if tx.send(()).is_err() {
                    // Receiver dropped; stop.
                    break;
                }
            }
        });

        // Create the notify watcher that feeds raw events.
        let mut watcher =
            notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                let event = match res {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("file watcher error: {e}");
                        return;
                    }
                };

                // Only care about modifications, creations, and removals.
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {}
                    _ => return,
                }

                // Check if any of the affected paths match our config file.
                let dominated = event
                    .paths
                    .iter()
                    .any(|p| normalize_path(p) == config_path_canon);
                if !dominated {
                    return;
                }

                let _ = raw_tx.send(());
            })?;

        watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;

        tracing::debug!(?config_path, ?watch_dir, "config watcher started");

        Ok(Self {
            _watcher: watcher,
            _shutdown_tx: shutdown_tx,
        })
    }

    /// Explicitly stop watching. This is equivalent to dropping the
    /// `ConfigWatcher`.
    pub fn stop(self) {
        drop(self);
    }
}

/// Best-effort path normalization: canonicalize if the path exists, otherwise
/// use `std::fs::canonicalize` on the parent and append the file name.
fn normalize_path(path: &PathBuf) -> PathBuf {
    if let Ok(p) = std::fs::canonicalize(path) {
        return p;
    }
    // File may not exist yet. Try canonicalizing the parent.
    if let (Some(parent), Some(file_name)) = (path.parent(), path.file_name()) {
        if let Ok(parent) = std::fs::canonicalize(parent) {
            return parent.join(file_name);
        }
    }
    path.clone()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    use tokio::time::{self, timeout};

    /// Helper: create a config file in a temp directory.
    fn create_config(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("config.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "prefix_key = \"C-a\"").unwrap();
        f.sync_all().unwrap();
        path
    }

    // 1. Watcher detects file modification.
    #[tokio::test]
    async fn detects_file_modification() {
        let tmp = tempdir().unwrap();
        let config_path = create_config(tmp.path());

        let (tx, mut rx) = mpsc::unbounded_channel();
        let _watcher =
            ConfigWatcher::watch(config_path.clone(), tx, Duration::from_millis(100)).unwrap();

        // Give the watcher a moment to start.
        time::sleep(Duration::from_millis(50)).await;

        // Modify the file.
        {
            let mut f = std::fs::File::create(&config_path).unwrap();
            writeln!(f, "prefix_key = \"C-b\"").unwrap();
            f.sync_all().unwrap();
        }

        // We should get a notification within a reasonable time.
        let result = timeout(Duration::from_secs(5), rx.recv()).await;
        assert!(
            result.is_ok(),
            "should receive notification after file modification"
        );
        assert_eq!(result.unwrap(), Some(()));
    }

    // 2. Watcher handles missing config file gracefully (watches directory).
    #[tokio::test]
    async fn handles_missing_file() {
        let tmp = tempdir().unwrap();
        let config_path = tmp.path().join("does_not_exist.toml");

        let (tx, mut rx) = mpsc::unbounded_channel();
        // Should not error even though the file doesn't exist.
        let _watcher =
            ConfigWatcher::watch(config_path.clone(), tx, Duration::from_millis(100)).unwrap();

        // Give the watcher a moment to start.
        time::sleep(Duration::from_millis(50)).await;

        // Create the file — watcher should detect it.
        {
            let mut f = std::fs::File::create(&config_path).unwrap();
            writeln!(f, "prefix_key = \"C-b\"").unwrap();
            f.sync_all().unwrap();
        }

        let result = timeout(Duration::from_secs(5), rx.recv()).await;
        assert!(
            result.is_ok(),
            "should receive notification when missing file is created"
        );
        assert_eq!(result.unwrap(), Some(()));
    }

    // 3. Debounce coalesces rapid changes into fewer notifications.
    #[tokio::test]
    async fn debounce_coalesces_rapid_changes() {
        let tmp = tempdir().unwrap();
        let config_path = create_config(tmp.path());

        let (tx, mut rx) = mpsc::unbounded_channel();
        let _watcher =
            ConfigWatcher::watch(config_path.clone(), tx, Duration::from_millis(300)).unwrap();

        // Give the watcher a moment to start.
        time::sleep(Duration::from_millis(50)).await;

        // Fire off multiple rapid writes.
        for i in 0..5 {
            let mut f = std::fs::File::create(&config_path).unwrap();
            writeln!(f, "history_limit = {}", 1000 + i).unwrap();
            f.sync_all().unwrap();
            time::sleep(Duration::from_millis(30)).await;
        }

        // Wait for the debounce to settle.
        time::sleep(Duration::from_millis(500)).await;

        // Drain whatever came through.
        let mut count = 0;
        while let Ok(Some(())) = rx.try_recv().map(Some) {
            count += 1;
        }

        // We should have far fewer notifications than the 5 writes.
        // With a 300 ms debounce and 30 ms intervals (total ~150 ms burst),
        // we expect exactly 1 notification.
        assert!(
            count >= 1 && count <= 2,
            "debounce should coalesce rapid changes: got {count} notifications for 5 writes"
        );
    }

    // 4. Dropping ConfigWatcher shuts down cleanly (no panic / hang).
    #[tokio::test]
    async fn drop_shuts_down_cleanly() {
        let tmp = tempdir().unwrap();
        let config_path = create_config(tmp.path());

        let (watcher, _rx) = ConfigWatcher::new(config_path).unwrap();
        watcher.stop();
        // If this test completes without hanging, shutdown works.
    }
}
