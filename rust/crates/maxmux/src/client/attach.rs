use crossterm::terminal::{disable_raw_mode, enable_raw_mode, size as terminal_size};
use tokio::net::UnixStream;

use base64::Engine;
use maxmux_config::schema::KeybindingValue;
use maxmux_core::layout::Rect;
use maxmux_core::terminal::{TerminalManager, VirtualTerminal};
use maxmux_input::keys;
use maxmux_input::router::{InputAction, InputRouter};
use maxmux_ipc::protocol::{ClientMessage, MetricsData, NoteData, ServerMessage, SessionState};
use maxmux_ipc::transport::SplitConnection;
use maxmux_renderer::border::BorderStyle;
use maxmux_renderer::compositor::{BorderConfig, Compositor};
use maxmux_statusbar::modules::{
    BatteryInfo, CpuInfo, MemoryInfo, ModuleContext, SessionInfo, StatusBarModule, SystemMetrics,
    WindowInfo, build_module_registry,
};
use maxmux_statusbar::types::ColorPair;
use std::collections::HashMap;
use std::io::Write;

use super::command_palette::{CommandEntry, CommandPalette, CommandPaletteAction};
use super::copy_mode::{CopyModeAction, CopyModeRenderer, CopyModeState};
use super::note_editor::{NoteEditor, NoteEditorAction};
use super::notes_list::{NotesList, NotesListAction, NotesListEntry};
use super::prefix_help::{PrefixHelp, PrefixHelpAction};
use super::rename_dialog::{RenameAction, RenameDialog};
use super::session_finder::{SessionFinder, SessionFinderAction, SessionFinderEntry};
use super::session_sidebar::{
    SessionSidebar, SidebarAction, SidebarPosition, SidebarSession, SidebarWindow,
};

// ---------------------------------------------------------------------------
// Overlay state
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum OverlayState {
    None,
    CommandPalette,
    PrefixHelp,
    CopyMode,
    SessionSidebar,
    SessionFinder,
    RenameSession,
    RenameWindow,
    NoteEditor,
    NotesList,
}

// ---------------------------------------------------------------------------
// Main client entry point
// ---------------------------------------------------------------------------

