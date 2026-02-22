import type { Socket } from "node:net";
import { randomUUID } from "node:crypto";
import { homedir } from "node:os";
import { debugLog } from "../debug.ts";
import { SessionManager, type Pane } from "../core/session.ts";
import { PtyManager } from "../core/pty.ts";
import { TerminalManager } from "../core/terminal.ts";
import { CommandRegistry, type CommandContext } from "../core/command.ts";
import {
  calculateLayout,
  splitLayout,
  removeFromLayout,
  getAllPaneIds,
  type Rect,
} from "../core/layout.ts";
import { KeybindingRegistry } from "../input/keybindings.ts";
import { HookRegistry } from "../plugins/hooks.ts";
import { loadPlugins } from "../plugins/loader.ts";
import { Broadcaster, type ServerMessage } from "./broadcast.ts";
import type { MaxMuxConfig } from "../config/schema.ts";
import { AutoSaver } from "../persistence/autosave.ts";
import { MetricsCollector } from "./metrics.ts";

export type ClientMessage =
  | { type: "attach"; sessionId?: string; cwd?: string }
  | { type: "detach" }
  | { type: "input"; paneId: string; data: string }
  | { type: "resize"; cols: number; rows: number }
  | { type: "command"; id: string; args?: Record<string, unknown> };

export class ServerHandler {
  readonly sessions: SessionManager;
  readonly ptys: PtyManager;
  readonly terminals: TerminalManager;
  readonly commands: CommandRegistry;
  readonly keybindings: KeybindingRegistry;
  readonly hooks: HookRegistry;
  readonly broadcaster: Broadcaster;
  private config: MaxMuxConfig;
  private clientCols: Map<string, number> = new Map();
  private clientRows: Map<string, number> = new Map();
  private clientCwd: Map<string, string> = new Map();
  private paneOutputBuffer: Map<string, string> = new Map();
  private static readonly OUTPUT_BUFFER_MAX = 64 * 1024; // 64KB per pane
  private autoSaver: AutoSaver | null = null;
  private metricsCollector: MetricsCollector;

  constructor(config: MaxMuxConfig) {
    this.config = config;
    this.sessions = new SessionManager();
    this.ptys = new PtyManager();
    this.terminals = new TerminalManager();
    this.commands = new CommandRegistry();
    this.keybindings = new KeybindingRegistry();
    this.hooks = new HookRegistry();
    this.broadcaster = new Broadcaster();

    this.metricsCollector = new MetricsCollector();
    this.keybindings.loadFromConfig(config.keybindings);
    this.registerDefaultCommands();
  }

  async init(): Promise<void> {
    // Load plugins
    await loadPlugins(this.config, this.commands, this.keybindings, this.hooks);

    // Start auto-save
    if (this.config.sessions.autoSave) {
      this.autoSaver = new AutoSaver(
        this.sessions,
        this.config.sessions.autoSaveInterval,
        this.config.sessions.savePath,
      );
      this.autoSaver.start();
    }

    // Start metrics collection
    const metricsInterval = this.config.statusBar.metricsInterval;
    this.metricsCollector.start(
      2000,
      Math.max(metricsInterval, 5000),
      (metrics) => {
        this.broadcaster.broadcast({ type: "metrics", data: metrics });
      },
    );
  }

  handleConnection(socket: Socket): string {
    const clientId = randomUUID().slice(0, 8);
    this.broadcaster.addClient(clientId, socket);
    return clientId;
  }

  handleDisconnect(clientId: string): void {
    const sessionId = this.broadcaster.getClientSession(clientId);
    if (sessionId) {
      const session = this.sessions.getSession(sessionId);
      if (session) {
        session.attachedClients = session.attachedClients.filter(
          (c) => c !== clientId,
        );
      }
    }
    this.broadcaster.removeClient(clientId);
    this.clientCols.delete(clientId);
    this.clientRows.delete(clientId);
    this.clientCwd.delete(clientId);
  }

  handleMessage(clientId: string, msg: ClientMessage): void {
    switch (msg.type) {
      case "attach":
        if (msg.cwd) this.clientCwd.set(clientId, msg.cwd);
        this.handleAttach(clientId, msg.sessionId);
        break;
      case "detach":
        this.handleDetach(clientId);
        break;
      case "input":
        this.handleInput(msg.paneId, msg.data);
        break;
      case "resize":
        this.handleResize(clientId, msg.cols, msg.rows);
        break;
      case "command":
        this.handleCommand(clientId, msg.id, msg.args);
        break;
    }
  }

