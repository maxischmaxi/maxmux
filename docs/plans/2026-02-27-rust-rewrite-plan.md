# MaxMux Rust Rewrite Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rewrite the MaxMux terminal multiplexer from TypeScript/Bun to Rust for performance, standalone binary distribution, and long-term maintainability.

**Architecture:** Bottom-up build in a `rust/` subdirectory. Cargo workspace with 8 crates. tokio async runtime, alacritty_terminal for VT emulation, JSON-lines IPC over Unix sockets. See `docs/plans/2026-02-27-rust-rewrite-design.md` for full design.

**Tech Stack:** Rust, tokio, alacritty_terminal, clap, crossterm, serde, nix, nucleo, mlua, rusqlite, notify, tracing

**Reference:** The existing TypeScript implementation lives in `src/`. Use it as the specification for behavior.

---

## Phase 1: Workspace Setup + PTY + VirtualTerminal

### Task 1.1: Initialize Cargo Workspace

**Files:**
- Create: `rust/Cargo.toml`
- Create: `rust/crates/maxmux-core/Cargo.toml`
- Create: `rust/crates/maxmux-core/src/lib.rs`

**Step 1: Create workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v4"] }
```

**Step 2: Create maxmux-core crate**

```toml
# crates/maxmux-core/Cargo.toml
[package]
name = "maxmux-core"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }
nix = { version = "0.29", features = ["process", "pty", "signal", "term", "fs"] }
alacritty_terminal = "0.24"
bytes = "1"

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

```rust
// crates/maxmux-core/src/lib.rs
pub mod pty;
pub mod terminal;
pub mod session;
pub mod layout;
pub mod command;
```

**Step 3: Verify it compiles**

Run: `cd rust && cargo check`
Expected: Compiles (with warnings about empty modules)

**Step 4: Commit**

```bash
git add rust/
git commit -m "feat(rust): initialize cargo workspace with maxmux-core crate"
```

---

### Task 1.2: PTY Spawning & I/O

**Files:**
- Create: `rust/crates/maxmux-core/src/pty.rs`
- Test: `rust/crates/maxmux-core/src/pty.rs` (inline tests)

**Context:** Port `src/core/bun-pty.ts` + `src/core/pty.ts`. The TS version uses `Bun.Terminal` + `setsid -c <shell>`. In Rust we use `nix::pty::openpty` + `nix::unistd::fork/execvp` with `setsid()` for controlling terminal.

**Step 1: Write the PTY types and PtyHandle**

```rust
// crates/maxmux-core/src/pty.rs
use std::collections::HashMap;
use std::os::fd::{AsRawFd, OwnedFd, FromRawFd, RawFd};
use std::ffi::CString;
use nix::pty::{openpty, Winsize};
use nix::unistd::{close, dup2, execvp, fork, setsid, ForkResult, Pid};
use nix::sys::signal::{kill, Signal};
use nix::sys::termios;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tracing;

#[derive(Error, Debug)]
pub enum PtyError {
    #[error("PTY open failed: {0}")]
    Open(#[from] nix::Error),
    #[error("PTY not found: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type PtyId = String;

pub struct PtyHandle {
    pub id: PtyId,
    pub pid: Pid,
    master_fd: RawFd,
    cols: u16,
    rows: u16,
    dead: bool,
}

pub struct PtyManager {
    ptys: HashMap<PtyId, PtyHandle>,
}
```

**Step 2: Implement spawn()**

The spawn function should:
1. `openpty()` to get master/slave FDs
2. `fork()` - child calls `setsid()`, sets up slave as stdin/stdout/stderr, `execvp(shell)`
3. Parent stores master FD, spawns tokio task to read output
4. Return PtyHandle with id, pid, master_fd

Key detail from TS: Uses `setsid -c <shell>` to establish controlling terminal. In Rust, we call `setsid()` + `ioctl(TIOCSCTTY)` in the child directly.

```rust
impl PtyManager {
    pub fn new() -> Self {
        Self { ptys: HashMap::new() }
    }

    pub fn spawn(
        &mut self,
        id: PtyId,
        shell: &str,
        cols: u16,
        rows: u16,
        cwd: Option<&str>,
        on_data: mpsc::UnboundedSender<(PtyId, Vec<u8>)>,
        on_exit: mpsc::UnboundedSender<(PtyId, i32)>,
    ) -> Result<&PtyHandle, PtyError> {
        let winsize = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let pty = openpty(&winsize, None)?;
        // ... fork + exec logic
    }
}
```

**Step 3: Implement write(), resize(), kill()**

```rust
impl PtyManager {
    pub fn write(&self, id: &str, data: &[u8]) -> Result<(), PtyError> {
        let handle = self.ptys.get(id).ok_or_else(|| PtyError::NotFound(id.into()))?;
        if handle.dead { return Ok(()); }
        nix::unistd::write(handle.master_fd, data)?;
        Ok(())
    }

    pub fn resize(&mut self, id: &str, cols: u16, rows: u16) -> Result<(), PtyError> {
        let handle = self.ptys.get_mut(id).ok_or_else(|| PtyError::NotFound(id.into()))?;
        if handle.dead { return Ok(()); }
        if handle.cols == cols && handle.rows == rows { return Ok(()); }
        let ws = Winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
        unsafe { nix::libc::ioctl(handle.master_fd, nix::libc::TIOCSWINSZ, &ws); }
        handle.cols = cols;
        handle.rows = rows;
        Ok(())
    }

    pub fn kill(&mut self, id: &str) -> Result<(), PtyError> {
        if let Some(mut handle) = self.ptys.remove(id) {
            handle.dead = true;
            let _ = kill(handle.pid, Signal::SIGTERM);
            let _ = close(handle.master_fd);
        }
        Ok(())
    }
}
```

**Step 4: Write integration test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_and_read_output() {
        let (data_tx, mut data_rx) = mpsc::unbounded_channel();
        let (exit_tx, _exit_rx) = mpsc::unbounded_channel();
        let mut mgr = PtyManager::new();

        mgr.spawn(
            "test-pane".into(),
            "/bin/echo",  // simple command that exits
            80, 24, None,
            data_tx, exit_tx,
        ).unwrap();

        // Should receive some output
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            data_rx.recv()
        ).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_resize_dedup() {
        // Resize with same dimensions should be no-op
        let mut mgr = PtyManager::new();
        // ... spawn + resize test
    }
}
```

**Step 5: Run tests**

Run: `cd rust && cargo test -p maxmux-core`
Expected: All tests pass

**Step 6: Commit**

```bash
git add rust/crates/maxmux-core/src/pty.rs
git commit -m "feat(rust): implement PtyManager with spawn, write, resize, kill"
```

---

### Task 1.3: VirtualTerminal Wrapper (alacritty_terminal)

**Files:**
- Create: `rust/crates/maxmux-core/src/terminal.rs`

**Context:** Port `src/core/terminal.ts`. Wraps `alacritty_terminal::Term` to provide:
- Write data (feed bytes to VT parser)
- Read grid cells (for compositor)
- Cursor position/style/visibility tracking
- Mouse tracking mode detection
- Bracketed paste mode detection
- Resize

**Step 1: Write the VirtualTerminal struct**

```rust
// crates/maxmux-core/src/terminal.rs
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::term::{Term, Config as TermConfig};
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Point, Line, Column};
use alacritty_terminal::vte::ansi::{CursorShape, Mode, PrivateMode};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Event listener that captures terminal events
#[derive(Clone)]
struct TermEventListener;

impl EventListener for TermEventListener {
    fn send_event(&self, _event: Event) {
        // We handle events by polling state, not via callbacks
    }
}

pub struct VirtualTerminal {
    term: Term<TermEventListener>,
    cols: u16,
    rows: u16,
}

impl VirtualTerminal {
    pub fn new(cols: u16, rows: u16, scrollback: usize) -> Self {
        let config = TermConfig::default();
        let size = alacritty_terminal::term::SizeInfo::new(
            cols as f32, rows as f32, 1.0, 1.0, 0.0, 0.0,
        );
        let term = Term::new(config, &size, TermEventListener);
        Self { term, cols, rows }
    }

    pub fn write(&mut self, data: &[u8]) {
        // Feed bytes through VT parser
        for byte in data {
            self.term.advance(*byte);
        }
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        let size = alacritty_terminal::term::SizeInfo::new(
            cols as f32, rows as f32, 1.0, 1.0, 0.0, 0.0,
        );
        self.term.resize(size);
    }

    pub fn cursor_position(&self) -> (u16, u16) {
        let point = self.term.grid().cursor.point;
        (point.column.0 as u16, point.line.0 as u16)
    }

    pub fn cursor_shape(&self) -> CursorShape {
        self.term.cursor_style().shape
    }

