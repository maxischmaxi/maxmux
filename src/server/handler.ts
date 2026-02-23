import type { Socket } from "node:net";
import { randomUUID } from "node:crypto";
import { homedir } from "node:os";
import { debugLog } from "../debug.ts";
import {
  SessionManager,
  type Pane,
  type Session,
  type Window,
} from "../core/session.ts";
import { PtyManager } from "../core/pty.ts";
import { TerminalManager } from "../core/terminal.ts";
import { CommandRegistry, type CommandContext } from "../core/command.ts";
import {
  calculateLayout,
  splitLayout,
  removeFromLayout,
  getAllPaneIds,
  findPaneInDirection,
  type Rect,
} from "../core/layout.ts";
import { KeybindingRegistry } from "../input/keybindings.ts";
import { HookRegistry } from "../plugins/hooks.ts";
import { loadPlugins } from "../plugins/loader.ts";
import { Broadcaster, type ServerMessage } from "./broadcast.ts";
import type { MaxMuxConfig } from "../config/schema.ts";
import { AutoSaver } from "../persistence/autosave.ts";
import {
  saveSession,
  loadSavedSessions,
  serializeSessions,
  remapLayoutIds,
  getAllPaneIdsFromSerialized,
  type SerializedSession,
} from "../persistence/store.ts";
import { MetricsCollector } from "./metrics.ts";
import { ProcessTracker } from "./process-tracker.ts";
import type { WindowTitleInfo } from "../plugins/types.ts";