pub async fn run(session_name: Option<String>) {
    let path = crate::server::daemon::socket_path();
    let stream = match UnixStream::connect(&path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect to server: {}", e);
            std::process::exit(1);
        }
    };

    // ── Load config ─────────────────────────────────────────────────────
    let config = match maxmux_config::load_config() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Config load failed ({}), using defaults", e);
            maxmux_config::MaxmuxConfig::default()
        }
    };

    let split = SplitConnection::new(stream);
    let mut reader = split.reader;
    let mut writer = split.writer;

    // ── Terminal setup ──────────────────────────────────────────────────
    enable_raw_mode().unwrap();
    let mut stdout = std::io::stdout();
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

    // ── Send attach + resize ────────────────────────────────────────────
    writer
        .send_message(&ClientMessage::Attach {
            session_id: session_name,
            cwd: std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().into_owned()),
        })
        .await
        .ok();
    writer
        .send_message(&ClientMessage::Resize { cols, rows })
        .await
        .ok();

    // ── Client state ────────────────────────────────────────────────────
    let mut compositor = Compositor::new(cols, rows);
    let mut input_router = InputRouter::new(&config.prefix_key, config.prefix_timeout);
    let mut terminals = TerminalManager::new();
    let mut pane_rects: HashMap<String, Rect> = HashMap::new();
    let mut active_pane = String::new();
    let mut active_session = String::new();
    let mut should_quit = false;

    // Load keybindings from config
    load_keybindings_from_config(&config, &mut input_router);

    // ── Overlay state ───────────────────────────────────────────────────
    let mut overlay = OverlayState::None;
    let mut command_palette: Option<CommandPalette> = None;
    let mut prefix_help: Option<PrefixHelp> = None;
    let mut copy_mode: Option<CopyModeState> = None;
    let mut session_sidebar: Option<SessionSidebar> = None;
    let mut session_finder: Option<SessionFinder> = None;
    let mut rename_dialog: Option<RenameDialog> = None;
    let mut note_editor: Option<NoteEditor> = None;
    let mut notes_list: Option<NotesList> = None;

    // ── Cached state for overlays + status bar ──────────────────────────
    let mut sessions_cache: Vec<SessionState> = Vec::new();
    let mut notes_cache: Vec<NoteData> = Vec::new();
    let module_registry = build_module_registry();
    let mut system_metrics = SystemMetrics::default();
    let mut prefix_active = false;

    // ── Config watcher ──────────────────────────────────────────────────
    let mut config_rx: Option<tokio::sync::mpsc::UnboundedReceiver<()>> = None;
    let _config_watcher: Option<maxmux_config::ConfigWatcher> =
        if let Some(config_path) = maxmux_config::find_config_file() {
            match maxmux_config::ConfigWatcher::new(config_path) {
                Ok((watcher, rx)) => {
                    config_rx = Some(rx);
                    Some(watcher)
                }
                Err(_) => None,
            }
        } else {
            None
        };

    // ── Stdin reader ────────────────────────────────────────────────────
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

    // ── SIGWINCH ────────────────────────────────────────────────────────
    let mut sigwinch =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::window_change()).unwrap();

    // ════════════════════════════════════════════════════════════════════
    // Main event loop
    // ════════════════════════════════════════════════════════════════════
    while !should_quit {
        tokio::select! {
            // ── stdin ───────────────────────────────────────────────
            Some(data) = stdin_rx.recv() => {
                // If overlay is active, route input to overlay
                if !matches!(overlay, OverlayState::None) {
                    let (key, _) = keys::parse_key(&data);
                    let key_str = keys::key_name(&key);
                    handle_overlay_key(
                        &key_str, &mut overlay,
                        &mut command_palette, &mut prefix_help, &mut copy_mode,
                        &mut session_sidebar, &mut session_finder,
                        &mut rename_dialog, &mut note_editor, &mut notes_list,
                        &mut writer, &mut should_quit,
                        &active_pane, &active_session,
                        &terminals, &pane_rects, &notes_cache,
                        (compositor.cols(), compositor.rows()),
                    ).await;
                    do_render(
                        &mut compositor, &terminals, &pane_rects, &active_pane,
                        &mut stdout, &module_registry, &sessions_cache,
                        &active_session, &system_metrics, prefix_active, &config,
                        &overlay, &command_palette, &prefix_help, &copy_mode,
                        &session_sidebar, &session_finder, &rename_dialog,
                        &note_editor, &notes_list,
                    );
                    continue;
                }

                // Normal input routing
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
                            prefix_active = false;
                            handle_command(
                                &cmd, &mut overlay,
                                &mut command_palette, &mut prefix_help, &mut copy_mode,
                                &mut session_sidebar, &mut session_finder,
                                &mut rename_dialog, &mut note_editor,
                                &mut writer, &mut should_quit,
                                &active_pane, &active_session,
                                &terminals, &sessions_cache, &input_router,
                            ).await;
                        }
                        InputAction::PrefixActivated => {
                            prefix_active = true;
                        }
                        InputAction::PrefixTimeout => {
                            prefix_active = false;
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
                                if pane_id != active_pane && !event.is_release {
                                    writer.send_message(&ClientMessage::Command {
                                        id: "pane:focus".to_string(),
                                        args: [("pane_id".to_string(), pane_id.clone())].into(),
                                    }).await.ok();
                                }
                                if let Some(vt) = terminals.get(&pane_id)
                                    && vt.is_mouse_tracking_active()
                                    && let Some(rect) = pane_rects.get(&pane_id)
                                {
                                    let local_x = event.x.saturating_sub(rect.x);
                                    let local_y = event.y.saturating_sub(rect.y);
                                    let encoded_mouse = maxmux_input::mouse::encode_sgr_mouse(
                                        event.button, local_x, local_y, event.is_release,
                                    );
                                    let encoded = base64::engine::general_purpose::STANDARD
                                        .encode(encoded_mouse.as_bytes());
                                    writer.send_message(&ClientMessage::Input {
                                        pane_id: pane_id.clone(),
                                        data: encoded,
                                    }).await.ok();
                                }
                            }
                        }
                    }
                }
                // Re-render after input processing (status bar may have changed)
                do_render(
                    &mut compositor, &terminals, &pane_rects, &active_pane,
                    &mut stdout, &module_registry, &sessions_cache,
                    &active_session, &system_metrics, prefix_active, &config,
                    &overlay, &command_palette, &prefix_help, &copy_mode,
                    &session_sidebar, &session_finder, &rename_dialog,
                    &note_editor, &notes_list,
                );
            }

            // ── server messages ─────────────────────────────────────
            msg = reader.read_message::<ServerMessage>() => {
                match msg {
                    Ok(msg) => {
                        match msg {
                            ServerMessage::Output { pane_id, data } => {
                                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&data)
                                    && let Some(vt) = terminals.get_mut(&pane_id)
                                {
                                    vt.write(&bytes);
                                }
                                do_render(
                                    &mut compositor, &terminals, &pane_rects, &active_pane,
                                    &mut stdout, &module_registry, &sessions_cache,
                                    &active_session, &system_metrics, prefix_active, &config,
                                    &overlay, &command_palette, &prefix_help, &copy_mode,
                                    &session_sidebar, &session_finder, &rename_dialog,
                                    &note_editor, &notes_list,
                                );
                            }
                            ServerMessage::State { sessions, active_session: active_sid } => {
                                sessions_cache = sessions;
                                if let Some(sid) = active_sid {
                                    active_session = sid;
                                }
                                if let Some(session) = sessions_cache.iter().find(|s| s.id == active_session)
                                    && let Some(window) = session.windows.iter().find(|w| w.id == session.active_window)
                                {
                                    active_pane = window.active_pane.clone();
                                }
                            }
                            ServerMessage::Layout { pane_rects: rects_data, .. } => {
                                pane_rects.clear();
                                for (id, rd) in &rects_data {
                                    pane_rects.insert(id.clone(), Rect {
                                        x: rd.x, y: rd.y, width: rd.width, height: rd.height,
                                    });
                                }
                                let pane_ids: std::collections::HashSet<String> =
                                    rects_data.keys().cloned().collect();
                                let existing: Vec<String> = terminals.get_all_ids();
                                for id in existing {
                                    if !pane_ids.contains(&id) {
                                        terminals.remove(&id);
                                    }
                                }
                                for (id, rd) in &rects_data {
                                    if terminals.get(id).is_none() {
                                        terminals.create(id.clone(), rd.width, rd.height, 10_000);
                                    } else if let Some(vt) = terminals.get_mut(id)
                                        && (vt.cols() != rd.width || vt.rows() != rd.height)
                                    {
                                        vt.resize(rd.width, rd.height);
                                    }
                                }
                                do_render(
                                    &mut compositor, &terminals, &pane_rects, &active_pane,
                                    &mut stdout, &module_registry, &sessions_cache,
                                    &active_session, &system_metrics, prefix_active, &config,
                                    &overlay, &command_palette, &prefix_help, &copy_mode,
                                    &session_sidebar, &session_finder, &rename_dialog,
                                    &note_editor, &notes_list,
                                );
                            }
                            ServerMessage::PaneExited { pane_id, .. } => {
                                terminals.remove(&pane_id);
                                pane_rects.remove(&pane_id);
                            }
                            ServerMessage::Metrics { data } => {
                                apply_metrics(&mut system_metrics, &data);
                                // Status bar will update on next render
                            }
                            ServerMessage::ProcessInfo { .. } => {
                                // TODO: dynamic window title updates
                            }
                            ServerMessage::NotesData { notes } => {
                                notes_cache = notes;
                                if matches!(overlay, OverlayState::None) {
                                    let entries: Vec<NotesListEntry> = notes_cache.iter().map(|n| {
                                        NotesListEntry { id: n.id.clone(), title: n.title.clone(), updated_at: n.updated_at }
                                    }).collect();
                                    notes_list = Some(NotesList::new(entries));
                                    overlay = OverlayState::NotesList;
                                    do_render(
                                        &mut compositor, &terminals, &pane_rects, &active_pane,
                                        &mut stdout, &module_registry, &sessions_cache,
                                        &active_session, &system_metrics, prefix_active, &config,
                                        &overlay, &command_palette, &prefix_help, &copy_mode,
                                        &session_sidebar, &session_finder, &rename_dialog,
                                        &note_editor, &notes_list,
                                    );
                                }
                            }
                            ServerMessage::NotesSaved { note } => {
                                if let Some(existing) = notes_cache.iter_mut().find(|n| n.id == note.id) {
                                    *existing = note;
                                } else {
                                    notes_cache.push(note);
                                }
                            }
                            ServerMessage::NotesDeleted { id } => {
                                notes_cache.retain(|n| n.id != id);
                                if matches!(overlay, OverlayState::NotesList) {
                                    let entries: Vec<NotesListEntry> = notes_cache.iter().map(|n| {
                                        NotesListEntry { id: n.id.clone(), title: n.title.clone(), updated_at: n.updated_at }
                                    }).collect();
                                    notes_list = Some(NotesList::new(entries));
                                }
                            }
                            ServerMessage::Result { data, .. } => {
                                if let Some(d) = data
                                    && d == serde_json::json!("detach")
                                {
                                    should_quit = true;
                                }
                            }
                            ServerMessage::Error { message } => {
                                tracing::error!("Server error: {}", message);
                            }
                            _ => {}
                        }
                    }
                    Err(_) => {
                        should_quit = true;
                    }
                }
            }

            // ── terminal resize ─────────────────────────────────────
            _ = sigwinch.recv() => {
                if let Ok((new_cols, new_rows)) = terminal_size() {
                    compositor.resize(new_cols, new_rows);
                    writer.send_message(&ClientMessage::Resize {
                        cols: new_cols, rows: new_rows,
                    }).await.ok();
                }
            }

            // ── config hot-reload ───────────────────────────────────
            result = async {
                if let Some(rx) = config_rx.as_mut() {
                    rx.recv().await
                } else {
                    std::future::pending().await
                }
            } => {
                if result.is_some()
                    && let Ok(new_config) = maxmux_config::load_config() {
                        tracing::info!("Config reloaded");
                        input_router = InputRouter::new(&new_config.prefix_key, new_config.prefix_timeout);
                        load_keybindings_from_config(&new_config, &mut input_router);
                    }
            }
        }
    }

    // ── Cleanup ─────────────────────────────────────────────────────────
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