    pub fn is_cursor_visible(&self) -> bool {
        !self.term.mode().contains(Mode::SHOW_CURSOR)
        // Note: check alacritty_terminal API for exact method
    }

    pub fn is_mouse_tracking_active(&self) -> bool {
        let mode = self.term.mode();
        mode.intersects(
            Mode::MOUSE_REPORT_CLICK
            | Mode::MOUSE_DRAG
            | Mode::MOUSE_MOTION
        )
    }

    pub fn is_bracketed_paste_active(&self) -> bool {
        self.term.mode().contains(Mode::BRACKETED_PASTE)
    }

    /// Read a cell at grid position (for compositor)
    pub fn cell(&self, col: u16, row: u16) -> &Cell {
        let point = Point::new(Line(row as i32), Column(col as usize));
        &self.term.grid()[point]
    }

    pub fn cols(&self) -> u16 { self.cols }
    pub fn rows(&self) -> u16 { self.rows }
}
```

**Step 2: Write TerminalManager**

```rust
pub struct TerminalManager {
    terminals: HashMap<String, VirtualTerminal>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self { terminals: HashMap::new() }
    }

    pub fn create(&mut self, id: String, cols: u16, rows: u16, scrollback: usize) {
        self.terminals.insert(id, VirtualTerminal::new(cols, rows, scrollback));
    }

    pub fn get(&self, id: &str) -> Option<&VirtualTerminal> {
        self.terminals.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut VirtualTerminal> {
        self.terminals.get_mut(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<VirtualTerminal> {
        self.terminals.remove(id)
    }
}
```

**Step 3: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_and_read_cursor() {
        let mut vt = VirtualTerminal::new(80, 24, 1000);
        vt.write(b"Hello, World!");
        let (col, row) = vt.cursor_position();
        assert_eq!(row, 0);
        assert_eq!(col, 13); // "Hello, World!" is 13 chars
    }

    #[test]
    fn test_resize() {
        let mut vt = VirtualTerminal::new(80, 24, 1000);
        vt.resize(120, 40);
        assert_eq!(vt.cols(), 120);
        assert_eq!(vt.rows(), 40);
    }

    #[test]
    fn test_mouse_tracking_default_off() {
        let vt = VirtualTerminal::new(80, 24, 1000);
        assert!(!vt.is_mouse_tracking_active());
    }

    #[test]
    fn test_bracketed_paste_default_off() {
        let vt = VirtualTerminal::new(80, 24, 1000);
        assert!(!vt.is_bracketed_paste_active());
    }
}
```

**Step 4: Run tests**

Run: `cd rust && cargo test -p maxmux-core -- terminal`
Expected: All tests pass

**Step 5: Commit**

```bash
git add rust/crates/maxmux-core/src/terminal.rs
git commit -m "feat(rust): implement VirtualTerminal wrapper around alacritty_terminal"
```

---

## Phase 2: Session Model + Layout Engine

### Task 2.1: Session/Window/Pane Data Model

**Files:**
- Create: `rust/crates/maxmux-core/src/session.rs`

**Context:** Port `src/core/session.ts`. Core hierarchy: Session → Window → Pane.

**Step 1: Define types**

```rust
// crates/maxmux-core/src/session.rs
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;

pub type PaneId = String;
pub type WindowId = String;
pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pane {
    pub id: PaneId,
    pub pid: Option<u32>,
    pub cwd: String,
    pub command: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SplitDirection {
    Horizontal, // left | right
    Vertical,   // top / bottom
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutNode {
    Leaf { pane_id: PaneId },
    Split {
        direction: SplitDirection,
        ratio: f64,
        children: Box<(LayoutNode, LayoutNode)>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    pub id: WindowId,
    pub name: String,
    pub panes: Vec<Pane>,
    pub layout: LayoutNode,
    pub active_pane: PaneId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub windows: Vec<Window>,
    pub active_window: WindowId,
    pub created_at: u64,
    pub attached_clients: Vec<String>,
}

pub struct SessionManager {
    sessions: HashMap<SessionId, Session>,
}
```

**Step 2: Implement SessionManager CRUD**

```rust
impl SessionManager {
    pub fn new() -> Self { Self { sessions: HashMap::new() } }

    pub fn create_session(&mut self, name: &str) -> &Session {
        let id = Uuid::new_v4().to_string();
        let session = Session {
            id: id.clone(),
            name: name.to_string(),
            windows: Vec::new(),
            active_window: String::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_secs(),
            attached_clients: Vec::new(),
        };
        self.sessions.insert(id.clone(), session);
        self.sessions.get(&id).unwrap()
    }

    pub fn get(&self, id: &str) -> Option<&Session> { self.sessions.get(id) }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Session> { self.sessions.get_mut(id) }
    pub fn all(&self) -> Vec<&Session> { self.sessions.values().collect() }
    pub fn remove(&mut self, id: &str) -> Option<Session> { self.sessions.remove(id) }

    pub fn add_window(&mut self, session_id: &str, window: Window) -> Option<&Window> {
        let session = self.sessions.get_mut(session_id)?;
        let win_id = window.id.clone();
        session.windows.push(window);
        if session.active_window.is_empty() {
            session.active_window = win_id.clone();
        }
        session.windows.iter().find(|w| w.id == win_id)
    }

    pub fn add_pane_to_window(
        &mut self, session_id: &str, window_id: &str, pane: Pane
    ) -> bool {
        if let Some(session) = self.sessions.get_mut(session_id) {
            if let Some(window) = session.windows.iter_mut().find(|w| w.id == window_id) {
                window.panes.push(pane);
                return true;
            }
        }
        false
    }
}
```

**Step 3: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let mut mgr = SessionManager::new();
        let session = mgr.create_session("main");
        assert_eq!(session.name, "main");
        assert!(mgr.all().len() == 1);
    }

    #[test]
    fn test_add_window_sets_active() {
        let mut mgr = SessionManager::new();
        let session = mgr.create_session("main");
        let sid = session.id.clone();

        let window = Window {
            id: "w1".into(),
            name: "shell".into(),
            panes: vec![],
            layout: LayoutNode::Leaf { pane_id: "p1".into() },
            active_pane: "p1".into(),
        };
        mgr.add_window(&sid, window);

        let s = mgr.get(&sid).unwrap();
        assert_eq!(s.active_window, "w1");
    }
}
```

**Step 4: Run tests, commit**

Run: `cd rust && cargo test -p maxmux-core -- session`

```bash
git commit -m "feat(rust): implement Session/Window/Pane data model and SessionManager"
```

---

### Task 2.2: Layout Engine

**Files:**
- Create: `rust/crates/maxmux-core/src/layout.rs`

**Context:** Port `src/core/layout.ts`. This is the most algorithm-heavy module. Binary tree layout with ratio-based splitting, border accounting, and smart direction-based pane navigation.

**Step 1: Define Rect and core types**

```rust
// crates/maxmux-core/src/layout.rs
use crate::session::{LayoutNode, SplitDirection, PaneId};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Up, Down, Left, Right,
}
```

**Step 2: Implement calculate_layout()**

Port the recursive layout calculation from TS. Key: splits account for 1-cell border between children.

```rust
pub fn calculate_layout(node: &LayoutNode, bounds: Rect) -> HashMap<PaneId, Rect> {
    let mut result = HashMap::new();
    calculate_layout_inner(node, bounds, &mut result);
    result
}

fn calculate_layout_inner(
    node: &LayoutNode,
    bounds: Rect,
    result: &mut HashMap<PaneId, Rect>,
) {
    match node {
        LayoutNode::Leaf { pane_id } => {
            result.insert(pane_id.clone(), bounds);
        }
        LayoutNode::Split { direction, ratio, children } => {
            let (first_bounds, second_bounds) = match direction {
                SplitDirection::Horizontal => {
                    // Left | Right, with 1-cell vertical border
                    let split_x = bounds.x + ((bounds.width as f64) * ratio).floor() as u16;
                    let first = Rect {
                        x: bounds.x,
                        y: bounds.y,
                        width: split_x - bounds.x,
                        height: bounds.height,
                    };
                    let second = Rect {
                        x: split_x + 1, // +1 for border
                        y: bounds.y,
                        width: bounds.x + bounds.width - split_x - 1,
                        height: bounds.height,
                    };
                    (first, second)
                }
                SplitDirection::Vertical => {
                    // Top / Bottom, with 1-cell horizontal border
                    let split_y = bounds.y + ((bounds.height as f64) * ratio).floor() as u16;
                    let first = Rect {
                        x: bounds.x,
                        y: bounds.y,
                        width: bounds.width,
                        height: split_y - bounds.y,
                    };
                    let second = Rect {
                        x: bounds.x,
                        y: split_y + 1,
                        width: bounds.width,
                        height: bounds.y + bounds.height - split_y - 1,
                    };
                    (first, second)
                }
            };
            calculate_layout_inner(&children.0, first_bounds, result);
            calculate_layout_inner(&children.1, second_bounds, result);
        }
    }
}
```

**Step 3: Implement split_layout() and remove_from_layout()**

```rust
/// Split an existing pane into two, returning new layout tree
pub fn split_layout(
    node: &LayoutNode,
    target_pane: &str,
    new_pane: &str,
    direction: SplitDirection,
) -> LayoutNode {
    match node {
        LayoutNode::Leaf { pane_id } if pane_id == target_pane => {
            LayoutNode::Split {
                direction,
                ratio: 0.5,
                children: Box::new((
                    LayoutNode::Leaf { pane_id: pane_id.clone() },
                    LayoutNode::Leaf { pane_id: new_pane.to_string() },
                )),
            }
        }
        LayoutNode::Split { direction: d, ratio, children } => {
            LayoutNode::Split {
                direction: d.clone(),
                ratio: *ratio,
                children: Box::new((
                    split_layout(&children.0, target_pane, new_pane, direction.clone()),
                    split_layout(&children.1, target_pane, new_pane, direction.clone()),
                )),
            }
        }
        other => other.clone(),
    }
}

