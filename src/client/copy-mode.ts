import type { VirtualTerminal } from "../core/terminal.ts";
import type { Rect } from "../core/layout.ts";
import * as ansi from "../renderer/ansi.ts";

// --- Types ---

export type CopyModePhase =
  | "navigate"
  | "visual-char"
  | "visual-line"
  | "search";

export interface CopyModeState {
  active: boolean;
  paneId: string;
  cursorRow: number; // absolute buffer index
  cursorCol: number;
  scrollOffset: number; // lines above viewport end (0 = live bottom)
  phase: CopyModePhase;
  // Visual selection anchor (absolute)
  anchorRow: number;
  anchorCol: number;
  // Search
  searchQuery: string;
  searchDirection: "forward" | "backward";
  searchMatches: Array<{ row: number; col: number; length: number }>;
  currentMatchIndex: number;
  // Multi-key (for gg)
  pendingKey: string | null;
  // Cache
  bufferLength: number;
  viewportRows: number;
  viewportCols: number;
}

export type CopyModeAction =
  | { type: "none" }
  | { type: "render" }
  | { type: "exit" }
  | { type: "yank"; text: string };

// --- State creation ---

export function createCopyModeState(
  paneId: string,
  term: VirtualTerminal,
): CopyModeState {
  const bufLen = term.getBufferLength();
  const baseY = term.getBaseY();
  const rows = term.getRows();
  const cols = term.getCols();

  return {
    active: true,
    paneId,
    cursorRow: baseY + term.getCursorY(),
    cursorCol: term.getCursorX(),
    scrollOffset: 0,
    phase: "navigate",
    anchorRow: 0,
    anchorCol: 0,
    searchQuery: "",
    searchDirection: "forward",
    searchMatches: [],
    currentMatchIndex: -1,
    pendingKey: null,
    bufferLength: bufLen,
    viewportRows: rows,
    viewportCols: cols,
  };
}

export function refreshBufferInfo(
  state: CopyModeState,
  term: VirtualTerminal,
): void {
  state.bufferLength = term.getBufferLength();
  state.viewportRows = term.getRows();
  state.viewportCols = term.getCols();
}

// --- Helpers ---

function clamp(val: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, val));
}

/** Ensure cursor is within the visible viewport, adjusting scrollOffset as needed */
export function ensureCursorVisible(state: CopyModeState): void {
  const maxScroll = Math.max(0, state.bufferLength - state.viewportRows);
  state.scrollOffset = clamp(state.scrollOffset, 0, maxScroll);

  const firstVisible =
    state.bufferLength - state.viewportRows - state.scrollOffset;
  const lastVisible = firstVisible + state.viewportRows - 1;

  if (state.cursorRow < firstVisible) {
    state.scrollOffset =
      state.bufferLength - state.viewportRows - state.cursorRow;
  } else if (state.cursorRow > lastVisible) {
    state.scrollOffset =
      state.bufferLength -
      state.viewportRows -
      (state.cursorRow - state.viewportRows + 1);
  }

  state.scrollOffset = clamp(state.scrollOffset, 0, maxScroll);
}

function getFirstVisibleLine(state: CopyModeState): number {
  return Math.max(
    0,
    state.bufferLength - state.viewportRows - state.scrollOffset,
  );
}

// --- Word movement helpers ---

function isWordChar(ch: string): boolean {
  return /[a-zA-Z0-9_]/.test(ch);
}

function moveWordForward(state: CopyModeState, term: VirtualTerminal): void {
  const maxRow = state.bufferLength - 1;
  let { cursorRow, cursorCol } = state;

  // Skip current word chars
  while (cursorRow <= maxRow) {
    const ch = term.getBufferCellChar(cursorRow, cursorCol);
    if (!isWordChar(ch)) break;
    cursorCol++;
    if (cursorCol >= state.viewportCols) {
      cursorCol = 0;
      cursorRow++;
    }
  }
  // Skip non-word chars
  while (cursorRow <= maxRow) {
    const ch = term.getBufferCellChar(cursorRow, cursorCol);
    if (isWordChar(ch)) break;
    cursorCol++;
    if (cursorCol >= state.viewportCols) {
      cursorCol = 0;
      cursorRow++;
    }
  }

  state.cursorRow = clamp(cursorRow, 0, maxRow);
  state.cursorCol = clamp(cursorCol, 0, state.viewportCols - 1);
}

