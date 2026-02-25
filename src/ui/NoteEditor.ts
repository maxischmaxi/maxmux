import * as ansi from "../renderer/ansi.ts";
import { renderBox, renderText } from "./components.ts";
import { deriveTitle } from "../persistence/notes-db.ts";

export interface NoteEditorState {
  noteId: string | null; // null = new note
  lines: string[];
  cursorRow: number;
  cursorCol: number;
  scrollOffset: number;
}

export function createNoteEditorState(
  noteId: string | null,
  content: string,
): NoteEditorState {
  const lines = content ? content.split("\n") : [""];
  return {
    noteId,
    lines,
    cursorRow: lines.length - 1,
    cursorCol: (lines[lines.length - 1] ?? "").length,
    scrollOffset: 0,
  };
}

export function getNoteContent(state: NoteEditorState): string {
  return state.lines.join("\n");
}

export function renderNoteEditor(
  state: NoteEditorState,
  cols: number,
  rows: number,
): string {
  const width = Math.min(80, cols - 4);
  const height = Math.min(30, rows - 4);
  const x = Math.floor((cols - width) / 2);
  const y = Math.floor((rows - height) / 2);

  const content = getNoteContent(state);
  const title = state.noteId ? deriveTitle(content) : "New Note";

  let out = renderBox({
    x,
    y,
    width,
    height,
    title,
    borderFg: "#89b4fa",
    bg: "#1e1e2e",
    fg: "#cdd6f4",
  });

  // Content area (inside box, excluding border rows and hint row)
  const contentHeight = height - 3; // top border + bottom border + hint row
  const contentWidth = width - 4; // 2 border chars + 2 padding chars

  // Adjust scroll offset to keep cursor visible
  if (state.cursorRow < state.scrollOffset) {
    state.scrollOffset = state.cursorRow;
  } else if (state.cursorRow >= state.scrollOffset + contentHeight) {
    state.scrollOffset = state.cursorRow - contentHeight + 1;
  }

  // Render visible lines
  for (let i = 0; i < contentHeight; i++) {
    const lineIdx = state.scrollOffset + i;
    const line =
      lineIdx < state.lines.length ? (state.lines[lineIdx] ?? "") : "";
    const display =
      line.length > contentWidth
        ? line.slice(0, contentWidth)
        : line + " ".repeat(contentWidth - line.length);
    out += renderText(x + 2, y + 1 + i, display, "#cdd6f4", "#1e1e2e");
  }

  // Hint at bottom
  const hint = "Ctrl+S: save & close  Esc: save & close";
  out += renderText(
    x + Math.floor((width - hint.length) / 2),
    y + height - 1,
    hint,
    "#585b70",
  );

  // Show cursor position
  const cursorScreenRow = y + 1 + (state.cursorRow - state.scrollOffset);
  const cursorScreenCol = x + 2 + Math.min(state.cursorCol, contentWidth - 1);
  out += ansi.moveTo(cursorScreenCol, cursorScreenRow);
  out += ansi.showCursor();

  return out;
}