/// Remove a pane from layout, collapsing its parent split
pub fn remove_from_layout(node: &LayoutNode, pane_id: &str) -> Option<LayoutNode> {
    match node {
        LayoutNode::Leaf { pane_id: id } if id == pane_id => None,
        LayoutNode::Leaf { .. } => Some(node.clone()),
        LayoutNode::Split { children, .. } => {
            let first = remove_from_layout(&children.0, pane_id);
            let second = remove_from_layout(&children.1, pane_id);
            match (first, second) {
                (None, None) => None,
                (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
                (Some(f), Some(s)) => Some(LayoutNode::Split {
                    direction: match node {
                        LayoutNode::Split { direction, .. } => direction.clone(),
                        _ => unreachable!(),
                    },
                    ratio: match node {
                        LayoutNode::Split { ratio, .. } => *ratio,
                        _ => unreachable!(),
                    },
                    children: Box::new((f, s)),
                }),
            }
        }
    }
}
```

**Step 4: Implement find_pane_in_direction()**

Port the smart navigation algorithm from `src/core/layout.ts`:
- Manhattan distance
- Perpendicular overlap preference
- Previous-pane tiebreaker within 10% tolerance

```rust
pub fn find_pane_in_direction(
    pane_rects: &HashMap<PaneId, Rect>,
    current_id: &str,
    direction: Direction,
    preferred_id: Option<&str>,
) -> Option<PaneId> {
    let current = pane_rects.get(current_id)?;
    let cx = current.x as f64 + current.width as f64 / 2.0;
    let cy = current.y as f64 + current.height as f64 / 2.0;

    let mut candidates: Vec<(&PaneId, &Rect, f64, bool)> = Vec::new();

    for (id, rect) in pane_rects {
        if id == current_id { continue; }
        let px = rect.x as f64 + rect.width as f64 / 2.0;
        let py = rect.y as f64 + rect.height as f64 / 2.0;

        let in_direction = match direction {
            Direction::Left => px < cx,
            Direction::Right => px > cx,
            Direction::Up => py < cy,
            Direction::Down => py > cy,
        };
        if !in_direction { continue; }

        let dist = (px - cx).abs() + (py - cy).abs();

        // Perpendicular overlap check
        let overlaps = match direction {
            Direction::Left | Direction::Right => {
                let r_top = rect.y;
                let r_bot = rect.y + rect.height;
                let c_top = current.y;
                let c_bot = current.y + current.height;
                r_top < c_bot && r_bot > c_top
            }
            Direction::Up | Direction::Down => {
                let r_left = rect.x;
                let r_right = rect.x + rect.width;
                let c_left = current.x;
                let c_right = current.x + current.width;
                r_left < c_right && r_right > c_left
            }
        };

        candidates.push((id, rect, dist, overlaps));
    }

    if candidates.is_empty() { return None; }

    // Prefer overlapping candidates
    let has_overlapping = candidates.iter().any(|(_, _, _, o)| *o);
    if has_overlapping {
        candidates.retain(|(_, _, _, o)| *o);
    }

    // Sort by distance
    candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

    let best_dist = candidates[0].2;

    // Tiebreaker: prefer previously focused pane if within 10%
    if let Some(pref) = preferred_id {
        if let Some(pref_candidate) = candidates.iter().find(|(id, _, _, _)| id.as_str() == pref) {
            if pref_candidate.2 <= best_dist * 1.1 {
                return Some(pref.to_string());
            }
        }
    }

    Some(candidates[0].0.clone())
}

pub fn get_all_pane_ids(node: &LayoutNode) -> Vec<PaneId> {
    match node {
        LayoutNode::Leaf { pane_id } => vec![pane_id.clone()],
        LayoutNode::Split { children, .. } => {
            let mut ids = get_all_pane_ids(&children.0);
            ids.extend(get_all_pane_ids(&children.1));
            ids
        }
    }
}
```

**Step 5: Write comprehensive tests**

Reference `src/core/layout.test.ts` for test cases. Must cover:
- Single pane fills entire bounds
- Horizontal split: left/right with border gap
- Vertical split: top/bottom with border gap
- Nested splits (3+ panes)
- split_layout immutability
- remove_from_layout collapse
- find_pane_in_direction with overlap preference
- find_pane_in_direction tiebreaker

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: &str) -> LayoutNode {
        LayoutNode::Leaf { pane_id: id.into() }
    }

    fn hsplit(ratio: f64, first: LayoutNode, second: LayoutNode) -> LayoutNode {
        LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio,
            children: Box::new((first, second)),
        }
    }

    fn vsplit(ratio: f64, first: LayoutNode, second: LayoutNode) -> LayoutNode {
        LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio,
            children: Box::new((first, second)),
        }
    }

    #[test]
    fn test_single_pane() {
        let layout = leaf("p1");
        let bounds = Rect { x: 0, y: 0, width: 80, height: 24 };
        let rects = calculate_layout(&layout, bounds);
        assert_eq!(rects.get("p1"), Some(&bounds));
    }

    #[test]
    fn test_horizontal_split() {
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect { x: 0, y: 0, width: 80, height: 24 };
        let rects = calculate_layout(&layout, bounds);
        let p1 = rects.get("p1").unwrap();
        let p2 = rects.get("p2").unwrap();
        assert_eq!(p1.x, 0);
        assert_eq!(p1.width, 40);
        assert_eq!(p2.x, 41); // 40 + 1 border
        assert!(p2.width > 0);
    }

    #[test]
    fn test_vertical_split() {
        let layout = vsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect { x: 0, y: 0, width: 80, height: 24 };
        let rects = calculate_layout(&layout, bounds);
        let p1 = rects.get("p1").unwrap();
        let p2 = rects.get("p2").unwrap();
        assert_eq!(p1.y, 0);
        assert_eq!(p1.height, 12);
        assert_eq!(p2.y, 13); // 12 + 1 border
    }

    #[test]
    fn test_remove_pane_collapses_split() {
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let result = remove_from_layout(&layout, "p1");
        assert!(matches!(result, Some(LayoutNode::Leaf { pane_id }) if pane_id == "p2"));
    }

    #[test]
    fn test_find_pane_right() {
        let layout = hsplit(0.5, leaf("p1"), leaf("p2"));
        let bounds = Rect { x: 0, y: 0, width: 80, height: 24 };
        let rects = calculate_layout(&layout, bounds);
        let found = find_pane_in_direction(&rects, "p1", Direction::Right, None);
        assert_eq!(found, Some("p2".into()));
    }

    #[test]
    fn test_find_pane_overlap_preference() {
        // T-shape: p1 top-left, p2 top-right, p3 full bottom
        let layout = vsplit(0.5,
            hsplit(0.5, leaf("p1"), leaf("p2")),
            leaf("p3"),
        );
        let bounds = Rect { x: 0, y: 0, width: 80, height: 24 };
        let rects = calculate_layout(&layout, bounds);
        // From p1, going down: p3 overlaps more than p2
        let found = find_pane_in_direction(&rects, "p1", Direction::Down, None);
        assert_eq!(found, Some("p3".into()));
    }
}
```

**Step 6: Run tests, commit**

Run: `cd rust && cargo test -p maxmux-core -- layout`

```bash
git commit -m "feat(rust): implement binary-tree layout engine with smart navigation"
```

---

### Task 2.3: Command Registry

**Files:**
- Create: `rust/crates/maxmux-core/src/command.rs`

**Context:** Port `src/core/command.ts`. Simple registry mapping command IDs to handlers.

**Step 1: Implement**

```rust
// crates/maxmux-core/src/command.rs
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Unknown command: {0}")]
    NotFound(String),
    #[error("Command failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct CommandContext {
    pub session_id: String,
    pub window_id: Option<String>,
    pub pane_id: Option<String>,
    pub args: HashMap<String, String>,
}

pub type CommandHandler = Box<
    dyn Fn(CommandContext) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + Send>>
        + Send
        + Sync
>;

pub struct CommandInfo {
    pub id: String,
    pub description: String,
    pub handler: CommandHandler,
}

pub struct CommandRegistry {
    commands: HashMap<String, CommandInfo>,
}

impl CommandRegistry {
    pub fn new() -> Self { Self { commands: HashMap::new() } }

    pub fn register(&mut self, id: String, description: String, handler: CommandHandler) {
        self.commands.insert(id.clone(), CommandInfo { id, description, handler });
    }

    pub async fn execute(&self, id: &str, ctx: CommandContext) -> Result<(), CommandError> {
        let cmd = self.commands.get(id).ok_or_else(|| CommandError::NotFound(id.into()))?;
        (cmd.handler)(ctx).await
    }

    pub fn has(&self, id: &str) -> bool { self.commands.contains_key(id) }

    pub fn list(&self) -> Vec<(&str, &str)> {
        self.commands.values().map(|c| (c.id.as_str(), c.description.as_str())).collect()
    }
}
```

**Step 2: Test, commit**

Run: `cd rust && cargo test -p maxmux-core -- command`

```bash
git commit -m "feat(rust): implement async CommandRegistry"
```

---

## Phase 3: Renderer

### Task 3.1: ANSI Utilities

**Files:**
- Create: `rust/crates/maxmux-renderer/Cargo.toml`
- Create: `rust/crates/maxmux-renderer/src/lib.rs`
- Create: `rust/crates/maxmux-renderer/src/ansi.rs`

**Context:** Port `src/renderer/ansi.ts`. Pure string builders for ANSI escape sequences.

**Step 1: Create maxmux-renderer crate**

```toml
# crates/maxmux-renderer/Cargo.toml
[package]
name = "maxmux-renderer"
version = "0.1.0"
edition = "2024"

[dependencies]
maxmux-core = { path = "../maxmux-core" }
```

**Step 2: Implement ANSI builders**

```rust
// crates/maxmux-renderer/src/ansi.rs

pub fn move_to(x: u16, y: u16) -> String {
    format!("\x1b[{};{}H", y + 1, x + 1) // ANSI is 1-based
}

pub fn hide_cursor() -> &'static str { "\x1b[?25l" }
pub fn show_cursor() -> &'static str { "\x1b[?25h" }
pub fn set_cursor_style(style: u8) -> String { format!("\x1b[{} q", style) }
pub fn reset_style() -> &'static str { "\x1b[0m" }
pub fn bold() -> &'static str { "\x1b[1m" }
pub fn dim() -> &'static str { "\x1b[2m" }
pub fn italic() -> &'static str { "\x1b[3m" }
pub fn underline() -> &'static str { "\x1b[4m" }
pub fn inverse() -> &'static str { "\x1b[7m" }
pub fn clear_screen() -> &'static str { "\x1b[2J" }
pub fn clear_line() -> &'static str { "\x1b[2K" }
pub fn enter_alt_screen() -> &'static str { "\x1b[?1049h" }
pub fn exit_alt_screen() -> &'static str { "\x1b[?1049l" }
pub fn enable_mouse() -> &'static str { "\x1b[?1000h\x1b[?1002h\x1b[?1006h" }
pub fn disable_mouse() -> &'static str { "\x1b[?1000l\x1b[?1002l\x1b[?1006l" }
pub fn enable_bracketed_paste() -> &'static str { "\x1b[?2004h" }
pub fn disable_bracketed_paste() -> &'static str { "\x1b[?2004l" }

pub fn fg_rgb(r: u8, g: u8, b: u8) -> String { format!("\x1b[38;2;{};{};{}m", r, g, b) }
pub fn bg_rgb(r: u8, g: u8, b: u8) -> String { format!("\x1b[48;2;{};{};{}m", r, g, b) }

pub fn fg_hex(hex: &str) -> String {
    let (r, g, b) = hex_to_rgb(hex);
    fg_rgb(r, g, b)
}

pub fn bg_hex(hex: &str) -> String {
    let (r, g, b) = hex_to_rgb(hex);
    bg_rgb(r, g, b)
}

fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    (r, g, b)
}
```

**Step 3: Tests, commit**

```bash
git commit -m "feat(rust): implement ANSI escape code utilities"
```

---

### Task 3.2: Screen Buffer with Dirty Tracking

**Files:**
- Create: `rust/crates/maxmux-renderer/src/screen.rs`

**Context:** Port `src/renderer/screen.ts`. Double-buffered 2D cell grid with dirty tracking for diff rendering.

**Step 1: Implement ScreenCell and ScreenBuffer**

```rust
// crates/maxmux-renderer/src/screen.rs

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenCell {
    pub ch: char,
    pub fg: Option<(u8, u8, u8)>,
    pub bg: Option<(u8, u8, u8)>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for ScreenCell {
    fn default() -> Self {
        Self { ch: ' ', fg: None, bg: None, bold: false, dim: false, italic: false, underline: false }
    }
}

pub struct ScreenBuffer {
    cols: u16,
    rows: u16,
    cells: Vec<Vec<ScreenCell>>,
    prev_cells: Vec<Vec<ScreenCell>>,
}

impl ScreenBuffer {
    pub fn new(cols: u16, rows: u16) -> Self { /* ... */ }
    pub fn resize(&mut self, cols: u16, rows: u16) { /* recreate grids */ }
    pub fn set(&mut self, x: u16, y: u16, cell: ScreenCell) { /* bounds check + set */ }
    pub fn write_string(&mut self, x: u16, y: u16, s: &str, fg: Option<(u8,u8,u8)>, bg: Option<(u8,u8,u8)>, bold: bool) { /* ... */ }
    pub fn fill_row(&mut self, y: u16, ch: char, fg: Option<(u8,u8,u8)>, bg: Option<(u8,u8,u8)>) { /* ... */ }
    pub fn snapshot(&mut self) { /* prev_cells = cells.clone() */ }

    /// Returns ANSI string for rendering. >50% dirty = full redraw, else diff.
    pub fn flush(&self) -> String {
        let total = self.cols as usize * self.rows as usize;
        let dirty_count = self.count_dirty();
        if dirty_count * 2 > total {
            self.full_render()
        } else {
            self.diff_render()
        }
    }
}
```

**Step 2: Tests for dirty tracking, commit**

```bash
git commit -m "feat(rust): implement double-buffered ScreenBuffer with dirty tracking"
```

---

### Task 3.3: Border Renderer

**Files:**
- Create: `rust/crates/maxmux-renderer/src/border.rs`

**Context:** Port `src/renderer/border.ts`. Renders borders between panes using box-drawing characters.

**Step 1: Implement border chars and rendering**

```rust
// crates/maxmux-renderer/src/border.rs
use crate::screen::ScreenBuffer;
use maxmux_core::layout::Rect;
use std::collections::HashMap;
use maxmux_core::session::PaneId;

pub struct BorderChars {
    pub horizontal: char,
    pub vertical: char,
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub tee_left: char,
    pub tee_right: char,
    pub tee_top: char,
    pub tee_bottom: char,
    pub cross: char,
}

pub const ROUNDED: BorderChars = BorderChars {
    horizontal: '─', vertical: '│',
    top_left: '╭', top_right: '╮',
    bottom_left: '╰', bottom_right: '╯',
    tee_left: '├', tee_right: '┤',
    tee_top: '┬', tee_bottom: '┴',
    cross: '┼',
};

pub const SHARP: BorderChars = BorderChars {
    horizontal: '─', vertical: '│',
    top_left: '┌', top_right: '┐',
    bottom_left: '└', bottom_right: '┘',
    tee_left: '├', tee_right: '┤',
    tee_top: '┬', tee_bottom: '┴',
    cross: '┼',
};

/// Render borders between panes onto the screen buffer
pub fn render_borders(
    screen: &mut ScreenBuffer,
    pane_rects: &HashMap<PaneId, Rect>,
    active_pane: &str,
    chars: &BorderChars,
    fg: (u8, u8, u8),
    active_fg: (u8, u8, u8),
) {
    // For each pair of adjacent panes, draw border line between them
    // Vertical borders: where pane1.x + pane1.width + 1 == pane2.x
    // Horizontal borders: where pane1.y + pane1.height + 1 == pane2.y
    // Active border color on the half adjacent to the active pane
    // ... (implementation follows TS logic)
}
```

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement border rendering with box-drawing characters"
```

---

### Task 3.4: Compositor

**Files:**
- Create: `rust/crates/maxmux-renderer/src/compositor.rs`

**Context:** Port `src/renderer/compositor.ts`. Composes pane contents + borders + status bar into final screen output.

**Step 1: Implement Compositor**

```rust
// crates/maxmux-renderer/src/compositor.rs
use crate::screen::{ScreenBuffer, ScreenCell};
use crate::border;
use crate::ansi;
use maxmux_core::terminal::VirtualTerminal;
use maxmux_core::layout::Rect;
use maxmux_core::session::PaneId;
use std::collections::HashMap;

pub struct Compositor {
    screen: ScreenBuffer,
    cols: u16,
    rows: u16,
}

impl Compositor {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            screen: ScreenBuffer::new(cols, rows),
            cols, rows,
        }
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.screen.resize(cols, rows);
    }

    /// Compose full screen and return ANSI output string
    pub fn compose(
        &mut self,
        terminals: &HashMap<PaneId, &VirtualTerminal>,
        pane_rects: &HashMap<PaneId, Rect>,
        active_pane: &str,
        status_bar_content: Option<&str>,
        border_config: &border::BorderConfig,
        zoomed_pane: Option<&str>,
    ) -> String {
        self.screen.snapshot();
        // 1. Clear
        // 2. Render pane contents (read cells from VirtualTerminal grids)
        // 3. Render borders
        // 4. Render status bar (last row)
        // 5. Flush (diff or full render)
        // 6. Position cursor at active pane's cursor location
        self.screen.flush()
    }
}
```

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement screen Compositor"
```

