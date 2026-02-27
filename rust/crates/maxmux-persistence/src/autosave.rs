//! Autosave module - periodically persists session state.
//!
//! This is a placeholder created by Task 9.2 so that the crate compiles.
//! Task 9.1 will provide the full implementation.

/// Handle returned by the autosave spawner, allowing callers to stop the timer.
pub struct AutosaveHandle {
    cancel: tokio::sync::oneshot::Sender<()>,
}

impl AutosaveHandle {
    /// Stop the autosave timer.
    pub fn stop(self) {
        let _ = self.cancel.send(());
    }
}
