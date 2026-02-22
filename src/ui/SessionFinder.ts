import * as ansi from "../renderer/ansi.ts";
import { renderBox, renderList, renderText } from "./components.ts";

export interface SessionFinderEntry {
  id: string;
  name: string;
  windowCount: number;
  attached: boolean;
}

export interface SessionFinderState {
  query: string;
  selectedIndex: number;
  allSessions: SessionFinderEntry[];
  filtered: SessionFinderEntry[];
}

export function createSessionFinderState(
  sessions: SessionFinderEntry[],
): SessionFinderState {
  return {
    query: "",
    selectedIndex: 0,
    allSessions: sessions,
    filtered: [...sessions],
  };
}

export function fuzzyMatch(query: string, text: string): boolean {
  if (query === "") return true;
  return text.toLowerCase().includes(query.toLowerCase());
}

export function updateFilter(state: SessionFinderState): void {
  state.filtered = state.allSessions.filter((s) =>
    fuzzyMatch(state.query, s.name),
  );
  // Clamp selectedIndex
  if (state.filtered.length === 0) {
    state.selectedIndex = 0;
  } else if (state.selectedIndex >= state.filtered.length) {
    state.selectedIndex = state.filtered.length - 1;
  }
}

export function renderSessionFinder(
  state: SessionFinderState,
  cols: number,
  rows: number,
): string {
  const maxItems = Math.min(state.filtered.length, rows - 8);
  const width = Math.min(50, cols - 4);
  const height = Math.max(6, maxItems + 5);
  const x = Math.floor((cols - width) / 2);
  const y = Math.floor((rows - height) / 2);

  let out = renderBox({
    x,
    y,
    width,
    height,
    title: "Find Session",
    borderFg: "#89b4fa",
    bg: "#1e1e2e",
    fg: "#cdd6f4",
  });

  // Query input line
  const queryDisplay = `> ${state.query}_`;
  out += renderText(x + 2, y + 1, queryDisplay, "#cdd6f4", "#1e1e2e");

  // Separator
  const sep = "─".repeat(width - 4);
  out += renderText(x + 2, y + 2, sep, "#585b70", "#1e1e2e");

  // Filtered session list
  if (state.filtered.length === 0) {
    out += renderText(x + 2, y + 3, "No matches", "#585b70", "#1e1e2e");
  } else {
    const items = state.filtered.slice(0, maxItems).map((s) => {
      const attached = s.attached ? " (attached)" : "";
      const label = `${s.name}${attached} - ${s.windowCount} win`;
      return label.length > width - 6
        ? label.slice(0, width - 9) + "..."
        : label;
    });

    out += renderList(
      x + 2,
      y + 3,
      items,
      state.selectedIndex,
      "#a6adc8",
      "#cdd6f4",
      "#313244",
    );
  }

  return out;
}
