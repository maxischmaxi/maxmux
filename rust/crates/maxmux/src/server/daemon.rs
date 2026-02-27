use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use tokio::net::UnixListener;
use tokio::sync::Mutex;

use maxmux_ipc::protocol::ServerMessage;

pub fn socket_path() -> PathBuf {
    let dir = dirs::home_dir()
        .expect("Could not determine home directory")
        .join(".maxmux");
    std::fs::create_dir_all(&dir).ok();
    dir.join("server.sock")
}

pub fn pid_path() -> PathBuf {
    dirs::home_dir()
        .expect("Could not determine home directory")
        .join(".maxmux")
        .join("server.pid")
}

pub async fn is_running() -> bool {
    tokio::net::UnixStream::connect(socket_path()).await.is_ok()
}

pub async fn ensure_running() {
    if !is_running().await {
        std::process::Command::new(std::env::current_exe().unwrap())
            .arg("server")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("Failed to start server");

        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if is_running().await {
                return;
            }
        }
        eprintln!("Server failed to start");
        std::process::exit(1);
    }
}

pub async fn run() {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    std::fs::write(pid_path(), std::process::id().to_string()).ok();

    let listener = UnixListener::bind(&path).expect("Failed to bind socket");
    tracing::info!("Server listening on {:?}", path);

    let state = super::handler::ServerState::new();
    let shared = Arc::new(Mutex::new(state));

    // Take the PTY data/exit receivers out of the state and spawn forwarding tasks
    let mut pty_data_rx;
    let mut pty_exit_rx;
    {
        let mut s = shared.lock().await;
        pty_data_rx = s.pty_data_rx.take().unwrap();
        pty_exit_rx = s.pty_exit_rx.take().unwrap();
    }

    // Spawn PTY data forwarding task
    let data_state = shared.clone();
    tokio::spawn(async move {
        while let Some((pty_id, data)) = pty_data_rx.recv().await {
            let mut s = data_state.lock().await;

            // Resolve PTY ID to pane ID
            let pane_id = match s.resolve_pty_to_pane(&pty_id) {
                Some(id) => id.to_string(),
                None => continue,
            };

            // Write to VirtualTerminal
            if let Some(vt) = s.terminals.get_mut(&pane_id) {
                vt.write(&data);
            }

            // Store in output buffer (cap at 512KB)
            if let Some(buf) = s.output_buffers.get_mut(&pane_id) {
                buf.extend_from_slice(&data);
                if buf.len() > 512 * 1024 {
                    let drain = buf.len() - 512 * 1024;
                    buf.drain(..drain);
                }
            }

            // Forward to clients in the pane's session
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            if let Some((session_id, _)) = s.pane_to_window.get(&pane_id) {
                let sid = session_id.clone();
                s.broadcaster.send_to_session(
                    &sid,
                    ServerMessage::Output {
                        pane_id: pane_id.clone(),
                        data: encoded,
                    },
                );
            }
        }
    });

    // Spawn PTY exit forwarding task
    let exit_state = shared.clone();
    tokio::spawn(async move {
        while let Some((pty_id, exit_code)) = pty_exit_rx.recv().await {
            let mut s = exit_state.lock().await;

            // Resolve PTY ID to pane ID
            let pane_id = match s.resolve_pty_to_pane(&pty_id) {
                Some(id) => id.to_string(),
                None => continue,
            };

            if let Some((session_id, _)) = s.pane_to_window.get(&pane_id) {
                let sid = session_id.clone();
                s.broadcaster.send_to_session(
                    &sid,
                    ServerMessage::PaneExited {
                        pane_id: pane_id.clone(),
                        exit_code,
                    },
                );
            }

            // Cleanup
            s.terminals.remove(&pane_id);
            s.output_buffers.remove(&pane_id);
            // Clean up PTY ID mappings
            if let Some(pty_id) = s.pane_to_pty.remove(&pane_id) {
                s.pty_to_pane.remove(&pty_id);
            }
        }
    });

    // Set up signal handlers
    let mut sigterm =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let state = shared.clone();
                        tokio::spawn(async move {
                            super::handler::handle_client(stream, state).await;
                        });
                    }
                    Err(e) => {
                        tracing::error!("Accept error: {}", e);
                    }
                }
            }
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM");
                break;
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received SIGINT");
                break;
            }
        }
    }

    // Cleanup
    let mut state = shared.lock().await;
    state.shutdown();
    drop(state);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(pid_path());
}