  private handleAttach(clientId: string, sessionId?: string): void {
    let session;

    if (sessionId) {
      session =
        this.sessions.getSession(sessionId) ||
        this.sessions.getSessionByName(sessionId);
    }

    if (!session) {
      session = this.sessions.getDefaultSession();
    }

    if (!session) {
      // Create a default session
      session = this.sessions.createSession("main");
      this.hooks.emit("session:created", session);
    }

    session.attachedClients.push(clientId);
    this.broadcaster.setClientSession(clientId, session.id);

    // If session has no windows, create one
    if (session.windows.length === 0) {
      const cols = this.clientCols.get(clientId) || 80;
      const rows = this.clientRows.get(clientId) || 24;
      this.createWindowWithPane(session.id, cols, rows);
    }

    // Send current state (sets activePaneId on client)
    this.sendStateToClient(clientId);

    // Replay buffered output for all panes in active window
    // This restores the terminal content that was visible before detach
    const activeWindow = this.sessions.getActiveWindow(session.id);
    if (activeWindow) {
      for (const pane of activeWindow.panes) {
        const buffered = this.paneOutputBuffer.get(pane.id);
        if (buffered) {
          this.broadcaster.send(clientId, {
            type: "output",
            paneId: pane.id,
            data: buffered,
          });
        }
      }
    }

    // Update metrics with cwd and pane info
    const clientCwd = this.clientCwd.get(clientId);
    if (clientCwd) {
      this.metricsCollector.setCwd(clientCwd);
    }
    if (activeWindow) {
      this.metricsCollector.setPaneInfo(
        activeWindow.name,
        activeWindow.panes.length,
      );
    }

    // Send current metrics to newly attached client
    this.broadcaster.send(clientId, {
      type: "metrics",
      data: this.metricsCollector.getMetrics(),
    });

    // Force PTY redraw: send SIGWINCH to all panes in active window
    // This makes the shell redraw its prompt at the correct position
    const cols = this.clientCols.get(clientId) || 80;
    const rows = this.clientRows.get(clientId) || 24;
    this.handleResize(clientId, cols, rows);
  }

  private handleDetach(clientId: string): void {
    const sessionId = this.broadcaster.getClientSession(clientId);
    if (sessionId) {
      const session = this.sessions.getSession(sessionId);
      if (session) {
        session.attachedClients = session.attachedClients.filter(
          (c) => c !== clientId,
        );
      }
    }
    this.broadcaster.send(clientId, { type: "error", message: "detached" });
  }

  private handleInput(paneId: string, data: string): void {
    // Data arrives base64-encoded from the client
    const decoded = Buffer.from(data, "base64").toString("binary");
    this.ptys.write(paneId, decoded);
  }

  private handleResize(clientId: string, cols: number, rows: number): void {
    this.clientCols.set(clientId, cols);
    this.clientRows.set(clientId, rows);

    const sessionId = this.broadcaster.getClientSession(clientId);
    if (!sessionId) return;

    const window = this.sessions.getActiveWindow(sessionId);
    if (!window) return;

    // Recalculate layout and resize all PTYs in the window
    const paneRects = calculateLayout(window.layout, {
      x: 0,
      y: 0,
      width: cols,
      height: rows - 1, // -1 for status bar
    });

    for (const [paneId, rect] of paneRects) {
      const paneRows = Math.max(1, rect.height);
      const paneCols = Math.max(1, rect.width);
      this.ptys.resize(paneId, paneCols, paneRows);
      this.terminals.resize(paneId, paneCols, paneRows);
    }

    this.sendLayoutToClient(clientId, window.layout, paneRects);
  }

  private handleCommand(
    clientId: string,
    commandId: string,
    args?: Record<string, unknown>,
  ): void {
    const sessionId = this.broadcaster.getClientSession(clientId);
    if (!sessionId) return;

    const window = this.sessions.getActiveWindow(sessionId);
    const ctx: CommandContext = {
      sessionId,
      windowId: window?.id,
      paneId: window?.activePane,
      args,
    };

    try {
      this.commands.execute(commandId, ctx);
    } catch (err) {
      this.broadcaster.send(clientId, {
        type: "error",
        message: `Command failed: ${err}`,
      });
    }
  }

  private createWindowWithPane(
    sessionId: string,
    cols: number,
    rows: number,
    name?: string,
  ): void {
    const window = this.sessions.addWindow(sessionId, name);
    if (!window) return;

    const paneId = getAllPaneIds(window.layout)[0]!;
    this.spawnPaneProcess(sessionId, window.id, paneId, cols, rows - 1);

    this.hooks.emit("window:created", window);
    this.broadcastState(sessionId);
  }