---

## Phase 4: Input Handling

### Task 4.1: Key Parser

**Files:**
- Create: `rust/crates/maxmux-input/Cargo.toml`
- Create: `rust/crates/maxmux-input/src/lib.rs`
- Create: `rust/crates/maxmux-input/src/keys.rs`

**Context:** Port key name parsing from `src/input/router.ts`. Converts raw bytes to key names like `"Up"`, `"C-a"`, `"M-x"`.

**Step 1: Create crate and implement**

Key sequences to recognize:
- `\x1b[A..D` → Up/Down/Right/Left
- `\x01..\x1a` → C-a..C-z
- `\x1b` + char → M-char (Alt combinations)
- Printable ASCII
- `\x7f` → Backspace

```rust
// crates/maxmux-input/src/keys.rs

#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    Char(char),
    Ctrl(char),       // C-a through C-z
    Alt(char),        // M-x
    Up, Down, Left, Right,
    Home, End,
    PageUp, PageDown,
    Backspace,
    Tab,
    Enter,
    Escape,
    Unknown(Vec<u8>),
}

/// Parse raw input bytes into a Key
pub fn parse_key(data: &[u8]) -> (Key, usize) {
    // ... byte-by-byte parsing
    // Returns (parsed key, bytes consumed)
}

/// Convert Key to the string format used in keybinding config
pub fn key_name(key: &Key) -> String {
    match key {
        Key::Char(c) => c.to_string(),
        Key::Ctrl(c) => format!("C-{}", c),
        Key::Alt(c) => format!("M-{}", c),
        Key::Up => "Up".into(),
        // ... etc
    }
}

/// Parse a config key name like "C-a" into the byte that triggers it
pub fn parse_prefix_key(name: &str) -> Option<u8> {
    if name.starts_with("C-") && name.len() == 3 {
        let c = name.as_bytes()[2];
        Some(c - b'a' + 1) // C-a = 0x01, C-b = 0x02, etc.
    } else {
        None
    }
}
```

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement key parser for terminal input"
```

---

### Task 4.2: Mouse Parser (SGR Format)

**Files:**
- Create: `rust/crates/maxmux-input/src/mouse.rs`

**Context:** Port `src/input/mouse.ts`. SGR mouse format: `\x1b[<Cb;Cx;CyM` (press) / `...m` (release).

**Step 1: Implement**

```rust
// crates/maxmux-input/src/mouse.rs

#[derive(Debug, Clone, PartialEq)]
pub struct MouseEvent {
    pub button: u8,
    pub x: u16,       // 0-based
    pub y: u16,       // 0-based
    pub is_release: bool,
}

pub const MOUSE_LEFT: u8 = 0;
pub const MOUSE_MIDDLE: u8 = 1;
pub const MOUSE_RIGHT: u8 = 2;
pub const SCROLL_UP: u8 = 64;
pub const SCROLL_DOWN: u8 = 65;

pub fn parse_sgr_mouse(data: &[u8]) -> Option<(MouseEvent, usize)> {
    // Parse: ESC [ < Cb ; Cx ; Cy M/m
    // Cb, Cx, Cy are decimal numbers
    // M = press, m = release
    // Coordinates are 1-based in protocol, convert to 0-based
    // Returns (event, bytes consumed)
}

pub fn encode_sgr_mouse(button: u8, x: u16, y: u16, is_release: bool) -> String {
    // Encode with 1-based coords for forwarding to PTY
    format!("\x1b[<{};{};{}{}", button, x + 1, y + 1, if is_release { 'm' } else { 'M' })
}

pub fn base_button(button: u8) -> u8 { button & 0b11 }
pub fn is_scroll(button: u8) -> bool { button & 64 != 0 }
pub fn is_motion(button: u8) -> bool { button & 32 != 0 }
```

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement SGR mouse event parser"
```

---

### Task 4.3: Keybinding Registry

**Files:**
- Create: `rust/crates/maxmux-input/src/keybindings.rs`

**Context:** Port `src/input/keybindings.ts`. Maps key names to command IDs with optional `unless` process conditions.

**Step 1: Implement**

```rust
// crates/maxmux-input/src/keybindings.rs
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Keybinding {
    pub command_id: String,
    pub unless: Vec<String>, // process names to exclude
}

pub struct KeybindingRegistry {
    bindings: HashMap<String, Keybinding>,
}

impl KeybindingRegistry {
    pub fn new() -> Self { Self { bindings: HashMap::new() } }

    pub fn bind(&mut self, key: String, command_id: String, unless: Vec<String>) {
        self.bindings.insert(key, Keybinding { command_id, unless });
    }

    pub fn resolve(&self, key: &str, current_process: Option<&str>) -> Option<&str> {
        let binding = self.bindings.get(key)?;
        if let Some(proc) = current_process {
            if binding.unless.iter().any(|u| u == proc) {
                return None;
            }
        }
        Some(&binding.command_id)
    }

    pub fn all(&self) -> &HashMap<String, Keybinding> { &self.bindings }
}
```

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement KeybindingRegistry with conditional unless"
```

---

### Task 4.4: Input Router with Prefix Mode

**Files:**
- Create: `rust/crates/maxmux-input/src/router.rs`

**Context:** Port `src/input/router.ts`. State machine: Normal → Prefix → dispatch/timeout. This is the central input handling logic.

**Step 1: Implement InputRouter**

```rust
// crates/maxmux-input/src/router.rs
use crate::keys::{self, Key};
use crate::keybindings::KeybindingRegistry;

#[derive(Debug, Clone, PartialEq)]
pub enum InputAction {
    Passthrough(Vec<u8>),
    Command(String),
    PrefixActivated,
    PrefixTimeout,
}

pub struct InputRouter {
    prefix_byte: u8,
    prefix_timeout_ms: u64,
    in_prefix_mode: bool,
    prefix_bindings: KeybindingRegistry,
    global_bindings: KeybindingRegistry,
}

impl InputRouter {
    pub fn new(prefix_key: &str, timeout_ms: u64) -> Self {
        let prefix_byte = keys::parse_prefix_key(prefix_key).unwrap_or(0x01);
        Self {
            prefix_byte,
            prefix_timeout_ms: timeout_ms,
            in_prefix_mode: false,
            prefix_bindings: KeybindingRegistry::new(),
            global_bindings: KeybindingRegistry::new(),
        }
    }

    pub fn handle_input(
        &mut self,
        data: &[u8],
        current_process: Option<&str>,
    ) -> Vec<InputAction> {
        let mut actions = Vec::new();
        let mut i = 0;

        while i < data.len() {
            if self.in_prefix_mode {
                let (key, consumed) = keys::parse_key(&data[i..]);
                let key_name = keys::key_name(&key);

                if let Some(cmd) = self.prefix_bindings.resolve(&key_name, current_process) {
                    actions.push(InputAction::Command(cmd.to_string()));
                } else {
                    // Unknown prefix key - passthrough
                    actions.push(InputAction::Passthrough(data[i..i+consumed].to_vec()));
                }
                self.in_prefix_mode = false;
                i += consumed;
            } else if data[i] == self.prefix_byte {
                self.in_prefix_mode = true;
                actions.push(InputAction::PrefixActivated);
                i += 1;
            } else {
                // Check global keybindings
                let (key, consumed) = keys::parse_key(&data[i..]);
                let key_name = keys::key_name(&key);

                if let Some(cmd) = self.global_bindings.resolve(&key_name, current_process) {
                    actions.push(InputAction::Command(cmd.to_string()));
                } else {
                    actions.push(InputAction::Passthrough(data[i..i+consumed].to_vec()));
                }
                i += consumed;
            }
        }
        actions
    }

    pub fn prefix_bindings_mut(&mut self) -> &mut KeybindingRegistry { &mut self.prefix_bindings }
    pub fn global_bindings_mut(&mut self) -> &mut KeybindingRegistry { &mut self.global_bindings }
    pub fn is_in_prefix_mode(&self) -> bool { self.in_prefix_mode }
    pub fn cancel_prefix(&mut self) { self.in_prefix_mode = false; }
}
```

**Step 2: Tests (reference src/input/router.test.ts), commit**

```bash
git commit -m "feat(rust): implement InputRouter with prefix mode state machine"
```

---

## Phase 5: IPC Protocol

### Task 5.1: Message Types

**Files:**
- Create: `rust/crates/maxmux-ipc/Cargo.toml`
- Create: `rust/crates/maxmux-ipc/src/lib.rs`
- Create: `rust/crates/maxmux-ipc/src/protocol.rs`

**Context:** Define all client→server and server→client message types as serde enums. Reference the message types documented in the exploration report.

**Step 1: Create crate and define messages**

```rust
// crates/maxmux-ipc/src/protocol.rs
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "attach")]
    Attach { session_id: Option<String>, cwd: Option<String> },
    #[serde(rename = "detach")]
    Detach,
    #[serde(rename = "input")]
    Input { pane_id: String, data: String }, // base64 encoded
    #[serde(rename = "resize")]
    Resize { cols: u16, rows: u16 },
    #[serde(rename = "command")]
    Command { id: String, args: Option<HashMap<String, String>> },
    #[serde(rename = "preview")]
    Preview { session_id: String, cols: u16, rows: u16 },
    #[serde(rename = "preview-stop")]
    PreviewStop,
    #[serde(rename = "remote-command")]
    RemoteCommand { command: String, args: Option<Vec<String>>, target: Option<String> },
    // Notes
    #[serde(rename = "notes:list")]
    NotesList,
    #[serde(rename = "notes:save")]
    NotesSave { id: Option<String>, title: String, content: String },
    #[serde(rename = "notes:delete")]
    NotesDelete { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "output")]
    Output { pane_id: String, data: String },
    #[serde(rename = "state")]
    State { sessions: Vec<SessionState>, active_session: Option<String> },
    #[serde(rename = "layout")]
    Layout { layout: serde_json::Value, pane_rects: HashMap<String, RectData> },
    #[serde(rename = "pane:exited")]
    PaneExited { pane_id: String, exit_code: i32 },
    #[serde(rename = "metrics")]
    Metrics { data: MetricsData },
    #[serde(rename = "cursor-state")]
    CursorState { panes: HashMap<String, CursorInfo> },
    #[serde(rename = "process-info")]
    ProcessInfo { panes: HashMap<String, String>, full: Option<HashMap<String, String>> },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "result")]
    Result { success: bool, data: Option<serde_json::Value>, error: Option<String> },
    // Preview
    #[serde(rename = "preview-layout")]
    PreviewLayout { layout: serde_json::Value, pane_rects: HashMap<String, RectData> },
    #[serde(rename = "preview-output")]
    PreviewOutput { pane_id: String, data: String },
    // Notes
    #[serde(rename = "notes:data")]
    NotesData { notes: Vec<NoteData> },
    #[serde(rename = "notes:saved")]
    NotesSaved { note: NoteData },
    #[serde(rename = "notes:deleted")]
    NotesDeleted { id: String },
}

