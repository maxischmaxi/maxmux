import { ServerConnection } from "./connection.ts";
import type { ServerMessage } from "../server/broadcast.ts";
import type { MaxMuxConfig } from "../config/schema.ts";
import type { Rect } from "../core/layout.ts";
import { findPaneInDirection } from "../core/layout.ts";
import type { LayoutNode } from "../core/session.ts";
import { TerminalManager } from "../core/terminal.ts";
import { InputRouter } from "../input/router.ts";
import { KeybindingRegistry } from "../input/keybindings.ts";
import { StatusBarRenderer } from "../statusbar/renderer.ts";
import type { SystemMetrics } from "../statusbar/types.ts";
import { getBorderChars, type BorderStyle } from "../renderer/border.ts";
import * as ansi from "../renderer/ansi.ts";
import {
  createSessionFinderState,
  updateFilter,
  renderSessionFinder,
} from "../ui/SessionFinder.ts";
import type { SessionFinderState } from "../ui/SessionFinder.ts";
import {
  createRenameDialogState,
  renderRenameDialog,
} from "../ui/RenameDialog.ts";
import type { RenameDialogState } from "../ui/RenameDialog.ts";
import { debugLog } from "../debug.ts";

interface SessionInfo {
  id: string;
  name: string;
  windows: Array<{
    id: string;
    name: string;
    paneCount: number;
    activePane: string;
  }>;
  activeWindow: string;
  attached: boolean;
}