function moveWordBackward(state: CopyModeState, term: VirtualTerminal): void {
  let { cursorRow, cursorCol } = state;

  // Move back one first
  cursorCol--;
  if (cursorCol < 0) {
    cursorRow--;
    cursorCol = state.viewportCols - 1;
  }
  if (cursorRow < 0) {
    cursorRow = 0;
    cursorCol = 0;
    return;
  }

  // Skip non-word chars backward
  while (cursorRow >= 0) {
    const ch = term.getBufferCellChar(cursorRow, cursorCol);
    if (isWordChar(ch)) break;
    cursorCol--;
    if (cursorCol < 0) {
      cursorRow--;
      cursorCol = state.viewportCols - 1;
    }
  }
  // Skip word chars backward to find start
  while (cursorRow >= 0) {
    if (cursorCol === 0) {
      if (cursorRow === 0) break;
      const prevCh = term.getBufferCellChar(cursorRow, cursorCol);
      if (!isWordChar(prevCh)) break;
      // At col 0 and it's a word char — check if previous line continues
      break;
    }
    const prevCh = term.getBufferCellChar(cursorRow, cursorCol - 1);
    if (!isWordChar(prevCh)) break;
    cursorCol--;
  }

  state.cursorRow = Math.max(0, cursorRow);
  state.cursorCol = Math.max(0, cursorCol);
}

function moveWordEnd(state: CopyModeState, term: VirtualTerminal): void {
  const maxRow = state.bufferLength - 1;
  let { cursorRow, cursorCol } = state;

  // Move forward one first
  cursorCol++;
  if (cursorCol >= state.viewportCols) {
    cursorCol = 0;
    cursorRow++;
  }
  if (cursorRow > maxRow) {
    cursorRow = maxRow;
    cursorCol = state.viewportCols - 1;
    return;
  }

  // Skip non-word chars
  while (cursorRow <= maxRow) {
    const ch = term.getBufferCellChar(cursorRow, cursorCol);
    if (isWordChar(ch)) break;
    cursorCol++;
    if (cursorCol >= state.viewportCols) {
      cursorCol = 0;
      cursorRow++;
    }
  }
  // Skip word chars to find end
  while (cursorRow <= maxRow) {
    const nextCol = cursorCol + 1;
    if (nextCol >= state.viewportCols) break;
    const nextCh = term.getBufferCellChar(cursorRow, nextCol);
    if (!isWordChar(nextCh)) break;
    cursorCol = nextCol;
  }

  state.cursorRow = clamp(cursorRow, 0, maxRow);
  state.cursorCol = clamp(cursorCol, 0, state.viewportCols - 1);
}

// --- Search ---

function findAllMatches(state: CopyModeState, term: VirtualTerminal): void {
  state.searchMatches = [];
  if (state.searchQuery.length === 0) return;

  const query = state.searchQuery.toLowerCase();
  for (let row = 0; row < state.bufferLength; row++) {
    const text = term.getBufferLineText(row).toLowerCase();
    let idx = 0;
    while ((idx = text.indexOf(query, idx)) !== -1) {
      state.searchMatches.push({ row, col: idx, length: query.length });
      idx += 1;
    }
  }
}

function jumpToNextMatch(state: CopyModeState): boolean {
  if (state.searchMatches.length === 0) return false;

  if (state.searchDirection === "forward") {
    // Find next match after cursor
    let idx = state.searchMatches.findIndex(
      (m) =>
        m.row > state.cursorRow ||
        (m.row === state.cursorRow && m.col > state.cursorCol),
    );
    if (idx === -1) idx = 0; // wrap around
    state.currentMatchIndex = idx;
  } else {
    // Find previous match before cursor
    let idx = -1;
    for (let i = state.searchMatches.length - 1; i >= 0; i--) {
      const m = state.searchMatches[i]!;
      if (
        m.row < state.cursorRow ||
        (m.row === state.cursorRow && m.col < state.cursorCol)
      ) {
        idx = i;
        break;
      }
    }
    if (idx === -1) idx = state.searchMatches.length - 1; // wrap around
    state.currentMatchIndex = idx;
  }

  const match = state.searchMatches[state.currentMatchIndex];
  if (match) {
    state.cursorRow = match.row;
    state.cursorCol = match.col;
    return true;
  }
  return false;
}

