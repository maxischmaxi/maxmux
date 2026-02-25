import * as ansi from "../renderer/ansi.ts";
import { renderBox, renderList, renderText } from "./components.ts";
import { deriveTitle } from "../persistence/notes-db.ts";

export interface NotesListEntry {
  id: string;
  content: string;
  created_at: number;
  updated_at: number;
}

export interface NotesListState {
  selectedIndex: number;
  notes: NotesListEntry[];
  confirmDelete: boolean; // true when awaiting delete confirmation
}

export function createNotesListState(notes: NotesListEntry[]): NotesListState {
  return {
    selectedIndex: 0,
    notes,
    confirmDelete: false,
  };
}

function formatDate(timestamp: number): string {
  const d = new Date(timestamp);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function renderNotesList(
  state: NotesListState,
  cols: number,
  rows: number,
): string {
  const maxItems = Math.min(state.notes.length, rows - 8);
  const width = Math.min(60, cols - 4);
  const height = Math.max(6, maxItems + 4);
  const x = Math.floor((cols - width) / 2);
  const y = Math.floor((rows - height) / 2);

  let out = renderBox({
    x,
    y,
    width,
    height,
    title: "Notes",
    borderFg: "#89b4fa",
    bg: "#1e1e2e",
    fg: "#cdd6f4",
  });

  if (state.notes.length === 0) {
    out += renderText(x + 2, y + 1, "No notes yet", "#585b70", "#1e1e2e");
  } else {
    const items = state.notes.slice(0, maxItems).map((n) => {
      const title = deriveTitle(n.content);
      const date = formatDate(n.updated_at);
      const maxTitleLen = width - date.length - 8;
      const displayTitle =
        title.length > maxTitleLen
          ? title.slice(0, maxTitleLen - 3) + "..."
          : title;
      return `${displayTitle.padEnd(maxTitleLen + 2)}${date}`;
    });

    out += renderList(
      x + 2,
      y + 1,
      items,
      state.selectedIndex,
      "#a6adc8",
      "#cdd6f4",
      "#313244",
      "#1e1e2e",
    );
  }

  // Hint / confirmation
  if (state.confirmDelete && state.notes.length > 0) {
    const hint = "Delete this note? y: yes  n: cancel";
    out += renderText(
      x + Math.floor((width - hint.length) / 2),
      y + height - 1,
      hint,
      "#f38ba8",
    );
  } else {
    const hint = "Enter: open  d: delete  Esc: close";
    out += renderText(
      x + Math.floor((width - hint.length) / 2),
      y + height - 1,
      hint,
      "#585b70",
    );
  }

  return out;
}
