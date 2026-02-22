<project>
  <name>MaxMux</name>
  <description>Modern terminal session manager (tmux alternative) with TypeScript config, plugin system, and auto-save</description>
</project>

<runtime>
  <tool>Bun</tool>
  <language>TypeScript (strict)</language>
  <commands>
    <run>bun run src/index.ts</run>
    <typecheck>npx tsc --noEmit</typecheck>
    <test>bun test</test>
    <install>bun install</install>
  </commands>
  <rules>
    - Always use `bun` instead of `node`/`ts-node`/`npm`/`npx`
    - Bun loads .env automatically — no dotenv
    - Use `Bun.file` over `node:fs` readFile/writeFile where possible
  </rules>
</runtime>

<architecture>
  <pattern>Client/Server over Unix Domain Socket (~/.maxmux/server.sock)</pattern>
  <protocol>Newline-delimited JSON messages</protocol>

  <server purpose="Daemon process — survives client disconnect">
    - PTY lifecycle (spawn, I/O, kill) via node-pty
    - Session/Window/Pane state management
    - Command execution + Plugin hooks
    - Auto-save sessions to ~/.maxmux/sessions/
    - Broadcasts state/output to connected clients
    - 64KB output ring buffer per pane (paneOutputBuffer) for replay on window switch / reattach
    - Cork/uncork batching on sendStateToClient to ensure atomic message delivery
  </server>

  <client purpose="Attaches to server, renders terminal UI">
    - Raw mode stdin, ANSI rendering to stdout
    - Two-layer input routing: global keybindings (no prefix) → prefix-key keybindings → passthrough to PTY
    - Compositor: client-side VirtualTerminal per pane + border rendering + status bar → screen
    - Async-safe rendering via pendingWrites counter (waits for xterm.js write callbacks before render)
    - UI overlays: session list, session finder (fuzzy), rename dialog, keybinding help
  </client>
</architecture>