// Supporting types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState { /* mirrors Session for serialization */ }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RectData { pub x: u16, pub y: u16, pub width: u16, pub height: u16 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorInfo { pub visible: bool, pub style: u8 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsData { pub cpu: f64, pub memory: f64, pub battery: Option<f64> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteData { pub id: String, pub title: String, pub content: String, pub created_at: u64, pub updated_at: u64 }
```

**Step 2: Roundtrip serialization tests, commit**

```bash
git commit -m "feat(rust): define IPC message protocol types with serde"
```

---

### Task 5.2: Transport Layer

**Files:**
- Create: `rust/crates/maxmux-ipc/src/transport.rs`

**Context:** JSON-lines codec over Unix sockets. Each message is one JSON object + newline.

**Step 1: Implement codec + transport**

```rust
// crates/maxmux-ipc/src/transport.rs
use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use crate::protocol::{ClientMessage, ServerMessage};

pub struct Connection {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: BufWriter<tokio::net::unix::OwnedWriteHalf>,
}

impl Connection {
    pub fn new(stream: UnixStream) -> Self {
        let (read, write) = stream.into_split();
        Self {
            reader: BufReader::new(read),
            writer: BufWriter::new(write),
        }
    }

    pub async fn read_client_message(&mut self) -> Option<ClientMessage> {
        let mut line = String::new();
        match self.reader.read_line(&mut line).await {
            Ok(0) => None, // EOF
            Ok(_) => serde_json::from_str(line.trim()).ok(),
            Err(_) => None,
        }
    }

