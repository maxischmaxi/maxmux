// PTY management module
//
// Manages pseudo-terminal (PTY) child processes for the terminal multiplexer.
// Uses nix::pty::openpty + fork/execvp for spawning shell processes.

use std::collections::HashMap;
use std::ffi::CString;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nix::libc;
use nix::pty::{Winsize, openpty};
use nix::sys::signal::{self, Signal};
use nix::sys::wait::{WaitStatus, waitpid};
use nix::unistd::{self, ForkResult, Pid};
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tracing;
use uuid::Uuid;

/// Type alias for PTY identifiers.
pub type PtyId = String;

/// Errors that can occur during PTY operations.
#[derive(Debug, Error)]
pub enum PtyError {
    #[error("failed to open PTY: {0}")]
    Open(String),

    #[error("PTY not found: {0}")]
    NotFound(PtyId),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Handle representing a single PTY child process.
pub struct PtyHandle {
    pub id: PtyId,
    pub pid: Pid,
    pub master_fd: OwnedFd,
    pub cols: u16,
    pub rows: u16,
    pub dead: Arc<AtomicBool>,
}

/// Manages a collection of PTY child processes.
pub struct PtyManager {
    ptys: HashMap<PtyId, PtyHandle>,
}

impl PtyManager {
    /// Create a new, empty PtyManager.
    pub fn new() -> Self {
        Self {
            ptys: HashMap::new(),
        }
    }

    /// Spawn a new PTY child process running the given shell.
    ///
    /// - `shell`: path to the shell executable (e.g. "/bin/bash")
    /// - `cols`, `rows`: initial terminal dimensions
    /// - `cwd`: optional working directory for the child
    /// - `on_data`: channel sender for (PtyId, data) when the child produces output
    /// - `on_exit`: channel sender for (PtyId, exit_code) when the child exits
    ///
    /// Returns the PtyId of the new PTY.
    pub fn spawn(
        &mut self,
        shell: &str,
        cols: u16,
        rows: u16,
        cwd: Option<&str>,
        on_data: mpsc::UnboundedSender<(PtyId, Vec<u8>)>,
        on_exit: mpsc::UnboundedSender<(PtyId, i32)>,
    ) -> Result<PtyId, PtyError> {
        let ws = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // Open a new PTY master/slave pair.
        let pty_result = openpty(Some(&ws), None::<&nix::sys::termios::Termios>)
            .map_err(|e| PtyError::Open(format!("openpty failed: {e}")))?;

        let master_fd = pty_result.master;
        let slave_fd = pty_result.slave;

        let master_raw: RawFd = master_fd.as_raw_fd();
        let slave_raw: RawFd = slave_fd.as_raw_fd();

        // Prepare CStrings for execvp before forking (allocation not safe after fork).
        let shell_cstr =
            CString::new(shell).map_err(|e| PtyError::Open(format!("invalid shell path: {e}")))?;
        let cwd_cstr = cwd
            .map(CString::new)
            .transpose()
            .map_err(|e| PtyError::Open(format!("invalid cwd path: {e}")))?;

        // Fork the process.
        // SAFETY: We are calling fork() which is inherently unsafe. After fork in the child,
        // we only call async-signal-safe functions (setsid, dup2, close, ioctl, chdir, execvp)
        // and libc functions that don't allocate.
        match unsafe { unistd::fork() } {
            Ok(ForkResult::Child) => {
                // === CHILD PROCESS ===

                // Become session leader.
                let _ = unistd::setsid();

                // Set the slave as the controlling terminal.
                // SAFETY: ioctl with TIOCSCTTY is async-signal-safe and we have a valid fd.
                unsafe { libc::ioctl(slave_raw, libc::TIOCSCTTY, 0) };

                // Redirect stdin/stdout/stderr to the slave fd.
                let _ = unistd::dup2(slave_raw, 0);
                let _ = unistd::dup2(slave_raw, 1);
                let _ = unistd::dup2(slave_raw, 2);

                // Close the original slave and master fds (they are no longer needed).
                if slave_raw > 2 {
                    // SAFETY: close is async-signal-safe.
                    unsafe {
                        let _ = libc::close(slave_raw);
                    }
                }
                // SAFETY: close is async-signal-safe.
                unsafe {
                    let _ = libc::close(master_raw);
                }

                // Change working directory if specified.
                if let Some(ref dir) = cwd_cstr {
                    // SAFETY: chdir is async-signal-safe.
                    let _ = unsafe { libc::chdir(dir.as_ptr()) };
                }

                // Set TERM environment variable.
                // SAFETY: setenv is not strictly async-signal-safe on all platforms,
                // but in practice this works and is commonly done in PTY implementations.
                unsafe {
                    let term_key = CString::new("TERM").unwrap_unchecked();
                    let term_val = CString::new("xterm-256color").unwrap_unchecked();
                    libc::setenv(term_key.as_ptr(), term_val.as_ptr(), 1);
                }

                // Execute the shell.
                let args: [&CString; 1] = [&shell_cstr];
                let _ = unistd::execvp(&shell_cstr, &args);

                // If execvp returns, it failed. Exit immediately.
                // SAFETY: _exit is async-signal-safe and always valid to call.
                unsafe { libc::_exit(1) }
            }
            Ok(ForkResult::Parent { child }) => {
                // === PARENT PROCESS ===

                // Close the slave fd in the parent (we only need the master).
                drop(slave_fd);

                let id = Uuid::new_v4().to_string();
                let dead = Arc::new(AtomicBool::new(false));

                // Dup the master fd for the async reader so we don't double-close.
                let reader_raw = nix::unistd::dup(master_fd.as_raw_fd())
                    .map_err(|e| PtyError::Open(format!("dup master fd failed: {e}")))?;

                let handle = PtyHandle {
                    id: id.clone(),
                    pid: child,
                    master_fd,
                    cols,
                    rows,
                    dead: dead.clone(),
                };

                self.ptys.insert(id.clone(), handle);

                // Spawn async reader task.
                let data_id = id.clone();
                let data_dead = dead.clone();
                tokio::spawn(async move {
                    // SAFETY: We own reader_raw (duped fd) and transfer ownership to File.
                    let mut file = unsafe { tokio::fs::File::from_raw_fd(reader_raw) };
                    let mut buf = vec![0u8; 4096];
                    loop {
                        if data_dead.load(Ordering::Relaxed) {
                            break;
                        }
                        match file.read(&mut buf).await {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                let data = buf[..n].to_vec();
                                if on_data.send((data_id.clone(), data)).is_err() {
                                    break; // receiver dropped
                                }
                            }
                            Err(e) => {
                                // EIO is expected when the child exits and the slave side closes.
                                if e.raw_os_error() == Some(libc::EIO) {
                                    break;
                                }
                                tracing::error!("pty read error: {e}");
                                break;
                            }
                        }
                    }
                });

                // Spawn async exit-watcher task.
                let exit_id = id.clone();
                let exit_pid = child;
                tokio::spawn(async move {
                    let status = tokio::task::spawn_blocking(move || waitpid(exit_pid, None)).await;

                    let exit_code = match status {
                        Ok(Ok(WaitStatus::Exited(_, code))) => code,
                        Ok(Ok(WaitStatus::Signaled(_, sig, _))) => 128 + sig as i32,
                        _ => -1,
                    };

                    let _ = on_exit.send((exit_id, exit_code));
                });

                Ok(id)
            }
            Err(e) => Err(PtyError::Open(format!("fork failed: {e}"))),
        }
    }

