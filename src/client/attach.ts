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
import {
  getBorderChars,
  getLineChars,
  type BorderStyle,
  type LineStyle,
  type BorderSegment,
  type BorderCellMeta,
} from "../renderer/border.ts";
import { getAllPaneIds } from "../core/layout.ts";
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
  type NoteEditorState,
  createNoteEditorState,
  renderNoteEditor,
  getNoteContent,
} from "../ui/NoteEditor.ts";
import {
  type NotesListState,
  type NotesListEntry,
  createNotesListState,
  updateNotesFilter,
  renderNotesList,
} from "../ui/NotesList.ts";
import {
  createSessionSidebarState,
  updateSidebarSessions,
  renderSessionSidebar,
  renderPreviewBar,
} from "../ui/SessionSidebar.ts";
import type { SessionSidebarState } from "../ui/SessionSidebar.ts";
import {
  parseSgrMouse,
  encodeSgrMouse,
  getBaseButton,
  isScrollEvent,
  isMotionEvent,
  MOUSE_LEFT,
  MOUSE_SCROLL_UP,
  MOUSE_SCROLL_DOWN,
} from "../input/mouse.ts";
import {
  createSelectionState,
  resetSelection,
  renderSelection,
  extractSelectedText,
  copyToClipboard,
} from "./selection.ts";
import {
  createCopyModeState,
  handleCopyModeInput,
  handleCopyModeScroll,
  renderCopyModePane,
  refreshBufferInfo,
  ensureCursorVisible,
  type CopyModeState,
} from "./copy-mode.ts";
import { renderPrefixHelp } from "../ui/PrefixHelp.ts";
import {
  createConfigErrorDialogState,
  renderConfigErrorDialog,
} from "../ui/ConfigErrorDialog.ts";
import { ConfigWatcher } from "../config/watcher.ts";
import { findConfigFile } from "../config/loader.ts";
import { debugLog, setDebugEnabled } from "../debug.ts";

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
  initialConfig: MaxMuxConfig,
  sessionId?: string,
): Promise<void> {
  let config = initialConfig;
  setDebugEnabled(config.debug);
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
  let previousPaneId = "";
  let showingOverlay = false;
  let showingPrefixHelp = false;
  let prefixActive = false;

  // Process name tracking for conditional keybindings (unless clause)
  const paneProcesses: Map<string, string> = new Map();

  // Client-side virtual terminals for compositor rendering
  const clientTerminals = new TerminalManager();
  let knownPaneIds = new Set<string>();
  let currentLayout: LayoutNode | null = null;
  let renderTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingWrites = 0;

  // Selection state (mouse drag text selection)
  const selectionState = createSelectionState();

  // Copy-mode state
  let copyModeActive = false;
  let copyModeState: CopyModeState | null = null;

  // Bracketed paste relay state
  let outerBracketedPaste = false;

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
      prefixActive || copyModeActive,
      cols,
      rows,
    );

    if (output) {
      let out = ansi.hideCursor();
      out += output;
      // Don't restore cursor while overlays, sidebar, or copy-mode are open
      if (!showingOverlay && !sidebarActive && !copyModeActive) {
        const activeTerm = clientTerminals.get(activePaneId);
        if (activeTerm) {
          out += ansi.setCursorStyle(activeTerm.getCursorStyle());
          out += positionCursor();
          if (activeTerm.isCursorVisible()) {
            out += ansi.showCursor();
          }
        }
      }
      process.stdout.write(out);
    }
  };

  // --- Copy-mode ---

  const enterCopyMode = (paneId?: string) => {
    const targetPaneId = paneId || activePaneId;
    const term = clientTerminals.get(targetPaneId);
    if (!term) return;
    if (selectionState.phase !== "idle") resetSelection(selectionState);
    copyModeState = createCopyModeState(targetPaneId, term);
    copyModeActive = true;
    renderCopyMode();
  };

  const exitCopyMode = () => {
    copyModeActive = false;
    copyModeState = null;
    process.stdout.write(ansi.clearScreen());
    renderScreen();
  };

  const renderCopyMode = () => {
    if (!copyModeState) return;
    const term = clientTerminals.get(copyModeState.paneId);
    const paneRect = paneRects.get(copyModeState.paneId);
    if (!term || !paneRect) return;

    const xOffset =
      sidebarActive && config.sessionList.sidebarPosition === "left"
        ? sidebarWidth + 1
        : 0;

    let out = ansi.hideCursor();
    out += renderCopyModePane(copyModeState, term, paneRect, xOffset);

    // Render other panes normally
    for (const [pid] of paneRects) {
      if (pid !== copyModeState.paneId) {
        out += renderPaneContent(pid, undefined, undefined, xOffset);
      }
    }

    out += renderBorders();
    process.stdout.write(out);
    drawStatusBar();
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

    const paneReachesRightEdge = rect.x + xOffset + rect.width >= cols;

    let out = "";
    for (let y = 0; y < paneHeight; y++) {
      if (y > 0 && paneReachesRightEdge && term.isLineWrapped(y)) {
        // Skip moveTo — let content flow naturally for soft-wrap URL detection.
        // After the previous full-width line, cursor auto-wraps to next row.
      } else {
        out += ansi.moveTo(rect.x + xOffset, rect.y + y);
      }
      out += term.renderLine(y);
    }
    return out;
  };

  // Collect all border cells from the layout tree with segment metadata
  const collectBorderCells = (
    node: LayoutNode,
    bounds: Rect,
    cells: Map<string, BorderCellMeta>,
  ): void => {
    if (node.type === "leaf") return;

    const { direction, ratio, children } = node;
    const firstChildPanes = getAllPaneIds(children[0]);
    const secondChildPanes = getAllPaneIds(children[1]);

    if (direction === "horizontal") {
      const splitX = Math.floor(bounds.x + bounds.width * ratio);
      const segment: BorderSegment = {
        orientation: "v",
        fixedCoord: splitX,
        start: bounds.y,
        end: bounds.y + bounds.height,
        firstChildPanes,
        secondChildPanes,
      };
      for (let y = bounds.y; y < bounds.y + bounds.height; y++) {
        const key = `${splitX},${y}`;
        const existing = cells.get(key);
        if (existing) {
          existing.segments.push(segment);
        } else {
          cells.set(key, { x: splitX, y, segments: [segment] });
        }
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
      const splitY = Math.floor(bounds.y + bounds.height * ratio);
      const segment: BorderSegment = {
        orientation: "h",
        fixedCoord: splitY,
        start: bounds.x,
        end: bounds.x + bounds.width,
        firstChildPanes,
        secondChildPanes,
      };
      for (let x = bounds.x; x < bounds.x + bounds.width; x++) {
        const key = `${x},${splitY}`;
        const existing = cells.get(key);
        if (existing) {
          existing.segments.push(segment);
        } else {
          cells.set(key, { x, y: splitY, segments: [segment] });
        }
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

  // Compute half-border color for a cell based on segment metadata
  const computeHalfBorderColor = (
    meta: BorderCellMeta,
    activePaneForBorders: string,
    borderFg: string,
    activeBorderFg: string,
  ): string => {
    for (const seg of meta.segments) {
      const inFirst = seg.firstChildPanes.includes(activePaneForBorders);
      const inSecond = seg.secondChildPanes.includes(activePaneForBorders);
      if (!inFirst && !inSecond) continue;

      const midpoint = Math.floor((seg.start + seg.end) / 2);
      // Variable axis coordinate: y for vertical lines, x for horizontal lines
      const coord = seg.orientation === "v" ? meta.y : meta.x;

      if (seg.orientation === "v") {
        // Vertical line (left|right split)
        // firstChild = left, secondChild = right
        if (inFirst && coord < midpoint) return activeBorderFg;
        if (inSecond && coord >= midpoint) return activeBorderFg;
      } else {
        // Horizontal line (top|bottom split)
        // firstChild = top, secondChild = bottom
        if (inFirst && coord < midpoint) return activeBorderFg;
        if (inSecond && coord >= midpoint) return activeBorderFg;
      }
    }
    return borderFg;
  };

  // Check if a cell is directly adjacent to the active pane (for junctions)
  const isAdjacentToActive = (
    x: number,
    y: number,
    rects: Map<string, Rect>,
    activePaneForBorders: string,
  ): boolean => {
    const activeRect = rects.get(activePaneForBorders);
    if (!activeRect) return false;
    // Check if (x,y) is on any edge of the active pane rect
    const adjRight =
      x === activeRect.x + activeRect.width &&
      y >= activeRect.y &&
      y < activeRect.y + activeRect.height;
    const adjLeft =
      x === activeRect.x - 1 &&
      y >= activeRect.y &&
      y < activeRect.y + activeRect.height;
    const adjBottom =
      y === activeRect.y + activeRect.height &&
      x >= activeRect.x &&
      x < activeRect.x + activeRect.width;
    const adjTop =
      y === activeRect.y - 1 &&
      x >= activeRect.x &&
      x < activeRect.x + activeRect.width;
    return adjRight || adjLeft || adjBottom || adjTop;
  };

  const renderBordersFor = (
    layout: LayoutNode | null,
    rects: Map<string, Rect>,
    boundsWidth: number,
    activePaneForBorders: string,
    xOffset = 0,
  ): string => {
    if (rects.size <= 1 || !layout) return "";

    const borderChars = getBorderChars(
      config.theme.border.style as BorderStyle,
    );
    const lineChars = getLineChars(config.theme.border.lineStyle as LineStyle);
    const borderFg = config.theme.border.fg;
    const activeBorderFg = config.theme.border.activeFg;
    const contentHeight = rows - 1;

    // Collect all border cells with segment metadata
    const cells = new Map<string, BorderCellMeta>();
    collectBorderCells(
      layout,
      { x: 0, y: 0, width: boundsWidth, height: contentHeight },
      cells,
    );

    // Build a Set of keys for neighbor lookup
    const cellKeys = new Set(cells.keys());

    let out = "";
    let currentColor = "";

    for (const [key, meta] of cells) {
      const { x, y } = meta;
      if (y >= contentHeight) continue;

      const hasUp = cellKeys.has(`${x},${y - 1}`);
      const hasDown = cellKeys.has(`${x},${y + 1}`);
      const hasLeft = cellKeys.has(`${x - 1},${y}`);
      const hasRight = cellKeys.has(`${x + 1},${y}`);

      // Determine if this is a junction (T-piece, cross, corner) or straight line
      const neighbors =
        (hasUp ? 1 : 0) +
        (hasDown ? 1 : 0) +
        (hasLeft ? 1 : 0) +
        (hasRight ? 1 : 0);
      const isStraightVertical = hasUp && hasDown && !hasLeft && !hasRight;
      const isStraightHorizontal = hasLeft && hasRight && !hasUp && !hasDown;
      const isStraight =
        isStraightVertical ||
        isStraightHorizontal ||
        (neighbors <= 1 && (hasUp || hasDown || hasLeft || hasRight));

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
        ch = lineChars.vertical;
      } else if (hasLeft && hasRight) {
        ch = lineChars.horizontal;
      } else if (hasDown && hasRight) {
        ch = borderChars.topLeft;
      } else if (hasDown && hasLeft) {
        ch = borderChars.topRight;
      } else if (hasUp && hasRight) {
        ch = borderChars.bottomLeft;
      } else if (hasUp && hasLeft) {
        ch = borderChars.bottomRight;
      } else if (hasUp || hasDown) {
        ch = lineChars.vertical;
      } else {
        ch = lineChars.horizontal;
      }

      // Color: straight segments use half-border, junctions use adjacency check
      let color: string;
      if (isStraight || neighbors <= 1) {
        color = computeHalfBorderColor(
          meta,
          activePaneForBorders,
          borderFg,
          activeBorderFg,
        );
      } else {
        color = isAdjacentToActive(x, y, rects, activePaneForBorders)
          ? activeBorderFg
          : borderFg;
      }

      const colorSwitch = color !== currentColor ? ansi.fgHex(color) : "";
      currentColor = color;

      out += ansi.moveTo(x + xOffset, y) + colorSwitch + ch;
    }

    out += ansi.resetStyle();
    return out;
  };

  const renderBorders = (): string => {
    return renderBordersFor(currentLayout, paneRects, cols, activePaneId);
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
    if (showingOverlay || showingPrefixHelp) return;

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
      const sidebarActivePane = isPreviewActive ? "" : activePaneId;
      out += renderBordersFor(
        useLayout,
        useRects,
        mainWidth,
        sidebarActivePane,
        mainXOffset,
      );

      // Render selection highlight in main area if active (non-preview only)
      if (!isPreviewActive && selectionState.phase !== "idle") {
        const selTerm = clientTerminals.get(selectionState.paneId);
        const selRect = paneRects.get(selectionState.paneId);
        if (selTerm && selRect) {
          out += renderSelection(selectionState, selTerm, selRect, mainXOffset);
        }
      }

      // Render preview bar if previewing a non-active session
      if (isPreviewActive) {
        const previewSession = sessions.find((s) => s.id === previewSessionId);
        if (previewSession) {
          const previewWindows = previewSession.windows.map((w, i) => ({
            name: w.name,
            index: i,
            isActive: w.id === previewSession.activeWindow,
          }));
          out += renderPreviewBar(
            previewSession.name,
            previewWindows,
            mainXOffset,
            mainWidth,
            rows - 2, // row above status bar
            {
              fg: config.theme.statusBar.fg,
              bg: config.theme.statusBar.bg,
              activeFg: config.theme.statusBar.active,
              borderFg: config.theme.border.fg,
            },
          );
        }
      }

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

      // Keep cursor hidden while sidebar is open — focus is on sidebar navigation
      // (both preview and non-preview modes)
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

      // Render selection highlight if active
      if (selectionState.phase !== "idle") {
        const selTerm = clientTerminals.get(selectionState.paneId);
        const selRect = paneRects.get(selectionState.paneId);
        if (selTerm && selRect) {
          out += renderSelection(selectionState, selTerm, selRect, 0);
        }
      }

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
    syncBracketedPaste();
  };

  const syncBracketedPaste = () => {
    const activeTerm = clientTerminals.get(activePaneId);
    const want = activeTerm ? activeTerm.isBracketedPasteActive() : false;
    if (want !== outerBracketedPaste) {
      outerBracketedPaste = want;
      process.stdout.write(
        want ? ansi.enableBracketedPaste() : ansi.disableBracketedPaste(),
      );
    }
  };

  const scheduleRender = () => {
    if (renderTimer || showingOverlay || showingPrefixHelp) return;
    renderTimer = setTimeout(() => {
      renderTimer = null;
      if (pendingWrites > 0 || previewPendingWrites > 0) {
        // xterm.js still processing writes — retry until ready
        scheduleRender();
        return;
      }
      if (copyModeActive) {
        renderCopyMode();
      } else {
        renderScreen();
      }
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
        clientTerminals.create(paneId, w, h, config.historyLimit);
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
              if (
                copyModeActive &&
                copyModeState &&
                copyModeState.paneId === msg.paneId
              ) {
                refreshBufferInfo(copyModeState, term);
              }
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
            const newActivePaneId = activeWindow?.activePane || "";
            if (newActivePaneId !== activePaneId && activePaneId !== "") {
              previousPaneId = activePaneId;
            }
            activePaneId = newActivePaneId;
            debugLog("client", `activePane=${activePaneId}`);

            // Session has no windows left — server should have migrated us.
            // If we still see an empty session, try to attach to another one.
            // Only exit if there are truly no sessions left.
            if (session.windows.length === 0) {
              const fallback = sessions.find(
                (s) => s.id !== activeSession && s.windows.length > 0,
              );
              if (fallback) {
                connection.send({ type: "attach", sessionId: fallback.id });
              } else {
                cleanup();
                process.stdout.write("\r\n[exited]\r\n");
                process.exit(0);
              }
            }
          } else {
            // Our session was deleted — server should have migrated us.
            // Try to attach to any available session as fallback.
            const fallback = sessions.find((s) => s.windows.length > 0);
            if (fallback) {
              connection.send({ type: "attach", sessionId: fallback.id });
            } else {
              cleanup();
              process.stdout.write("\r\n[exited]\r\n");
              process.exit(0);
            }
          }

          // Update sidebar sessions if sidebar is open
          if (sidebarActive && sidebarState) {
            const sidebarEntries = (msg.sessions as SessionInfo[]).map((s) => ({
              id: s.id,
              name: s.name,
              windowCount: s.windows.length,
              windows: s.windows.map((w, i) => ({
                name: w.name,
                index: i,
                isActive: w.id === s.activeWindow,
              })),
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

        case "cursor-state": {
          const panes = msg.panes as Record<
            string,
            { cursorVisible: boolean; cursorStyle: number }
          >;
          for (const [paneId, state] of Object.entries(panes)) {
            const term = clientTerminals.get(paneId);
            if (term) {
              term.setCursorVisible(state.cursorVisible);
              term.setCursorStyle(state.cursorStyle);
            }
          }
          scheduleRender();
          break;
        }

        case "pane:exited":
          paneRects.delete(msg.paneId);
          clientTerminals.remove(msg.paneId);
          knownPaneIds.delete(msg.paneId);
          paneProcesses.delete(msg.paneId);
          debugLog("client", `pane exited: ${msg.paneId}`);
          scheduleRender();
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

        case "process-info": {
          const panes = msg.panes as Record<string, string>;
          if (msg.full) {
            // Full refresh from sendStateToClient — replace to clear stale entries
            paneProcesses.clear();
          }
          for (const [paneId, name] of Object.entries(panes)) {
            paneProcesses.set(paneId, name);
          }
          break;
        }

        case "metrics":
          statusBarRenderer.updateMetrics(msg.data as SystemMetrics);
          drawStatusBar();
          break;

        case "notes:data":
          showNotesList((msg as any).notes);
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
          if (showingPrefixHelp) {
            showingPrefixHelp = false;
            process.stdout.write(ansi.clearScreen());
            renderScreen();
          }
          handleCommand(action.commandId);
          break;

        case "prefix-activated":
          prefixActive = true;
          if (
            config.showPrefixHelp &&
            !showingOverlay &&
            !showingPrefixHelp &&
            !copyModeActive &&
            !sidebarActive
          ) {
            showingPrefixHelp = true;
            const bindings = keybindings.list();
            process.stdout.write(
              ansi.hideCursor() + renderPrefixHelp(bindings, cols, rows),
            );
          }
          drawStatusBar();
          break;

        case "prefix-timeout":
          if (showingPrefixHelp) {
            showingPrefixHelp = false;
            process.stdout.write(ansi.clearScreen());
            renderScreen();
          }
          prefixActive = false;
          drawStatusBar();
          break;
      }
    },
    () => paneProcesses.get(activePaneId),
  );

  // --- Live Config Reload ---

  const configWatcher = new ConfigWatcher(
    findConfigFile(),
    (newConfig) => {
      config = newConfig;
      setDebugEnabled(config.debug);
      // Reload keybindings
      keybindings.clear();
      keybindings.loadFromConfig(config.keybindings);
      globalKeybindings.clear();
      globalKeybindings.loadFromConfig(config.globalKeybindings);
      // Update InputRouter prefix settings
      inputRouter.updateConfig(config.prefixKey, config.prefixTimeout);
      // Update StatusBar
      statusBarRenderer.updateConfig(config.statusBar);
      // Full re-render
      process.stdout.write(ansi.clearScreen());
      renderScreen();
    },
    (errorMessage) => {
      // Show error dialog overlay
      showingOverlay = true;
      const errorState = createConfigErrorDialogState(errorMessage);
      process.stdout.write(
        ansi.hideCursor() + renderConfigErrorDialog(errorState, cols, rows),
      );
      const onDismiss = () => {
        process.stdin.removeListener("data", onDismiss);
        showingOverlay = false;
        process.stdout.write(ansi.clearScreen());
        renderScreen();
      };
      process.stdin.on("data", onDismiss);
    },
  );
  configWatcher.start();

  const handleCommand = (commandId: string) => {
    // Reset prefix state after command execution
    prefixActive = false;
    drawStatusBar();

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
        debugLog(
          "nav",
          `dir=${dir} active=${activePaneId} inRects=${paneRects.has(activePaneId)} rectsKeys=[${[...paneRects.keys()].join(",")}] proc=${paneProcesses.get(activePaneId)}`,
        );
        const targetId = findPaneInDirection(
          paneRects,
          activePaneId,
          dir,
          previousPaneId,
        );
        debugLog("nav", `target=${targetId}`);
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

      case "copy-mode:enter":
        enterCopyMode();
        return;

      case "notes:create":
        showNoteEditor(null, "");
        return;

      case "notes:list":
        connection.send({ type: "notes:list" } as any);
        return;
    }

    // Forward to server
    connection.send({ type: "command", id: commandId });
    drawStatusBar();
  };

  const showKeybindingsOverlay = () => {
    showingOverlay = true;
    process.stdout.write(
      ansi.hideCursor() + ansi.clearScreen() + ansi.moveToOrigin(),
    );
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
    process.stdout.write(
      ansi.hideCursor() + ansi.clearScreen() + ansi.moveToOrigin(),
    );
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
    // Clear any active selection when opening sidebar
    if (selectionState.phase !== "idle") {
      resetSelection(selectionState);
    }
    sidebarActive = true;
    sidebarWidth = Math.min(
      config.sessionList.sidebarWidth,
      Math.floor(cols / 2),
    );

    const entries = sessions.map((s) => ({
      id: s.id,
      name: s.name,
      windowCount: s.windows.length,
      windows: s.windows.map((w, i) => ({
        name: w.name,
        index: i,
        isActive: w.id === s.activeWindow,
      })),
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

    // Start new preview (rows - 1 to leave space for preview bar)
    previewSessionId = selected.id;
    const mainWidth = cols - sidebarWidth - 1;
    connection.send({
      type: "preview",
      sessionId: selected.id,
      cols: mainWidth,
      rows: rows - 1,
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

    let prevFinderHeight = 0;

    const redrawFinder = () => {
      const maxItems = Math.min(finderState.filtered.length, rows - 8);
      const width = Math.min(50, cols - 4);
      const height = Math.max(6, maxItems + 5);

      // Clear leftover rows from the previous (taller) render
      if (prevFinderHeight > height) {
        const x = Math.floor((cols - width) / 2);
        const y = Math.floor((rows - height) / 2);
        const prevY = Math.floor((rows - prevFinderHeight) / 2);
        const blank = " ".repeat(width);
        let clear = "";
        // Clear rows above the new box (old box started higher)
        for (let row = prevY; row < y; row++) {
          clear += ansi.moveTo(x, row) + blank;
        }
        // Clear rows below the new box (old box extended lower)
        const newBottom = y + height;
        const prevBottom = prevY + prevFinderHeight;
        for (let row = newBottom; row < prevBottom; row++) {
          clear += ansi.moveTo(x, row) + blank;
        }
        if (clear) {
          process.stdout.write(clear + ansi.resetStyle());
        }
      }

      prevFinderHeight = height;
      process.stdout.write(
        ansi.hideCursor() + renderSessionFinder(finderState, cols, rows),
      );
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
      process.stdout.write(
        ansi.hideCursor() + renderRenameDialog(renameState, cols, rows),
      );
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
      process.stdout.write(
        ansi.hideCursor() + renderRenameDialog(dialogState, cols, rows),
      );
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

  const showNoteEditor = (noteId: string | null, content: string) => {
    showingOverlay = true;
    const editorState = createNoteEditorState(noteId, content);

    const redrawEditor = () => {
      process.stdout.write(
        ansi.hideCursor() + renderNoteEditor(editorState, cols, rows),
      );
    };

    const closeEditor = (save: boolean) => {
      process.stdin.removeListener("data", onEditorData);
      if (save) {
        const noteContent = getNoteContent(editorState);
        connection.send({
          type: "notes:save",
          noteId: editorState.noteId ?? undefined,
          content: noteContent,
        } as any);
      }
      showingOverlay = false;
      process.stdout.write(ansi.clearScreen());
      renderScreen();
    };

    const onEditorData = (data: Buffer) => {
      const bytes = Array.from(data);

      // Escape — save & close
      if (bytes.length === 1 && bytes[0] === 0x1b) {
        closeEditor(true);
        return;
      }

      // Ctrl+S — save & close
      if (bytes.length === 1 && bytes[0] === 0x13) {
        closeEditor(true);
        return;
      }

      // Enter — new line
      if (bytes.length === 1 && bytes[0] === 0x0d) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        const before = line.slice(0, editorState.cursorCol);
        const after = line.slice(editorState.cursorCol);
        editorState.lines[editorState.cursorRow] = before;
        editorState.lines.splice(editorState.cursorRow + 1, 0, after);
        editorState.cursorRow++;
        editorState.cursorCol = 0;
        redrawEditor();
        return;
      }

      // Backspace
      if (bytes.length === 1 && bytes[0] === 0x7f) {
        if (editorState.cursorCol > 0) {
          const line = editorState.lines[editorState.cursorRow] ?? "";
          editorState.lines[editorState.cursorRow] =
            line.slice(0, editorState.cursorCol - 1) +
            line.slice(editorState.cursorCol);
          editorState.cursorCol--;
        } else if (editorState.cursorRow > 0) {
          const prevLine = editorState.lines[editorState.cursorRow - 1] ?? "";
          const curLine = editorState.lines[editorState.cursorRow] ?? "";
          editorState.cursorCol = prevLine.length;
          editorState.lines[editorState.cursorRow - 1] = prevLine + curLine;
          editorState.lines.splice(editorState.cursorRow, 1);
          editorState.cursorRow--;
        }
        redrawEditor();
        return;
      }

      // Arrow Up
      if (
        bytes.length === 3 &&
        bytes[0] === 0x1b &&
        bytes[1] === 0x5b &&
        bytes[2] === 0x41
      ) {
        if (editorState.cursorRow > 0) {
          editorState.cursorRow--;
          editorState.cursorCol = Math.min(
            editorState.cursorCol,
            (editorState.lines[editorState.cursorRow] ?? "").length,
          );
        }
        redrawEditor();
        return;
      }

      // Arrow Down
      if (
        bytes.length === 3 &&
        bytes[0] === 0x1b &&
        bytes[1] === 0x5b &&
        bytes[2] === 0x42
      ) {
        if (editorState.cursorRow < editorState.lines.length - 1) {
          editorState.cursorRow++;
          editorState.cursorCol = Math.min(
            editorState.cursorCol,
            (editorState.lines[editorState.cursorRow] ?? "").length,
          );
        }
        redrawEditor();
        return;
      }

      // Arrow Right
      if (
        bytes.length === 3 &&
        bytes[0] === 0x1b &&
        bytes[1] === 0x5b &&
        bytes[2] === 0x43
      ) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        if (editorState.cursorCol < line.length) {
          editorState.cursorCol++;
        } else if (editorState.cursorRow < editorState.lines.length - 1) {
          editorState.cursorRow++;
          editorState.cursorCol = 0;
        }
        redrawEditor();
        return;
      }

      // Arrow Left
      if (
        bytes.length === 3 &&
        bytes[0] === 0x1b &&
        bytes[1] === 0x5b &&
        bytes[2] === 0x44
      ) {
        if (editorState.cursorCol > 0) {
          editorState.cursorCol--;
        } else if (editorState.cursorRow > 0) {
          editorState.cursorRow--;
          editorState.cursorCol = (
            editorState.lines[editorState.cursorRow] ?? ""
          ).length;
        }
        redrawEditor();
        return;
      }

      // Tab — insert 2 spaces
      if (bytes.length === 1 && bytes[0] === 0x09) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        editorState.lines[editorState.cursorRow] =
          line.slice(0, editorState.cursorCol) +
          "  " +
          line.slice(editorState.cursorCol);
        editorState.cursorCol += 2;
        redrawEditor();
        return;
      }

      // Ctrl+A — move to beginning of line
      if (bytes.length === 1 && bytes[0] === 0x01) {
        editorState.cursorCol = 0;
        redrawEditor();
        return;
      }

      // Ctrl+E — move to end of line
      if (bytes.length === 1 && bytes[0] === 0x05) {
        editorState.cursorCol = (
          editorState.lines[editorState.cursorRow] ?? ""
        ).length;
        redrawEditor();
        return;
      }

      // Ctrl+B — move back one character
      if (bytes.length === 1 && bytes[0] === 0x02) {
        if (editorState.cursorCol > 0) {
          editorState.cursorCol--;
        } else if (editorState.cursorRow > 0) {
          editorState.cursorRow--;
          editorState.cursorCol = (
            editorState.lines[editorState.cursorRow] ?? ""
          ).length;
        }
        redrawEditor();
        return;
      }

      // Ctrl+F — move forward one character
      if (bytes.length === 1 && bytes[0] === 0x06) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        if (editorState.cursorCol < line.length) {
          editorState.cursorCol++;
        } else if (editorState.cursorRow < editorState.lines.length - 1) {
          editorState.cursorRow++;
          editorState.cursorCol = 0;
        }
        redrawEditor();
        return;
      }

      // Ctrl+D — delete character under cursor (forward delete)
      if (bytes.length === 1 && bytes[0] === 0x04) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        if (editorState.cursorCol < line.length) {
          editorState.lines[editorState.cursorRow] =
            line.slice(0, editorState.cursorCol) +
            line.slice(editorState.cursorCol + 1);
        } else if (editorState.cursorRow < editorState.lines.length - 1) {
          const nextLine = editorState.lines[editorState.cursorRow + 1] ?? "";
          editorState.lines[editorState.cursorRow] = line + nextLine;
          editorState.lines.splice(editorState.cursorRow + 1, 1);
        }
        redrawEditor();
        return;
      }

      // Ctrl+K — kill from cursor to end of line
      if (bytes.length === 1 && bytes[0] === 0x0b) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        if (editorState.cursorCol < line.length) {
          editorState.lines[editorState.cursorRow] = line.slice(
            0,
            editorState.cursorCol,
          );
        } else if (editorState.cursorRow < editorState.lines.length - 1) {
          const nextLine = editorState.lines[editorState.cursorRow + 1] ?? "";
          editorState.lines[editorState.cursorRow] = line + nextLine;
          editorState.lines.splice(editorState.cursorRow + 1, 1);
        }
        redrawEditor();
        return;
      }

      // Ctrl+U — delete from cursor to beginning of line
      if (bytes.length === 1 && bytes[0] === 0x15) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        editorState.lines[editorState.cursorRow] = line.slice(
          editorState.cursorCol,
        );
        editorState.cursorCol = 0;
        redrawEditor();
        return;
      }

      // Ctrl+W — delete word before cursor
      if (bytes.length === 1 && bytes[0] === 0x17) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        if (editorState.cursorCol > 0) {
          const before = line.slice(0, editorState.cursorCol);
          const after = line.slice(editorState.cursorCol);
          const trimmed = before.replace(/\s+$/, "");
          const wordStart = trimmed.search(/\S+$/);
          const newCol = wordStart === -1 ? 0 : wordStart;
          editorState.lines[editorState.cursorRow] =
            line.slice(0, newCol) + after;
          editorState.cursorCol = newCol;
        }
        redrawEditor();
        return;
      }

      // Ctrl+H — delete character before cursor (same as backspace)
      if (bytes.length === 1 && bytes[0] === 0x08) {
        if (editorState.cursorCol > 0) {
          const line = editorState.lines[editorState.cursorRow] ?? "";
          editorState.lines[editorState.cursorRow] =
            line.slice(0, editorState.cursorCol - 1) +
            line.slice(editorState.cursorCol);
          editorState.cursorCol--;
        } else if (editorState.cursorRow > 0) {
          const prevLine = editorState.lines[editorState.cursorRow - 1] ?? "";
          const curLine = editorState.lines[editorState.cursorRow] ?? "";
          editorState.cursorCol = prevLine.length;
          editorState.lines[editorState.cursorRow - 1] = prevLine + curLine;
          editorState.lines.splice(editorState.cursorRow, 1);
          editorState.cursorRow--;
        }
        redrawEditor();
        return;
      }

      // Ctrl+T — transpose characters
      if (bytes.length === 1 && bytes[0] === 0x14) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        if (editorState.cursorCol > 0 && editorState.cursorCol <= line.length) {
          const pos =
            editorState.cursorCol < line.length
              ? editorState.cursorCol
              : editorState.cursorCol - 1;
          if (pos > 0) {
            const chars = line.split("");
            const tmp = chars[pos - 1]!;
            chars[pos - 1] = chars[pos]!;
            chars[pos] = tmp;
            editorState.lines[editorState.cursorRow] = chars.join("");
            editorState.cursorCol = Math.min(pos + 1, line.length);
          }
        }
        redrawEditor();
        return;
      }

      // Ctrl+L — redraw editor
      if (bytes.length === 1 && bytes[0] === 0x0c) {
        process.stdout.write(ansi.clearScreen());
        redrawEditor();
        return;
      }

      // Alt+B — move back one word
      if (bytes.length === 2 && bytes[0] === 0x1b && bytes[1] === 0x62) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        if (editorState.cursorCol > 0) {
          let pos = editorState.cursorCol - 1;
          while (pos > 0 && /\s/.test(line[pos] ?? "")) pos--;
          while (pos > 0 && /\S/.test(line[pos - 1] ?? "")) pos--;
          editorState.cursorCol = pos;
        } else if (editorState.cursorRow > 0) {
          editorState.cursorRow--;
          editorState.cursorCol = (
            editorState.lines[editorState.cursorRow] ?? ""
          ).length;
        }
        redrawEditor();
        return;
      }

      // Alt+F — move forward one word
      if (bytes.length === 2 && bytes[0] === 0x1b && bytes[1] === 0x66) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        if (editorState.cursorCol < line.length) {
          let pos = editorState.cursorCol;
          while (pos < line.length && /\S/.test(line[pos] ?? "")) pos++;
          while (pos < line.length && /\s/.test(line[pos] ?? "")) pos++;
          editorState.cursorCol = pos;
        } else if (editorState.cursorRow < editorState.lines.length - 1) {
          editorState.cursorRow++;
          editorState.cursorCol = 0;
        }
        redrawEditor();
        return;
      }

      // Alt+D — delete word after cursor
      if (bytes.length === 2 && bytes[0] === 0x1b && bytes[1] === 0x64) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        if (editorState.cursorCol < line.length) {
          let pos = editorState.cursorCol;
          while (pos < line.length && /\s/.test(line[pos] ?? "")) pos++;
          while (pos < line.length && /\S/.test(line[pos] ?? "")) pos++;
          editorState.lines[editorState.cursorRow] =
            line.slice(0, editorState.cursorCol) + line.slice(pos);
        } else if (editorState.cursorRow < editorState.lines.length - 1) {
          const nextLine = editorState.lines[editorState.cursorRow + 1] ?? "";
          editorState.lines[editorState.cursorRow] = line + nextLine;
          editorState.lines.splice(editorState.cursorRow + 1, 1);
        }
        redrawEditor();
        return;
      }

      // Alt+Backspace — delete word before cursor
      if (bytes.length === 2 && bytes[0] === 0x1b && bytes[1] === 0x7f) {
        const line = editorState.lines[editorState.cursorRow] ?? "";
        if (editorState.cursorCol > 0) {
          const before = line.slice(0, editorState.cursorCol);
          const after = line.slice(editorState.cursorCol);
          const trimmed = before.replace(/\s+$/, "");
          const wordStart = trimmed.search(/\S+$/);
          const newCol = wordStart === -1 ? 0 : wordStart;
          editorState.lines[editorState.cursorRow] =
            line.slice(0, newCol) + after;
          editorState.cursorCol = newCol;
        }
        redrawEditor();
        return;
      }

      // Ctrl+P — move up one line (same as Arrow Up)
      if (bytes.length === 1 && bytes[0] === 0x10) {
        if (editorState.cursorRow > 0) {
          editorState.cursorRow--;
          editorState.cursorCol = Math.min(
            editorState.cursorCol,
            (editorState.lines[editorState.cursorRow] ?? "").length,
          );
        }
        redrawEditor();
        return;
      }

      // Ctrl+N — move down one line (same as Arrow Down)
      if (bytes.length === 1 && bytes[0] === 0x0e) {
        if (editorState.cursorRow < editorState.lines.length - 1) {
          editorState.cursorRow++;
          editorState.cursorCol = Math.min(
            editorState.cursorCol,
            (editorState.lines[editorState.cursorRow] ?? "").length,
          );
        }
        redrawEditor();
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
        const line = editorState.lines[editorState.cursorRow] ?? "";
        editorState.lines[editorState.cursorRow] =
          line.slice(0, editorState.cursorCol) +
          str +
          line.slice(editorState.cursorCol);
        editorState.cursorCol += str.length;
        redrawEditor();
      }
    };

    process.stdin.on("data", onEditorData);
    redrawEditor();
  };

  const showNotesList = (notes: NotesListEntry[]) => {
    showingOverlay = true;
    const listState = createNotesListState(notes);

    const redrawList = () => {
      process.stdout.write(
        ansi.hideCursor() + renderNotesList(listState, cols, rows),
      );
    };

    const closeList = () => {
      process.stdin.removeListener("data", onListData);
      showingOverlay = false;
      process.stdout.write(ansi.clearScreen());
      renderScreen();
    };

    const onListData = (data: Buffer) => {
      const bytes = Array.from(data);

      // Escape
      if (bytes.length === 1 && bytes[0] === 0x1b) {
        closeList();
        return;
      }

      // Delete confirmation mode
      if (listState.confirmDelete) {
        if (bytes.length === 1 && bytes[0] === 0x79) {
          // 'y'
          const note = listState.filtered[listState.selectedIndex];
          if (note) {
            connection.send({ type: "notes:delete", noteId: note.id } as any);
            // Remove from both allNotes and filtered
            const allIdx = listState.allNotes.indexOf(note);
            if (allIdx !== -1) listState.allNotes.splice(allIdx, 1);
            listState.filtered.splice(listState.selectedIndex, 1);
            if (
              listState.selectedIndex >= listState.filtered.length &&
              listState.selectedIndex > 0
            ) {
              listState.selectedIndex--;
            }
          }
          listState.confirmDelete = false;
          redrawList();
        } else {
          listState.confirmDelete = false;
          redrawList();
        }
        return;
      }

      // Enter — open note in editor
      if (bytes.length === 1 && bytes[0] === 0x0d) {
        const note = listState.filtered[listState.selectedIndex];
        if (note) {
          closeList();
          showNoteEditor(note.id, note.content);
        }
        return;
      }

      // Arrow Up
      if (
        bytes.length === 3 &&
        bytes[0] === 0x1b &&
        bytes[1] === 0x5b &&
        bytes[2] === 0x41
      ) {
        if (listState.selectedIndex > 0) {
          listState.selectedIndex--;
          redrawList();
        }
        return;
      }

      // Arrow Down
      if (
        bytes.length === 3 &&
        bytes[0] === 0x1b &&
        bytes[1] === 0x5b &&
        bytes[2] === 0x42
      ) {
        if (listState.selectedIndex < listState.filtered.length - 1) {
          listState.selectedIndex++;
          redrawList();
        }
        return;
      }

      // Backspace — delete last query character
      if (bytes.length === 1 && bytes[0] === 0x7f) {
        if (listState.query.length > 0) {
          listState.query = listState.query.slice(0, -1);
          updateNotesFilter(listState);
          redrawList();
        }
        return;
      }

      // Printable ASCII characters — append to query
      if (bytes.length === 1 && bytes[0]! >= 0x20 && bytes[0]! < 0x7f) {
        listState.query += String.fromCharCode(bytes[0]!);
        updateNotesFilter(listState);
        redrawList();
        return;
      }
    };

    process.stdin.on("data", onListData);
    redrawList();
  };

  // Cleanup
  let cleaned = false;
  const cleanup = () => {
    if (cleaned) return;
    cleaned = true;
    if (renderTimer) clearTimeout(renderTimer);
    configWatcher.stop();
    inputRouter.destroy();
    clientTerminals.removeAll();
    previewTerminals.removeAll();
    try {
      process.stdin.setRawMode?.(false);
    } catch {}
    process.stdin.pause();
    if (config.mouse) {
      process.stdout.write(ansi.disableMouse());
    }
    if (outerBracketedPaste) {
      process.stdout.write(ansi.disableBracketedPaste());
    }
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
  if (config.mouse) {
    process.stdout.write(ansi.enableMouse());
  }

  // Send initial resize then attach
  connection.send({ type: "resize", cols, rows });
  connection.send({ type: "attach", sessionId, cwd: process.cwd() });

  // Mouse event handler — routes mouse clicks to the correct pane
  // Implements drag-to-select text with copy-to-clipboard on release
  const handleMouseEvent = (event: import("../input/mouse.ts").MouseEvent) => {
    const baseButton = getBaseButton(event.button);
    const motion = isMotionEvent(event.button);

    // Calculate sidebar offset
    const xOffset =
      sidebarActive && config.sessionList.sidebarPosition === "left"
        ? sidebarWidth + 1
        : 0;

    // Adjust screen coordinates for sidebar offset
    const screenX = event.x - xOffset;
    const screenY = event.y;

    // Ignore clicks on sidebar area
    if (sidebarActive && event.x < xOffset) return;
    // Ignore clicks on status bar row
    if (screenY >= rows - 1) return;

    // Determine which rects/terminals to use (preview vs main)
    const isPreviewActive =
      sidebarActive &&
      previewSessionId &&
      previewSessionId !== activeSession &&
      previewPaneRects.size > 0;
    const useRects = isPreviewActive ? previewPaneRects : paneRects;

    // --- Handle ongoing selection (phase !== idle) ---
    if (selectionState.phase !== "idle") {
      const selRect = paneRects.get(selectionState.paneId);
      if (!selRect) {
        resetSelection(selectionState);
        scheduleRender();
        return;
      }

      if (motion && baseButton === MOUSE_LEFT) {
        // Mouse drag — update selection end, clamp to pane bounds
        const newCol = Math.max(
          0,
          Math.min(screenX - selRect.x, selRect.width - 1),
        );
        const newRow = Math.max(
          0,
          Math.min(screenY - selRect.y, selRect.height - 1),
        );
        // Only start selecting once the mouse moves to a different cell
        if (
          selectionState.phase === "pressed" &&
          newCol === selectionState.startCol &&
          newRow === selectionState.startRow
        ) {
          return;
        }
        selectionState.phase = "selecting";
        selectionState.endCol = newCol;
        selectionState.endRow = newRow;
        scheduleRender();
        return;
      }

      if (event.isRelease && baseButton === MOUSE_LEFT) {
        if (selectionState.phase === "selecting") {
          // Drag ended — extract text and copy to clipboard
          const selTerm = clientTerminals.get(selectionState.paneId);
          if (selTerm) {
            const text = extractSelectedText(selectionState, selTerm);
            if (text.length > 0) {
              copyToClipboard(text);
            }
          }
          resetSelection(selectionState);
          scheduleRender();
        } else {
          // Was "pressed" but no drag occurred — treat as click
          const clickPaneId = selectionState.paneId;
          resetSelection(selectionState);
          if (!isPreviewActive && clickPaneId !== activePaneId) {
            connection.send({
              type: "command",
              id: "pane:focus",
              args: { paneId: clickPaneId },
            });
          }
        }
        return;
      }

      // Non-left events while selecting — ignore
      return;
    }

    // --- No active selection ---

    // Hit test: find which pane contains the event
    let targetPaneId: string | null = null;
    let targetRect: Rect | null = null;
    for (const [paneId, rect] of useRects) {
      if (
        screenX >= rect.x &&
        screenX < rect.x + rect.width &&
        screenY >= rect.y &&
        screenY < rect.y + rect.height
      ) {
        targetPaneId = paneId;
        targetRect = rect;
        break;
      }
    }

    // Click landed on a border or outside any pane
    if (!targetPaneId || !targetRect) return;

    // Forward mouse events to PTY if the app has enabled mouse tracking
    if (!isPreviewActive) {
      const targetTerm = clientTerminals.get(targetPaneId);
      if (targetTerm && targetTerm.isMouseTrackingActive()) {
        const localX = screenX - targetRect.x;
        const localY = screenY - targetRect.y;
        const encoded = encodeSgrMouse(
          event.button,
          localX,
          localY,
          event.isRelease,
        );
        connection.send({
          type: "input",
          paneId: targetPaneId,
          data: Buffer.from(encoded).toString("base64"),
        });
        return;
      }
    }

    // No mouse tracking — handle MaxMux-level mouse behavior
    if (
      !event.isRelease &&
      !motion &&
      baseButton === MOUSE_LEFT &&
      !isScrollEvent(event.button)
    ) {
      // Left press — start potential selection
      selectionState.phase = "pressed";
      selectionState.paneId = targetPaneId;
      selectionState.startCol = screenX - targetRect.x;
      selectionState.startRow = screenY - targetRect.y;
      selectionState.endCol = selectionState.startCol;
      selectionState.endRow = selectionState.startRow;
    }

    // Scroll events on non-mouse-tracking panes → enter/control copy-mode
    if (isScrollEvent(event.button)) {
      const scrollBtn = getBaseButton(event.button);
      if (scrollBtn === MOUSE_SCROLL_UP && !copyModeActive) {
        enterCopyMode(targetPaneId);
        if (copyModeState) {
          copyModeState.scrollOffset = Math.min(
            3,
            Math.max(
              0,
              copyModeState.bufferLength - copyModeState.viewportRows,
            ),
          );
          ensureCursorVisible(copyModeState);
          renderCopyMode();
        }
      }
    }
  };

  // Handle stdin
  process.stdin.on("data", (data: Buffer) => {
    if (showingOverlay) return;

    // Copy-mode input routing
    if (copyModeActive && copyModeState) {
      // Let prefix key through so prefix commands still work
      if (data.length === 1 && data[0] === parsePrefixKey(config.prefixKey)) {
        inputRouter.handleInput(data);
        return;
      }
      // Mouse events in copy-mode
      if (
        config.mouse &&
        data.length >= 3 &&
        data[0] === 0x1b &&
        data[1] === 0x5b &&
        data[2] === 0x3c
      ) {
        const result = parseSgrMouse(data, 0);
        if (result) {
          const baseBtn = getBaseButton(result.event.button);
          if (isScrollEvent(result.event.button)) {
            const scrollUp = baseBtn === MOUSE_SCROLL_UP;
            const action = handleCopyModeScroll(copyModeState, scrollUp);
            switch (action.type) {
              case "exit":
                exitCopyMode();
                break;
              case "render":
                renderCopyMode();
                break;
            }
          }
        }
        return;
      }
      const term = clientTerminals.get(copyModeState.paneId);
      if (!term) {
        exitCopyMode();
        return;
      }
      const action = handleCopyModeInput(copyModeState, data, term);
      switch (action.type) {
        case "exit":
          exitCopyMode();
          break;
        case "yank":
          copyToClipboard(action.text);
          exitCopyMode();
          break;
        case "render":
          renderCopyMode();
          break;
      }
      return;
    }

    if (sidebarActive) {
      // In sidebar mode, ignore mouse events (v1)
      if (
        config.mouse &&
        data.length >= 3 &&
        data[0] === 0x1b &&
        data[1] === 0x5b &&
        data[2] === 0x3c
      ) {
        return;
      }
      handleSidebarInput(data);
      return;
    }

    // Check for SGR mouse sequences
    if (
      config.mouse &&
      data.length >= 3 &&
      data[0] === 0x1b &&
      data[1] === 0x5b &&
      data[2] === 0x3c
    ) {
      let offset = 0;
      while (offset < data.length) {
        // Check if remaining data starts with SGR mouse prefix
        if (
          offset + 2 < data.length &&
          data[offset] === 0x1b &&
          data[offset + 1] === 0x5b &&
          data[offset + 2] === 0x3c
        ) {
          const result = parseSgrMouse(data, offset);
          if (result) {
            handleMouseEvent(result.event);
            offset += result.consumed;
            continue;
          }
        }
        // Remaining data is not a mouse sequence — pass to input router
        inputRouter.handleInput(data.subarray(offset));
        break;
      }
      return;
    }

    inputRouter.handleInput(data);
  });

  // Handle terminal resize
  // Bun does not fire process.stdout "resize" events — listen for
  // SIGWINCH directly and keep stdout.on("resize") as Node.js fallback.
  const onTerminalResize = () => {
    const newCols = process.stdout.columns || 80;
    const newRows = process.stdout.rows || 24;
    if (newCols === cols && newRows === rows) return; // dedup
    cols = newCols;
    rows = newRows;

    if (sidebarActive) {
      // Reclamp sidebar width
      sidebarWidth = Math.min(
        config.sessionList.sidebarWidth,
        Math.floor(cols / 2),
      );
      const mainWidth = cols - sidebarWidth - 1;
      connection.send({ type: "resize", cols: mainWidth, rows });

      // Re-request preview if active (rows - 1 for preview bar)
      if (previewSessionId && previewSessionId !== activeSession) {
        connection.send({
          type: "preview",
          sessionId: previewSessionId,
          cols: mainWidth,
          rows: rows - 1,
        });
      }
    } else {
      connection.send({ type: "resize", cols, rows });
    }
    // Server will send updated layout which triggers syncTerminals + renderScreen
  };
  process.stdout.on("resize", onTerminalResize);
  process.on("SIGWINCH", onTerminalResize);

  // Periodically refresh status bar (configurable interval)
  const refreshInterval = config.statusBar.refreshInterval || 1000;
  const statusInterval = setInterval(() => {
    if (!showingOverlay) drawStatusBar();
  }, refreshInterval);

  // Handle clean exit
  process.on("SIGINT", () => {
    // Bun's raw mode may not fully disable ISIG, causing Ctrl+C to
    // fire SIGINT instead of delivering byte 0x03 to stdin.
    // Re-emit as stdin data so it flows through the normal input pipeline.
    if (!cleaned) {
      process.stdin.emit("data", Buffer.from([0x03]));
    }
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
      process.stdout.write(
        "\x1b[?2004l\x1b[?1002l\x1b[?1006l\x1b[?1049l\x1b[?25h\x1b[0m",
      );
    }
  });
}
