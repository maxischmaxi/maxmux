use crossterm::terminal::{disable_raw_mode, enable_raw_mode, size as terminal_size};
use tokio::net::UnixStream;

use base64::Engine;
use maxmux_core::layout::Rect;
use maxmux_core::terminal::{TerminalManager, VirtualTerminal};
use maxmux_input::router::{InputAction, InputRouter};
use maxmux_ipc::protocol::{ClientMessage, ServerMessage};
use maxmux_ipc::transport::SplitConnection;
use maxmux_renderer::border::BorderStyle;
use maxmux_renderer::compositor::{BorderConfig, Compositor};
use std::collections::HashMap;
use std::io::Write;

pub async fn run(session_name: Option<String>) {
    let path = crate::server::daemon::socket_path();
    let stream = match UnixStream::connect(&path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect to server: {}", e);
            std::process::exit(1);
        }
    };

    let split = SplitConnection::new(stream);
    let mut reader = split.reader;
    let mut writer = split.writer;

    // Terminal setup
    enable_raw_mode().unwrap();
    let mut stdout = std::io::stdout();
    // Enter alt screen, hide cursor, enable mouse, enable bracketed paste
    write!(
        stdout,
        "{}{}{}{}",
        maxmux_renderer::ansi::enter_alt_screen(),
        maxmux_renderer::ansi::hide_cursor(),
        maxmux_renderer::ansi::enable_mouse(),
        maxmux_renderer::ansi::enable_bracketed_paste(),
    )
    .unwrap();
    stdout.flush().unwrap();

    let (cols, rows) = terminal_size().unwrap();

    // Send attach
    writer
        .send_message(&ClientMessage::Attach {
            session_id: session_name,
            cwd: std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().into_owned()),
        })
        .await
        .ok();

    // Send initial resize
    writer
        .send_message(&ClientMessage::Resize { cols, rows })
        .await
        .ok();

    // Client state
    let mut compositor = Compositor::new(cols, rows);
    let mut input_router = InputRouter::new("C-a", 0);
    let mut terminals = TerminalManager::new();
    let mut pane_rects: HashMap<String, Rect> = HashMap::new();
    let mut active_pane = String::new();
    let mut active_session = String::new();
    let mut should_quit = false;

    // Load default keybindings
    load_default_keybindings(&mut input_router);

    // Stdin reader: spawn a task that reads raw bytes from stdin
    let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    std::thread::spawn(move || {
        use std::io::Read;
        let stdin = std::io::stdin();
        let mut handle = stdin.lock();
        let mut buf = [0u8; 4096];
        loop {
            match handle.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = stdin_tx.send(buf[..n].to_vec());
                }
                Err(_) => break,
            }
        }
    });

    // SIGWINCH handler for terminal resize
    let mut sigwinch =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::window_change()).unwrap();

    // Main event loop
    while !should_quit {
        tokio::select! {
            // Handle stdin input
            Some(data) = stdin_rx.recv() => {
                let actions = input_router.handle_input(&data, None);
                for action in actions {
                    match action {
                        InputAction::Passthrough(bytes) => {
                            if !active_pane.is_empty() {
                                let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                                writer.send_message(&ClientMessage::Input {
                                    pane_id: active_pane.clone(),
                                    data: encoded,
                                }).await.ok();
                            }
                        }
                        InputAction::Command(cmd) => {
                            if cmd == "session:detach" {
                                should_quit = true;
                                break;
                            }
                            writer.send_message(&ClientMessage::Command {
                                id: cmd,
                                args: HashMap::new(),
                            }).await.ok();
                        }
                        InputAction::PrefixActivated => {
                            // Could show prefix indicator
                        }
                        InputAction::Mouse(event) => {
                            // Find which pane was clicked
                            let clicked_pane = pane_rects.iter()
                                .find(|(_, rect)| {
                                    event.x >= rect.x && event.x < rect.x + rect.width
                                    && event.y >= rect.y && event.y < rect.y + rect.height
                                })
                                .map(|(id, _)| id.clone());

                            if let Some(pane_id) = clicked_pane {
                                // If clicking a different pane, focus it
                                if pane_id != active_pane && !event.is_release {
                                    writer.send_message(&ClientMessage::Command {
                                        id: "pane:focus".to_string(),
                                        args: [("pane_id".to_string(), pane_id.clone())].into(),
                                    }).await.ok();
                                }

                                // Forward mouse to PTY if it's tracking mouse
                                if let Some(vt) = terminals.get(&pane_id) {
                                    if vt.is_mouse_tracking_active() {
                                        if let Some(rect) = pane_rects.get(&pane_id) {
                                            let local_x = event.x.saturating_sub(rect.x);
                                            let local_y = event.y.saturating_sub(rect.y);
                                            let encoded_mouse = maxmux_input::mouse::encode_sgr_mouse(
                                                event.button, local_x, local_y, event.is_release,
                                            );
                                            let encoded = base64::engine::general_purpose::STANDARD.encode(encoded_mouse.as_bytes());
                                            writer.send_message(&ClientMessage::Input {
                                                pane_id: pane_id.clone(),
                                                data: encoded,
                                            }).await.ok();
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Handle server messages
            msg = reader.read_message::<ServerMessage>() => {
                match msg {
                    Ok(msg) => {
                        match msg {
                            ServerMessage::Output { pane_id, data } => {
                                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&data) {
                                    if let Some(vt) = terminals.get_mut(&pane_id) {
                                        vt.write(&bytes);
                                    }
                                }
                                render_screen(&mut compositor, &terminals, &pane_rects, &active_pane, &mut stdout);
                            }
                            ServerMessage::State { sessions, active_session: active_sid } => {
                                if let Some(sid) = active_sid {
                                    active_session = sid;
                                }
                                // Update active pane from state
                                if let Some(session) = sessions.iter().find(|s| s.id == active_session) {
                                    if let Some(window) = session.windows.iter().find(|w| w.id == session.active_window) {
                                        active_pane = window.active_pane.clone();
                                    }
                                }
                            }
                            ServerMessage::Layout { pane_rects: rects_data, .. } => {
                                // Update pane rects
                                pane_rects.clear();
                                for (id, rd) in &rects_data {
                                    pane_rects.insert(id.clone(), Rect {
                                        x: rd.x, y: rd.y, width: rd.width, height: rd.height,
                                    });
                                }

                                // Ensure VTs exist for all panes, remove old ones
                                let pane_ids: std::collections::HashSet<String> = rects_data.keys().cloned().collect();

                                // Remove VTs not in layout
                                let existing: Vec<String> = terminals.get_all_ids();
                                for id in existing {
                                    if !pane_ids.contains(&id) {
                                        terminals.remove(&id);
                                    }
                                }

                                // Create VTs for new panes
                                for (id, rd) in &rects_data {
                                    if terminals.get(id).is_none() {
                                        terminals.create(id.clone(), rd.width, rd.height, 10_000);
                                    } else if let Some(vt) = terminals.get_mut(id) {
                                        if vt.cols() != rd.width || vt.rows() != rd.height {
                                            vt.resize(rd.width, rd.height);
                                        }
                                    }
                                }

                                render_screen(&mut compositor, &terminals, &pane_rects, &active_pane, &mut stdout);
                            }
                            ServerMessage::PaneExited { pane_id, .. } => {
                                terminals.remove(&pane_id);
                                pane_rects.remove(&pane_id);
                            }
                            ServerMessage::CursorState { .. } => {}
                            ServerMessage::Result { data, .. } => {
                                if let Some(d) = data {
                                    if d == serde_json::json!("detach") {
                                        should_quit = true;
                                    }
                                }
                            }
                            ServerMessage::Error { message } => {
                                tracing::error!("Server error: {}", message);
                            }
                            _ => {}
                        }
                    }
                    Err(_) => {
                        // Server disconnected
                        should_quit = true;
                    }
                }
            }

            // Handle terminal resize
            _ = sigwinch.recv() => {
                if let Ok((new_cols, new_rows)) = terminal_size() {
                    compositor.resize(new_cols, new_rows);
                    writer.send_message(&ClientMessage::Resize {
                        cols: new_cols, rows: new_rows,
                    }).await.ok();
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode().ok();
    write!(
        stdout,
        "{}{}{}{}",
        maxmux_renderer::ansi::disable_mouse(),
        maxmux_renderer::ansi::disable_bracketed_paste(),
        maxmux_renderer::ansi::show_cursor(),
        maxmux_renderer::ansi::exit_alt_screen(),
    )
    .ok();
    stdout.flush().ok();
}

fn render_screen(
    compositor: &mut Compositor,
    terminals: &TerminalManager,
    pane_rects: &HashMap<String, Rect>,
    active_pane: &str,
    stdout: &mut std::io::Stdout,
) {
    // Build terminal references map
    let term_refs: HashMap<String, &VirtualTerminal> = pane_rects
        .keys()
        .filter_map(|id| terminals.get(id).map(|vt| (id.clone(), vt)))
        .collect();

    let border_config = BorderConfig {
        style: BorderStyle::Rounded,
        fg: (88, 91, 112),          // #585b70 (catppuccin surface2)
        active_fg: (203, 166, 247), // #cba6f7 (catppuccin mauve)
    };

    let (output, _cursor) = compositor.compose(
        &term_refs,
        pane_rects,
        active_pane,
        None, // no status bar yet
        &border_config,
        None, // no zoom
    );

    write!(stdout, "{}", output).ok();
    stdout.flush().ok();
}

fn load_default_keybindings(router: &mut InputRouter) {
    let prefix = router.prefix_bindings_mut();
    prefix.bind("c".into(), "window:create".into(), vec![]);
    prefix.bind("n".into(), "window:next".into(), vec![]);
    prefix.bind("p".into(), "window:previous".into(), vec![]);
    prefix.bind("&".into(), "window:close".into(), vec![]);
    prefix.bind("%".into(), "pane:split-horizontal".into(), vec![]);
    prefix.bind("\"".into(), "pane:split-vertical".into(), vec![]);
    prefix.bind("Up".into(), "pane:focus-up".into(), vec![]);
    prefix.bind("Down".into(), "pane:focus-down".into(), vec![]);
    prefix.bind("Left".into(), "pane:focus-left".into(), vec![]);
    prefix.bind("Right".into(), "pane:focus-right".into(), vec![]);
    prefix.bind("x".into(), "pane:close".into(), vec![]);
    prefix.bind("d".into(), "session:detach".into(), vec![]);

    let global = router.global_bindings_mut();
    global.bind(
        "C-h".into(),
        "pane:focus-left".into(),
        vec!["vim".into(), "nvim".into()],
    );
    global.bind(
        "C-j".into(),
        "pane:focus-down".into(),
        vec!["vim".into(), "nvim".into()],
    );
    global.bind(
        "C-k".into(),
        "pane:focus-up".into(),
        vec!["vim".into(), "nvim".into()],
    );
    global.bind(
        "C-l".into(),
        "pane:focus-right".into(),
        vec!["vim".into(), "nvim".into()],
    );
}
