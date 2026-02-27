use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine;
use tokio::net::UnixStream;
use tokio::sync::{mpsc, Mutex};

use maxmux_core::command::CommandRegistry;
use maxmux_core::layout::{self, Rect, calculate_layout};
use maxmux_core::pty::{PtyId, PtyManager};
use maxmux_core::session::{
    LayoutNode, Pane, PaneId, SessionManager, SplitDirection, Window,
};
use maxmux_core::terminal::TerminalManager;
use maxmux_ipc::protocol::*;
use maxmux_ipc::transport::SplitConnection;

use super::broadcast::Broadcaster;

pub struct ServerState {
    pub sessions: SessionManager,
    pub ptys: PtyManager,
    pub terminals: TerminalManager,
    pub commands: CommandRegistry,
    pub broadcaster: Broadcaster,
    // Channels for PTY data/exit events
    pub pty_data_tx: mpsc::UnboundedSender<(PtyId, Vec<u8>)>,
    pub pty_data_rx: Option<mpsc::UnboundedReceiver<(PtyId, Vec<u8>)>>,
    pub pty_exit_tx: mpsc::UnboundedSender<(PtyId, i32)>,
    pub pty_exit_rx: Option<mpsc::UnboundedReceiver<(PtyId, i32)>>,
    // Client dimensions
    pub client_sizes: HashMap<String, (u16, u16)>,
    pub client_cwds: HashMap<String, String>,
    // PTY ID -> pane ID mapping (since PtyManager generates its own IDs)
    pub pty_to_pane: HashMap<PtyId, PaneId>,
    pub pane_to_pty: HashMap<PaneId, PtyId>,
    // Pane-to-session/window mapping
    pub pane_to_window: HashMap<PaneId, (String, String)>, // pane_id -> (session_id, window_id)
    // Output buffers (for replay on attach)
    pub output_buffers: HashMap<PaneId, Vec<u8>>,
}

impl ServerState {
    pub fn new() -> Self {
        let (pty_data_tx, pty_data_rx) = mpsc::unbounded_channel();
        let (pty_exit_tx, pty_exit_rx) = mpsc::unbounded_channel();
        Self {
            sessions: SessionManager::new(),
            ptys: PtyManager::new(),
            terminals: TerminalManager::new(),
            commands: CommandRegistry::new(),
            broadcaster: Broadcaster::new(),
            pty_data_tx,
            pty_data_rx: Some(pty_data_rx),
            pty_exit_tx,
            pty_exit_rx: Some(pty_exit_rx),
            client_sizes: HashMap::new(),
            client_cwds: HashMap::new(),
            pty_to_pane: HashMap::new(),
            pane_to_pty: HashMap::new(),
            pane_to_window: HashMap::new(),
            output_buffers: HashMap::new(),
        }
    }

    /// Spawn a new pane process and set up PTY + VirtualTerminal.
    ///
    /// Since PtyManager generates its own internal IDs, we maintain a mapping
    /// between pane IDs (used in the session model) and PTY IDs.
    pub fn spawn_pane(
        &mut self,
        pane_id: &str,
        shell: &str,
        cols: u16,
        rows: u16,
        cwd: Option<&str>,
    ) -> Result<(), String> {
        // 1. Create VirtualTerminal
        self.terminals.create(pane_id.to_string(), cols, rows, 10_000);

        // 2. Spawn PTY (PtyManager generates its own ID)
        let pty_id = self
            .ptys
            .spawn(
                shell,
                cols,
                rows,
                cwd,
                self.pty_data_tx.clone(),
                self.pty_exit_tx.clone(),
            )
            .map_err(|e| e.to_string())?;

        // 3. Map PTY ID <-> pane ID
        self.pty_to_pane
            .insert(pty_id.clone(), pane_id.to_string());
        self.pane_to_pty
            .insert(pane_id.to_string(), pty_id);

        // 4. Initialize output buffer
        self.output_buffers
            .insert(pane_id.to_string(), Vec::new());

        Ok(())
    }

    /// Write bytes to a pane's PTY by looking up the PTY ID mapping.
    pub fn write_to_pane(&self, pane_id: &str, data: &[u8]) -> Result<usize, String> {
        let pty_id = self
            .pane_to_pty
            .get(pane_id)
            .ok_or_else(|| format!("No PTY for pane {pane_id}"))?;
        self.ptys.write(pty_id, data).map_err(|e| e.to_string())
    }