export async function attachToSession(
  config: MaxMuxConfig,
  sessionId?: string,
): Promise<void> {
  const keybindings = new KeybindingRegistry();
  keybindings.loadFromConfig(config.keybindings);
  const globalKeybindings = new KeybindingRegistry();
  globalKeybindings.loadFromConfig(config.globalKeybindings);
  let cols = process.stdout.columns || 80;
  let rows = process.stdout.rows || 24;

  let paneRects: Map<string, Rect> = new Map();
  let sessions: SessionInfo[] = [];
  let activeSession = "";
  let activePaneId = "";
  let showingOverlay = false;
  let prefixActive = false;

  // Client-side virtual terminals for compositor rendering
  const clientTerminals = new TerminalManager();
  let knownPaneIds = new Set<string>();
  let renderTimer: ReturnType<typeof setTimeout> | null = null;
  let currentLayout: LayoutNode | null = null;
  let pendingWrites = 0;

  // StatusBar renderer
  const statusBarRenderer = new StatusBarRenderer(
    config.statusBar,
    config.theme.statusBar,
  );

  // Draw the status bar (positions itself via StatusBarRenderer)
  const drawStatusBar = () => {
    const session = sessions.find((s) => s.id === activeSession);
    if (!session) return;

    const windowInfos = session.windows.map((w, i) => ({
      id: w.id,
      name: w.name,
      index: i,
      paneCount: w.paneCount,
      isActive: w.id === session.activeWindow,
    }));

    const output = statusBarRenderer.render(
      { id: session.id, name: session.name },
      windowInfos,
      prefixActive,
      cols,
      rows,
    );

    if (output) {
      let out = "\x1b7"; // save cursor
      out += output;
      out += "\x1b8"; // restore cursor
      process.stdout.write(out);
    }
  };

  // --- Compositor rendering ---

  const renderPaneContent = (paneId: string): string => {
    const rect = paneRects.get(paneId);
    const term = clientTerminals.get(paneId);
    if (!rect || !term) return "";

    const contentHeight = rows - 1;
    const paneHeight = Math.min(rect.height, contentHeight - rect.y);
    if (paneHeight <= 0) return "";

    let out = "";
    for (let y = 0; y < paneHeight; y++) {
      out += ansi.moveTo(rect.x, rect.y + y);
      out += term.renderLine(y);
    }
    return out;
  };

  // Collect all border cells from the layout tree
  const collectBorderCells = (
    node: LayoutNode,
    bounds: Rect,
    cells: Set<string>,
  ): void => {
    if (node.type === "leaf") return;

    const { direction, ratio, children } = node;

    if (direction === "horizontal") {
      // Vertical border at splitX column, spanning full height
      const splitX = Math.floor(bounds.x + bounds.width * ratio);
      for (let y = bounds.y; y < bounds.y + bounds.height; y++) {
        cells.add(`${splitX},${y}`);
      }
      const firstBounds: Rect = {
        x: bounds.x,
        y: bounds.y,
        width: splitX - bounds.x,
        height: bounds.height,
      };
      const secondBounds: Rect = {
        x: splitX + 1,
        y: bounds.y,
        width: bounds.x + bounds.width - splitX - 1,
        height: bounds.height,
      };
      collectBorderCells(children[0], firstBounds, cells);
      collectBorderCells(children[1], secondBounds, cells);
    } else {
      // Horizontal border at splitY row, spanning full width
      const splitY = Math.floor(bounds.y + bounds.height * ratio);
      for (let x = bounds.x; x < bounds.x + bounds.width; x++) {
        cells.add(`${x},${splitY}`);
      }
      const firstBounds: Rect = {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: splitY - bounds.y,
      };
      const secondBounds: Rect = {
        x: bounds.x,
        y: splitY + 1,
        width: bounds.width,
        height: bounds.y + bounds.height - splitY - 1,
      };
      collectBorderCells(children[0], firstBounds, cells);
      collectBorderCells(children[1], secondBounds, cells);
    }
  };

  const renderBorders = (): string => {
    if (paneRects.size <= 1 || !currentLayout) return "";

    const borderChars = getBorderChars(
      config.theme.border.style as BorderStyle,
    );
    const borderFg = config.theme.border.fg;
    const contentHeight = rows - 1;

    // Collect all border cells from layout tree
    const cells = new Set<string>();
    collectBorderCells(
      currentLayout,
      { x: 0, y: 0, width: cols, height: contentHeight },
      cells,
    );

    let out = ansi.fgHex(borderFg);

    for (const key of cells) {
      const sep = key.indexOf(",");
      const x = parseInt(key.substring(0, sep), 10);
      const y = parseInt(key.substring(sep + 1), 10);
      if (y >= contentHeight) continue;

      const hasUp = cells.has(`${x},${y - 1}`);
      const hasDown = cells.has(`${x},${y + 1}`);
      const hasLeft = cells.has(`${x - 1},${y}`);
      const hasRight = cells.has(`${x + 1},${y}`);

      let ch: string;
      if (hasUp && hasDown && hasLeft && hasRight) {
        ch = borderChars.cross;
      } else if (hasUp && hasDown && hasRight) {
        ch = borderChars.teeLeft;
      } else if (hasUp && hasDown && hasLeft) {
        ch = borderChars.teeRight;
      } else if (hasLeft && hasRight && hasDown) {
        ch = borderChars.teeTop;
      } else if (hasLeft && hasRight && hasUp) {
        ch = borderChars.teeBottom;
      } else if (hasUp && hasDown) {
        ch = borderChars.vertical;
      } else if (hasLeft && hasRight) {
        ch = borderChars.horizontal;
      } else if (hasDown && hasRight) {
        ch = borderChars.topLeft;
      } else if (hasDown && hasLeft) {
        ch = borderChars.topRight;
      } else if (hasUp && hasRight) {
        ch = borderChars.bottomLeft;
      } else if (hasUp && hasLeft) {
        ch = borderChars.bottomRight;
      } else if (hasUp || hasDown) {
        ch = borderChars.vertical;
      } else {
        ch = borderChars.horizontal;
      }

      out += ansi.moveTo(x, y) + ch;
    }

    out += ansi.resetStyle();
    return out;
  };

  const positionCursor = (): string => {
    const activeTerm = clientTerminals.get(activePaneId);
    const activeRect = paneRects.get(activePaneId);
    if (activeTerm && activeRect) {
      return ansi.moveTo(
        activeRect.x + activeTerm.getCursorX(),
        activeRect.y + activeTerm.getCursorY(),
      );
    }
    return "";
  };

  const renderScreen = () => {
    if (showingOverlay || paneRects.size === 0) return;

    let out = ansi.hideCursor();

    for (const [paneId] of paneRects) {
      out += renderPaneContent(paneId);
    }

    out += renderBorders();
    out += positionCursor();
    out += ansi.showCursor();

    process.stdout.write(out);
    drawStatusBar();
  };

  const scheduleRender = () => {
    if (renderTimer || showingOverlay) return;
    renderTimer = setTimeout(() => {
      renderTimer = null;
      if (pendingWrites > 0) {
        // xterm.js still processing writes — retry until ready
        scheduleRender();
        return;
      }
      renderScreen();
    }, 0);
  };

  // Sync client-side VirtualTerminals with server layout
  const syncTerminals = () => {
    const newPaneIds = new Set(paneRects.keys());

    // Remove terminals for panes no longer in layout
    for (const id of knownPaneIds) {
      if (!newPaneIds.has(id)) {
        clientTerminals.remove(id);
      }
    }

    // Create or resize terminals to match layout
    for (const [paneId, rect] of paneRects) {
      const w = Math.max(1, rect.width);
      const h = Math.max(1, rect.height);
      const existing = clientTerminals.get(paneId);
      if (existing) {
        if (existing.getCols() !== w || existing.getRows() !== h) {
          existing.resize(w, h);
        }
      } else {
        clientTerminals.create(paneId, w, h);
      }
    }

    knownPaneIds = newPaneIds;
  };

  // --- Server connection ---

  const connection = new ServerConnection(
    (msg: ServerMessage) => {
      switch (msg.type) {
        case "output": {
          debugLog(
            "client",
            `output pane=${msg.paneId} len=${msg.data.length}`,
          );
          const term = clientTerminals.get(msg.paneId);
          if (term) {
            pendingWrites++;
            term.write(msg.data, () => {
              pendingWrites--;
              scheduleRender();
            });
          }
          break;
        }

        case "state":
          sessions = msg.sessions as SessionInfo[];
          activeSession = msg.activeSession;
          debugLog(
            "client",
            `state: ${sessions.length} sessions, active=${activeSession}`,
          );

          // Update activePaneId
          const session = sessions.find((s) => s.id === activeSession);
          if (session) {
            const activeWindow = session.windows.find(
              (w) => w.id === session.activeWindow,
            );
            activePaneId = activeWindow?.activePane || "";
            debugLog("client", `activePane=${activePaneId}`);

            // Session has no windows left (all panes exited) → detach
            if (session.windows.length === 0) {
              cleanup();
              process.stdout.write("\r\n[exited]\r\n");
              process.exit(0);
            }
          } else {
            // Our session was deleted entirely → detach
            cleanup();
            process.stdout.write("\r\n[exited]\r\n");
            process.exit(0);
          }

          scheduleRender();
          break;

        case "layout":
          currentLayout = msg.layout as LayoutNode;
          paneRects = new Map(
            Object.entries(msg.paneRects as Record<string, Rect>),
          );
          debugLog(
            "client",
            `layout: ${paneRects.size} panes, rects=${JSON.stringify([...paneRects.keys()])}`,
          );
          syncTerminals();
          scheduleRender();
          break;

        case "pane:exited":
          paneRects.delete(msg.paneId);
          clientTerminals.remove(msg.paneId);
          knownPaneIds.delete(msg.paneId);
          debugLog("client", `pane exited: ${msg.paneId}`);
          break;

        case "metrics":
          statusBarRenderer.updateMetrics(msg.data as SystemMetrics);
          drawStatusBar();
          break;

        case "error":
          if (msg.message === "detached") {
            cleanup();
            process.stdout.write("\r\n[detached]\r\n");
            process.exit(0);
          } else if (msg.message === "server-shutdown") {
            cleanup();
            process.stdout.write("\r\n[server killed]\r\n");
            process.exit(0);
          }
          break;
      }
    },
    () => {
      cleanup();
      process.stdout.write("\r\n[server disconnected]\r\n");
      process.exit(1);
    },
  );

  // Input router
  const inputRouter = new InputRouter(
    config.prefixKey,
    config.prefixTimeout,
    keybindings,
    globalKeybindings,
    (action) => {
      switch (action.type) {
        case "passthrough":
          if (activePaneId) {
            connection.send({
              type: "input",
              paneId: activePaneId,
              data: action.data.toString("base64"),
            });
          }
          break;

        case "command":
          handleCommand(action.commandId);
          break;

        case "prefix-activated":
          prefixActive = true;
          drawStatusBar();
          break;

        case "prefix-timeout":
          prefixActive = false;
          drawStatusBar();
          break;
      }
    },
  );

  const handleCommand = (commandId: string) => {
    // Reset prefix state after command execution
    prefixActive = false;

    switch (commandId) {
      case "session:detach":
        connection.send({ type: "detach" });
        return;

      case "server:kill":
        connection.send({ type: "command", id: "server:kill" });
        cleanup();
        process.stdout.write("\r\n[server killed]\r\n");
        process.exit(0);
        return;

      case "pane:zoom":
        // TODO: implement zoom toggle
        return;

      case "pane:focus-up":
      case "pane:focus-down":
      case "pane:focus-left":
      case "pane:focus-right": {
        const dir = commandId.split("-").pop() as
          | "up"
          | "down"
          | "left"
          | "right";
        const targetId = findPaneInDirection(paneRects, activePaneId, dir);
        if (targetId) {
          connection.send({
            type: "command",
            id: "pane:focus",
            args: { paneId: targetId },
          });
        }
        return;
      }

      case "keybindings:show":
        showKeybindingsOverlay();
        return;

      case "command-palette":
        return;

      case "session:list":
        showSessionList();
        return;

      case "session:find":
        showSessionFinder();
        return;

      case "session:rename":
        showRenameDialog();
        return;
    }

    // Forward to server
    connection.send({ type: "command", id: commandId });
    drawStatusBar();
  };

  const showKeybindingsOverlay = () => {
    showingOverlay = true;
    process.stdout.write(ansi.clearScreen() + ansi.moveToOrigin());
    process.stdout.write("MaxMux Keybindings (press any key to close)\r\n");
    process.stdout.write("\u2500".repeat(50) + "\r\n\n");

    const bindings = keybindings.list();
    for (const { key, commandId } of bindings) {
      process.stdout.write(
        `  prefix + ${key.padEnd(8)} \u2192 ${commandId}\r\n`,
      );
    }

    const globals = globalKeybindings.list();
    if (globals.length > 0) {
      process.stdout.write("\r\n  Global (no prefix):\r\n");
      for (const { key, commandId } of globals) {
        process.stdout.write(`  ${key.padEnd(14)} \u2192 ${commandId}\r\n`);
      }
    }

    process.stdout.write(`\n  prefix = ${config.prefixKey}\r\n`);

    const onData = () => {
      process.stdin.removeListener("data", onData);
      showingOverlay = false;
      process.stdout.write(ansi.clearScreen());
      renderScreen();
    };
    process.stdin.on("data", onData);
  };

  const showSessionList = () => {
    showingOverlay = true;
    process.stdout.write(ansi.clearScreen() + ansi.moveToOrigin());
    process.stdout.write("Sessions (press any key to close)\r\n");
    process.stdout.write("\u2500".repeat(50) + "\r\n\n");

    for (const session of sessions) {
      const marker = session.id === activeSession ? " *" : "";
      const attached = session.attached ? " (attached)" : "";
      process.stdout.write(
        `  ${session.name}${marker}${attached} - ${session.windows.length} window(s)\r\n`,
      );
    }

    const onData = () => {
      process.stdin.removeListener("data", onData);
      showingOverlay = false;
      process.stdout.write(ansi.clearScreen());
      renderScreen();
    };
    process.stdin.on("data", onData);
  };

  const showSessionFinder = () => {
    showingOverlay = true;
    const finderState: SessionFinderState = createSessionFinderState(
      sessions.map((s) => ({
        id: s.id,
        name: s.name,
        windowCount: s.windows.length,
        attached: s.attached,
      })),
    );

    const redrawFinder = () => {
      process.stdout.write(renderSessionFinder(finderState, cols, rows));
    };

    const closeFinder = () => {
      process.stdin.removeListener("data", onFinderData);
      showingOverlay = false;
      process.stdout.write(ansi.clearScreen());
      renderScreen();
    };

    const onFinderData = (data: Buffer) => {
      const bytes = Array.from(data);

      // Escape (0x1b alone)
      if (bytes.length === 1 && bytes[0] === 0x1b) {
        closeFinder();
        return;
      }

      // Enter (0x0d)
      if (bytes.length === 1 && bytes[0] === 0x0d) {
        const selected = finderState.filtered[finderState.selectedIndex];
        if (selected) {
          closeFinder();
          connection.send({ type: "attach", sessionId: selected.id });
        }
        return;
      }

      // Backspace (0x7f)
      if (bytes.length === 1 && bytes[0] === 0x7f) {
        if (finderState.query.length > 0) {
          finderState.query = finderState.query.slice(0, -1);
          updateFilter(finderState);
          redrawFinder();
        }
        return;
      }

      // Arrow Up (0x1b 0x5b 0x41)
      if (
        bytes.length === 3 &&
        bytes[0] === 0x1b &&
        bytes[1] === 0x5b &&
        bytes[2] === 0x41
      ) {
        if (finderState.selectedIndex > 0) {
          finderState.selectedIndex--;
          redrawFinder();
        }
        return;
      }

      // Arrow Down (0x1b 0x5b 0x42)
      if (
        bytes.length === 3 &&
        bytes[0] === 0x1b &&
        bytes[1] === 0x5b &&
        bytes[2] === 0x42
      ) {
        if (finderState.selectedIndex < finderState.filtered.length - 1) {
          finderState.selectedIndex++;
          redrawFinder();
        }
        return;
      }

      // Printable characters
      const str = data.toString("utf-8");
      const firstByte = bytes[0];
      if (
        str.length > 0 &&
        firstByte !== undefined &&
        firstByte >= 0x20 &&
        firstByte < 0x7f
      ) {
        finderState.query += str;
        updateFilter(finderState);
        redrawFinder();
      }
    };

    process.stdin.on("data", onFinderData);
    redrawFinder();
  };

  const showRenameDialog = () => {
    const session = sessions.find((s) => s.id === activeSession);
    if (!session) return;

    showingOverlay = true;
    const renameState: RenameDialogState = createRenameDialogState(
      "Rename Session",
      session.name,
    );

    const redrawDialog = () => {
      process.stdout.write(renderRenameDialog(renameState, cols, rows));
    };

    const closeDialog = () => {
      process.stdin.removeListener("data", onRenameData);
      showingOverlay = false;
      process.stdout.write(ansi.clearScreen());
      renderScreen();
    };

    const onRenameData = (data: Buffer) => {
      const bytes = Array.from(data);

      // Escape
      if (bytes.length === 1 && bytes[0] === 0x1b) {
        closeDialog();
        return;
      }

      // Enter — confirm rename
      if (bytes.length === 1 && bytes[0] === 0x0d) {
        const newName = renameState.value.trim();
        if (newName.length > 0) {
          connection.send({
            type: "command",
            id: "session:rename",
            args: { name: newName },
          });
        }
        closeDialog();
        return;
      }

      // Backspace
      if (bytes.length === 1 && bytes[0] === 0x7f) {
        if (renameState.value.length > 0) {
          renameState.value = renameState.value.slice(0, -1);
          redrawDialog();
        }
        return;
      }

      // Ctrl+U — clear input
      if (bytes.length === 1 && bytes[0] === 0x15) {
        renameState.value = "";
        redrawDialog();
        return;
      }

      // Printable characters
      const str = data.toString("utf-8");
      const firstByte = bytes[0];
      if (
        str.length > 0 &&
        firstByte !== undefined &&
        firstByte >= 0x20 &&
        firstByte < 0x7f
      ) {
        renameState.value += str;
        redrawDialog();
      }
    };

    process.stdin.on("data", onRenameData);
    redrawDialog();
  };

  // Cleanup
  let cleaned = false;
  const cleanup = () => {
    if (cleaned) return;
    cleaned = true;
    if (renderTimer) clearTimeout(renderTimer);
    inputRouter.destroy();
    clientTerminals.removeAll();
    try {
      process.stdin.setRawMode?.(false);
    } catch {}
    process.stdin.pause();
    process.stdout.write(ansi.exitAltScreen());
    process.stdout.write(ansi.showCursor());
    process.stdout.write(ansi.resetStyle());
    connection.disconnect();
  };

  try {
    await connection.connect();
  } catch {
    console.error("Could not connect to server. Is it running?");
    process.exit(1);
  }

  // Enter raw mode + alt screen
  process.stdin.setRawMode?.(true);
  process.stdin.resume();
  process.stdout.write(ansi.enterAltScreen());
  process.stdout.write(ansi.clearScreen());

  // Send initial resize then attach
  connection.send({ type: "resize", cols, rows });
  connection.send({ type: "attach", sessionId, cwd: process.cwd() });

  // Handle stdin
  process.stdin.on("data", (data: Buffer) => {
    if (showingOverlay) return;
    inputRouter.handleInput(data);
  });

  // Handle terminal resize
  process.stdout.on("resize", () => {
    cols = process.stdout.columns || 80;
    rows = process.stdout.rows || 24;
    connection.send({ type: "resize", cols, rows });
    // Server will send updated layout which triggers syncTerminals + renderScreen
  });

  // Periodically refresh status bar (configurable interval)
  const refreshInterval = config.statusBar.refreshInterval || 1000;
  const statusInterval = setInterval(() => {
    if (!showingOverlay) drawStatusBar();
  }, refreshInterval);

  // Handle clean exit
  process.on("SIGINT", () => {
    // Don't exit on Ctrl+C, pass through to PTY
  });

  process.on("SIGTERM", () => {
    clearInterval(statusInterval);
    cleanup();
    process.exit(0);
  });

  // Safety net
  process.on("exit", () => {
    clearInterval(statusInterval);
    if (!cleaned) {
      try {
        process.stdin.setRawMode?.(false);
      } catch {}
      process.stdout.write("\x1b[?1049l\x1b[?25h\x1b[0m");
    }
  });
}
