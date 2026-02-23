import { ServerConnection } from "./connection.ts";
import type { ServerMessage } from "../server/broadcast.ts";
import type { MaxMuxConfig } from "../config/schema.ts";
import type { Rect } from "../core/layout.ts";
import { findPaneInDirection } from "../core/layout.ts";
import type { LayoutNode } from "../core/session.ts";
import { TerminalManager } from "../core/terminal.ts";
import { InputRouter, parsePrefixKey } from "../input/router.ts";
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
import {
  createSessionSidebarState,
  updateSidebarSessions,
  renderSessionSidebar,
} from "../ui/SessionSidebar.ts";
import type { SessionSidebarState } from "../ui/SessionSidebar.ts";
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
  let currentLayout: LayoutNode | null = null;
  let renderTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingWrites = 0;

  // Sidebar state
  let sidebarActive = false;
  let sidebarState: SessionSidebarState | null = null;
  let sidebarWidth = config.sessionList.sidebarWidth;
  let sidebarNeedsClear = false; // Flag to clear main area on next render (state transitions only)

  // Preview state (for sidebar)
  const previewTerminals = new TerminalManager();
  let previewPendingWrites = 0;
  let previewPaneRects: Map<string, Rect> = new Map();
  let previewLayout: LayoutNode | null = null;
  let previewSessionId = "";
  let previewKnownPaneIds = new Set<string>();

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
      let out = ansi.hideCursor();
      out += output;
      // Reposition cursor explicitly instead of relying on save/restore
      // which can propagate stale cursor positions
      const activeTerm = clientTerminals.get(activePaneId);
      if (activeTerm) {
        out += ansi.setCursorStyle(activeTerm.getCursorStyle());
        const xOffset =
          sidebarActive && config.sessionList.sidebarPosition === "left"
            ? sidebarWidth + 1
            : 0;
        out += positionCursor(xOffset);
        // Only show cursor if the application wants it visible (DECTCEM)
        if (activeTerm.isCursorVisible()) {
          out += ansi.showCursor();
        }
      }
      process.stdout.write(out);
    }
  };

  // --- Compositor rendering ---

  const renderPaneContent = (
    paneId: string,
    rects?: Map<string, Rect>,
    terminals?: TerminalManager,
    xOffset = 0,
  ): string => {
    const useRects = rects || paneRects;
    const useTerminals = terminals || clientTerminals;
    const rect = useRects.get(paneId);
    const term = useTerminals.get(paneId);
    if (!rect || !term) return "";

    const contentHeight = rows - 1;
    const paneHeight = Math.min(rect.height, contentHeight - rect.y);
    if (paneHeight <= 0) return "";

    let out = "";
    for (let y = 0; y < paneHeight; y++) {
      out += ansi.moveTo(rect.x + xOffset, rect.y + y);
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

  const renderBordersFor = (
    layout: LayoutNode | null,
    rects: Map<string, Rect>,
    boundsWidth: number,
    xOffset = 0,
  ): string => {
    if (rects.size <= 1 || !layout) return "";

    const borderChars = getBorderChars(
      config.theme.border.style as BorderStyle,
    );
    const borderFg = config.theme.border.fg;
    const contentHeight = rows - 1;

    // Collect all border cells from layout tree
    const cells = new Set<string>();
    collectBorderCells(
      layout,
      { x: 0, y: 0, width: boundsWidth, height: contentHeight },
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

      out += ansi.moveTo(x + xOffset, y) + ch;
    }

    out += ansi.resetStyle();
    return out;
  };

  const renderBorders = (): string => {
    return renderBordersFor(currentLayout, paneRects, cols);
  };

  const positionCursor = (xOffset = 0): string => {
    const activeTerm = clientTerminals.get(activePaneId);
    const activeRect = paneRects.get(activePaneId);
    if (activeTerm && activeRect) {
      return ansi.moveTo(
        activeRect.x + activeTerm.getCursorX() + xOffset,
        activeRect.y + activeTerm.getCursorY(),
      );
    }
    // Fallback: hide cursor to avoid ghost cursor at wrong position
    return ansi.hideCursor();
  };

  const renderScreen = () => {
    if (showingOverlay) return;

    let out = ansi.hideCursor();

    if (sidebarActive && sidebarState) {
      const sidebarPos = config.sessionList.sidebarPosition;
      const mainXOffset = sidebarPos === "left" ? sidebarWidth + 1 : 0; // +1 for border
      const mainWidth = cols - sidebarWidth - 1;
      const contentHeight = rows - 1;

      // Determine which panes/terminals to render in main area
      // Only switch to preview rendering once preview data has actually arrived
      const isPreviewActive =
        previewSessionId &&
        previewSessionId !== activeSession &&
        previewPaneRects.size > 0;
      const useRects = isPreviewActive ? previewPaneRects : paneRects;
      const useTerminals = isPreviewActive ? previewTerminals : clientTerminals;
      const useLayout = isPreviewActive ? previewLayout : currentLayout;

      // Only clear main area on state transitions (sidebar open, preview switch)
      // to avoid flicker from full-area clear on every frame
      if (sidebarNeedsClear) {
        sidebarNeedsClear = false;
        out += ansi.resetStyle();
        for (let y = 0; y < contentHeight; y++) {
          out += ansi.moveTo(mainXOffset, y) + " ".repeat(mainWidth);
        }
      }

      // Render main area panes
      for (const [paneId] of useRects) {
        out += renderPaneContent(paneId, useRects, useTerminals, mainXOffset);
      }

      // Render main area borders
      out += renderBordersFor(useLayout, useRects, mainWidth, mainXOffset);

      // Render sidebar
      const sidebarScreenX = sidebarPos === "left" ? 0 : cols - sidebarWidth;
      out += renderSessionSidebar(
        sidebarState,
        sidebarWidth,
        contentHeight,
        sidebarScreenX,
        sidebarPos,
        config.theme.border.style as BorderStyle,
        {
          fg: config.theme.statusBar.fg,
          bg: config.theme.statusBar.bg,
          activeFg: config.theme.statusBar.active,
          borderFg: config.theme.border.fg,
        },
      );

      // Position cursor: hide when showing preview (user can't interact),
      // show normally when viewing active session
      if (isPreviewActive) {
        // Keep cursor hidden — preview is read-only
      } else {
        const activeTerm = clientTerminals.get(activePaneId);
        if (activeTerm) {
          out += ansi.setCursorStyle(activeTerm.getCursorStyle());
        }
        out += positionCursor(mainXOffset);
        // Only show cursor if the application wants it visible (DECTCEM)
        if (!activeTerm || activeTerm.isCursorVisible()) {
          out += ansi.showCursor();
        }
      }
    } else {
      if (paneRects.size === 0) {
        out += ansi.showCursor();
        process.stdout.write(out);
        return;
      }

      for (const [paneId] of paneRects) {
        out += renderPaneContent(paneId);
      }

      out += renderBorders();
      const activeTerm = clientTerminals.get(activePaneId);
      if (activeTerm) {
        out += ansi.setCursorStyle(activeTerm.getCursorStyle());
      }
      out += positionCursor();
      // Only show cursor if the application wants it visible (DECTCEM)
      if (!activeTerm || activeTerm.isCursorVisible()) {
        out += ansi.showCursor();
      }
    }

    process.stdout.write(out);
    drawStatusBar();
  };

  const scheduleRender = () => {
    if (renderTimer || showingOverlay) return;
    renderTimer = setTimeout(() => {
      renderTimer = null;
      if (pendingWrites > 0 || previewPendingWrites > 0) {
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

  // Sync preview VirtualTerminals with preview layout
  const syncPreviewTerminals = () => {
    const newPaneIds = new Set(previewPaneRects.keys());

    for (const id of previewKnownPaneIds) {
      if (!newPaneIds.has(id)) {
        previewTerminals.remove(id);
      }
    }

    for (const [paneId, rect] of previewPaneRects) {
      const w = Math.max(1, rect.width);
      const h = Math.max(1, rect.height);
      const existing = previewTerminals.get(paneId);
      if (existing) {
        if (existing.getCols() !== w || existing.getRows() !== h) {
          existing.resize(w, h);
        }
      } else {
        previewTerminals.create(paneId, w, h);
      }
    }

    previewKnownPaneIds = newPaneIds;
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

        case "state": {
          const prevActiveSession = activeSession;
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

          // Update sidebar sessions if sidebar is open
          if (sidebarActive && sidebarState) {
            const sidebarEntries = (msg.sessions as SessionInfo[]).map((s) => ({
              id: s.id,
              name: s.name,
              windowCount: s.windows.length,
              attached: s.attached,
              isActive: s.id === msg.activeSession,
            }));
            updateSidebarSessions(sidebarState, sidebarEntries);
            // Active session changed (e.g. new session created) — move selection
            if (activeSession !== prevActiveSession) {
              const newIdx = sidebarState.sessions.findIndex(
                (s) => s.id === activeSession,
              );
              if (newIdx >= 0) {
                sidebarState.selectedIndex = newIdx;
              }
            }
            // If previewed session was deleted, fallback to active
            if (
              previewSessionId &&
              !sidebarEntries.find((s) => s.id === previewSessionId)
            ) {
              previewSessionId = "";
              clearPreviewState();
            }
          }

          scheduleRender();
          break;
        }

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

        case "preview-output": {
          if (!sidebarActive) break;
          const previewTerm = previewTerminals.get(msg.paneId);
          if (previewTerm) {
            previewPendingWrites++;
            previewTerm.write(msg.data, () => {
              previewPendingWrites--;
              scheduleRender();
            });
          }
          break;
        }

        case "preview-layout":
          if (!sidebarActive) break;
          previewLayout = msg.layout as LayoutNode;
          previewPaneRects = new Map(
            Object.entries(msg.paneRects as Record<string, Rect>),
          );
          syncPreviewTerminals();
          scheduleRender();
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

      case "session:create":
        showNewSessionDialog();
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

  const showSessionListOverlay = () => {
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

  const showSessionSidebar = () => {
    sidebarActive = true;
    sidebarWidth = Math.min(
      config.sessionList.sidebarWidth,
      Math.floor(cols / 2),
    );

    const entries = sessions.map((s) => ({
      id: s.id,
      name: s.name,
      windowCount: s.windows.length,
      attached: s.attached,
      isActive: s.id === activeSession,
    }));

    sidebarState = createSessionSidebarState(entries, activeSession);

    // Resize server to reduced width (main area)
    const mainWidth = cols - sidebarWidth - 1;
    connection.send({ type: "resize", cols: mainWidth, rows });

    process.stdout.write(ansi.clearScreen());
    renderScreen();
  };

  const clearPreviewState = () => {
    previewPendingWrites = 0;
    previewTerminals.removeAll();
    previewKnownPaneIds = new Set();
    previewPaneRects = new Map();
    previewLayout = null;
  };

  const closeSidebar = (skipResize = false) => {
    if (!sidebarActive) return;

    // Stop preview
    if (previewSessionId) {
      connection.send({ type: "preview-stop" });
      previewSessionId = "";
      clearPreviewState();
    }

    sidebarActive = false;
    sidebarState = null;

    // Resize back to full width (skip if we're about to attach to a new session
    // which will trigger its own resize via handleAttach)
    if (!skipResize) {
      connection.send({ type: "resize", cols, rows });
    }

    process.stdout.write(ansi.clearScreen());
    renderScreen();
  };

  const updatePreview = () => {
    if (!sidebarState || !sidebarActive) return;

    const selected = sidebarState.sessions[sidebarState.selectedIndex];
    if (!selected) return;

    // If selected is the active session, no preview needed
    if (selected.id === activeSession) {
      if (previewSessionId && previewSessionId !== activeSession) {
        connection.send({ type: "preview-stop" });
        clearPreviewState();
        sidebarNeedsClear = true;
      }
      previewSessionId = "";
      scheduleRender();
      return;
    }

    // If already previewing this session, nothing to do
    if (previewSessionId === selected.id) return;

    // Stop old preview
    if (previewSessionId) {
      connection.send({ type: "preview-stop" });
      clearPreviewState();
    }

    // Switching preview session — need to clear main area
    sidebarNeedsClear = true;

    // Start new preview
    previewSessionId = selected.id;
    const mainWidth = cols - sidebarWidth - 1;
    connection.send({
      type: "preview",
      sessionId: selected.id,
      cols: mainWidth,
      rows,
    });
  };

  const handleSidebarInput = (data: Buffer) => {
    // If the input router is in prefix mode, forward directly so prefix
    // commands (e.g. C-a + Up for pane:focus-up) work while sidebar is open
    if (inputRouter.isPrefixActive()) {
      inputRouter.handleInput(data);
      return;
    }

    const bytes = Array.from(data);

    // Escape — close sidebar without switching
    if (bytes.length === 1 && bytes[0] === 0x1b) {
      closeSidebar();
      return;
    }

    // Enter — switch to selected session, close sidebar
    if (bytes.length === 1 && bytes[0] === 0x0d) {
      if (sidebarState) {
        const selected = sidebarState.sessions[sidebarState.selectedIndex];
        if (selected && selected.id !== activeSession) {
          // Skip resize in closeSidebar — the attach will send resize with full width
          closeSidebar(true);
          connection.send({ type: "resize", cols, rows });
          connection.send({ type: "attach", sessionId: selected.id });
        } else {
          closeSidebar();
        }
      }
      return;
    }

    // j or Arrow Down — move selection down
    if (
      (bytes.length === 1 && bytes[0] === 0x6a) || // 'j'
      (bytes.length === 3 &&
        bytes[0] === 0x1b &&
        bytes[1] === 0x5b &&
        bytes[2] === 0x42) // Arrow Down
    ) {
      if (
        sidebarState &&
        sidebarState.selectedIndex < sidebarState.sessions.length - 1
      ) {
        sidebarState.selectedIndex++;
        updatePreview();
        scheduleRender();
      }
      return;
    }

    // k or Arrow Up — move selection up
    if (
      (bytes.length === 1 && bytes[0] === 0x6b) || // 'k'
      (bytes.length === 3 &&
        bytes[0] === 0x1b &&
        bytes[1] === 0x5b &&
        bytes[2] === 0x41) // Arrow Up
    ) {
      if (sidebarState && sidebarState.selectedIndex > 0) {
        sidebarState.selectedIndex--;
        updatePreview();
        scheduleRender();
      }
      return;
    }

    // Only forward the prefix key to the input router so prefix commands
    // work while sidebar is open. All other input is swallowed (no PTY passthrough).
    if (data.length === 1 && data[0] === parsePrefixKey(config.prefixKey)) {
      inputRouter.handleInput(data);
    }
  };

  const showSessionList = () => {
    if (config.sessionList.mode === "overlay") {
      showSessionListOverlay();
    } else {
      showSessionSidebar();
    }
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

  const showNewSessionDialog = () => {
    showingOverlay = true;
    const dialogState: RenameDialogState = createRenameDialogState(
      "New Session",
      "",
    );

    const redrawDialog = () => {
      process.stdout.write(renderRenameDialog(dialogState, cols, rows));
    };

    const closeDialog = () => {
      process.stdin.removeListener("data", onNewSessionData);
      showingOverlay = false;
      process.stdout.write(ansi.clearScreen());
      renderScreen();
    };

    const onNewSessionData = (data: Buffer) => {
      const bytes = Array.from(data);

      // Escape — cancel
      if (bytes.length === 1 && bytes[0] === 0x1b) {
        closeDialog();
        return;
      }

      // Enter — create session
      if (bytes.length === 1 && bytes[0] === 0x0d) {
        const name = dialogState.value.trim();
        closeDialog();
        connection.send({
          type: "command",
          id: "session:create",
          args: name.length > 0 ? { name } : {},
        });
        return;
      }

      // Backspace
      if (bytes.length === 1 && bytes[0] === 0x7f) {
        if (dialogState.value.length > 0) {
          dialogState.value = dialogState.value.slice(0, -1);
          redrawDialog();
        }
        return;
      }

      // Ctrl+U — clear input
      if (bytes.length === 1 && bytes[0] === 0x15) {
        dialogState.value = "";
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
        dialogState.value += str;
        redrawDialog();
      }
    };

    process.stdin.on("data", onNewSessionData);
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
    previewTerminals.removeAll();
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
    if (sidebarActive) {
      handleSidebarInput(data);
      return;
    }
    inputRouter.handleInput(data);
  });

  // Handle terminal resize
  process.stdout.on("resize", () => {
    cols = process.stdout.columns || 80;
    rows = process.stdout.rows || 24;

    if (sidebarActive) {
      // Reclamp sidebar width
      sidebarWidth = Math.min(
        config.sessionList.sidebarWidth,
        Math.floor(cols / 2),
      );
      const mainWidth = cols - sidebarWidth - 1;
      connection.send({ type: "resize", cols: mainWidth, rows });

      // Re-request preview if active
      if (previewSessionId && previewSessionId !== activeSession) {
        connection.send({
          type: "preview",
          sessionId: previewSessionId,
          cols: mainWidth,
          rows,
        });
      }
    } else {
      connection.send({ type: "resize", cols, rows });
    }
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