function jumpToPrevMatch(state: CopyModeState): boolean {
  if (state.searchMatches.length === 0) return false;

  // Reverse direction for this jump
  const origDir = state.searchDirection;
  state.searchDirection = origDir === "forward" ? "backward" : "forward";
  const result = jumpToNextMatch(state);
  state.searchDirection = origDir;
  return result;
}

// --- Selection text extraction ---

function getSelectedText(state: CopyModeState, term: VirtualTerminal): string {
  if (state.phase === "navigate") return "";

  let startRow = state.anchorRow;
  let startCol = state.anchorCol;
  let endRow = state.cursorRow;
  let endCol = state.cursorCol;

  // Normalize: start <= end
  if (startRow > endRow || (startRow === endRow && startCol > endCol)) {
    [startRow, endRow] = [endRow, startRow];
    [startCol, endCol] = [endCol, startCol];
  }

  if (state.phase === "visual-line") {
    startCol = 0;
    endCol = state.viewportCols - 1;
  }

  return term.getBufferTextRange(startRow, startCol, endRow, endCol);
}

// --- Input handler ---

export function handleCopyModeInput(
  state: CopyModeState,
  data: Buffer,
  term: VirtualTerminal,
): CopyModeAction {
  refreshBufferInfo(state, term);

  // Search mode input
  if (state.phase === "search") {
    return handleSearchInput(state, data, term);
  }

  const bytes = Array.from(data);
  const maxRow = state.bufferLength - 1;
  const halfPage = Math.floor(state.viewportRows / 2);

  // Clear pending key on timeout or non-g key
  if (state.pendingKey === "g") {
    const str = data.toString("utf-8");
    if (str === "g") {
      state.pendingKey = null;
      state.cursorRow = 0;
      state.cursorCol = 0;
      state.scrollOffset = Math.max(0, state.bufferLength - state.viewportRows);
      ensureCursorVisible(state);
      return { type: "render" };
    }
    // Any other key cancels gg
    state.pendingKey = null;
  }

  // Escape or Ctrl+C
  if (bytes.length === 1 && (bytes[0] === 0x1b || bytes[0] === 0x03)) {
    if (state.phase === "visual-char" || state.phase === "visual-line") {
      state.phase = "navigate";
      return { type: "render" };
    }
    return { type: "exit" };
  }

  // q - exit
  if (bytes.length === 1 && bytes[0] === 0x71) {
    return { type: "exit" };
  }

  const str = data.toString("utf-8");

  // y or Enter — yank
  if (str === "y" || (bytes.length === 1 && bytes[0] === 0x0d)) {
    if (state.phase === "visual-char" || state.phase === "visual-line") {
      const text = getSelectedText(state, term);
      if (text.length > 0) {
        return { type: "yank", text };
      }
    }
    if (str === "y") return { type: "none" };
    return { type: "exit" };
  }

  // Navigation keys
  switch (str) {
    case "h": // left
      state.cursorCol = Math.max(0, state.cursorCol - 1);
      ensureCursorVisible(state);
      return { type: "render" };

    case "j": // down
      state.cursorRow = Math.min(maxRow, state.cursorRow + 1);
      ensureCursorVisible(state);
      return { type: "render" };

    case "k": // up
      state.cursorRow = Math.max(0, state.cursorRow - 1);
      ensureCursorVisible(state);
      return { type: "render" };

    case "l": // right
      state.cursorCol = Math.min(state.viewportCols - 1, state.cursorCol + 1);
      ensureCursorVisible(state);
      return { type: "render" };

    case "0": // line start
      state.cursorCol = 0;
      return { type: "render" };

    case "$": // line end
      state.cursorCol = state.viewportCols - 1;
      return { type: "render" };

    case "w": // word forward
      moveWordForward(state, term);
      ensureCursorVisible(state);
      return { type: "render" };

    case "b": // word backward
      moveWordBackward(state, term);
      ensureCursorVisible(state);
      return { type: "render" };

    case "e": // word end
      moveWordEnd(state, term);
      ensureCursorVisible(state);
      return { type: "render" };

    case "g": // first g of gg
      state.pendingKey = "g";
      return { type: "none" };

    case "G": // buffer end
      state.cursorRow = maxRow;
      state.scrollOffset = 0;
      ensureCursorVisible(state);
      return { type: "render" };

    case "H": {
      // top of viewport
      const first = getFirstVisibleLine(state);
      state.cursorRow = first;
      return { type: "render" };
    }

    case "M": {
      // middle of viewport
      const first = getFirstVisibleLine(state);
      state.cursorRow = first + Math.floor(state.viewportRows / 2);
      return { type: "render" };
    }

    case "L": {
      // bottom of viewport
      const first = getFirstVisibleLine(state);
      state.cursorRow = Math.min(first + state.viewportRows - 1, maxRow);
      return { type: "render" };
    }

    case "v": // toggle visual-char
      if (state.phase === "visual-char") {
        state.phase = "navigate";
      } else {
        state.phase = "visual-char";
        state.anchorRow = state.cursorRow;
        state.anchorCol = state.cursorCol;
      }
      return { type: "render" };

    case "V": // toggle visual-line
      if (state.phase === "visual-line") {
        state.phase = "navigate";
      } else {
        state.phase = "visual-line";
        state.anchorRow = state.cursorRow;
        state.anchorCol = state.cursorCol;
      }
      return { type: "render" };

    case "/": // search forward
      state.phase = "search";
      state.searchDirection = "forward";
      state.searchQuery = "";
      return { type: "render" };

    case "?": // search backward
      state.phase = "search";
      state.searchDirection = "backward";
      state.searchQuery = "";
      return { type: "render" };

    case "n": // next match
      if (jumpToNextMatch(state)) {
        ensureCursorVisible(state);
        return { type: "render" };
      }
      return { type: "none" };

    case "N": // previous match
      if (jumpToPrevMatch(state)) {
        ensureCursorVisible(state);
        return { type: "render" };
      }
      return { type: "none" };
  }

  // Ctrl+u — half page up
  if (bytes.length === 1 && bytes[0] === 0x15) {
    state.cursorRow = Math.max(0, state.cursorRow - halfPage);
    state.scrollOffset = Math.min(
      Math.max(0, state.bufferLength - state.viewportRows),
      state.scrollOffset + halfPage,
    );
    ensureCursorVisible(state);
    return { type: "render" };
  }

  // Ctrl+d — half page down
  if (bytes.length === 1 && bytes[0] === 0x04) {
    state.cursorRow = Math.min(maxRow, state.cursorRow + halfPage);
    state.scrollOffset = Math.max(0, state.scrollOffset - halfPage);
    ensureCursorVisible(state);
    return { type: "render" };
  }

  // Ctrl+b — full page up
  if (bytes.length === 1 && bytes[0] === 0x02) {
    state.cursorRow = Math.max(0, state.cursorRow - state.viewportRows);
    state.scrollOffset = Math.min(
      Math.max(0, state.bufferLength - state.viewportRows),
      state.scrollOffset + state.viewportRows,
    );
    ensureCursorVisible(state);
    return { type: "render" };
  }

  // Ctrl+f — full page down
  if (bytes.length === 1 && bytes[0] === 0x06) {
    state.cursorRow = Math.min(maxRow, state.cursorRow + state.viewportRows);
    state.scrollOffset = Math.max(0, state.scrollOffset - state.viewportRows);
    ensureCursorVisible(state);
    return { type: "render" };
  }

  // Arrow keys
  if (bytes.length === 3 && bytes[0] === 0x1b && bytes[1] === 0x5b) {
    switch (bytes[2]) {
      case 0x41: // Up
        state.cursorRow = Math.max(0, state.cursorRow - 1);
        ensureCursorVisible(state);
        return { type: "render" };
      case 0x42: // Down
        state.cursorRow = Math.min(maxRow, state.cursorRow + 1);
        ensureCursorVisible(state);
        return { type: "render" };
      case 0x43: // Right
        state.cursorCol = Math.min(state.viewportCols - 1, state.cursorCol + 1);
        return { type: "render" };
      case 0x44: // Left
        state.cursorCol = Math.max(0, state.cursorCol - 1);
        return { type: "render" };
    }
  }

  return { type: "none" };
}