  private spawnPaneProcess(
    sessionId: string,
    windowId: string,
    paneId: string,
    cols: number,
    rows: number,
  ): void {
    // Use the cwd of the first attached client, fall back to HOME
    let cwd = process.env.HOME || homedir();
    const session = this.sessions.getSession(sessionId);
    if (session) {
      for (const cid of session.attachedClients) {
        const clientCwd = this.clientCwd.get(cid);
        if (clientCwd) {
          cwd = clientCwd;
          break;
        }
      }
    }
    const safeCols = Math.max(1, cols);
    const safeRows = Math.max(1, rows);

    debugLog(
      "server",
      `spawnPaneProcess pane=${paneId} shell=${this.config.shell} cwd=${cwd} ${safeCols}x${safeRows}`,
    );

    // Create virtual terminal
    this.terminals.create(paneId, safeCols, safeRows);

    // Spawn PTY
    this.ptys.spawn(
      paneId,
      this.config.shell,
      cwd,
      safeCols,
      safeRows,
      (data: string) => {
        debugLog(
          "server",
          `pty output pane=${paneId} len=${data.length}: ${JSON.stringify(data.slice(0, 80))}`,
        );
        // Write to virtual terminal
        this.terminals.write(paneId, data);
        // Append to output ring buffer
        const existing = this.paneOutputBuffer.get(paneId) || "";
        let combined = existing + data;
        if (combined.length > ServerHandler.OUTPUT_BUFFER_MAX) {
          combined = combined.slice(
            combined.length - ServerHandler.OUTPUT_BUFFER_MAX,
          );
        }
        this.paneOutputBuffer.set(paneId, combined);
        // Forward output to clients
        this.broadcaster.sendToSession(sessionId, {
          type: "output",
          paneId,
          data,
        });
      },
      (exitCode: number) => {
        this.terminals.remove(paneId);
        this.paneOutputBuffer.delete(paneId);
        this.sessions.removePaneFromWindow(sessionId, windowId, paneId);

        const window = this.sessions.getActiveWindow(sessionId);
        if (window) {
          const newLayout = removeFromLayout(window.layout, paneId);
          if (newLayout) {
            window.layout = newLayout;
          }
        }

        this.broadcaster.sendToSession(sessionId, {
          type: "pane:exited",
          paneId,
          exitCode,
        });

        // If window has no panes left, remove it
        const session = this.sessions.getSession(sessionId);
        if (session) {
          const w = session.windows.find((w) => w.id === windowId);
          if (w && w.panes.length === 0) {
            this.sessions.removeWindow(sessionId, windowId);
            this.hooks.emit("window:closed", w);

            // If session has no windows, cleanup
            if (session.windows.length === 0) {
              this.hooks.emit("session:closed", session);
              this.sessions.deleteSession(sessionId);
            }
          }
        }

        this.broadcastState(sessionId);
      },
    );

    // Add pane to session tree
    const pane: Pane = {
      id: paneId,
      pid: this.ptys.getPid(paneId) || 0,
      cwd,
      command: this.config.shell,
      title: this.config.shell,
    };
    this.sessions.addPaneToWindow(sessionId, windowId, pane);
    this.hooks.emit("pane:created", pane);
  }