    /// Write bytes to a PTY's master fd.
    pub fn write(&self, id: &PtyId, data: &[u8]) -> Result<usize, PtyError> {
        let handle = self
            .ptys
            .get(id)
            .ok_or_else(|| PtyError::NotFound(id.clone()))?;
        if handle.dead.load(Ordering::Relaxed) {
            return Err(PtyError::NotFound(id.clone()));
        }
        unistd::write(&handle.master_fd, data).map_err(|e| PtyError::Io(e.into()))
    }

    /// Resize a PTY's terminal dimensions.
    ///
    /// If the new dimensions match the current ones, this is a no-op.
    pub fn resize(&mut self, id: &PtyId, cols: u16, rows: u16) -> Result<(), PtyError> {
        let handle = self
            .ptys
            .get_mut(id)
            .ok_or_else(|| PtyError::NotFound(id.clone()))?;
        if handle.dead.load(Ordering::Relaxed) {
            return Err(PtyError::NotFound(id.clone()));
        }

        // Dedup: skip if dimensions unchanged.
        if handle.cols == cols && handle.rows == rows {
            return Ok(());
        }

        let ws = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // SAFETY: TIOCSWINSZ ioctl with a valid Winsize struct on a valid fd.
        let ret = unsafe {
            libc::ioctl(
                handle.master_fd.as_raw_fd(),
                libc::TIOCSWINSZ,
                &ws as *const Winsize,
            )
        };
        if ret < 0 {
            return Err(PtyError::Io(std::io::Error::last_os_error()));
        }

        handle.cols = cols;
        handle.rows = rows;
        Ok(())
    }

    /// Kill a PTY process and remove it from the manager.
    ///
    /// Sends SIGTERM to the child, marks the handle as dead, and drops the master fd.
    pub fn kill(&mut self, id: &PtyId) -> Result<(), PtyError> {
        let handle = self
            .ptys
            .remove(id)
            .ok_or_else(|| PtyError::NotFound(id.clone()))?;
        handle.dead.store(true, Ordering::Relaxed);
        let _ = signal::kill(handle.pid, Signal::SIGTERM);
        // master_fd is dropped here, closing the fd.
        Ok(())
    }

