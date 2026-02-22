# MaxMux

A modern terminal session manager — like tmux, but built with TypeScript.

```bash
┌─ editor ──────────────────┬─ terminal ─────────────┐
│                           │ $ npm run build        │
│  ~/src/app.ts             │ ✓ Done in 0.42s        │
│                           │ $ █                    │
│                           │                        │
│                           │                        │
├─ logs ────────────────────┴────────────────────────┤
│ [2025-01-15 14:32:01] Server started on :3000      │
│ [2025-01-15 14:32:05] GET /api/users 200 12ms      │
└────────────────────────────────────────────────────┘
 [main] 0:editor* 1:logs                        14:32
```

**Why MaxMux?**

- **TypeScript config** — `maxmux.config.ts` instead of `.tmux.conf`
- **Plugin system** — extend with hooks, not shell scripts
- **Auto-save** — sessions persist and restore automatically
- **Persistent windows** — switch between windows without losing terminal content
- **Flexible keybindings** — prefix-based, global, or both — bind any key to any command
- **Modern stack** — built on Bun, node-pty, and xterm-headless

---

## Installation

### Standalone Binary (recommended)

Download the prebuilt binary for your platform from [GitHub Releases](https://github.com/maxmux-terminal/maxmux/releases/latest):

```bash
# Linux (x64)
curl -fsSL https://github.com/maxmux-terminal/maxmux/releases/latest/download/maxmux-linux-x64 -o maxmux
chmod +x maxmux
sudo mv maxmux /usr/local/bin/

# Linux (ARM64)
curl -fsSL https://github.com/maxmux-terminal/maxmux/releases/latest/download/maxmux-linux-arm64 -o maxmux
chmod +x maxmux
sudo mv maxmux /usr/local/bin/

# macOS (Apple Silicon)
curl -fsSL https://github.com/maxmux-terminal/maxmux/releases/latest/download/maxmux-darwin-arm64 -o maxmux
chmod +x maxmux
sudo mv maxmux /usr/local/bin/

# macOS (Intel)
curl -fsSL https://github.com/maxmux-terminal/maxmux/releases/latest/download/maxmux-darwin-x64 -o maxmux
chmod +x maxmux
sudo mv maxmux /usr/local/bin/
```

### Via npm

```bash
npm install -g maxmux
```

### Via Bun

```bash
bun add -g maxmux
```

### From Source

```bash
git clone https://github.com/maxmux-terminal/maxmux.git
cd maxmux
bun install

# Run directly
bun run src/index.ts

# Or build a standalone binary
bun run build
sudo mv maxmux /usr/local/bin/
```

---

## Quick Start

```bash
# Start MaxMux (launches server + attaches to a new session)
maxmux

# You're now inside a MaxMux session.
# The default prefix key is Ctrl+a.
# Press Ctrl+a, then a key to run a command:

#   Ctrl+a c    → create a new window
#   Ctrl+a n    → next window
#   Ctrl+a %    → split pane horizontally
#   Ctrl+a "    → split pane vertically
#   Ctrl+a d    → detach (return to your normal shell)

# Reattach to the session
maxmux attach
```

---

## Keybindings

All default keybindings use a **prefix key** (default: `Ctrl+a`).
Press the prefix, release, then press the action key.

### Windows

| Keys           | Action            |
| -------------- | ----------------- |
| `prefix` + `c` | Create new window |
| `prefix` + `n` | Next window       |
| `prefix` + `p` | Previous window   |
| `prefix` + `,` | Rename window     |
| `prefix` + `&` | Close window      |

Windows preserve their terminal content — programs like vim keep running in the background when you switch windows, and the full screen is restored when you switch back.

### Panes

| Keys               | Action                          |
| ------------------ | ------------------------------- |
| `prefix` + `%`     | Split horizontally (left/right) |
| `prefix` + `"`     | Split vertically (top/bottom)   |
| `prefix` + `o`     | Cycle to next pane              |
| `prefix` + `x`     | Close current pane              |
| `prefix` + `z`     | Toggle pane zoom (fullscreen)   |
| `prefix` + `Arrow` | Focus pane in direction         |

### Sessions

| Keys           | Action               |
| -------------- | -------------------- |
| `prefix` + `d` | Detach from session  |
| `prefix` + `s` | Session list         |
| `prefix` + `f` | Fuzzy session finder |
| `prefix` + `$` | Rename session       |

### Other

| Keys           | Action               |
| -------------- | -------------------- |
| `prefix` + `Q` | Kill server          |
| `prefix` + `:` | Command palette      |
| `prefix` + `?` | Show all keybindings |

---

## CLI Commands

```bash
maxmux                          # Start server + attach to default session
maxmux new-session [-s name]    # Create a new named session
maxmux attach [-t name]         # Attach to an existing session
maxmux ls                       # List all sessions
maxmux kill-session -t name     # Kill a session
maxmux kill-server              # Stop the MaxMux server
maxmux --help                   # Show help
maxmux --version                # Show version
```

---

## Configuration

Create a `maxmux.config.ts` in your project directory or at `~/.config/maxmux/maxmux.config.ts`:

```typescript
import { defineConfig } from "maxmux";

export default defineConfig({
  // Prefix key (default: Ctrl+a)
  prefixKey: "C-a",

  // Timeout for prefix mode in ms (0 = no timeout)
  prefixTimeout: 0,

  // Default shell
  shell: "/bin/zsh",

  // Theme (Catppuccin Mocha defaults)
  theme: {
    statusBar: {
      bg: "#1e1e2e",
      fg: "#cdd6f4",
      active: "#89b4fa",
    },
    border: {
      style: "rounded", // 'rounded' | 'sharp' | 'double' | 'none'
      fg: "#585b70",
      activeFg: "#89b4fa",
    },
  },

  // Prefix keybindings (prefix + key → command)
  keybindings: {
    c: "window:create",
    n: "window:next",
    p: "window:previous",
    "%": "pane:split-horizontal",
    '"': "pane:split-vertical",
    // ... see examples/maxmux.config.ts for full list
  },

  // Global keybindings (fire immediately, no prefix needed)
  globalKeybindings: {
    // Empty by default — see "Custom Keybindings" below
  },

  // Session persistence
  sessions: {
    autoSave: true,
    autoSaveInterval: 30_000,
    autoRestore: true,
  },

  // Plugins
  plugins: [],
});
```

The config is fully type-safe — your editor provides autocomplete for every option.

---

## Custom Keybindings

MaxMux has a flexible keybinding system. Every command can be bound to any key, in two ways:

### Prefix Keybindings

These require pressing the prefix key first (like tmux). This is the default mode.

```typescript
keybindings: {
  // tmux-style (default): prefix + Arrow
  Up: 'pane:focus-up',
  Down: 'pane:focus-down',
  Left: 'pane:focus-left',
  Right: 'pane:focus-right',

  // Or vim-style: prefix + hjkl
  h: 'pane:focus-left',
  j: 'pane:focus-down',
  k: 'pane:focus-up',
  l: 'pane:focus-right',
}
```

### Global Keybindings

These fire immediately without the prefix key. Useful for frequently used actions.
Supports Ctrl combinations (`C-a` through `C-z`), arrow keys, and regular characters.

```typescript
globalKeybindings: {
  // Vim-style pane navigation without prefix
  'C-h': 'pane:focus-left',
  'C-j': 'pane:focus-down',
  'C-k': 'pane:focus-up',
  'C-l': 'pane:focus-right',
}
```

> **Note:** Global keybindings override normal terminal input for those keys.
> For example, `C-l` normally clears the screen — binding it globally will
> prevent that. Choose keys carefully.

### Available Commands

| Command                 | Description               |
| ----------------------- | ------------------------- |
| `window:create`         | Create a new window       |
| `window:next`           | Switch to next window     |
| `window:previous`       | Switch to previous window |
| `window:rename`         | Rename current window     |
| `window:close`          | Close current window      |
| `pane:split-horizontal` | Split pane left/right     |
| `pane:split-vertical`   | Split pane top/bottom     |
| `pane:next`             | Cycle to next pane        |
| `pane:close`            | Close current pane        |
| `pane:zoom`             | Toggle pane zoom          |
| `pane:focus-up`         | Focus pane above          |
| `pane:focus-down`       | Focus pane below          |
| `pane:focus-left`       | Focus pane to the left    |
| `pane:focus-right`      | Focus pane to the right   |
| `session:list`          | Show session list         |
| `session:find`          | Fuzzy session finder      |
| `session:rename`        | Rename current session    |
| `session:detach`        | Detach from session       |
| `server:kill`           | Kill the MaxMux server    |
| `command-palette`       | Open command palette      |
| `keybindings:show`      | Show all keybindings      |

### Priority

When a key is pressed, MaxMux checks in this order:

1. **Prefix key** — enters prefix mode
2. **Prefix keybinding** (if in prefix mode) — executes bound command
3. **Global keybinding** — executes bound command
4. **Passthrough** — sends input to the terminal

---

## Plugins

Plugins hook into MaxMux lifecycle events to add features:

```typescript
import { defineConfig } from "maxmux";
import type { MaxMuxPlugin } from "maxmux";

function gitBranch(): MaxMuxPlugin {
  return {
    name: "git-branch",
    setup(ctx) {
      ctx.on("render:statusbar", (items) => {
        return [...items, { text: " main", align: "right" }];
      });
    },
  };
}

export default defineConfig({
  plugins: [gitBranch()],
});
```

### Plugin Events

| Event              | Description                                 |
| ------------------ | ------------------------------------------- |
| `session:created`  | A new session was created                   |
| `session:closed`   | A session was closed                        |
| `window:created`   | A new window was created                    |
| `window:closed`    | A window was closed                         |
| `pane:created`     | A new pane was created                      |
| `pane:closed`      | A pane was closed                           |
| `render:statusbar` | Status bar is being rendered (modify items) |
| `config:loaded`    | Configuration was loaded (modify config)    |

---

## Architecture

MaxMux uses a client/server architecture over Unix domain sockets:

```text
Server (~/.maxmux/server.sock)          Clients
┌────────────────────────┐
│  Session Manager       │◄────────── maxmux (attach)
│  PTY Manager           │◄────────── maxmux (attach)
│  Terminal Buffers      │◄────────── maxmux ls (CLI)
│  Plugin System         │
│  Auto-Save             │
└────────────────────────┘
```

- The **server** runs as a background daemon, managing PTY processes, sessions, and state
- Each pane has a **virtual terminal buffer** on both server and client for accurate rendering
- **Clients** connect to render the UI, forward input, and display output
- Sessions and their programs (vim, htop, etc.) survive when clients disconnect — just `maxmux attach` to reattach
- Switching windows replays the terminal buffer, so you see exactly what was on screen

---

## Requirements

- **Linux** or **macOS** (Windows not yet supported)
- When installing from npm/source: **Bun** >= 1.0

The standalone binary has no external dependencies.

---

## License

MIT
