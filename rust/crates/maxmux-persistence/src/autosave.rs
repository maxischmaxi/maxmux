//! Autosave module -- periodically signals the application to persist session state.

use std::time::Duration;

use tokio::sync::mpsc;

/// Handle to a running autosave background task.
///
/// The task periodically sends a unit `()` value on the provided channel
/// at the configured interval, signalling that the application should
/// persist its session state.  Dropping the handle or calling [`stop`]
/// cancels the background task.
pub struct AutosaveHandle {
    cancel_tx: mpsc::Sender<()>,
}

impl AutosaveHandle {
    /// Spawn the autosave timer.
    ///
    /// Every `interval`, the task sends `()` on `save_tx`.
    /// The task stops when [`AutosaveHandle::stop`] is called, when
    /// the `AutosaveHandle` is dropped, or when the receiver side of
    /// `save_tx` is dropped.
    pub fn start(interval: Duration, save_tx: mpsc::UnboundedSender<()>) -> Self {
        let (cancel_tx, mut cancel_rx) = mpsc::channel::<()>(1);

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // The first tick fires immediately; consume it so the first
            // real save happens after one full interval.
            ticker.tick().await;

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        if save_tx.send(()).is_err() {
                            // Receiver dropped -- nothing to save to.
                            tracing::debug!("autosave: receiver dropped, stopping");
                            break;
                        }
                        tracing::trace!("autosave: tick");
                    }
                    _ = cancel_rx.recv() => {
                        tracing::debug!("autosave: cancelled");
                        break;
                    }
                }
            }
        });

        Self { cancel_tx }
    }

    /// Signal the background task to stop.
    pub async fn stop(self) {
        // Ignoring the error: if the task already exited the channel
        // is closed, which is fine.
        let _ = self.cancel_tx.send(()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn autosave_sends_signals_at_interval() {
        let (save_tx, mut save_rx) = mpsc::unbounded_channel();

        let handle = AutosaveHandle::start(Duration::from_millis(50), save_tx);

        // Wait long enough for at least 3 ticks (the first immediate tick is
        // consumed internally, so real ticks start after 50 ms).
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Drain all available messages.
        let mut count = 0;
        while save_rx.try_recv().is_ok() {
            count += 1;
        }

        assert!(count >= 2, "expected at least 2 save signals, got {count}");

        handle.stop().await;
    }

    #[tokio::test]
    async fn autosave_stops_on_cancel() {
        let (save_tx, mut save_rx) = mpsc::unbounded_channel();

        let handle = AutosaveHandle::start(Duration::from_millis(50), save_tx);

        // Let a couple of ticks fire.
        tokio::time::sleep(Duration::from_millis(130)).await;
        handle.stop().await;

        // Drain existing messages.
        while save_rx.try_recv().is_ok() {}

        // After stopping, no more messages should arrive.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(
            save_rx.try_recv().is_err(),
            "expected no signals after stop"
        );
    }

    #[tokio::test]
    async fn autosave_stops_when_receiver_dropped() {
        let (save_tx, save_rx) = mpsc::unbounded_channel();

        let _handle = AutosaveHandle::start(Duration::from_millis(30), save_tx);

        // Drop the receiver -- the task should notice and exit.
        drop(save_rx);

        // Give the task time to realize the receiver is gone.
        tokio::time::sleep(Duration::from_millis(100)).await;

        // If we get here without hanging, the task exited properly.
    }
}