<structure>
  <entrypoint>src/index.ts</entrypoint>

  <module path="src/core/" purpose="Shared kernel">
    <file name="session.ts" exports="Pane, Window, Session, LayoutNode, SessionManager">Session tree: Session → Window[] → Pane[]. LayoutNode is binary tree (leaf | split).</file>
    <file name="layout.ts" exports="Rect, calculateLayout, splitLayout, removeFromLayout, findPaneInDirection, getAllPaneIds">Binary-tree layout engine. calculateLayout maps LayoutNode → Map&lt;paneId, Rect&gt;.</file>
    <file name="pty.ts" exports="PtyHandle, PtyManager">PTY spawn/write/resize/kill via node-pty.</file>
    <file name="terminal.ts" exports="VirtualTerminal, TerminalManager">Headless xterm buffers per pane. write(data, onProcessed?) is async — callback fires after xterm parses data. readLines() returns string[], renderLine(row) returns ANSI-escaped string with full color/attribute support.</file>
    <file name="command.ts" exports="CommandContext, Command, CommandRegistry">Command registry. Commands have id (e.g. "window:create") and execute(ctx).</file>
  </module>

  <module path="src/server/" purpose="Server daemon">
    <file name="daemon.ts" exports="getSocketPath, getPidPath, isServerRunning, startServer, startServerDaemon">Unix socket listener. startServerDaemon forks detached child process.</file>
    <file name="handler.ts" exports="ClientMessage, ServerHandler">Central orchestrator. Owns SessionManager, PtyManager, TerminalManager, CommandRegistry, Broadcaster. Registers all default commands. sendStateToClient() uses cork/uncork to batch state+layout+output replay.</file>
    <file name="broadcast.ts" exports="ServerMessage, Broadcaster">Routes messages to clients. Tracks clientId → sessionId mapping. cork()/uncork() for batched writes. notifyShutdown() for clean server exit.</file>
  </module>

  <module path="src/client/" purpose="Client process">
    <file name="attach.ts" exports="attachToSession">Main client loop. Compositor-based rendering: client-side TerminalManager, renderPaneContent(), renderBorders() (layout-tree-based with junction chars), positionCursor(). pendingWrites counter ensures render waits for xterm.js async write completion. Handles overlays (session list, finder, rename dialog, keybinding help).</file>
    <file name="connection.ts" exports="ServerConnection">Unix socket client wrapper with newline-delimited JSON parsing.</file>
    <file name="cli.ts" exports="listSessions, killSession, killServer">Non-interactive CLI commands (ls, kill-session, kill-server).</file>
  </module>

  <module path="src/input/" purpose="Input system (client-side)">
    <file name="router.ts" exports="InputAction, parsePrefixKey, InputRouter">Two-layer input state machine. Priority: prefix key → global keybindings → passthrough. parseGlobalKeyName() handles Ctrl combos (C-a..C-z = bytes 0x01-0x1a), arrows, printable chars. parseKeyName() for prefix mode (arrows + printable only).</file>
    <file name="keybindings.ts" exports="KeybindingRegistry">Map&lt;key, commandId&gt;. Loaded from config. Used for both prefix and global keybinding registries.</file>
    <file name="defaults.ts" exports="DEFAULT_KEYBINDINGS, DEFAULT_GLOBAL_KEYBINDINGS">tmux-compatible default bindings.</file>
  </module>

  <module path="src/renderer/" purpose="Screen rendering (client-side)">
    <file name="compositor.ts" exports="Compositor">Composes pane buffers + borders + status bar into screen output. Diff-based rendering.</file>
    <file name="screen.ts" exports="ScreenCell, ScreenBuffer">2D grid of cells with snapshot/getDirty for incremental updates.</file>
    <file name="ansi.ts">ANSI escape helpers: cursor, colors (hex→RGB), alt screen, styles.</file>
    <file name="border.ts" exports="BorderStyle, BorderChars, getBorderChars">Border character sets: rounded, sharp, double, none. Includes junction chars: teeLeft(├), teeRight(┤), teeTop(┬), teeBottom(┴), cross(┼).</file>
  </module>

  <module path="src/config/" purpose="Configuration system">
    <file name="schema.ts" exports="ConfigSchema, MaxMuxConfig, defineConfig">Zod v4 schema (import from "zod/v4"). defineConfig() validates user config. Includes keybindings + globalKeybindings fields.</file>
    <file name="loader.ts" exports="loadConfig">Searches ./maxmux.config.ts → ~/.config/maxmux/maxmux.config.ts. Merges with defaults.</file>
    <file name="defaults.ts" exports="DEFAULT_KEYBINDINGS, DEFAULT_GLOBAL_KEYBINDINGS, DEFAULT_CONFIG">Default keybindings (tmux-style) + global keybindings (empty) + full default config (Catppuccin Mocha theme).</file>
  </module>

  <module path="src/plugins/" purpose="Plugin system">
    <file name="types.ts" exports="StatusBarItem, PluginEvents, PluginContext, MaxMuxPlugin">Plugin interface: setup(ctx) receives commands, keybindings, event hooks.</file>
    <file name="hooks.ts" exports="HookRegistry">EventEmitter pattern. emitWaterfall for statusbar items (pipe through handlers).</file>
    <file name="loader.ts" exports="loadPlugins">Iterates config.plugins, calls setup() with PluginContext.</file>
  </module>

  <module path="src/persistence/" purpose="Session persistence">
    <file name="store.ts" exports="saveSession, loadSavedSessions">Serialize/deserialize sessions to ~/.maxmux/sessions/sessions.json.</file>
    <file name="autosave.ts" exports="AutoSaver">setInterval-based auto-save (default 30s).</file>
  </module>

  <module path="src/ui/" purpose="UI overlay components">
    <file name="components.ts" exports="renderBox, renderText, renderList">Reusable UI primitives for overlays (box with border, positioned text, selectable list).</file>
    <file name="StatusBar.ts">Status bar formatting helpers.</file>
    <file name="CommandPalette.ts">Command palette overlay state + rendering.</file>
    <file name="SessionPicker.ts">Session picker overlay state + rendering.</file>
    <file name="SessionFinder.ts" exports="SessionFinderEntry, SessionFinderState, createSessionFinderState, fuzzyMatch, updateFilter, renderSessionFinder">Interactive fuzzy session finder overlay. Case-insensitive substring match, keyboard navigation.</file>
    <file name="RenameDialog.ts" exports="RenameDialogState, createRenameDialogState, renderRenameDialog">Text input dialog for renaming sessions. Supports typing, backspace, Ctrl+U clear.</file>
  </module>
</structure>

<commands>
  All commands are identified by namespaced IDs and can be bound to any key (prefix or global).

  <server-commands note="Executed on server, forwarded via IPC">
    window:create, window:next, window:previous, window:close, window:rename
    pane:split-horizontal, pane:split-vertical, pane:next, pane:close, pane:zoom
    pane:focus (args: { paneId }), pane:focus-up, pane:focus-down, pane:focus-left, pane:focus-right
    session:rename (args: { name }), session:detach, session:list
    server:kill
  </server-commands>

  <client-commands note="Handled directly in attach.ts">
    session:detach, server:kill (with client cleanup)
    session:list (static overlay), session:find (fuzzy finder overlay), session:rename (rename dialog)
    keybindings:show, command-palette
    pane:focus-up/down/left/right (resolves target via findPaneInDirection, then sends pane:focus to server)
  </client-commands>