export type ClientMessage =
  | { type: "attach"; sessionId?: string; cwd?: string }
  | { type: "detach" }
  | { type: "input"; paneId: string; data: string }
  | { type: "resize"; cols: number; rows: number }
  | { type: "command"; id: string; args?: Record<string, unknown> }
  | { type: "preview"; sessionId: string; cols: number; rows: number }
  | { type: "preview-stop" }
  | {
      type: "remote-command";
      command: string;
      args?: Record<string, unknown>;
      target?: string;
    };

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
  private lastSessionId: string | null = null;
  private autoSaver: AutoSaver | null = null;
  private metricsCollector: MetricsCollector;
  private processTracker: ProcessTracker;

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
    this.processTracker = new ProcessTracker();
    this.keybindings.loadFromConfig(config.keybindings);
    this.registerDefaultCommands();
  }

  async init(): Promise<void> {
    // Load plugins
    await loadPlugins(this.config, this.commands, this.keybindings, this.hooks);

    // Restore sessions from disk
    if (this.config.sessions.autoRestore) {
      const saved = await loadSavedSessions(this.config.sessions.savePath);
      for (const s of saved) {
        this.restoreSession(s);
      }
    }

    // Start auto-save
    if (this.config.sessions.autoSave) {
      this.autoSaver = new AutoSaver(
        this.sessions,
        this.config.sessions.autoSaveInterval,
        this.config.sessions.savePath,
      );
      this.autoSaver.start();
    }

    // Start process tracker for dynamic window titles
    if (this.config.automaticRename) {
      this.processTracker.start(
        this.config.automaticRenameInterval,
        () => {
          const panes: Array<{ paneId: string; pid: number; command: string }> =
            [];
          for (const session of this.sessions.listSessions()) {
            for (const window of session.windows) {
              for (const pane of window.panes) {
                panes.push({
                  paneId: pane.id,
                  pid: pane.pid,
                  command: pane.command,
                });
              }
            }
          }
          return panes;
        },
        (paneId, processName) => {
          this.handleProcessChange(paneId, processName);
        },
      );
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
    this.broadcaster.removeClient(clientId); // also clears preview state
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
      case "preview":
        this.handlePreview(clientId, msg.sessionId, msg.cols, msg.rows);
        break;
      case "preview-stop":
        this.handlePreviewStop(clientId);
        break;
      case "remote-command":
        this.handleRemoteCommand(clientId, msg.command, msg.args, msg.target);
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

    if (!session && this.lastSessionId) {
      session = this.sessions.getSession(this.lastSessionId);
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
    // Skip broadcast — sendStateToClient below will send everything
    if (session.windows.length === 0) {
      const cols = this.clientCols.get(clientId) || 80;
      const rows = this.clientRows.get(clientId) || 24;
      this.createWindowWithPane(session.id, cols, rows, undefined, true, false);
    }

    // Send current state + layout + output replay (all batched via cork/uncork)
    // sendStateToClient already replays output buffer for active window panes
    this.sendStateToClient(clientId);

    // Update metrics with cwd and pane info
    const activeWindow = this.sessions.getActiveWindow(session.id);
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

    // Ensure PTYs are sized correctly for this client.
    // PtyManager dedup prevents unnecessary SIGWINCH if sizes match.
    const cols = this.clientCols.get(clientId) || 80;
    const rows = this.clientRows.get(clientId) || 24;
    this.handleResize(clientId, cols, rows);
  }

  private handleDetach(clientId: string): void {
    const sessionId = this.broadcaster.getClientSession(clientId);
    if (sessionId) {
      this.lastSessionId = sessionId;
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
    // session:create needs clientId for attach — handle specially
    if (commandId === "session:create") {
      this.handleSessionCreate(clientId, args);
      return;
    }

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

  private handleSessionCreate(
    clientId: string,
    args?: Record<string, unknown>,
  ): void {
    const name = (args?.name as string | undefined) || undefined;

    // Remove client from current session
    const oldSessionId = this.broadcaster.getClientSession(clientId);
    if (oldSessionId) {
      const oldSession = this.sessions.getSession(oldSessionId);
      if (oldSession) {
        oldSession.attachedClients = oldSession.attachedClients.filter(
          (c) => c !== clientId,
        );
      }
    }

    // Create new session and attach client
    const session = this.sessions.createSession(name);
    this.hooks.emit("session:created", session);
    session.attachedClients.push(clientId);
    this.broadcaster.setClientSession(clientId, session.id);

    // Create a window with a pane (skip broadcast — sendStateToClient below handles it)
    const sessionCols = this.clientCols.get(clientId) || 80;
    const sessionRows = this.clientRows.get(clientId) || 24;
    this.createWindowWithPane(
      session.id,
      sessionCols,
      sessionRows,
      undefined,
      true,
      false,
    );

    // Send state to client (includes window switch + output replay)
    this.sendStateToClient(clientId);

    // Broadcast updated state to old session's clients
    if (oldSessionId) {
      this.broadcastState(oldSessionId);
    }

    this.saveImmediate();
  }

  private handlePreview(
    clientId: string,
    sessionId: string,
    cols: number,
    rows: number,
  ): void {
    const session = this.sessions.getSession(sessionId);
    if (!session) return;

    this.broadcaster.setClientPreview(clientId, sessionId, cols, rows);

    const window = this.sessions.getActiveWindow(sessionId);
    if (!window) return;

    // Cork for batched delivery
    this.broadcaster.cork(clientId);

    // Calculate layout at preview dimensions
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
      type: "preview-layout",
      layout: window.layout,
      paneRects: rectsObj,
    });

    // Replay output buffer for all panes in preview session's active window
    for (const pane of window.panes) {
      const buffered = this.paneOutputBuffer.get(pane.id);
      if (buffered) {
        this.broadcaster.send(clientId, {
          type: "preview-output",
          paneId: pane.id,
          data: buffered,
        });
      }
    }

    this.broadcaster.uncork(clientId);
  }

  private handlePreviewStop(clientId: string): void {
    this.broadcaster.clearClientPreview(clientId);
  }

  private createWindowWithPane(
    sessionId: string,
    cols: number,
    rows: number,
    name?: string,
    switchTo = true,
    broadcast = true,
  ): void {
    const window = this.sessions.addWindow(sessionId, name);
    if (!window) return;

    if (switchTo) {
      const session = this.sessions.getSession(sessionId);
      if (session) session.activeWindow = window.id;
    }

    const paneId = getAllPaneIds(window.layout)[0]!;
    this.spawnPaneProcess(sessionId, window.id, paneId, cols, rows - 1);

    this.hooks.emit("window:created", window);
    if (broadcast) this.broadcastState(sessionId, true);
    this.saveImmediate();
  }

  private spawnPaneProcess(
    sessionId: string,
    windowId: string,
    paneId: string,
    cols: number,
    rows: number,
    cwdOverride?: string,
  ): void {
    // Use override cwd (e.g. from session restore), or the cwd of the first attached client, fall back to HOME
    let cwd = cwdOverride || process.env.HOME || homedir();
    if (!cwdOverride) {
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
    }
    const safeCols = Math.max(1, cols);
    const safeRows = Math.max(1, rows);

    debugLog(
      "server",
      `spawnPaneProcess pane=${paneId} shell=${this.config.shell} cwd=${cwd} ${safeCols}x${safeRows}`,
    );

    // Create virtual terminal
    const serverTerm = this.terminals.create(paneId, safeCols, safeRows);

    // Forward terminal query responses (DA, DSR, etc.) back to the PTY
    // so programs like fzf that query terminal capabilities get a reply
    serverTerm.onData((response) => {
      this.ptys.write(paneId, response);
    });

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
        // Forward to preview clients watching this session
        this.broadcaster.sendPreviewToSession(sessionId, {
          type: "preview-output",
          paneId,
          data,
        });
      },
      (exitCode: number) => {
        this.terminals.remove(paneId);
        this.paneOutputBuffer.delete(paneId);
        this.processTracker.removePanes([paneId]);
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
              const fallbackId = this.migrateClientsFromEmptySession(sessionId);
              this.sessions.deleteSession(sessionId);

              if (fallbackId) {
                this.broadcastState(fallbackId, true);
                this.saveImmediate();
                return;
              }
            }
          }
        }

        this.broadcastState(sessionId, true);
        this.saveImmediate();
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

  private async saveImmediate(): Promise<void> {
    if (!this.config.sessions.autoSave) return;
    await saveSession(this.sessions, this.config.sessions.savePath);
    this.hooks.emit("session:saved", {
      sessions: serializeSessions(this.sessions),
    });
  }

  private restoreSession(data: SerializedSession): void {
    const session = this.sessions.createSession(data.name);

    for (const wData of data.windows) {
      const window = this.sessions.addWindow(session.id, wData.name);
      if (!window) continue;

      const oldToNew = new Map<string, string>();
      const oldPaneIds = getAllPaneIdsFromSerialized(wData.layout);
      const defaultPaneId = getAllPaneIds(window.layout)[0]!;

      for (let i = 0; i < oldPaneIds.length; i++) {
        if (i === 0) {
          oldToNew.set(oldPaneIds[i]!, defaultPaneId);
        } else {
          oldToNew.set(oldPaneIds[i]!, randomUUID().slice(0, 8));
        }
      }

      window.layout = remapLayoutIds(wData.layout, oldToNew);

      const cols = 80;
      const rows = 24;
      for (const pData of wData.panes) {
        const newId = oldToNew.get(pData.id) || defaultPaneId;
        this.spawnPaneProcess(
          session.id,
          window.id,
          newId,
          cols,
          rows - 1,
          pData.cwd,
        );
      }

      if (wData.activePane && oldToNew.has(wData.activePane)) {
        window.activePane = oldToNew.get(wData.activePane)!;
      }
    }

    if (session.windows.length > 0) {
      session.activeWindow = session.windows[0]!.id;
    }
  }

  private registerDefaultCommands(): void {
    this.commands.register({
      id: "window:create",
      description: "Create a new window",
      execute: (ctx) => {
        const switchTo = this.config.switchToNewWindow;
        const cols = 80;
        const rows = 24;
        // Use first client's dimensions if available
        for (const [cid] of this.clientCols) {
          const sid = this.broadcaster.getClientSession(cid);
          if (sid === ctx.sessionId) {
            const c = this.clientCols.get(cid) || 80;
            const r = this.clientRows.get(cid) || 24;
            this.createWindowWithPane(ctx.sessionId, c, r, undefined, switchTo);
            return;
          }
        }
        this.createWindowWithPane(
          ctx.sessionId,
          cols,
          rows,
          undefined,
          switchTo,
        );
      },
    });

    this.commands.register({
      id: "window:next",
      description: "Switch to next window",
      execute: (ctx) => {
        this.sessions.switchWindow(ctx.sessionId, "next");
        this.broadcastState(ctx.sessionId, true);
      },
    });

    this.commands.register({
      id: "window:previous",
      description: "Switch to previous window",
      execute: (ctx) => {
        this.sessions.switchWindow(ctx.sessionId, "previous");
        this.broadcastState(ctx.sessionId, true);
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
          const fallbackId = this.migrateClientsFromEmptySession(ctx.sessionId);
          this.sessions.deleteSession(ctx.sessionId);

          if (fallbackId) {
            this.broadcastState(fallbackId, true);
            this.saveImmediate();
            return;
          }
        }

        this.broadcastState(ctx.sessionId, true);
        this.saveImmediate();
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

        this.broadcastState(ctx.sessionId, true);
        this.saveImmediate();
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
      id: "session:create",
      description: "Create a new session",
      execute: (_ctx) => {
        // Handled specially in handleCommand (needs clientId)
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
            this.saveImmediate();
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
    this.saveImmediate();
  }

  sendStateToClient(clientId: string, replay = true): void {
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
      // so that window switches restore visible terminal content.
      // Skip replay for pure state updates (e.g. pane:focus) to avoid
      // duplicating output into client VirtualTerminals that already have it.
      if (replay) {
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

  broadcastState(sessionId: string, replay = false): void {
    const clients = this.broadcaster.getSessionClients(sessionId);
    for (const clientId of clients) {
      this.sendStateToClient(clientId, replay);
    }
  }

  private handleProcessChange(paneId: string, processName: string): void {
    // Find which session/window this pane belongs to
    for (const session of this.sessions.listSessions()) {
      for (const window of session.windows) {
        const pane = window.panes.find((p) => p.id === paneId);
        if (!pane) continue;

        // Update pane title
        pane.title = processName;

        // Build default window title from all pane processes
        const processes = window.panes.map((p) => ({
          paneId: p.id,
          name: p.title,
        }));
        const defaultTitle = processes.map((p) => p.name).join(", ");

        // Run through plugin waterfall
        const info: WindowTitleInfo = {
          windowId: window.id,
          title: defaultTitle,
          processes,
        };
        const result = this.hooks.emitWaterfall("window:title", info);

        // Only broadcast if title actually changed
        if (window.name !== result.title) {
          window.name = result.title;
          this.broadcastState(session.id);
        }
        return;
      }
    }
  }

  // --- Remote CLI command handling ---

  private resolveSessionForCli(target?: string): Session | undefined {
    // Explicit target: match by name or ID
    if (target) {
      return (
        this.sessions.getSessionByName(target) ||
        this.sessions.getSession(target)
      );
    }
    // First session with attached clients
    for (const session of this.sessions.listSessions()) {
      if (session.attachedClients.length > 0) return session;
    }
    // Fallback: default session
    return this.sessions.getDefaultSession();
  }

  /**
   * When a session becomes empty (no windows), migrate its clients to
   * another session instead of letting them disconnect.  Returns the
   * fallback session ID if clients were migrated, or null if no
   * fallback session exists (i.e. this was the last session).
   */
  private migrateClientsFromEmptySession(sessionId: string): string | null {
    const clients = this.broadcaster.getSessionClients(sessionId);
    if (clients.length === 0) return null;

    // Find a fallback session (any session that isn't the one being deleted)
    const fallback = this.sessions
      .listSessions()
      .find((s) => s.id !== sessionId && s.windows.length > 0);

    if (!fallback) return null;

    // Migrate every client to the fallback session
    const session = this.sessions.getSession(sessionId);
    for (const clientId of clients) {
      // Remove from old session's attachedClients
      if (session) {
        session.attachedClients = session.attachedClients.filter(
          (c) => c !== clientId,
        );
      }
      // Attach to fallback
      this.broadcaster.setClientSession(clientId, fallback.id);
      fallback.attachedClients.push(clientId);
    }

    return fallback.id;
  }

  private getSessionClientDimensions(sessionId: string): {
    cols: number;
    rows: number;
  } {
    for (const [cid] of this.clientCols) {
      const sid = this.broadcaster.getClientSession(cid);
      if (sid === sessionId) {
        return {
          cols: this.clientCols.get(cid) || 80,
          rows: this.clientRows.get(cid) || 24,
        };
      }
    }
    return { cols: 80, rows: 24 };
  }

  private resolveFormatVariable(
    name: string,
    session: Session,
    window: Window,
    paneRects: Map<string, Rect>,
  ): string {
    switch (name) {
      case "session_name":
        return session.name;
      case "session_id":
        return session.id;
      case "window_name":
        return window.name;
      case "window_id":
        return window.id;
      case "window_index": {
        const idx = session.windows.findIndex((w) => w.id === window.id);
        return String(idx >= 0 ? idx : 0);
      }
      case "pane_id":
        return window.activePane;
      case "pane_index": {
        const idx = window.panes.findIndex((p) => p.id === window.activePane);
        return String(idx >= 0 ? idx : 0);
      }
      case "pane_at_left":
        return findPaneInDirection(paneRects, window.activePane, "left")
          ? "0"
          : "1";
      case "pane_at_right":
        return findPaneInDirection(paneRects, window.activePane, "right")
          ? "0"
          : "1";
      case "pane_at_top":
        return findPaneInDirection(paneRects, window.activePane, "up")
          ? "0"
          : "1";
      case "pane_at_bottom":
        return findPaneInDirection(paneRects, window.activePane, "down")
          ? "0"
          : "1";
      default:
        return `#{${name}}`;
    }
  }

  private handleRemoteCommand(
    clientId: string,
    command: string,
    args?: Record<string, unknown>,
    target?: string,
  ): void {
    try {
      switch (command) {
        case "select-pane":
          this.handleRemoteSelectPane(clientId, args, target);
          break;
        case "display-message":
          this.handleRemoteDisplayMessage(clientId, args, target);
          break;
        case "select-window":
          this.handleRemoteSelectWindow(clientId, args, target);
          break;
        case "split-window":
          this.handleRemoteSplitWindow(clientId, args, target);
          break;
        case "new-window":
          this.handleRemoteNewWindow(clientId, args, target);
          break;
        case "send-command":
          this.handleRemoteSendCommand(clientId, args, target);
          break;
        default:
          this.broadcaster.send(clientId, {
            type: "result",
            success: false,
            error: `Unknown remote command: ${command}`,
          });
      }
    } catch (err) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: String(err),
      });
    }
  }

  private handleRemoteSelectPane(
    clientId: string,
    args?: Record<string, unknown>,
    target?: string,
  ): void {
    const session = this.resolveSessionForCli(target);
    if (!session) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No session found",
      });
      return;
    }

    const direction = args?.direction as
      | "up"
      | "down"
      | "left"
      | "right"
      | undefined;
    if (!direction) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No direction specified",
      });
      return;
    }

    const window = this.sessions.getActiveWindow(session.id);
    if (!window) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No active window",
      });
      return;
    }

    const { cols, rows } = this.getSessionClientDimensions(session.id);
    const paneRects = calculateLayout(window.layout, {
      x: 0,
      y: 0,
      width: cols,
      height: rows - 1,
    });
    const targetPane = findPaneInDirection(
      paneRects,
      window.activePane,
      direction,
    );

    if (targetPane) {
      this.sessions.setActivePane(session.id, targetPane);
      this.broadcastState(session.id);
      this.broadcaster.send(clientId, { type: "result", success: true });
    } else {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No pane in that direction",
      });
    }
  }

  private handleRemoteDisplayMessage(
    clientId: string,
    args?: Record<string, unknown>,
    target?: string,
  ): void {
    const session = this.resolveSessionForCli(target);
    if (!session) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No session found",
      });
      return;
    }

    const format = args?.format as string | undefined;
    if (!format) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No format string",
      });
      return;
    }

    const window = this.sessions.getActiveWindow(session.id);
    if (!window) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No active window",
      });
      return;
    }

    const { cols, rows } = this.getSessionClientDimensions(session.id);
    const paneRects = calculateLayout(window.layout, {
      x: 0,
      y: 0,
      width: cols,
      height: rows - 1,
    });

    const result = format.replace(/#\{(\w+)\}/g, (_match, name: string) => {
      return this.resolveFormatVariable(name, session, window, paneRects);
    });

    this.broadcaster.send(clientId, {
      type: "result",
      success: true,
      data: result,
    });
  }

  private handleRemoteSelectWindow(
    clientId: string,
    args?: Record<string, unknown>,
    target?: string,
  ): void {
    const session = this.resolveSessionForCli(target);
    if (!session) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No session found",
      });
      return;
    }

    const direction = args?.direction as "next" | "previous" | undefined;
    if (!direction) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No direction specified",
      });
      return;
    }

    this.sessions.switchWindow(session.id, direction);
    this.broadcastState(session.id, true);
    this.broadcaster.send(clientId, { type: "result", success: true });
  }

  private handleRemoteSplitWindow(
    clientId: string,
    args?: Record<string, unknown>,
    target?: string,
  ): void {
    const session = this.resolveSessionForCli(target);
    if (!session) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No session found",
      });
      return;
    }

    const direction = args?.direction as "horizontal" | "vertical" | undefined;
    if (!direction) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No direction specified",
      });
      return;
    }

    this.splitPane(session.id, direction);
    this.broadcaster.send(clientId, { type: "result", success: true });
  }

  private handleRemoteNewWindow(
    clientId: string,
    _args?: Record<string, unknown>,
    target?: string,
  ): void {
    const session = this.resolveSessionForCli(target);
    if (!session) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No session found",
      });
      return;
    }

    const { cols, rows } = this.getSessionClientDimensions(session.id);
    this.createWindowWithPane(session.id, cols, rows);
    this.broadcaster.send(clientId, { type: "result", success: true });
  }

  private handleRemoteSendCommand(
    clientId: string,
    args?: Record<string, unknown>,
    target?: string,
  ): void {
    const session = this.resolveSessionForCli(target);
    if (!session) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No session found",
      });
      return;
    }

    const commandId = args?.id as string | undefined;
    if (!commandId) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: "No command ID specified",
      });
      return;
    }

    const window = this.sessions.getActiveWindow(session.id);
    const ctx: CommandContext = {
      sessionId: session.id,
      windowId: window?.id,
      paneId: window?.activePane,
      args: args?.commandArgs as Record<string, unknown> | undefined,
    };

    try {
      this.commands.execute(commandId, ctx);
      this.broadcaster.send(clientId, { type: "result", success: true });
    } catch (err) {
      this.broadcaster.send(clientId, {
        type: "result",
        success: false,
        error: String(err),
      });
    }
  }

  shutdown(): void {
    this.processTracker.stop();
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