  private registerDefaultCommands(): void {
    this.commands.register({
      id: "window:create",
      description: "Create a new window",
      execute: (ctx) => {
        const cols = 80;
        const rows = 24;
        // Use first client's dimensions if available
        for (const [cid] of this.clientCols) {
          const sid = this.broadcaster.getClientSession(cid);
          if (sid === ctx.sessionId) {
            const c = this.clientCols.get(cid) || 80;
            const r = this.clientRows.get(cid) || 24;
            this.createWindowWithPane(ctx.sessionId, c, r);
            return;
          }
        }
        this.createWindowWithPane(ctx.sessionId, cols, rows);
      },
    });

    this.commands.register({
      id: "window:next",
      description: "Switch to next window",
      execute: (ctx) => {
        this.sessions.switchWindow(ctx.sessionId, "next");
        this.broadcastState(ctx.sessionId);
      },
    });

    this.commands.register({
      id: "window:previous",
      description: "Switch to previous window",
      execute: (ctx) => {
        this.sessions.switchWindow(ctx.sessionId, "previous");
        this.broadcastState(ctx.sessionId);
      },
    });

    this.commands.register({
      id: "window:close",
      description: "Close current window",
      execute: (ctx) => {
        if (!ctx.windowId) return;
        const session = this.sessions.getSession(ctx.sessionId);
        if (!session) return;
        const window = session.windows.find((w) => w.id === ctx.windowId);
        if (!window) return;

        // Kill all PTYs in window
        for (const pane of window.panes) {
          this.ptys.kill(pane.id);
          this.terminals.remove(pane.id);
        }

        this.sessions.removeWindow(ctx.sessionId, ctx.windowId);
        this.hooks.emit("window:closed", window);

        if (session.windows.length === 0) {
          this.hooks.emit("session:closed", session);
          this.sessions.deleteSession(ctx.sessionId);
        }

        this.broadcastState(ctx.sessionId);
      },
    });

    this.commands.register({
      id: "pane:split-horizontal",
      description: "Split pane horizontally",
      execute: (ctx) => {
        this.splitPane(ctx.sessionId, "horizontal");
      },
    });

    this.commands.register({
      id: "pane:split-vertical",
      description: "Split pane vertically",
      execute: (ctx) => {
        this.splitPane(ctx.sessionId, "vertical");
      },
    });

    this.commands.register({
      id: "pane:next",
      description: "Switch to next pane",
      execute: (ctx) => {
        this.sessions.switchPane(ctx.sessionId, "next");
        this.broadcastState(ctx.sessionId);
      },
    });

    this.commands.register({
      id: "pane:close",
      description: "Close current pane",
      execute: (ctx) => {
        if (!ctx.paneId || !ctx.windowId) return;
        this.ptys.kill(ctx.paneId);
        this.terminals.remove(ctx.paneId);

        const window = this.sessions.getActiveWindow(ctx.sessionId);
        if (window) {
          const newLayout = removeFromLayout(window.layout, ctx.paneId);
          if (newLayout) {
            window.layout = newLayout;
          }
          this.sessions.removePaneFromWindow(
            ctx.sessionId,
            ctx.windowId,
            ctx.paneId,
          );

          if (window.panes.length === 0) {
            this.sessions.removeWindow(ctx.sessionId, ctx.windowId);
          }
        }

        this.broadcastState(ctx.sessionId);
      },
    });

    this.commands.register({
      id: "session:detach",
      description: "Detach from session",
      execute: (_ctx) => {
        // This is handled client-side
      },
    });

    this.commands.register({
      id: "session:list",
      description: "List sessions",
      execute: (_ctx) => {
        // Sends state which includes all sessions
      },
    });

    this.commands.register({
      id: "pane:zoom",
      description: "Toggle pane zoom",
      execute: (_ctx) => {
        // Handled client-side (zoom state is per-client)
      },
    });

    this.commands.register({
      id: "pane:focus-up",
      description: "Focus pane above",
      execute: (_ctx) => {
        // Layout-aware focus handled via state
      },
    });

    this.commands.register({
      id: "pane:focus-down",
      description: "Focus pane below",
      execute: (_ctx) => {},
    });

    this.commands.register({
      id: "pane:focus-left",
      description: "Focus pane to the left",
      execute: (_ctx) => {},
    });

    this.commands.register({
      id: "pane:focus-right",
      description: "Focus pane to the right",
      execute: (_ctx) => {},
    });

    this.commands.register({
      id: "pane:focus",
      description: "Focus a specific pane by ID",
      execute: (ctx) => {
        const paneId = ctx.args?.paneId as string | undefined;
        if (paneId && ctx.sessionId) {
          this.sessions.setActivePane(ctx.sessionId, paneId);
          this.broadcastState(ctx.sessionId);
        }
      },
    });

    this.commands.register({
      id: "command-palette",
      description: "Open command palette",
      execute: (_ctx) => {
        // Handled client-side
      },
    });

    this.commands.register({
      id: "keybindings:show",
      description: "Show keybindings",
      execute: (_ctx) => {
        // Handled client-side
      },
    });

    this.commands.register({
      id: "window:rename",
      description: "Rename current window",
      execute: (_ctx) => {
        // Handled client-side (needs input)
      },
    });

    this.commands.register({
      id: "session:rename",
      description: "Rename current session",
      execute: (ctx) => {
        const newName = ctx.args?.name as string | undefined;
        if (newName && ctx.sessionId) {
          const session = this.sessions.getSession(ctx.sessionId);
          if (session) {
            session.name = newName;
            this.broadcastState(ctx.sessionId);
          }
        }
      },
    });
  }