    pub async fn read_server_message(&mut self) -> Option<ServerMessage> {
        let mut line = String::new();
        match self.reader.read_line(&mut line).await {
            Ok(0) => None,
            Ok(_) => serde_json::from_str(line.trim()).ok(),
            Err(_) => None,
        }
    }

    pub async fn send<T: serde::Serialize>(&mut self, msg: &T) -> std::io::Result<()> {
        let json = serde_json::to_string(msg)?;
        self.writer.write_all(json.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }
}
```

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement JSON-lines transport over Unix sockets"
```

---

## Phase 6: Server + Client (First Working Multiplexer)

### Task 6.1: Server Daemon

**Files:**
- Create: `rust/crates/maxmux/Cargo.toml`
- Create: `rust/crates/maxmux/src/main.rs`
- Create: `rust/crates/maxmux/src/server/mod.rs`
- Create: `rust/crates/maxmux/src/server/daemon.rs`

**Context:** Port `src/server/daemon.ts`. Unix socket listener at `~/.maxmux/server.sock`, PID file management, signal handling.

**Step 1: Create binary crate with clap CLI**

```toml
# crates/maxmux/Cargo.toml
[package]
name = "maxmux"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "maxmux"
path = "src/main.rs"

[dependencies]
maxmux-core = { path = "../maxmux-core" }
maxmux-renderer = { path = "../maxmux-renderer" }
maxmux-input = { path = "../maxmux-input" }
maxmux-ipc = { path = "../maxmux-ipc" }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
clap = { version = "4", features = ["derive"] }
crossterm = "0.28"
nix = { version = "0.29", features = ["process", "signal"] }
dirs = "6"
```

```rust
// crates/maxmux/src/main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "maxmux", version, about = "A modern terminal multiplexer")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Attach to a session
    Attach { session: Option<String> },
    /// Create a new session
    NewSession { #[arg(short, long)] name: Option<String> },
    /// List sessions
    #[command(alias = "ls")]
    ListSessions,
    /// Kill the server
    KillServer,
    // Internal: run as server daemon
    #[command(hide = true)]
    Server,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::init();
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Server) => server::daemon::run().await,
        Some(Commands::Attach { session }) => { /* client attach */ }
        Some(Commands::ListSessions) => { /* list sessions */ }
        Some(Commands::KillServer) => { /* kill server */ }
        None => {
            // Default: ensure server running, then attach
            server::daemon::ensure_running().await;
            // client::attach()
        }
        _ => {}
    }
}

mod server;
mod client;
```

**Step 2: Implement daemon with socket listener**

```rust
// crates/maxmux/src/server/daemon.rs
use tokio::net::UnixListener;
use std::path::PathBuf;

pub fn socket_path() -> PathBuf {
    let dir = dirs::home_dir().unwrap().join(".maxmux");
    std::fs::create_dir_all(&dir).ok();
    dir.join("server.sock")
}

pub fn pid_path() -> PathBuf {
    dirs::home_dir().unwrap().join(".maxmux").join("server.pid")
}

pub async fn is_running() -> bool {
    tokio::net::UnixStream::connect(socket_path()).await.is_ok()
}

pub async fn ensure_running() {
    if !is_running().await {
        // Spawn detached server process
        std::process::Command::new(std::env::current_exe().unwrap())
            .arg("server")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("Failed to start server");

        // Wait for socket
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if is_running().await { return; }
        }
        eprintln!("Server failed to start");
        std::process::exit(1);
    }
}

pub async fn run() {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);

    // Write PID file
    std::fs::write(pid_path(), std::process::id().to_string()).ok();

    let listener = UnixListener::bind(&path).expect("Failed to bind socket");
    tracing::info!("Server listening on {:?}", path);

    // Signal handling
    let mut sigterm = tokio::signal::unix::signal(
        tokio::signal::unix::SignalKind::terminate()
    ).unwrap();

    loop {
        tokio::select! {
            Ok((stream, _)) = listener.accept() => {
                // Spawn handler task per client
                tokio::spawn(async move {
                    // handler::handle_client(stream).await
                });
            }
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM, shutting down");
                break;
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received SIGINT, shutting down");
                break;
            }
        }
    }

    // Cleanup
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(pid_path());
}
```

**Step 3: Commit**

```bash
git commit -m "feat(rust): implement server daemon with Unix socket listener"
```

---

### Task 6.2: Server Handler

**Files:**
- Create: `rust/crates/maxmux/src/server/handler.rs`
- Create: `rust/crates/maxmux/src/server/broadcast.rs`

**Context:** Port `src/server/handler.ts` (the 2000-line beast) and `src/server/broadcast.ts`. This is the core server logic. Start minimal: attach, input, resize, basic commands (window:create, pane:split, pane:focus).

**Step 1: Implement shared server state**

```rust
// crates/maxmux/src/server/handler.rs
use std::sync::Arc;
use tokio::sync::Mutex;
use maxmux_core::session::SessionManager;
use maxmux_core::pty::PtyManager;
use maxmux_core::terminal::TerminalManager;
use maxmux_core::command::CommandRegistry;

pub struct ServerState {
    pub sessions: SessionManager,
    pub ptys: PtyManager,
    pub terminals: TerminalManager,
    pub commands: CommandRegistry,
}

pub type SharedState = Arc<Mutex<ServerState>>;
```

**Step 2: Implement client handler loop**

Wire up: attach → create default session+window+pane if needed, input → write to PTY, resize → recalculate layout, command dispatch.

**Step 3: Implement broadcast**

```rust
// crates/maxmux/src/server/broadcast.rs
use tokio::sync::mpsc;
use maxmux_ipc::protocol::ServerMessage;
use std::collections::HashMap;

pub struct Broadcaster {
    clients: HashMap<String, mpsc::UnboundedSender<ServerMessage>>,
    client_sessions: HashMap<String, String>,
}

impl Broadcaster {
    pub fn new() -> Self { /* ... */ }
    pub fn add_client(&mut self, id: String, tx: mpsc::UnboundedSender<ServerMessage>) { /* ... */ }
    pub fn remove_client(&mut self, id: &str) { /* ... */ }
    pub fn send(&self, client_id: &str, msg: ServerMessage) { /* ... */ }
    pub fn send_to_session(&self, session_id: &str, msg: ServerMessage) { /* ... */ }
}
```

**Step 4: Register core commands**

Register: `window:create`, `window:next`, `window:previous`, `window:close`, `pane:split-horizontal`, `pane:split-vertical`, `pane:focus-up/down/left/right`, `pane:close`, `pane:zoom`, `session:create`, `session:detach`, `server:kill`

**Step 5: Commit**

```bash
git commit -m "feat(rust): implement ServerHandler with core commands and Broadcaster"
```

---

### Task 6.3: Client Attach

**Files:**
- Create: `rust/crates/maxmux/src/client/mod.rs`
- Create: `rust/crates/maxmux/src/client/attach.rs`

**Context:** Port `src/client/attach.ts`. Connect to server, enter raw mode, handle input→server and server→render loop.

**Step 1: Implement attach loop**

```rust
// crates/maxmux/src/client/attach.rs
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
use crossterm::event::{self, Event};
use tokio::net::UnixStream;
use maxmux_ipc::transport::Connection;
use maxmux_ipc::protocol::{ClientMessage, ServerMessage};
use maxmux_renderer::compositor::Compositor;
use maxmux_input::router::InputRouter;

pub async fn attach(session_name: Option<String>) -> std::io::Result<()> {
    let stream = UnixStream::connect(super::server::daemon::socket_path()).await?;
    let mut conn = Connection::new(stream);

    // Enter raw mode + alt screen
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    // write alt screen + hide cursor + enable mouse

    let (cols, rows) = crossterm::terminal::size()?;

    // Send attach message
    conn.send(&ClientMessage::Attach {
        session_id: session_name,
        cwd: std::env::current_dir().ok().map(|p| p.to_string_lossy().into()),
    }).await?;

    // Send initial resize
    conn.send(&ClientMessage::Resize { cols, rows }).await?;

    let mut compositor = Compositor::new(cols, rows);
    let mut input_router = InputRouter::new("C-a", 0);
    // ... load default keybindings

    // Main event loop: read stdin + read socket concurrently
    // On stdin: route through InputRouter, send commands/input to server
    // On server message: update state, re-render

    // Cleanup
    disable_raw_mode()?;
    // exit alt screen + show cursor + disable mouse
    Ok(())
}
```

**Step 2: Wire up the event loop with tokio::select!**

Two concurrent tasks:
1. `stdin_reader`: reads raw bytes, feeds to InputRouter
2. `socket_reader`: reads ServerMessages, updates compositor

**Step 3: Integration test - start server, attach client, verify output**

**Step 4: Commit**

```bash
git commit -m "feat(rust): implement client attach with raw mode and event loop"
```

---

### Task 6.4: Wire Everything Together

**Files:**
- Modify: `rust/crates/maxmux/src/main.rs`

**Context:** Connect all CLI commands to implementations. This is when the binary becomes usable.

**Step 1: Wire up main.rs dispatch**

**Step 2: Manual smoke test**

Run: `cd rust && cargo build && ./target/debug/maxmux`
Expected: Server starts, client attaches, shell prompt appears, can type commands, Ctrl+a c creates new window, Ctrl+a % splits pane.

**Step 3: Commit**

```bash
git commit -m "feat(rust): wire up CLI dispatcher - first working multiplexer"
```

---

## Phase 7: Configuration

### Task 7.1: Config Schema & Loader

**Files:**
- Create: `rust/crates/maxmux-config/Cargo.toml`
- Create: `rust/crates/maxmux-config/src/lib.rs`
- Create: `rust/crates/maxmux-config/src/schema.rs`
- Create: `rust/crates/maxmux-config/src/loader.rs`

**Context:** Port `src/config/schema.ts` and `src/config/loader.ts`. TOML config with serde deserialization and defaults.

**Step 1: Define config struct with serde defaults**

All fields from the TS config schema, with `#[serde(default)]` for each.

**Step 2: Implement loader**

Search: `~/.config/maxmux/config.toml`, then CWD `./maxmux.toml`. Parse with `toml` crate, validate, merge with defaults.

**Step 3: Tests, commit**

```bash
git commit -m "feat(rust): implement TOML config schema and loader"
```

---

### Task 7.2: Config File Watcher

**Files:**
- Create: `rust/crates/maxmux-config/src/watcher.rs`

**Context:** Port `src/config/watcher.ts`. Use `notify` crate for filesystem watching.

**Step 1: Implement watcher with debounce**

```rust
use notify::{Watcher, RecursiveMode, Event};
use tokio::sync::mpsc;

pub async fn watch_config(
    path: std::path::PathBuf,
    tx: mpsc::UnboundedSender<()>,  // signals config change
) {
    // notify watcher with debounce
}
```

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement config file watcher with live reload"
```

---

## Phase 8: Status Bar

### Task 8.1: Status Bar Renderer & Themes

**Files:**
- Create: `rust/crates/maxmux-statusbar/Cargo.toml`
- Create: `rust/crates/maxmux-statusbar/src/lib.rs`
- Create: `rust/crates/maxmux-statusbar/src/renderer.rs`
- Create: `rust/crates/maxmux-statusbar/src/themes/mod.rs`
- Create: `rust/crates/maxmux-statusbar/src/themes/catppuccin.rs` (+ 6 more)

**Context:** Port `src/statusbar/renderer.ts` and `src/statusbar/themes/`. 7 built-in themes, powerline/rounded/flat separators.

**Step 1: Define theme types and implement all 7 themes**

**Step 2: Implement status bar renderer that produces a single ANSI line**

**Step 3: Tests, commit**

```bash
git commit -m "feat(rust): implement status bar renderer with 7 themes"
```

---

### Task 8.2: Status Bar Modules

**Files:**
- Create: `rust/crates/maxmux-statusbar/src/modules/mod.rs`
- Create: `rust/crates/maxmux-statusbar/src/modules/session.rs` (+ 14 more)

**Context:** Port all 15 status bar modules from `src/statusbar/modules/`. Each module renders a short text segment.

**Step 1: Define module trait and implement all 15 modules**

Modules: session, windows, cwd, git, cpu, ram, battery, network, datetime, hostname, user, notes, pane_info, prefix, custom

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement all 15 status bar modules"
```

---

## Phase 9: Persistence

### Task 9.1: Session Persistence

**Files:**
- Create: `rust/crates/maxmux-persistence/Cargo.toml`
- Create: `rust/crates/maxmux-persistence/src/lib.rs`
- Create: `rust/crates/maxmux-persistence/src/store.rs`
- Create: `rust/crates/maxmux-persistence/src/autosave.rs`

**Context:** Port `src/persistence/store.ts` and `src/persistence/autosave.ts`. JSON file for session structure, interval-based autosave.

**Step 1: Implement save/load**

Save to `~/.maxmux/sessions/sessions.json`. Load on startup. Autosave every N seconds (configurable, default 30).

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement session persistence with autosave"
```

---

### Task 9.2: Notes Database

**Files:**
- Create: `rust/crates/maxmux-persistence/src/notes.rs`

**Context:** Port `src/persistence/notes-db.ts`. SQLite database at `~/.maxmux/notes.db` with WAL mode.

**Step 1: Implement CRUD with rusqlite**

```rust
use rusqlite::{Connection, params};

pub struct NotesDb { conn: Connection }

impl NotesDb {
    pub fn open() -> Self {
        let path = dirs::home_dir().unwrap().join(".maxmux").join("notes.db");
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )", []
        ).unwrap();
        Self { conn }
    }

    pub fn list(&self) -> Vec<Note> { /* ... */ }
    pub fn save(&self, note: &Note) { /* ... */ }
    pub fn delete(&self, id: &str) { /* ... */ }
}
```

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement SQLite notes database"
```