    /// Kill all PTY processes managed by this manager.
    pub fn kill_all(&mut self) {
        let ids: Vec<PtyId> = self.ptys.keys().cloned().collect();
        for id in ids {
            let _ = self.kill(&id);
        }
    }

    /// Get a reference to a PtyHandle by id.
    pub fn get(&self, id: &PtyId) -> Option<&PtyHandle> {
        self.ptys.get(id)
    }

    /// Get the Pid of a PTY by id.
    pub fn get_pid(&self, id: &PtyId) -> Option<Pid> {
        self.ptys.get(id).map(|h| h.pid)
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use tokio::time::{Duration, timeout};

    fn default_shell() -> String {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }

    #[tokio::test]
    async fn test_spawn_and_read_output() {
        let mut mgr = PtyManager::new();
        let (data_tx, mut data_rx) = mpsc::unbounded_channel();
        let (exit_tx, _exit_rx) = mpsc::unbounded_channel();

        let id = mgr
            .spawn("/bin/echo", 80, 24, None, data_tx, exit_tx)
            .expect("spawn should succeed");

        // We should receive the output "hello\r\n" (PTY translates \n to \r\n).
        // Actually /bin/echo with no args just outputs a newline.
        // Let's read whatever comes back within 2 seconds.
        let result = timeout(Duration::from_secs(2), data_rx.recv()).await;
        assert!(result.is_ok(), "should receive data within 2 seconds");
        let (recv_id, data) = result.unwrap().expect("channel should not be closed");
        assert_eq!(recv_id, id);
        assert!(!data.is_empty(), "should receive non-empty data");

        // Clean up.
        let _ = mgr.kill(&id);
    }

    #[tokio::test]
    async fn test_write_to_pty() {
        let mut mgr = PtyManager::new();
        let (data_tx, mut data_rx) = mpsc::unbounded_channel();
        let (exit_tx, _exit_rx) = mpsc::unbounded_channel();

        let id = mgr
            .spawn("/bin/cat", 80, 24, None, data_tx, exit_tx)
            .expect("spawn should succeed");

        // Give the child process a moment to start.
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Write "test\n" to the PTY. cat should echo it back.
        mgr.write(&id, b"test\n").expect("write should succeed");

        // Collect data until we see "test" in the output.
        let mut collected = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            let remaining = deadline.duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match timeout(remaining, data_rx.recv()).await {
                Ok(Some((_id, data))) => {
                    collected.extend_from_slice(&data);
                    if collected.windows(4).any(|w| w == b"test") {
                        break;
                    }
                }
                _ => break,
            }
        }

        let output = String::from_utf8_lossy(&collected);
        assert!(
            output.contains("test"),
            "should see 'test' echoed back, got: {output:?}"
        );

        let _ = mgr.kill(&id);
    }

    #[tokio::test]
    async fn test_resize_dedup() {
        let mut mgr = PtyManager::new();
        let (data_tx, _data_rx) = mpsc::unbounded_channel();
        let (exit_tx, _exit_rx) = mpsc::unbounded_channel();

        let shell = default_shell();
        let id = mgr
            .spawn(&shell, 80, 24, None, data_tx, exit_tx)
            .expect("spawn should succeed");

        // Resize to same dimensions - should be a no-op (dedup).
        mgr.resize(&id, 80, 24)
            .expect("resize to same should succeed");

        // Verify dimensions unchanged.
        let handle = mgr.get(&id).unwrap();
        assert_eq!(handle.cols, 80);
        assert_eq!(handle.rows, 24);

        // Resize to different dimensions.
        mgr.resize(&id, 120, 40)
            .expect("resize to different should succeed");

        let handle = mgr.get(&id).unwrap();
        assert_eq!(handle.cols, 120);
        assert_eq!(handle.rows, 40);

        let _ = mgr.kill(&id);
    }

    #[tokio::test]
    async fn test_kill() {
        let mut mgr = PtyManager::new();
        let (data_tx, _data_rx) = mpsc::unbounded_channel();
        let (exit_tx, _exit_rx) = mpsc::unbounded_channel();

        let shell = default_shell();
        let id = mgr
            .spawn(&shell, 80, 24, None, data_tx, exit_tx)
            .expect("spawn should succeed");

        assert!(mgr.get(&id).is_some(), "should exist before kill");

        mgr.kill(&id).expect("kill should succeed");

        assert!(mgr.get(&id).is_none(), "should be removed after kill");

        // Killing again should return NotFound.
        let err = mgr.kill(&id);
        assert!(err.is_err(), "killing again should fail");
    }
}