    /// Resize a pane's PTY.
    pub fn resize_pane(&mut self, pane_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let pty_id = self
            .pane_to_pty
            .get(pane_id)
            .cloned()
            .ok_or_else(|| format!("No PTY for pane {pane_id}"))?;
        self.ptys
            .resize(&pty_id, cols, rows)
            .map_err(|e| e.to_string())
    }

    /// Kill a pane's PTY.
    pub fn kill_pane_pty(&mut self, pane_id: &str) -> Result<(), String> {
        if let Some(pty_id) = self.pane_to_pty.remove(pane_id) {
            self.pty_to_pane.remove(&pty_id);
            self.ptys.kill(&pty_id).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Get the process PID for a pane.
    pub fn get_pane_pid(&self, pane_id: &str) -> Option<u32> {
        let pty_id = self.pane_to_pty.get(pane_id)?;
        self.ptys.get_pid(pty_id).map(|pid| pid.as_raw() as u32)
    }

    /// Resolve a PTY ID to the corresponding pane ID.
    pub fn resolve_pty_to_pane(&self, pty_id: &str) -> Option<&str> {
        self.pty_to_pane.get(pty_id).map(|s| s.as_str())
    }

    /// Create a session with one window and one pane.
    pub fn create_default_session(
        &mut self,
        name: &str,
        cwd: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> String {
        let session = self.sessions.create_session(name);
        let session_id = session.id.clone();

        let pane_id = uuid::Uuid::new_v4().to_string();
        let window_id = uuid::Uuid::new_v4().to_string();

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let pane = Pane {
            id: pane_id.clone(),
            pid: None,
            cwd: cwd.unwrap_or("/").to_string(),
            command: shell.clone(),
            title: "bash".to_string(),
        };
        let window = Window {
            id: window_id.clone(),
            name: "shell".to_string(),
            panes: vec![pane],
            layout: LayoutNode::Leaf {
                pane_id: pane_id.clone(),
            },
            active_pane: pane_id.clone(),
        };
        self.sessions.add_window(&session_id, window);
        self.pane_to_window
            .insert(pane_id.clone(), (session_id.clone(), window_id.clone()));

        let _ = self.spawn_pane(&pane_id, &shell, cols, rows, cwd);

        // Update PID in the session model
        if let Some(pid) = self.get_pane_pid(&pane_id) {
            if let Some(session) = self.sessions.get_mut(&session_id) {
                if let Some(window) = session.windows.iter_mut().find(|w| w.id == window_id) {
                    if let Some(pane) = window.panes.iter_mut().find(|p| p.id == pane_id) {
                        pane.pid = Some(pid);
                    }
                }
            }
        }

        session_id
    }

    pub fn shutdown(&mut self) {
        self.ptys.kill_all();
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedState = Arc<Mutex<ServerState>>;

/// Handle a single client connection.
pub async fn handle_client(stream: UnixStream, state: SharedState) {
    let client_id = uuid::Uuid::new_v4().to_string();
    let split = SplitConnection::new(stream);
    let mut reader = split.reader;
    let writer = split.writer;

    // Create a channel for server -> client messages
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    {
        let mut s = state.lock().await;
        s.broadcaster.add_client(client_id.clone(), tx);
    }

    // Spawn writer task: forwards messages from channel to socket
    let writer_handle = tokio::spawn(async move {
        let mut writer = writer;
        while let Some(msg) = rx.recv().await {
            if writer.send_message(&msg).await.is_err() {
                break;
            }
        }
    });

    // Read messages from client
    loop {
        match reader.read_message::<ClientMessage>().await {
            Ok(msg) => {
                let mut s = state.lock().await;
                handle_message(&client_id, msg, &mut s);
            }
            Err(_) => break, // Client disconnected
        }
    }

    // Cleanup
    {
        let mut s = state.lock().await;
        // Get session ID before removing client from broadcaster
        let session_id = s
            .broadcaster
            .get_client_session(&client_id)
            .map(|s| s.to_string());
        s.broadcaster.remove_client(&client_id);
        s.client_sizes.remove(&client_id);
        s.client_cwds.remove(&client_id);
        // Detach from session
        if let Some(sid) = session_id {
            if let Some(session) = s.sessions.get_mut(&sid) {
                session.attached_clients.retain(|c| c != &client_id);
            }
        }
    }
    writer_handle.abort();
}

fn handle_message(client_id: &str, msg: ClientMessage, state: &mut ServerState) {
    match msg {
        ClientMessage::Attach { session_id, cwd } => {
            handle_attach(client_id, session_id, cwd, state);
        }
        ClientMessage::Detach => {
            // Detach client from session
            let session_id = state
                .broadcaster
                .get_client_session(client_id)
                .map(|s| s.to_string());
            if let Some(sid) = session_id {
                if let Some(session) = state.sessions.get_mut(&sid) {
                    session.attached_clients.retain(|c| c != client_id);
                }
            }
        }
        ClientMessage::Input { pane_id, data } => {
            // Base64 decode and write to PTY
            if let Ok(bytes) =
                base64::engine::general_purpose::STANDARD.decode(&data)
            {
                let _ = state.write_to_pane(&pane_id, &bytes);
            }
        }
        ClientMessage::Resize { cols, rows } => {
            handle_resize(client_id, cols, rows, state);
        }
        ClientMessage::Command { id, args: _ } => {
            let session_id = state
                .broadcaster
                .get_client_session(client_id)
                .map(|s| s.to_string());
            if let Some(session_id) = session_id {
                // Handle built-in commands directly
                match id.as_str() {
                    "window:create" => {
                        let (cols, rows) =
                            state.client_sizes.get(client_id).copied().unwrap_or((80, 24));
                        let cwd = state.client_cwds.get(client_id).cloned();
                        handle_window_create(
                            client_id,
                            &session_id,
                            cols,
                            rows,
                            cwd.as_deref(),
                            state,
                        );
                    }
                    "window:next" | "window:previous" => {
                        handle_window_switch(
                            client_id,
                            &session_id,
                            id == "window:next",
                            state,
                        );
                    }
                    "window:close" => {
                        handle_window_close(client_id, &session_id, state);
                    }
                    "pane:split-horizontal" | "pane:split-vertical" => {
                        let dir = if id == "pane:split-horizontal" {
                            SplitDirection::Horizontal
                        } else {
                            SplitDirection::Vertical
                        };
                        let (cols, rows) =
                            state.client_sizes.get(client_id).copied().unwrap_or((80, 24));
                        handle_split(client_id, &session_id, dir, cols, rows, state);
                    }
                    "pane:focus-up" | "pane:focus-down" | "pane:focus-left"
                    | "pane:focus-right" => {
                        let dir = match id.as_str() {
                            "pane:focus-up" => layout::Direction::Up,
                            "pane:focus-down" => layout::Direction::Down,
                            "pane:focus-left" => layout::Direction::Left,
                            _ => layout::Direction::Right,
                        };
                        handle_focus(client_id, &session_id, dir, state);
                    }
                    "pane:close" => {
                        handle_pane_close(client_id, &session_id, state);
                    }
                    "session:detach" => {
                        state.broadcaster.send(
                            client_id,
                            ServerMessage::Result {
                                success: true,
                                data: Some(serde_json::json!("detach")),
                                error: None,
                            },
                        );
                    }
                    "server:kill" => {
                        state.shutdown();
                        std::process::exit(0);
                    }
                    _ => {
                        tracing::warn!("Unknown command: {}", id);
                    }
                }
            }
        }
        _ => {}
    }
}

fn handle_attach(
    client_id: &str,
    session_id: Option<String>,
    cwd: Option<String>,
    state: &mut ServerState,
) {
    let (cols, rows) = state
        .client_sizes
        .get(client_id)
        .copied()
        .unwrap_or((80, 24));

    // Store CWD
    if let Some(ref c) = cwd {
        state.client_cwds.insert(client_id.to_string(), c.clone());
    }

    // Find or create session
    let sid = if let Some(ref id) = session_id {
        if state.sessions.get(id).is_some() {
            id.clone()
        } else if let Some(s) = state.sessions.find_by_name(id) {
            s.id.clone()
        } else {
            state.create_default_session(id, cwd.as_deref(), cols, rows)
        }
    } else {
        // Attach to first session or create default
        let first_id = state.sessions.all().first().map(|s| s.id.clone());
        if let Some(id) = first_id {
            id
        } else {
            state.create_default_session("main", cwd.as_deref(), cols, rows)
        }
    };

    // Record client in session
    state.broadcaster.set_client_session(client_id, &sid);
    if let Some(session) = state.sessions.get_mut(&sid) {
        if !session.attached_clients.contains(&client_id.to_string()) {
            session.attached_clients.push(client_id.to_string());
        }
    }

    // Send state
    send_state_to_client(client_id, &sid, state);

    // Send layout
    send_layout_to_client(client_id, &sid, cols, rows, state);

    // Replay output buffers for all panes in active window
    replay_output_buffers(client_id, &sid, state);
}

fn replay_output_buffers(client_id: &str, session_id: &str, state: &ServerState) {
    let pane_ids: Vec<String> = if let Some(session) = state.sessions.get(session_id) {
        if let Some(window) = session.windows.iter().find(|w| w.id == session.active_window) {
            window.panes.iter().map(|p| p.id.clone()).collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    for pane_id in pane_ids {
        if let Some(buf) = state.output_buffers.get(&pane_id) {
            if !buf.is_empty() {
                let data = base64::engine::general_purpose::STANDARD.encode(buf);
                state.broadcaster.send(
                    client_id,
                    ServerMessage::Output {
                        pane_id: pane_id.clone(),
                        data,
                    },
                );
            }
        }
    }
}

fn handle_resize(client_id: &str, cols: u16, rows: u16, state: &mut ServerState) {
    state
        .client_sizes
        .insert(client_id.to_string(), (cols, rows));

    let sid = match state
        .broadcaster
        .get_client_session(client_id)
        .map(|s| s.to_string())
    {
        Some(s) => s,
        None => return,
    };

    send_layout_to_client(client_id, &sid, cols, rows, state);

    // Resize all panes in active window
    let pane_rects = compute_active_window_rects(&sid, cols, rows, state);
    for (pane_id, rect) in &pane_rects {
        let _ = state.resize_pane(pane_id, rect.width, rect.height);
        if let Some(vt) = state.terminals.get_mut(pane_id) {
            vt.resize(rect.width, rect.height);
        }
    }
}

fn handle_window_create(
    client_id: &str,
    session_id: &str,
    cols: u16,
    rows: u16,
    cwd: Option<&str>,
    state: &mut ServerState,
) {
    let pane_id = uuid::Uuid::new_v4().to_string();
    let window_id = uuid::Uuid::new_v4().to_string();
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    let pane = Pane {
        id: pane_id.clone(),
        pid: None,
        cwd: cwd.unwrap_or("/").to_string(),
        command: shell.clone(),
        title: "shell".to_string(),
    };
    let window = Window {
        id: window_id.clone(),
        name: "shell".to_string(),
        panes: vec![pane],
        layout: LayoutNode::Leaf {
            pane_id: pane_id.clone(),
        },
        active_pane: pane_id.clone(),
    };

    state.sessions.add_window(session_id, window);
    state.pane_to_window.insert(
        pane_id.clone(),
        (session_id.to_string(), window_id.clone()),
    );

    // Switch to new window
    if let Some(session) = state.sessions.get_mut(session_id) {
        session.active_window = window_id;
    }

    let status_bar_rows = 1u16;
    let content_height = rows.saturating_sub(status_bar_rows);
    let _ = state.spawn_pane(&pane_id, &shell, cols, content_height, cwd);

    send_state_to_client(client_id, session_id, state);
    send_layout_to_client(client_id, session_id, cols, rows, state);
}

fn handle_split(
    client_id: &str,
    session_id: &str,
    direction: SplitDirection,
    cols: u16,
    rows: u16,
    state: &mut ServerState,
) {
    let new_pane_id = uuid::Uuid::new_v4().to_string();
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    // Get cwd for new pane from the target pane or client cwd
    let cwd_for_new_pane: String;
    let window_id: String;

    if let Some(session) = state.sessions.get_mut(session_id) {
        if let Some(window) = session
            .windows
            .iter_mut()
            .find(|w| w.id == session.active_window)
        {
            let target = window.active_pane.clone();
            window.layout =
                layout::split_layout(&window.layout, &target, &new_pane_id, direction);

            cwd_for_new_pane = window
                .panes
                .iter()
                .find(|p| p.id == target)
                .map(|p| p.cwd.clone())
                .unwrap_or_else(|| "/".to_string());

            let pane = Pane {
                id: new_pane_id.clone(),
                pid: None,
                cwd: cwd_for_new_pane.clone(),
                command: shell.clone(),
                title: "shell".to_string(),
            };
            window.panes.push(pane);
            window.active_pane = new_pane_id.clone();
            window_id = window.id.clone();
        } else {
            return;
        }
    } else {
        return;
    }

    state.pane_to_window.insert(
        new_pane_id.clone(),
        (session_id.to_string(), window_id),
    );

    // Calculate layout and spawn pane at correct size
    let status_bar_rows = 1u16;
    let content_height = rows.saturating_sub(status_bar_rows);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: cols,
        height: content_height,
    };

    if let Some(session) = state.sessions.get(session_id) {
        if let Some(window) = session
            .windows
            .iter()
            .find(|w| w.id == session.active_window)
        {
            let rects = calculate_layout(&window.layout, bounds);
            if let Some(rect) = rects.get(&new_pane_id) {
                let spawn_cwd = state
                    .client_cwds
                    .get(client_id)
                    .cloned()
                    .unwrap_or_else(|| cwd_for_new_pane.clone());
                let _ = state.spawn_pane(
                    &new_pane_id,
                    &shell,
                    rect.width,
                    rect.height,
                    Some(&spawn_cwd),
                );
            }
            // Resize existing panes to their new dimensions
            for (pid, rect) in &rects {
                if pid != &new_pane_id {
                    let _ = state.resize_pane(pid, rect.width, rect.height);
                    if let Some(vt) = state.terminals.get_mut(pid) {
                        vt.resize(rect.width, rect.height);
                    }
                }
            }
        }
    }

    send_state_to_client(client_id, session_id, state);
    send_layout_to_client(client_id, session_id, cols, rows, state);
}

fn handle_focus(
    client_id: &str,
    session_id: &str,
    dir: layout::Direction,
    state: &mut ServerState,
) {
    let (cols, rows) = state
        .client_sizes
        .get(client_id)
        .copied()
        .unwrap_or((80, 24));

    if let Some(session) = state.sessions.get_mut(session_id) {
        if let Some(window) = session
            .windows
            .iter_mut()
            .find(|w| w.id == session.active_window)
        {
            let status_bar_rows = 1u16;
            let content_height = rows.saturating_sub(status_bar_rows);
            let bounds = Rect {
                x: 0,
                y: 0,
                width: cols,
                height: content_height,
            };
            let rects = calculate_layout(&window.layout, bounds);

            if let Some(new_pane) =
                layout::find_pane_in_direction(&rects, &window.active_pane, dir, None)
            {
                window.active_pane = new_pane;
            }
        }
    }

    send_state_to_client(client_id, session_id, state);
    send_layout_to_client(client_id, session_id, cols, rows, state);
}

fn handle_window_switch(
    client_id: &str,
    session_id: &str,
    next: bool,
    state: &mut ServerState,
) {
    let (cols, rows) = state
        .client_sizes
        .get(client_id)
        .copied()
        .unwrap_or((80, 24));

    if let Some(session) = state.sessions.get_mut(session_id) {
        if session.windows.len() <= 1 {
            return;
        }
        let idx = session
            .windows
            .iter()
            .position(|w| w.id == session.active_window)
            .unwrap_or(0);
        let new_idx = if next {
            (idx + 1) % session.windows.len()
        } else if idx == 0 {
            session.windows.len() - 1
        } else {
            idx - 1
        };
        session.active_window = session.windows[new_idx].id.clone();
    }

    send_state_to_client(client_id, session_id, state);
    send_layout_to_client(client_id, session_id, cols, rows, state);
}

fn handle_window_close(client_id: &str, session_id: &str, state: &mut ServerState) {
    let (cols, rows) = state
        .client_sizes
        .get(client_id)
        .copied()
        .unwrap_or((80, 24));

    // Collect pane IDs and window ID from active window
    let close_info: Option<(Vec<String>, String)> =
        if let Some(session) = state.sessions.get(session_id) {
            session
                .windows
                .iter()
                .find(|w| w.id == session.active_window)
                .map(|window| {
                    let pane_ids: Vec<String> = window.panes.iter().map(|p| p.id.clone()).collect();
                    (pane_ids, window.id.clone())
                })
        } else {
            None
        };

    if let Some((pane_ids, window_id)) = close_info {
        for pid in &pane_ids {
            let _ = state.kill_pane_pty(pid);
            state.terminals.remove(pid);
            state.output_buffers.remove(pid);
            state.pane_to_window.remove(pid);
        }

        state.sessions.remove_window(session_id, &window_id);
    }

    // Switch to remaining window or handle empty session
    if let Some(session) = state.sessions.get(session_id) {
        if session.windows.is_empty() {
            // Last window closed - detach client
            state.broadcaster.send(
                client_id,
                ServerMessage::Result {
                    success: true,
                    data: Some(serde_json::json!("detach")),
                    error: None,
                },
            );
            return;
        }
    }

    send_state_to_client(client_id, session_id, state);
    send_layout_to_client(client_id, session_id, cols, rows, state);
}

fn handle_pane_close(client_id: &str, session_id: &str, state: &mut ServerState) {
    let (cols, rows) = state
        .client_sizes
        .get(client_id)
        .copied()
        .unwrap_or((80, 24));

    // Collect needed information before mutating
    let close_info: Option<(String, String)> =
        if let Some(session) = state.sessions.get(session_id) {
            session
                .windows
                .iter()
                .find(|w| w.id == session.active_window)
                .map(|window| (window.active_pane.clone(), window.id.clone()))
        } else {
            None
        };

    if let Some((pane_id, window_id)) = close_info {
        // Kill PTY
        let _ = state.kill_pane_pty(&pane_id);
        state.terminals.remove(&pane_id);
        state.output_buffers.remove(&pane_id);
        state.pane_to_window.remove(&pane_id);

        // Remove from layout and pane list
        if let Some(session) = state.sessions.get_mut(session_id) {
            if let Some(window) = session.windows.iter_mut().find(|w| w.id == window_id) {
                window.panes.retain(|p| p.id != pane_id);
                if let Some(new_layout) = layout::remove_from_layout(&window.layout, &pane_id) {
                    window.layout = new_layout;
                    // Set new active pane
                    let remaining = layout::get_all_pane_ids(&window.layout);
                    if let Some(first) = remaining.first() {
                        window.active_pane = first.clone();
                    }
                }

                if window.panes.is_empty() {
                    let wid = window.id.clone();
                    state.sessions.remove_window(session_id, &wid);
                }
            }
        }
    }

    // Check if session still has windows
    if let Some(session) = state.sessions.get(session_id) {
        if session.windows.is_empty() {
            state.broadcaster.send(
                client_id,
                ServerMessage::Result {
                    success: true,
                    data: Some(serde_json::json!("detach")),
                    error: None,
                },
            );
            return;
        }
    }

    send_state_to_client(client_id, session_id, state);
    send_layout_to_client(client_id, session_id, cols, rows, state);
}

// ---- Helper functions ----

/// Compute the layout rects for the active window's panes.
fn compute_active_window_rects(
    session_id: &str,
    cols: u16,
    rows: u16,
    state: &ServerState,
) -> HashMap<String, Rect> {
    let status_bar_rows = 1u16;
    let content_height = rows.saturating_sub(status_bar_rows);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: cols,
        height: content_height,
    };

    if let Some(session) = state.sessions.get(session_id) {
        if let Some(window) = session
            .windows
            .iter()
            .find(|w| w.id == session.active_window)
        {
            return calculate_layout(&window.layout, bounds);
        }
    }
    HashMap::new()
}

/// Send full state to a client.
fn send_state_to_client(client_id: &str, session_id: &str, state: &ServerState) {
    let sessions: Vec<SessionState> = state
        .sessions
        .all()
        .iter()
        .map(|s| SessionState {
            id: s.id.clone(),
            name: s.name.clone(),
            windows: s
                .windows
                .iter()
                .map(|w| WindowState {
                    id: w.id.clone(),
                    name: w.name.clone(),
                    pane_count: w.panes.len(),
                    active_pane: w.active_pane.clone(),
                })
                .collect(),
            active_window: s.active_window.clone(),
            attached_clients: s.attached_clients.clone(),
        })
        .collect();

    state.broadcaster.send(
        client_id,
        ServerMessage::State {
            sessions,
            active_session: Some(session_id.to_string()),
        },
    );
}

/// Send layout + pane rects to client.
fn send_layout_to_client(
    client_id: &str,
    session_id: &str,
    cols: u16,
    rows: u16,
    state: &ServerState,
) {
    if let Some(session) = state.sessions.get(session_id) {
        if let Some(window) = session
            .windows
            .iter()
            .find(|w| w.id == session.active_window)
        {
            let status_bar_rows = 1u16;
            let content_height = rows.saturating_sub(status_bar_rows);
            let bounds = Rect {
                x: 0,
                y: 0,
                width: cols,
                height: content_height,
            };
            let rects = calculate_layout(&window.layout, bounds);

            let pane_rects: HashMap<String, RectData> = rects
                .iter()
                .map(|(id, r)| {
                    (
                        id.clone(),
                        RectData {
                            x: r.x,
                            y: r.y,
                            width: r.width,
                            height: r.height,
                        },
                    )
                })
                .collect();

            state.broadcaster.send(
                client_id,
                ServerMessage::Layout {
                    layout: serde_json::to_value(&window.layout).unwrap_or_default(),
                    pane_rects,
                },
            );
        }
    }
}