---

## Phase 10: Lua Plugin System

### Task 10.1: Hook Registry & Lua Runtime

**Files:**
- Create: `rust/crates/maxmux-plugins/Cargo.toml`
- Create: `rust/crates/maxmux-plugins/src/lib.rs`
- Create: `rust/crates/maxmux-plugins/src/hooks.rs`
- Create: `rust/crates/maxmux-plugins/src/lua.rs`

**Context:** Port `src/plugins/hooks.ts` hook pattern. Add Lua scripting via `mlua` with LuaJIT backend.

**Step 1: Implement hook registry (waterfall pattern)**

```rust
// Hooks: on_session_created, on_window_created, on_pane_created, etc.
// Waterfall: on_render_statusbar (transform data through handler chain)
```

**Step 2: Implement Lua runtime with API surface**

Expose to Lua: `maxmux.command()`, `maxmux.keybind()`, `maxmux.on()`, `maxmux.sessions()`, etc.

**Step 3: Implement plugin loader**

Load `.lua` files from `~/.config/maxmux/plugins/`

**Step 4: Tests, commit**

```bash
git commit -m "feat(rust): implement Lua plugin system with hook registry"
```

---

## Phase 11: UI Overlays

### Task 11.1: Copy Mode

**Files:**
- Create: `rust/crates/maxmux/src/client/copy_mode.rs`