// --- Search input handler ---

function handleSearchInput(
  state: CopyModeState,
  data: Buffer,
  term: VirtualTerminal,
): CopyModeAction {
  const bytes = Array.from(data);

  // Escape or Ctrl+C — cancel search
  if (bytes.length === 1 && (bytes[0] === 0x1b || bytes[0] === 0x03)) {
    state.phase = "navigate";
    state.searchQuery = "";
    return { type: "render" };
  }

  // Enter — execute search
  if (bytes.length === 1 && bytes[0] === 0x0d) {
    findAllMatches(state, term);
    state.phase = "navigate";
    if (state.searchMatches.length > 0) {
      jumpToNextMatch(state);
      ensureCursorVisible(state);
    }
    return { type: "render" };
  }

  // Backspace
  if (bytes.length === 1 && bytes[0] === 0x7f) {
    if (state.searchQuery.length > 0) {
      state.searchQuery = state.searchQuery.slice(0, -1);
    }
    return { type: "render" };
  }

  // Ctrl+U — clear
  if (bytes.length === 1 && bytes[0] === 0x15) {
    state.searchQuery = "";
    return { type: "render" };
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
    state.searchQuery += str;
    return { type: "render" };
  }

  return { type: "none" };
}

// --- Mouse scroll in copy mode ---

