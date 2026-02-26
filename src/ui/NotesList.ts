import { Fzf } from "fzf";
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
  query: string;
  selectedIndex: number;
  allNotes: NotesListEntry[];
  filtered: NotesListEntry[];
  confirmDelete: boolean; // true when awaiting delete confirmation
}

export function createNotesListState(notes: NotesListEntry[]): NotesListState {
  return {
    query: "",
    selectedIndex: 0,
    allNotes: notes,
    filtered: [...notes],
    confirmDelete: false,
  };
}

export function updateNotesFilter(state: NotesListState): void {
  if (state.query === "") {
    state.filtered = [...state.allNotes];
  } else {
    const fzf = new Fzf(state.allNotes, {
      selector: (n) => deriveTitle(n.content) + " " + n.content,
    });
    state.filtered = fzf.find(state.query).map((r) => r.item);
  }
  if (state.filtered.length === 0) {
    state.selectedIndex = 0;
  } else if (state.selectedIndex >= state.filtered.length) {
    state.selectedIndex = state.filtered.length - 1;
  }
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
  const maxItems = Math.min(state.filtered.length, rows - 10);
  const width = Math.min(60, cols - 4);
  const height = Math.max(8, maxItems + 6);
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

  // Query input line
  const queryDisplay = `> ${state.query}_`;
  out += renderText(x + 2, y + 1, queryDisplay, "#cdd6f4", "#1e1e2e");

  // Separator
  const sep = "\u2500".repeat(width - 4);
  out += renderText(x + 2, y + 2, sep, "#585b70", "#1e1e2e");

  if (state.filtered.length === 0) {
    const msg = state.allNotes.length === 0 ? "No notes yet" : "No matches";
    out += renderText(x + 2, y + 3, msg, "#585b70", "#1e1e2e");
  } else {
    const items = state.filtered.slice(0, maxItems).map((n) => {
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
      y + 3,
      items,
      state.selectedIndex,
      "#a6adc8",
      "#cdd6f4",
      "#313244",
      "#1e1e2e",
    );
  }

  // Hint / confirmation
  if (state.confirmDelete && state.filtered.length > 0) {
    const hint = "Delete this note? y: yes  n: cancel";
    out += renderText(
      x + Math.floor((width - hint.length) / 2),
      y + height - 1,
      hint,
      "#f38ba8",
    );
  } else {
    const hint = "Enter: open  d: delete  /: search  Esc: close";
    out += renderText(
      x + Math.floor((width - hint.length) / 2),
      y + height - 1,
      hint,
      "#585b70",
    );
  }

  return out;
}