**Context:** Port `src/client/copy-mode.ts`. Vi-style scrollback navigation with visual selection and yank.

**Step 1: Implement copy mode state machine**

Navigate (hjkl, w/b, 0/$, g/G, C-u/C-d) → Visual char/line (v/V) → Yank (y)

**Step 2: Implement rendering (highlighted selection overlay)**

**Step 3: Tests, commit**

```bash
git commit -m "feat(rust): implement vi-style copy mode"
```

---

### Task 11.2: Session Finder (Fuzzy Search)

**Files:**
- Create: `rust/crates/maxmux/src/client/session_finder.rs`

**Context:** Port `src/ui/SessionFinder.ts`. Fuzzy search over sessions using `nucleo`.

**Step 1: Implement fuzzy search overlay**

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement fuzzy session finder with nucleo"
```

---

### Task 11.3: Command Palette

**Files:**
- Create: `rust/crates/maxmux/src/client/command_palette.rs`

**Context:** Port `src/ui/CommandPalette.ts`. Fuzzy search over all registered commands.

**Step 1: Implement command palette overlay**

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement command palette"
```

---

### Task 11.4: Note Editor & Notes List

**Files:**
- Create: `rust/crates/maxmux/src/client/note_editor.rs`
- Create: `rust/crates/maxmux/src/client/notes_list.rs`

**Context:** Port `src/ui/NoteEditor.ts` and `src/ui/NotesList.ts`.

**Step 1: Implement note editor overlay (text input + save)**

**Step 2: Implement notes list overlay (search + select + delete)**

**Step 3: Tests, commit**

```bash
git commit -m "feat(rust): implement notes editor and list overlays"
```

---

### Task 11.5: Other UI Components

**Files:**
- Create: `rust/crates/maxmux/src/client/prefix_help.rs`
- Create: `rust/crates/maxmux/src/client/rename_dialog.rs`
- Create: `rust/crates/maxmux/src/client/session_sidebar.rs`

**Context:** Port `PrefixHelp.ts`, `RenameDialog.ts`, `SessionSidebar.ts`.

**Step 1: Implement all three overlays**

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement prefix help, rename dialog, and session sidebar"
```

---

## Phase 12: Polish & Feature Parity

### Task 12.1: Process Tracker

**Files:**
- Create: `rust/crates/maxmux/src/server/process_tracker.rs`

**Context:** Port `src/server/process-tracker.ts`. Read `/proc/[pid]/stat`, `/proc/[pid]/cmdline`, `/proc/[pid]/cwd` for dynamic window titles and CWD tracking.

**Step 1: Implement with tokio interval (1s default)**

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement process tracker for dynamic titles"
```

---

### Task 12.2: Bracketed Paste Relay

**Files:**
- Modify: `rust/crates/maxmux/src/client/attach.rs`

**Context:** Port the bracketed paste relay from commit `1e326ba`. Detect `CSI ? 2004 h/l` in pane output, relay to outer terminal.

**Step 1: Add detection in output handler**

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): relay bracketed paste mode from inner app"
```

---

### Task 12.3: Mouse Text Selection

**Files:**
- Create: `rust/crates/maxmux/src/client/selection.rs`

**Context:** Port `src/client/selection.ts`. Drag to select text, copy to clipboard.

**Step 1: Implement mouse drag selection state machine**

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement mouse text selection"
```

---

### Task 12.4: System Metrics

**Files:**
- Create: `rust/crates/maxmux/src/server/metrics.rs`

**Context:** Port `src/server/metrics.ts`. CPU, RAM, battery, network status for status bar modules.

**Step 1: Implement metrics collection (read /proc/stat, /proc/meminfo, etc.)**

**Step 2: Tests, commit**

```bash
git commit -m "feat(rust): implement system metrics collection"
```

---

### Task 12.5: Remote CLI Commands

**Files:**
- Modify: `rust/crates/maxmux/src/main.rs`
- Modify: `rust/crates/maxmux/src/server/handler.rs`

**Context:** Port the remote CLI commands from `src/client/cli.ts` and `src/index.ts`. Commands like `maxmux select-pane -L`, `maxmux split-window -h`, `maxmux display-message -p '#{session_name}'`.

**Step 1: Add CLI subcommands for remote control**

**Step 2: Implement format variable resolution (#{session_name}, etc.)**

**Step 3: Tests, commit**

```bash
git commit -m "feat(rust): implement remote CLI commands"
```

---

### Task 12.6: Final Integration & Smoke Testing

**Step 1: Full feature comparison against TypeScript version**

Create checklist:
- [ ] Multiple sessions
- [ ] Multiple windows per session
- [ ] Pane splitting (horizontal/vertical)
- [ ] Pane navigation (directional + next)
- [ ] Pane zoom
- [ ] Copy mode (vi-style)
- [ ] Mouse click to focus pane
- [ ] Mouse drag to select text
- [ ] Session finder (fuzzy)
- [ ] Command palette
- [ ] Notes (create, list, search, delete)
- [ ] Status bar with all modules
- [ ] All 7 themes
- [ ] Config file (TOML)
- [ ] Live config reload
- [ ] Session persistence + autosave
- [ ] Process tracking / dynamic titles
- [ ] Bracketed paste relay
- [ ] Remote CLI commands
- [ ] Lua plugins
- [ ] Proper cleanup on exit

**Step 2: Fix any discrepancies**

**Step 3: Commit**

```bash
git commit -m "feat(rust): complete feature parity with TypeScript version"
```