</commands>

<ipc>
  <client-to-server>
    { type: "attach", sessionId?: string, cwd?: string }
    { type: "detach" }
    { type: "input", paneId: string, data: string }  &lt;!-- base64 encoded --&gt;
    { type: "resize", cols: number, rows: number }
    { type: "command", id: string, args?: Record&lt;string, unknown&gt; }
  </client-to-server>
  <server-to-client>
    { type: "output", paneId: string, data: string }
    { type: "state", sessions: [...], activeSession: string }
    { type: "layout", layout: LayoutNode, paneRects: Record&lt;string, Rect&gt; }
    { type: "pane:exited", paneId: string, exitCode: number }
    { type: "metrics", data: SystemMetrics }
    { type: "error", message: string }  &lt;!-- "detached" | "server-shutdown" --&gt;
  </server-to-client>
</ipc>

<keybinding-system>
  Two binding layers, both mapping key names to command IDs:

  1. keybindings (prefix-based): Activated after pressing prefix key (default C-a).
     Key names: printable chars ("c", "%", "$"), arrows ("Up", "Down", "Left", "Right").

  2. globalKeybindings (no prefix): Fire immediately on keypress, checked after prefix key but before passthrough.
     Key names: Ctrl combos ("C-h", "C-j", "C-k", "C-l"), arrows, printable chars.
     WARNING: Global bindings override normal terminal input for those keys.

  Priority order: prefix key detection → prefix mode keybinding → global keybinding → passthrough to PTY.
  Config: keybindings: Record&lt;string, string&gt;, globalKeybindings: Record&lt;string, string&gt;.
</keybinding-system>

<rendering>
  Client-side compositor in attach.ts:
  - Client maintains TerminalManager with one VirtualTerminal per visible pane
  - VirtualTerminal.write(data, callback) is ASYNC — xterm.js processes data asynchronously
  - pendingWrites counter tracks in-flight writes, scheduleRender() retries until pendingWrites === 0
  - renderScreen() composites: renderPaneContent() for each pane → renderBorders() → positionCursor() → drawStatusBar()
  - renderBorders() uses layout-tree traversal (collectBorderCells) to build complete border cell map, then determines junction characters (├┤┬┴┼) by checking 4 neighbors
  - Server uses cork/uncork in sendStateToClient() to batch state+layout+output into one socket write
  - Window switch replays paneOutputBuffer (64KB ring buffer) so terminal content persists across window switches
  - Overlays (showingOverlay flag) block both input passthrough and rendering
</rendering>

<dependencies>
  <dep name="node-pty">PTY management (installed with --ignore-scripts, needs native rebuild for production)</dep>
  <dep name="@xterm/headless">Headless VT100/xterm parser v6. write() is ASYNC — always use callback parameter for render timing.</dep>
  <dep name="zod">Config schema validation — IMPORTANT: import from "zod/v4" (v4 API)</dep>
</dependencies>

<conventions>
  - Linter auto-formats to double quotes + semicolons — follow this style
  - IDs are randomUUID().slice(0, 8) — 8-char hex strings
  - All file imports use .ts extensions (bundler moduleResolution)
  - Commands use namespaced IDs: "window:create", "pane:split-horizontal", "session:detach"
  - Client-side commands (detach, zoom, overlays) are handled in attach.ts, server commands forwarded via IPC
  - Prefix key "C-a" = Ctrl+a = byte 0x01. Parsed by parsePrefixKey() in input/router.ts
  - Global keybindings "C-h" = Ctrl+h = byte 0x08. Parsed by parseGlobalKeyName() in input/router.ts
  - LayoutNode is a recursive binary tree: { type: "leaf", paneId } | { type: "split", direction, ratio, children: [LayoutNode, LayoutNode] }
  - Theme colors are hex strings, converted via fgHex()/bgHex() in renderer/ansi.ts
  - VirtualTerminal.write() is async — NEVER read buffer immediately after write, use the callback
  - Overlays set showingOverlay=true, which blocks stdin passthrough and rendering. Always reset + renderScreen() on close.
</conventions>

<cli>
  maxmux                          # Start server + attach default session
  maxmux new-session [-s name]    # New session
  maxmux attach [-t session]      # Attach to existing
  maxmux ls                       # List sessions
  maxmux kill-session -t name     # Kill session
  maxmux kill-server              # Stop server
</cli>