// ═══════════════════════════════════════════════════════════════════════════
// Command dispatch – opens overlays or forwards to server
// ═══════════════════════════════════════════════════════════════════════════

#[allow(clippy::too_many_arguments)]
async fn handle_command(
    cmd: &str,
    overlay: &mut OverlayState,
    command_palette: &mut Option<CommandPalette>,
    prefix_help: &mut Option<PrefixHelp>,
    copy_mode: &mut Option<CopyModeState>,
    session_sidebar: &mut Option<SessionSidebar>,
    session_finder: &mut Option<SessionFinder>,
    rename_dialog: &mut Option<RenameDialog>,
    note_editor: &mut Option<NoteEditor>,
    writer: &mut maxmux_ipc::transport::MessageWriter,
    should_quit: &mut bool,
    active_pane: &str,
    active_session: &str,
    terminals: &TerminalManager,
    sessions_cache: &[SessionState],
    input_router: &InputRouter,
) {
    match cmd {
        "session:detach" => {
            *should_quit = true;
        }
        "command-palette" => {
            *command_palette = Some(CommandPalette::new(build_command_entries()));
            *overlay = OverlayState::CommandPalette;
        }
        "keybindings:show" => {
            let bindings: Vec<(String, String)> = input_router
                .prefix_bindings()
                .all()
                .iter()
                .map(|(k, v)| (k.clone(), v.command_id.clone()))
                .collect();
            *prefix_help = Some(PrefixHelp::from_command_bindings(&bindings));
            *overlay = OverlayState::PrefixHelp;
        }
        "copy-mode:enter" => {
            if let Some(vt) = terminals.get(active_pane) {
                let buffer_lines = vt.total_lines();
                let viewport_height = vt.rows() as usize;
                *copy_mode = Some(CopyModeState::new(
                    active_pane.to_string(),
                    buffer_lines,
                    viewport_height,
                ));
                *overlay = OverlayState::CopyMode;
            }
        }
        "session:find" => {
            let entries: Vec<SessionFinderEntry> = sessions_cache
                .iter()
                .map(|s| SessionFinderEntry {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    window_count: s.windows.len(),
                    attached: !s.attached_clients.is_empty(),
                })
                .collect();
            *session_finder = Some(SessionFinder::new(entries));
            *overlay = OverlayState::SessionFinder;
        }
        "session:list" => {
            let sidebar_sessions: Vec<SidebarSession> = sessions_cache
                .iter()
                .map(|s| SidebarSession {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    is_active: s.id == active_session,
                    is_attached: !s.attached_clients.is_empty(),
                    windows: s
                        .windows
                        .iter()
                        .enumerate()
                        .map(|(i, w)| SidebarWindow {
                            id: w.id.clone(),
                            name: w.name.clone(),
                            index: i,
                            is_active: w.id == s.active_window,
                        })
                        .collect(),
                })
                .collect();
            *session_sidebar = Some(SessionSidebar::new(sidebar_sessions, SidebarPosition::Left));
            *overlay = OverlayState::SessionSidebar;
        }
        "session:rename" => {
            let current_name = sessions_cache
                .iter()
                .find(|s| s.id == active_session)
                .map(|s| s.name.clone())
                .unwrap_or_default();
            *rename_dialog = Some(RenameDialog::new("Rename Session", current_name));
            *overlay = OverlayState::RenameSession;
        }
        "window:rename" => {
            let current_name = sessions_cache
                .iter()
                .find(|s| s.id == active_session)
                .and_then(|s| s.windows.iter().find(|w| w.id == s.active_window))
                .map(|w| w.name.clone())
                .unwrap_or_default();
            *rename_dialog = Some(RenameDialog::new("Rename Window", current_name));
            *overlay = OverlayState::RenameWindow;
        }
        "notes:create" => {
            *note_editor = Some(NoteEditor::new());
            *overlay = OverlayState::NoteEditor;
        }
        "notes:list" => {
            writer.send_message(&ClientMessage::NotesList).await.ok();
        }
        _ => {
            // Forward all other commands to server
            writer
                .send_message(&ClientMessage::Command {
                    id: cmd.to_string(),
                    args: HashMap::new(),
                })
                .await
                .ok();
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Overlay key routing
// ═══════════════════════════════════════════════════════════════════════════

#[allow(clippy::too_many_arguments)]
async fn handle_overlay_key(
    key: &str,
    overlay: &mut OverlayState,
    command_palette: &mut Option<CommandPalette>,
    prefix_help: &mut Option<PrefixHelp>,
    copy_mode: &mut Option<CopyModeState>,
    session_sidebar: &mut Option<SessionSidebar>,
    session_finder: &mut Option<SessionFinder>,
    rename_dialog: &mut Option<RenameDialog>,
    note_editor: &mut Option<NoteEditor>,
    notes_list: &mut Option<NotesList>,
    writer: &mut maxmux_ipc::transport::MessageWriter,
    should_quit: &mut bool,
    active_pane: &str,
    _active_session: &str,
    terminals: &TerminalManager,
    _pane_rects: &HashMap<String, Rect>,
    notes_cache: &[NoteData],
    terminal_size: (u16, u16),
) {
    let _ = should_quit; // available for future use
    match overlay {
        OverlayState::CommandPalette => {
            if let Some(cp) = command_palette.as_mut() {
                match cp.handle_key(key) {
                    CommandPaletteAction::Execute(cmd) => {
                        *overlay = OverlayState::None;
                        *command_palette = None;
                        writer
                            .send_message(&ClientMessage::Command {
                                id: cmd,
                                args: HashMap::new(),
                            })
                            .await
                            .ok();
                    }
                    CommandPaletteAction::Close => {
                        *overlay = OverlayState::None;
                        *command_palette = None;
                    }
                    CommandPaletteAction::None => {}
                }
            }
        }
        OverlayState::PrefixHelp => {
            if let Some(ph) = prefix_help.as_mut() {
                match ph.handle_key(key) {
                    PrefixHelpAction::Close => {
                        *overlay = OverlayState::None;
                        *prefix_help = None;
                    }
                    PrefixHelpAction::None => {}
                }
            }
        }
        OverlayState::CopyMode => {
            if let Some(cm) = copy_mode.as_mut() {
                let viewport_height = terminals
                    .get(active_pane)
                    .map(|vt| vt.rows() as usize)
                    .unwrap_or(24);
                match cm.handle_key(key, viewport_height) {
                    CopyModeAction::Exit => {
                        *overlay = OverlayState::None;
                        *copy_mode = None;
                    }
                    CopyModeAction::Yank(text) => {
                        if !text.is_empty() {
                            // Copy to system clipboard via OSC 52
                            let b64 =
                                base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
                            let osc52 = format!("\x1b]52;c;{}\x07", b64);
                            let mut out = std::io::stdout();
                            write!(out, "{}", osc52).ok();
                            out.flush().ok();
                        }
                        *overlay = OverlayState::None;
                        *copy_mode = None;
                    }
                    CopyModeAction::ScrollChanged | CopyModeAction::None => {}
                }
            }
        }
        OverlayState::SessionFinder => {
            if let Some(sf) = session_finder.as_mut() {
                match sf.handle_key(key) {
                    SessionFinderAction::Select(session_id) => {
                        *overlay = OverlayState::None;
                        *session_finder = None;
                        writer
                            .send_message(&ClientMessage::Attach {
                                session_id: Some(session_id),
                                cwd: None,
                            })
                            .await
                            .ok();
                    }
                    SessionFinderAction::Close => {
                        *overlay = OverlayState::None;
                        *session_finder = None;
                    }
                    SessionFinderAction::None => {}
                }
            }
        }
        OverlayState::SessionSidebar => {
            if let Some(sb) = session_sidebar.as_mut() {
                match sb.handle_key(key) {
                    SidebarAction::SwitchSession(session_id) => {
                        *overlay = OverlayState::None;
                        *session_sidebar = None;
                        writer
                            .send_message(&ClientMessage::Attach {
                                session_id: Some(session_id),
                                cwd: None,
                            })
                            .await
                            .ok();
                    }
                    SidebarAction::Close => {
                        *overlay = OverlayState::None;
                        *session_sidebar = None;
                    }
                    SidebarAction::None => {}
                }
            }
        }
        OverlayState::RenameSession | OverlayState::RenameWindow => {
            if let Some(rd) = rename_dialog.as_mut() {
                match rd.handle_key(key) {
                    RenameAction::Confirm(new_name) => {
                        let cmd = if matches!(overlay, OverlayState::RenameSession) {
                            "session:set-name"
                        } else {
                            "window:set-name"
                        };
                        writer
                            .send_message(&ClientMessage::Command {
                                id: cmd.to_string(),
                                args: [("name".to_string(), new_name)].into(),
                            })
                            .await
                            .ok();
                        *overlay = OverlayState::None;
                        *rename_dialog = None;
                    }
                    RenameAction::Cancel => {
                        *overlay = OverlayState::None;
                        *rename_dialog = None;
                    }
                    RenameAction::None => {}
                }
            }
        }
        OverlayState::NoteEditor => {
            if let Some(ne) = note_editor.as_mut() {
                let visible_rows = terminal_size.1.saturating_sub(6) as usize;
                match ne.handle_key(key, visible_rows) {
                    NoteEditorAction::Save { id, title, content } => {
                        writer
                            .send_message(&ClientMessage::NotesSave { id, title, content })
                            .await
                            .ok();
                        *overlay = OverlayState::None;
                        *note_editor = None;
                    }
                    NoteEditorAction::Close => {
                        *overlay = OverlayState::None;
                        *note_editor = None;
                    }
                    NoteEditorAction::None => {}
                }
            }
        }
        OverlayState::NotesList => {
            if let Some(nl) = notes_list.as_mut() {
                match nl.handle_key(key) {
                    NotesListAction::Open(note_id) => {
                        if let Some(note) = notes_cache.iter().find(|n| n.id == note_id) {
                            *note_editor =
                                Some(NoteEditor::with_content(note.id.clone(), &note.content));
                            *overlay = OverlayState::NoteEditor;
                            *notes_list = None;
                        }
                    }
                    NotesListAction::Delete(note_id) => {
                        writer
                            .send_message(&ClientMessage::NotesDelete { id: note_id })
                            .await
                            .ok();
                    }
                    NotesListAction::Create => {
                        *note_editor = Some(NoteEditor::new());
                        *overlay = OverlayState::NoteEditor;
                        *notes_list = None;
                    }
                    NotesListAction::Close => {
                        *overlay = OverlayState::None;
                        *notes_list = None;
                    }
                    NotesListAction::None => {}
                }
            }
        }
        OverlayState::None => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Rendering
// ═══════════════════════════════════════════════════════════════════════════

#[allow(clippy::too_many_arguments)]
fn do_render(
    compositor: &mut Compositor,
    terminals: &TerminalManager,
    pane_rects: &HashMap<String, Rect>,
    active_pane: &str,
    stdout: &mut std::io::Stdout,
    module_registry: &HashMap<String, Box<dyn StatusBarModule>>,
    sessions_cache: &[SessionState],
    active_session: &str,
    system_metrics: &SystemMetrics,
    prefix_active: bool,
    config: &maxmux_config::MaxmuxConfig,
    overlay: &OverlayState,
    command_palette: &Option<CommandPalette>,
    prefix_help: &Option<PrefixHelp>,
    copy_mode: &Option<CopyModeState>,
    session_sidebar: &Option<SessionSidebar>,
    session_finder: &Option<SessionFinder>,
    rename_dialog: &Option<RenameDialog>,
    note_editor: &Option<NoteEditor>,
    notes_list: &Option<NotesList>,
) {
    let cols = compositor.cols();
    let rows = compositor.rows();

    // ── Build status bar ────────────────────────────────────────────────
    let status_bar_line = if config.status_bar.enabled {
        let theme = maxmux_statusbar::resolve_theme(&config.status_bar.theme);
        let active_session_info = sessions_cache.iter().find(|s| s.id == active_session);

        let ctx = ModuleContext {
            session: SessionInfo {
                id: active_session.to_string(),
                name: active_session_info
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "main".into()),
            },
            windows: active_session_info
                .map(|s| {
                    s.windows
                        .iter()
                        .enumerate()
                        .map(|(i, w)| WindowInfo {
                            id: w.id.clone(),
                            name: w.name.clone(),
                            index: i,
                            pane_count: w.pane_count,
                            is_active: w.id == s.active_window,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            metrics: system_metrics.clone(),
            prefix_active,
            cols,
            rows,
            colors: ColorPair::new(&theme.bar.bg, &theme.bar.fg),
            theme_colors: theme.clone(),
            module_config: HashMap::new(),
            icons: true,
        };

        let sep_style = match config.status_bar.separator.style {
            maxmux_config::SeparatorStyle::Powerline => "powerline",
            maxmux_config::SeparatorStyle::Rounded => "rounded",
            maxmux_config::SeparatorStyle::Flat => "flat",
            maxmux_config::SeparatorStyle::Arrow => "arrow",
            maxmux_config::SeparatorStyle::Slant => "slant",
        };

        let left_segments: Vec<_> = config
            .status_bar
            .left
            .iter()
            .filter_map(|id| module_registry.get(id.as_str()))
            .flat_map(|m| m.render(&ctx))
            .collect();
        let right_segments: Vec<_> = config
            .status_bar
            .right
            .iter()
            .filter_map(|id| module_registry.get(id.as_str()))
            .flat_map(|m| m.render(&ctx))
            .collect();

        Some(maxmux_statusbar::render_segments(
            &left_segments,
            &right_segments,
            &theme,
            sep_style,
            cols,
            rows.saturating_sub(1),
            prefix_active,
        ))
    } else {
        None
    };

    // ── Compose panes ───────────────────────────────────────────────────
    let term_refs: HashMap<String, &VirtualTerminal> = pane_rects
        .keys()
        .filter_map(|id| terminals.get(id).map(|vt| (id.clone(), vt)))
        .collect();

    let border_config = BorderConfig {
        style: BorderStyle::Rounded,
        fg: (88, 91, 112),
        active_fg: (203, 166, 247),
    };

    let (output, _cursor) = compositor.compose(
        &term_refs,
        pane_rects,
        active_pane,
        status_bar_line.as_deref(),
        &border_config,
        None,
    );

    write!(stdout, "{}", output).ok();

    // ── Overlay on top ──────────────────────────────────────────────────
    let overlay_output = match overlay {
        OverlayState::CommandPalette => command_palette.as_ref().map(|cp| cp.render(cols, rows)),
        OverlayState::PrefixHelp => prefix_help.as_ref().map(|ph| ph.render(cols, rows)),
        OverlayState::CopyMode => copy_mode.as_ref().and_then(|cm| {
            let pane_rect = pane_rects.get(&cm.pane_id)?;
            let vt = terminals.get(&cm.pane_id)?;
            Some(CopyModeRenderer::render(
                cm,
                |row, col| vt.cell_char(row, col),
                |row| vt.line_length(row),
                pane_rect.x,
                pane_rect.y,
                pane_rect.width,
                pane_rect.height,
            ))
        }),
        OverlayState::SessionSidebar => session_sidebar.as_ref().map(|sb| sb.render(cols, rows)),
        OverlayState::SessionFinder => session_finder.as_ref().map(|sf| sf.render(cols, rows)),
        OverlayState::RenameSession | OverlayState::RenameWindow => {
            rename_dialog.as_ref().map(|rd| rd.render(cols, rows))
        }
        OverlayState::NoteEditor => note_editor.as_ref().map(|ne| ne.render(cols, rows)),
        OverlayState::NotesList => notes_list.as_ref().map(|nl| nl.render(cols, rows)),
        OverlayState::None => None,
    };

    if let Some(ov) = overlay_output {
        write!(stdout, "{}", ov).ok();
    }

    stdout.flush().ok();
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn apply_metrics(metrics: &mut SystemMetrics, data: &MetricsData) {
    metrics.cpu = Some(CpuInfo { usage: data.cpu });
    metrics.memory = Some(MemoryInfo {
        used_mb: 0.0,
        total_mb: 0.0,
        percentage: data.memory,
    });
    if let Some(bat) = data.battery {
        metrics.battery = Some(BatteryInfo {
            level: bat as u8,
            charging: false,
            present: true,
        });
    }
}

fn load_keybindings_from_config(config: &maxmux_config::MaxmuxConfig, router: &mut InputRouter) {
    let prefix = router.prefix_bindings_mut();
    for (key, value) in &config.keybindings {
        let (command, unless) = match value {
            KeybindingValue::Command(cmd) => (cmd.clone(), vec![]),
            KeybindingValue::Conditional { command, unless } => (command.clone(), unless.clone()),
        };
        prefix.bind(key.clone(), command, unless);
    }
    let global = router.global_bindings_mut();
    for (key, value) in &config.global_keybindings {
        let (command, unless) = match value {
            KeybindingValue::Command(cmd) => (cmd.clone(), vec![]),
            KeybindingValue::Conditional { command, unless } => (command.clone(), unless.clone()),
        };
        global.bind(key.clone(), command, unless);
    }
}

fn build_command_entries() -> Vec<CommandEntry> {
    vec![
        CommandEntry {
            id: "pane:split-horizontal".into(),
            description: "Split pane horizontally".into(),
        },
        CommandEntry {
            id: "pane:split-vertical".into(),
            description: "Split pane vertically".into(),
        },
        CommandEntry {
            id: "window:create".into(),
            description: "Create new window".into(),
        },
        CommandEntry {
            id: "window:next".into(),
            description: "Next window".into(),
        },
        CommandEntry {
            id: "window:previous".into(),
            description: "Previous window".into(),
        },
        CommandEntry {
            id: "window:close".into(),
            description: "Close window".into(),
        },
        CommandEntry {
            id: "window:rename".into(),
            description: "Rename window".into(),
        },
        CommandEntry {
            id: "pane:close".into(),
            description: "Close pane".into(),
        },
        CommandEntry {
            id: "pane:zoom".into(),
            description: "Toggle pane zoom".into(),
        },
        CommandEntry {
            id: "session:detach".into(),
            description: "Detach from session".into(),
        },
        CommandEntry {
            id: "session:find".into(),
            description: "Find session".into(),
        },
        CommandEntry {
            id: "session:list".into(),
            description: "Session list".into(),
        },
        CommandEntry {
            id: "session:rename".into(),
            description: "Rename session".into(),
        },
        CommandEntry {
            id: "copy-mode:enter".into(),
            description: "Enter copy mode".into(),
        },
        CommandEntry {
            id: "notes:create".into(),
            description: "Create note".into(),
        },
        CommandEntry {
            id: "notes:list".into(),
            description: "List notes".into(),
        },
        CommandEntry {
            id: "keybindings:show".into(),
            description: "Show keybindings".into(),
        },
    ]
}