export function handleCopyModeScroll(
  state: CopyModeState,
  scrollUp: boolean,
  lines = 3,
): CopyModeAction {
  const maxScroll = Math.max(0, state.bufferLength - state.viewportRows);

  if (scrollUp) {
    const newOffset = Math.min(maxScroll, state.scrollOffset + lines);
    const delta = newOffset - state.scrollOffset;
    state.scrollOffset = newOffset;
    // Move cursor with scroll to keep it visible
    state.cursorRow = Math.max(0, state.cursorRow - delta);
  } else {
    const newOffset = Math.max(0, state.scrollOffset - lines);
    const delta = state.scrollOffset - newOffset;
    state.scrollOffset = newOffset;
    // Move cursor with scroll to keep it visible
    state.cursorRow = Math.min(state.bufferLength - 1, state.cursorRow + delta);
    // If scrolled all the way to bottom, exit copy mode
    if (state.scrollOffset === 0) {
      return { type: "exit" };
    }
  }

  ensureCursorVisible(state);
  return { type: "render" };
}

// --- Rendering ---

export function renderCopyModePane(
  state: CopyModeState,
  term: VirtualTerminal,
  paneRect: Rect,
  xOffset: number,
): string {
  let out = "";

  const firstVisible = getFirstVisibleLine(state);

  // 1. Render buffer lines
  for (let y = 0; y < paneRect.height; y++) {
    const absRow = firstVisible + y;
    out += ansi.moveTo(paneRect.x + xOffset, paneRect.y + y);
    if (absRow >= 0 && absRow < state.bufferLength) {
      out += term.renderBufferLine(absRow);
    } else {
      out += ansi.resetStyle() + " ".repeat(paneRect.width);
    }
  }

  // 2. Selection overlay (visual-char / visual-line)
  if (state.phase === "visual-char" || state.phase === "visual-line") {
    out += renderVisualOverlay(state, term, paneRect, xOffset, firstVisible);
  }

  // 3. Search highlights
  if (state.searchMatches.length > 0) {
    out += renderSearchHighlights(state, term, paneRect, xOffset, firstVisible);
  }

  // 4. Cursor (inverse cell)
  {
    const cursorScreenY = state.cursorRow - firstVisible;
    if (cursorScreenY >= 0 && cursorScreenY < paneRect.height) {
      const ch = term.getBufferCellChar(state.cursorRow, state.cursorCol);
      out += ansi.moveTo(
        paneRect.x + state.cursorCol + xOffset,
        paneRect.y + cursorScreenY,
      );
      out += "\x1b[7m" + ch + "\x1b[27m";
    }
  }

  // 5. Position indicator [line/total] top-right
  {
    const lineNum = state.cursorRow + 1;
    const total = state.bufferLength;
    const indicator = `[${lineNum}/${total}]`;
    const ix = paneRect.x + paneRect.width - indicator.length + xOffset;
    if (ix >= paneRect.x + xOffset) {
      out += ansi.moveTo(ix, paneRect.y);
      out += ansi.resetStyle() + ansi.bold() + ansi.fgHex("#fab387");
      out += indicator;
      out += ansi.resetStyle();
    }
  }

  // 6. Search bar (when in search phase)
  if (state.phase === "search") {
    const lastY = paneRect.y + paneRect.height - 1;
    const prefix = state.searchDirection === "forward" ? "/" : "?";
    const barText = prefix + state.searchQuery;
    const displayText = barText.slice(0, paneRect.width - 1) + "\u2588"; // cursor block
    out += ansi.moveTo(paneRect.x + xOffset, lastY);
    out += ansi.resetStyle() + ansi.bgHex("#313244") + ansi.fgHex("#cdd6f4");
    out += displayText.padEnd(paneRect.width);
    out += ansi.resetStyle();
  }

  return out;
}

