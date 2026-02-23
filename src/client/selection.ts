import type { VirtualTerminal } from "../core/terminal.ts";
import type { Rect } from "../core/layout.ts";
import { moveTo } from "../renderer/ansi.ts";

export type SelectionPhase = "idle" | "pressed" | "selecting";

export interface SelectionState {
  phase: SelectionPhase;
  paneId: string;
  startCol: number;
  startRow: number;
  endCol: number;
  endRow: number;
}

export function createSelectionState(): SelectionState {
  return {
    phase: "idle",
    paneId: "",
    startCol: 0,
    startRow: 0,
    endCol: 0,
    endRow: 0,
  };
}

export function resetSelection(state: SelectionState): void {
  state.phase = "idle";
  state.paneId = "";
  state.startCol = 0;
  state.startRow = 0;
  state.endCol = 0;
  state.endRow = 0;
}

/** Return normalized (start <= end) range */
export function normalizeRange(state: SelectionState): {
  startRow: number;
  startCol: number;
  endRow: number;
  endCol: number;
} {
  let { startRow, startCol, endRow, endCol } = state;
  if (startRow > endRow || (startRow === endRow && startCol > endCol)) {
    [startRow, endRow] = [endRow, startRow];
    [startCol, endCol] = [endCol, startCol];
  }
  return { startRow, startCol, endRow, endCol };
}

/**
 * Render selection highlight using inverse video.
 * Outputs ANSI sequences that overlay inverse on selected cells.
 */
export function renderSelection(
  state: SelectionState,
  term: VirtualTerminal,
  paneRect: Rect,
  xOffset: number,
): string {
  if (state.phase === "idle") return "";

  const { startRow, startCol, endRow, endCol } = normalizeRange(state);
  let out = "";

  for (let row = startRow; row <= endRow; row++) {
    const lineStart = row === startRow ? startCol : 0;
    const lineEnd = row === endRow ? endCol : Math.max(0, paneRect.width - 1);

    // Collect chars for this row segment
    let chars = "";
    for (let col = lineStart; col <= lineEnd; col++) {
      chars += term.getCellChar(row, col);
    }

    out += moveTo(paneRect.x + lineStart + xOffset, paneRect.y + row);
    out += "\x1b[7m"; // inverse
    out += chars;
    out += "\x1b[27m"; // reset inverse
  }

  return out;
}

/**
 * Extract selected text from the terminal buffer.
 */
export function extractSelectedText(
  state: SelectionState,
  term: VirtualTerminal,
): string {
  const { startRow, startCol, endRow, endCol } = normalizeRange(state);
  return term.getTextRange(startRow, startCol, endRow, endCol);
}

/**
 * Copy text to system clipboard using OSC 52 escape sequence.
 * This works in most modern terminal emulators.
 */
export function copyToClipboard(text: string): void {
  const b64 = Buffer.from(text, "utf-8").toString("base64");
  process.stdout.write(`\x1b]52;c;${b64}\x1b\\`);
}
