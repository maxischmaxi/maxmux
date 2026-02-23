import { describe, test, expect } from "bun:test";
import {
  createCopyModeState,
  handleCopyModeInput,
  handleCopyModeScroll,
  ensureCursorVisible,
  refreshBufferInfo,
  type CopyModeState,
} from "./copy-mode.ts";
import { VirtualTerminal } from "../core/terminal.ts";

function makeTerm(
  lines: string[],
  cols = 80,
  rows = 24,
  scrollback = 1000,
): VirtualTerminal {
  const term = new VirtualTerminal("test", cols, rows, scrollback);
  // Write lines to terminal to populate buffer
  for (const line of lines) {
    term.write(line + "\r\n");
  }
  return term;
}

function makeTermSync(
  lines: string[],
  cols = 80,
  rows = 24,
  scrollback = 1000,
): Promise<VirtualTerminal> {
  return new Promise((resolve) => {
    const term = new VirtualTerminal("test", cols, rows, scrollback);
    let remaining = lines.length;
    if (remaining === 0) {
      resolve(term);
      return;
    }
    for (const line of lines) {
      term.write(line + "\r\n", () => {
        remaining--;
        if (remaining === 0) resolve(term);
      });
    }
  });
}

describe("copy-mode", () => {
  test("createCopyModeState initializes correctly", async () => {
    const term = await makeTermSync(["hello", "world"]);
    const state = createCopyModeState("pane1", term);

    expect(state.active).toBe(true);
    expect(state.paneId).toBe("pane1");
    expect(state.phase).toBe("navigate");
    expect(state.scrollOffset).toBe(0);
    expect(state.viewportRows).toBe(24);
    expect(state.viewportCols).toBe(80);
  });

  test("h/j/k/l navigation", async () => {
    const term = await makeTermSync(["line1", "line2", "line3"]);
    const state = createCopyModeState("pane1", term);
    const startRow = state.cursorRow;
    const startCol = state.cursorCol;

    // j moves down
    handleCopyModeInput(state, Buffer.from("j"), term);
    expect(state.cursorRow).toBe(startRow + 1);

    // k moves up
    handleCopyModeInput(state, Buffer.from("k"), term);
    expect(state.cursorRow).toBe(startRow);

    // l moves right
    handleCopyModeInput(state, Buffer.from("l"), term);
    expect(state.cursorCol).toBe(startCol + 1);

    // h moves left
    handleCopyModeInput(state, Buffer.from("h"), term);
    expect(state.cursorCol).toBe(startCol);
  });

  test("0 and $ move to line start/end", async () => {
    const term = await makeTermSync(["hello world"]);
    const state = createCopyModeState("pane1", term);
    state.cursorCol = 5;

    handleCopyModeInput(state, Buffer.from("0"), term);
    expect(state.cursorCol).toBe(0);

    handleCopyModeInput(state, Buffer.from("$"), term);
    expect(state.cursorCol).toBe(state.viewportCols - 1);
  });

  test("G moves to buffer end", async () => {
    const term = await makeTermSync(["line1", "line2", "line3"]);
    const state = createCopyModeState("pane1", term);
    state.cursorRow = 0;
    state.scrollOffset = 10;

    handleCopyModeInput(state, Buffer.from("G"), term);
    expect(state.cursorRow).toBe(state.bufferLength - 1);
    expect(state.scrollOffset).toBe(0);
  });

  test("gg moves to buffer top", async () => {
    const term = await makeTermSync(["line1", "line2", "line3"]);
    const state = createCopyModeState("pane1", term);

    // First g sets pendingKey
    const action1 = handleCopyModeInput(state, Buffer.from("g"), term);
    expect(action1.type).toBe("none");
    expect(state.pendingKey).toBe("g");

    // Second g completes gg
    const action2 = handleCopyModeInput(state, Buffer.from("g"), term);
    expect(action2.type).toBe("render");
    expect(state.cursorRow).toBe(0);
    expect(state.pendingKey).toBeNull();
  });

  test("v toggles visual-char mode", async () => {
    const term = await makeTermSync(["hello"]);
    const state = createCopyModeState("pane1", term);

    handleCopyModeInput(state, Buffer.from("v"), term);
    expect(state.phase).toBe("visual-char");
    expect(state.anchorRow).toBe(state.cursorRow);
    expect(state.anchorCol).toBe(state.cursorCol);

    // Toggle off
    handleCopyModeInput(state, Buffer.from("v"), term);
    expect(state.phase).toBe("navigate");
  });

  test("V toggles visual-line mode", async () => {
    const term = await makeTermSync(["hello"]);
    const state = createCopyModeState("pane1", term);

    handleCopyModeInput(state, Buffer.from("V"), term);
    expect(state.phase).toBe("visual-line");

    handleCopyModeInput(state, Buffer.from("V"), term);
    expect(state.phase).toBe("navigate");
  });

  test("Escape exits from navigate mode", async () => {
    const term = await makeTermSync(["hello"]);
    const state = createCopyModeState("pane1", term);

    const action = handleCopyModeInput(state, Buffer.from("\x1b"), term);
    expect(action.type).toBe("exit");
  });

  test("Escape returns from visual to navigate", async () => {
    const term = await makeTermSync(["hello"]);
    const state = createCopyModeState("pane1", term);

    handleCopyModeInput(state, Buffer.from("v"), term);
    expect(state.phase).toBe("visual-char");

    const action = handleCopyModeInput(state, Buffer.from("\x1b"), term);
    expect(action.type).toBe("render");
    expect(state.phase).toBe("navigate");
  });

  test("q exits copy mode", async () => {
    const term = await makeTermSync(["hello"]);
    const state = createCopyModeState("pane1", term);

    const action = handleCopyModeInput(state, Buffer.from("q"), term);
    expect(action.type).toBe("exit");
  });

  test("/ starts search forward", async () => {
    const term = await makeTermSync(["hello"]);
    const state = createCopyModeState("pane1", term);

    handleCopyModeInput(state, Buffer.from("/"), term);
    expect(state.phase).toBe("search");
    expect(state.searchDirection).toBe("forward");
    expect(state.searchQuery).toBe("");
  });

  test("? starts search backward", async () => {
    const term = await makeTermSync(["hello"]);
    const state = createCopyModeState("pane1", term);

    handleCopyModeInput(state, Buffer.from("?"), term);
    expect(state.phase).toBe("search");
    expect(state.searchDirection).toBe("backward");
  });

  test("search input and escape cancels", async () => {
    const term = await makeTermSync(["hello world"]);
    const state = createCopyModeState("pane1", term);

    // Enter search mode
    handleCopyModeInput(state, Buffer.from("/"), term);
    expect(state.phase).toBe("search");

    // Type query
    handleCopyModeInput(state, Buffer.from("h"), term);
    handleCopyModeInput(state, Buffer.from("e"), term);
    expect(state.searchQuery).toBe("he");

    // Backspace
    handleCopyModeInput(state, Buffer.from("\x7f"), term);
    expect(state.searchQuery).toBe("h");

    // Escape cancels
    handleCopyModeInput(state, Buffer.from("\x1b"), term);
    expect(state.phase).toBe("navigate");
    expect(state.searchQuery).toBe("");
  });

  test("y in visual mode returns yank action", async () => {
    const term = await makeTermSync(["hello world"]);
    const state = createCopyModeState("pane1", term);

    // Move cursor up to the line with text (cursor is on empty line after \r\n)
    handleCopyModeInput(state, Buffer.from("k"), term);
    // Move to col 0
    handleCopyModeInput(state, Buffer.from("0"), term);

    // Enter visual mode and select some text
    handleCopyModeInput(state, Buffer.from("v"), term);
    handleCopyModeInput(state, Buffer.from("l"), term);
    handleCopyModeInput(state, Buffer.from("l"), term);

    const action = handleCopyModeInput(state, Buffer.from("y"), term);
    expect(action.type).toBe("yank");
    if (action.type === "yank") {
      expect(action.text).toBe("hel");
    }
  });

  test("scroll up enters copy mode, scroll down exits at bottom", async () => {
    // Need enough lines to have scrollback
    const lines = Array.from({ length: 50 }, (_, i) => `line ${i}`);
    const term = await makeTermSync(lines, 80, 24, 100);
    const state = createCopyModeState("pane1", term);
    refreshBufferInfo(state, term);

    // Scroll up
    const upAction = handleCopyModeScroll(state, true, 3);
    expect(upAction.type).toBe("render");
    expect(state.scrollOffset).toBeGreaterThan(0);

    // Scroll back to bottom
    state.scrollOffset = 1;
    const downAction = handleCopyModeScroll(state, false, 3);
    expect(downAction.type).toBe("exit");
  });

  test("ensureCursorVisible adjusts scrollOffset", async () => {
    const term = await makeTermSync(
      Array.from({ length: 50 }, (_, i) => `line ${i}`),
      80,
      24,
      100,
    );
    const state = createCopyModeState("pane1", term);
    refreshBufferInfo(state, term);

    // Move cursor way up
    state.cursorRow = 0;
    ensureCursorVisible(state);
    // scrollOffset should have increased to show line 0
    expect(state.scrollOffset).toBeGreaterThan(0);
  });

  test("Ctrl+u moves half page up", async () => {
    const term = await makeTermSync(
      Array.from({ length: 50 }, (_, i) => `line ${i}`),
      80,
      24,
      100,
    );
    const state = createCopyModeState("pane1", term);
    const startRow = state.cursorRow;

    // Ctrl+u = 0x15
    handleCopyModeInput(state, Buffer.from([0x15]), term);
    expect(state.cursorRow).toBeLessThan(startRow);
  });

  test("Ctrl+d moves half page down", async () => {
    const term = await makeTermSync(
      Array.from({ length: 50 }, (_, i) => `line ${i}`),
      80,
      24,
      100,
    );
    const state = createCopyModeState("pane1", term);

    // First move up
    state.cursorRow = 10;
    state.scrollOffset = 20;
    const startRow = state.cursorRow;

    // Ctrl+d = 0x04
    handleCopyModeInput(state, Buffer.from([0x04]), term);
    expect(state.cursorRow).toBeGreaterThan(startRow);
  });

  test("arrow keys work", async () => {
    const term = await makeTermSync(["hello", "world"]);
    const state = createCopyModeState("pane1", term);
    const startRow = state.cursorRow;

    // Arrow Up: \x1b[A
    handleCopyModeInput(state, Buffer.from([0x1b, 0x5b, 0x41]), term);
    expect(state.cursorRow).toBe(startRow - 1);

    // Arrow Down: \x1b[B
    handleCopyModeInput(state, Buffer.from([0x1b, 0x5b, 0x42]), term);
    expect(state.cursorRow).toBe(startRow);

    // Arrow Right: \x1b[C
    const startCol = state.cursorCol;
    handleCopyModeInput(state, Buffer.from([0x1b, 0x5b, 0x43]), term);
    expect(state.cursorCol).toBe(startCol + 1);

    // Arrow Left: \x1b[D
    handleCopyModeInput(state, Buffer.from([0x1b, 0x5b, 0x44]), term);
    expect(state.cursorCol).toBe(startCol);
  });

  test("H/M/L move to top/middle/bottom of viewport", async () => {
    const lines = Array.from({ length: 50 }, (_, i) => `line ${i}`);
    const term = await makeTermSync(lines, 80, 24, 100);
    const state = createCopyModeState("pane1", term);
    refreshBufferInfo(state, term);

    handleCopyModeInput(state, Buffer.from("H"), term);
    const topRow = state.cursorRow;

    handleCopyModeInput(state, Buffer.from("L"), term);
    const bottomRow = state.cursorRow;

    handleCopyModeInput(state, Buffer.from("M"), term);
    const midRow = state.cursorRow;

    // Middle should be between top and bottom
    expect(midRow).toBeGreaterThan(topRow);
    expect(midRow).toBeLessThan(bottomRow);
  });
});