function renderVisualOverlay(
  state: CopyModeState,
  term: VirtualTerminal,
  paneRect: Rect,
  xOffset: number,
  firstVisible: number,
): string {
  let startRow = state.anchorRow;
  let startCol = state.anchorCol;
  let endRow = state.cursorRow;
  let endCol = state.cursorCol;

  // Normalize
  if (startRow > endRow || (startRow === endRow && startCol > endCol)) {
    [startRow, endRow] = [endRow, startRow];
    [startCol, endCol] = [endCol, startCol];
  }

  if (state.phase === "visual-line") {
    startCol = 0;
    endCol = state.viewportCols - 1;
  }

  let out = "";
  const lastVisible = firstVisible + paneRect.height - 1;

  for (
    let row = Math.max(startRow, firstVisible);
    row <= Math.min(endRow, lastVisible);
    row++
  ) {
    const screenY = row - firstVisible;
    const lineStart = row === startRow ? startCol : 0;
    const lineEnd = row === endRow ? endCol : state.viewportCols - 1;

    let chars = "";
    for (let col = lineStart; col <= lineEnd; col++) {
      chars += term.getBufferCellChar(row, col);
    }

    out += ansi.moveTo(paneRect.x + lineStart + xOffset, paneRect.y + screenY);
    out += "\x1b[7m" + chars + "\x1b[27m";
  }

  return out;
}

function renderSearchHighlights(
  state: CopyModeState,
  term: VirtualTerminal,
  paneRect: Rect,
  xOffset: number,
  firstVisible: number,
): string {
  let out = "";
  const lastVisible = firstVisible + paneRect.height - 1;

  for (let i = 0; i < state.searchMatches.length; i++) {
    const match = state.searchMatches[i]!;
    if (match.row < firstVisible || match.row > lastVisible) continue;

    const screenY = match.row - firstVisible;
    const isCurrentMatch = i === state.currentMatchIndex;

    let chars = "";
    for (
      let c = match.col;
      c < match.col + match.length && c < state.viewportCols;
      c++
    ) {
      chars += term.getBufferCellChar(match.row, c);
    }

    out += ansi.moveTo(paneRect.x + match.col + xOffset, paneRect.y + screenY);
    if (isCurrentMatch) {
      out += ansi.bgHex("#fab387") + ansi.fgHex("#1e1e2e"); // orange for current
    } else {
      out += ansi.bgHex("#f9e2af") + ansi.fgHex("#1e1e2e"); // yellow for others
    }
    out += chars;
    out += ansi.resetStyle();
  }

  return out;
}