  private splitPane(
    sessionId: string,
    direction: "horizontal" | "vertical",
  ): void {
    const window = this.sessions.getActiveWindow(sessionId);
    if (!window) return;

    const currentPaneId = window.activePane;
    const newPaneId = randomUUID().slice(0, 8);

    // Update layout
    window.layout = splitLayout(
      window.layout,
      currentPaneId,
      newPaneId,
      direction,
    );

    // Get dimensions from first connected client
    let cols = 80;
    let rows = 24;
    for (const [cid] of this.clientCols) {
      const sid = this.broadcaster.getClientSession(cid);
      if (sid === sessionId) {
        cols = this.clientCols.get(cid) || 80;
        rows = this.clientRows.get(cid) || 24;
        break;
      }
    }

    // Calculate new layout rects
    const paneRects = calculateLayout(window.layout, {
      x: 0,
      y: 0,
      width: cols,
      height: rows - 1,
    });

    // Resize existing panes
    for (const [pId, rect] of paneRects) {
      if (pId !== newPaneId) {
        this.ptys.resize(
          pId,
          Math.max(1, rect.width),
          Math.max(1, rect.height),
        );
        this.terminals.resize(
          pId,
          Math.max(1, rect.width),
          Math.max(1, rect.height),
        );
      }
    }

    // Spawn new pane
    const newRect = paneRects.get(newPaneId);
    const newCols = newRect
      ? Math.max(1, newRect.width)
      : Math.max(1, Math.floor(cols / 2));
    const newRows = newRect
      ? Math.max(1, newRect.height)
      : Math.max(1, rows - 1);

    this.spawnPaneProcess(sessionId, window.id, newPaneId, newCols, newRows);

    window.activePane = newPaneId;
    this.broadcastState(sessionId);
  }

  sendStateToClient(clientId: string): void {
    const sessionId = this.broadcaster.getClientSession(clientId);
    if (!sessionId) return;

    // Cork the socket to batch state + layout + output replay into one write
    this.broadcaster.cork(clientId);

    const sessions = this.sessions.listSessions().map((s) => ({
      id: s.id,
      name: s.name,
      windows: s.windows.map((w) => ({
        id: w.id,
        name: w.name,
        paneCount: w.panes.length,
        activePane: w.activePane,
      })),
      activeWindow: s.activeWindow,
      attached: s.attachedClients.length > 0,
    }));

    this.broadcaster.send(clientId, {
      type: "state",
      sessions,
      activeSession: sessionId,
    });

    // Send layout
    const window = this.sessions.getActiveWindow(sessionId);
    if (window) {
      const cols = this.clientCols.get(clientId) || 80;
      const rows = this.clientRows.get(clientId) || 24;
      const paneRects = calculateLayout(window.layout, {
        x: 0,
        y: 0,
        width: cols,
        height: rows - 1,
      });

      const rectsObj: Record<string, Rect> = {};
      for (const [id, rect] of paneRects) {
        rectsObj[id] = rect;
      }

      this.broadcaster.send(clientId, {
        type: "layout",
        layout: window.layout,
        paneRects: rectsObj,
      });

      // Replay buffered output for all panes in the active window
      // so that window switches restore visible terminal content
      for (const pane of window.panes) {
        const buffered = this.paneOutputBuffer.get(pane.id);
        if (buffered) {
          this.broadcaster.send(clientId, {
            type: "output",
            paneId: pane.id,
            data: buffered,
          });
        }
      }
    }

    // Uncork — flushes all batched messages as one chunk
    this.broadcaster.uncork(clientId);
  }

  private sendLayoutToClient(
    clientId: string,
    layout: any,
    paneRects: Map<string, Rect>,
  ): void {
    const rectsObj: Record<string, Rect> = {};
    for (const [id, rect] of paneRects) {
      rectsObj[id] = rect;
    }

    this.broadcaster.send(clientId, {
      type: "layout",
      layout,
      paneRects: rectsObj,
    });
  }

  broadcastState(sessionId: string): void {
    const clients = this.broadcaster.getSessionClients(sessionId);
    for (const clientId of clients) {
      this.sendStateToClient(clientId);
    }
  }

  shutdown(): void {
    this.metricsCollector.stop();
    if (this.autoSaver) {
      this.autoSaver.stop();
      this.autoSaver.saveNow();
    }
    this.broadcaster.notifyShutdown();
    this.ptys.killAll();
    this.terminals.removeAll();
    this.hooks.clear();
  }
}
