# MaxMux Rust Rewrite Design

## Context

MaxMux is a modern terminal session manager (~14,800 lines TypeScript/Bun) with client-server architecture, binary-tree pane layouts, copy-mode, notes system, plugin system, status bar with 15 modules and 7 themes, fuzzy session finder, and more.

This document describes the design for a complete rewrite in Rust, targeting performance, standalone binary distribution, and long-term maintainability.

## Decisions

- **Runtime:** tokio (async)
- **VT Emulation:** alacritty_terminal
- **Config Format:** TOML (`~/.config/maxmux/config.toml`)
- **Plugin System:** Lua scripting (mlua + LuaJIT)
- **Approach:** Bottom-up, layer by layer
- **Location:** `rust/` subdirectory alongside existing `src/`

## Crate Structure

Cargo workspace with focused crates for fast incremental compilation and clear dependency boundaries.

```text
rust/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── maxmux/             # Binary crate (CLI entry point)
│   │   └── src/
│   │       ├── main.rs     # CLI dispatcher (clap)
│   │       ├── server/     # Daemon, handler, broadcast
│   │       └── client/     # Attach, input, UI overlays
│   ├── maxmux-core/        # Session/Window/Pane models, layout engine, PTY, VTerminal
│   ├── maxmux-renderer/    # Compositor, screen buffer, borders, ANSI utils
│   ├── maxmux-input/       # InputRouter (prefix FSM), keybindings, mouse parser
│   ├── maxmux-ipc/         # Protocol types (serde), codec, Unix socket transport
│   ├── maxmux-statusbar/   # 15 modules, 7 themes, renderer
│   ├── maxmux-config/      # TOML schema, loader, live-reload watcher
│   ├── maxmux-persistence/ # Session save/restore, autosave, SQLite notes DB
│   └── maxmux-plugins/     # Lua hook system, plugin loader
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.x (full) | Async runtime |
| serde / serde_json | 1.x | Serialization |
| alacritty_terminal | 0.24 | VT emulation (grid-based) |
| clap | 4.x (derive) | CLI parsing |
| crossterm | 0.28 | Raw terminal mode, queries |
| mlua | 0.10 (luajit, send) | Lua plugin runtime |
| rusqlite | 0.32 (bundled) | Notes SQLite DB |
| notify | 7.x | Config file watcher |
| nucleo | 0.5 | Fuzzy matching (Helix engine) |
| tracing | 0.1 | Structured logging |
| thiserror | 2.x | Error types |
| uuid | 1.x (v4) | Session/Pane IDs |
| nix | 0.29 | PTY/Unix/signals |
| toml | 0.8 | Config parsing |
| dirs | 6.x | XDG paths |

## Architecture

```sql
┌──────────────────────────────────────────────────────────────────┐
│                        maxmux binary                              │
│                                                                    │
│  CLI (clap)                                                      │
│  ├── maxmux                    → start server + attach           │
│  ├── maxmux attach [session]   → attach to existing             │
│  ├── maxmux new-session        → create session                 │
│  ├── maxmux kill-server        → stop daemon                    │
│  ├── maxmux list-sessions      → list sessions                  │
│  └── maxmux <remote-cmd>       → send command to server         │
│                                                                    │
│  SERVER (daemon)                CLIENT (attach)                  │
│  ┌─────────────────────┐      ┌───────────────────────┐         │
│  │ SessionManager      │      │ InputRouter            │         │
│  │ PtyManager          │◄════►│ Compositor             │         │
│  │ TerminalManager     │tokio │ StatusBar              │         │
│  │ CommandRegistry     │Unix  │ Overlays               │         │
│  │ PluginManager(Lua)  │Socket│   CopyMode             │         │
│  │ ConfigWatcher       │      │   SessionFinder        │         │
│  │ AutoSave            │      │   NoteEditor           │         │
│  │ NotesDB             │      │   CommandPalette       │         │
│  │ Metrics             │      │   PrefixHelp           │         │
│  │ ProcessTracker      │      │ Raw Terminal I/O       │         │
│  └─────────────────────┘      └───────────────────────┘         │
└──────────────────────────────────────────────────────────────────┘
```

### Data Model

```text
Session (id, name, windows[], activeWindow)
  └── Window (id, name, panes[], layout: LayoutNode, activePane)
      └── Pane (id, pid, cwd, command, title)
          └── PTY (child process, server-side)
          └── VirtualTerminal (alacritty_terminal::Term, server-side)
