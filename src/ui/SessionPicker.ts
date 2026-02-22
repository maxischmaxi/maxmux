import * as ansi from "../renderer/ansi.ts";
import { renderBox, renderList } from "./components.ts";

export interface SessionPickerState {
  visible: boolean;
  selectedIndex: number;
  sessions: Array<{
    id: string;
    name: string;
    windowCount: number;
    attached: boolean;
  }>;
}

export function createSessionPickerState(): SessionPickerState {
  return {
    visible: false,
    selectedIndex: 0,
    sessions: [],
  };
}

export function renderSessionPicker(
  state: SessionPickerState,
  cols: number,
  rows: number,
): string {
  if (!state.visible) return "";

  const width = Math.min(50, cols - 4);
  const height = Math.min(state.sessions.length + 4, rows - 4);
  const x = Math.floor((cols - width) / 2);
  const y = Math.floor((rows - height) / 2);

  let out = renderBox({
    x,
    y,
    width,
    height,
    title: "Sessions",
    borderFg: "#89b4fa",
    bg: "#1e1e2e",
    fg: "#cdd6f4",
  });

  const items = state.sessions.map((s) => {
    const attached = s.attached ? " (attached)" : "";
    return `${s.name}${attached} - ${s.windowCount} window(s)`;
  });

  out += renderList(
    x + 2,
    y + 2,
    items,
    state.selectedIndex,
    "#a6adc8",
    "#cdd6f4",
    "#313244",
  );

  return out;
}