```

### Layout Engine

Binary tree (`LayoutNode` enum):
- `Leaf { pane_id }` — single pane
- `Split { direction: H|V, ratio: f64, first, second }` — recursive split

`calculate_layout(node, bounds: Rect) -> Vec<(PaneId, Rect)>` computes pane rectangles.

### Async Model

- Server: `tokio::main` with tasks for client sockets, PTY output readers, timers
- Client: `tokio::main` with tasks for stdin, socket, render ticks
- IPC: `tokio::net::UnixListener` / `UnixStream`, JSON lines codec

### Ownership

- Session/Window/Pane structs are server-owned
- Client receives serialized state snapshots (no shared ownership)
- VirtualTerminal (alacritty_terminal::Term) is server-side only
- Screen buffer is client-side only

### IPC Protocol

JSON-lines over Unix socket (same semantics as current, messages defined via serde enums):

**Client → Server:** attach, detach, input, resize, command, remote-command, notes:*
**Server → Client:** output, state, layout, pane:exited, metrics, process-info, cursor-state, error, result, notes:*

## Build Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| 1 | PTY + VirtualTerminal | Spawn, read, write, resize a single pane |
| 2 | Session/Window/Pane model + Layout engine | Binary-tree layout calculations |
| 3 | Screen, Compositor, Borders, ANSI | Render a static layout to terminal |
| 4 | Key parser, Mouse parser, InputRouter + PrefixMode | Correct input routing |
| 5 | IPC Protocol, Codec, Transport | Server-client communication |
| 6 | Server daemon + Client attach | **First working multiplexer** |
| 7 | TOML config, loader, live-reload watcher | Configurable with hot-reload |
| 8 | StatusBar modules + themes | Full status bar |
| 9 | Session persistence, autosave, Notes DB | Persistence + notes |
| 10 | Lua plugin system | Extensibility |
| 11 | CopyMode, SessionFinder, CommandPalette, NoteEditor | All UI overlays |
| 12 | ProcessTracker, BracketedPaste, MouseSelection | Feature parity |

After Phase 6, a basic but functional multiplexer exists. Phases 7-12 add features incrementally.

## Config Format

TOML replaces the current TypeScript config. Example:

```toml
prefix_key = "C-a"
prefix_timeout = 0
shell = "/bin/zsh"
new_pane_cwd = "inherit"
history_limit = 10000
mouse = true

[theme.border]
style = "rounded"
fg = "#6c7086"
active_fg = "#cba6f7"

[status_bar]
enabled = true
position = "bottom"
theme = "catppuccin-mocha"
separator = "powerline"

[status_bar.left]
modules = ["session", "windows"]

[status_bar.right]
modules = ["git", "cpu", "ram", "datetime"]

[keybindings]
c = "window:create"
n = "window:next"
p = "window:previous"

[global_keybindings]
"C-h" = "pane:focus-left"
"C-j" = "pane:focus-down"
"C-k" = "pane:focus-up"
"C-l" = "pane:focus-right"

[sessions]
auto_save = true
auto_save_interval = 30
auto_restore = true
```

## Key Technical Decisions

1. **nix for PTY** (not portable-pty): Direct control, less abstraction, Linux-only is acceptable
2. **nucleo for fuzzy search** (not skim): Helix's engine, extremely fast, async-ready
3. **crossterm for raw mode only**: No TUI framework — we render directly to stdout like the TS version
4. **alacritty_terminal grid access**: Read cells directly from the grid for compositor, no intermediate buffer
5. **Lua plugins from the start** (Phase 10): mlua with LuaJIT for near-native speed
