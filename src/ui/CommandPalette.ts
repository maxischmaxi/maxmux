import type { Command } from "../core/command.ts";
import * as ansi from "../renderer/ansi.ts";
import { renderBox, renderList, renderText } from "./components.ts";

export interface CommandPaletteState {
  visible: boolean;
  query: string;
  selectedIndex: number;
  filteredCommands: Command[];
}

export function createCommandPaletteState(): CommandPaletteState {
  return {
    visible: false,
    query: "",
    selectedIndex: 0,
    filteredCommands: [],
  };
}

export function filterCommands(commands: Command[], query: string): Command[] {
  if (!query) return commands;
  const lower = query.toLowerCase();
  return commands.filter(
    (c) =>
      c.id.toLowerCase().includes(lower) ||
      c.description.toLowerCase().includes(lower),
  );
}

export function renderCommandPalette(
  state: CommandPaletteState,
  cols: number,
  rows: number,
): string {
  if (!state.visible) return "";

  const width = Math.min(60, cols - 4);
  const height = Math.min(state.filteredCommands.length + 4, rows - 4);
  const x = Math.floor((cols - width) / 2);
  const y = Math.floor((rows - height) / 2);

  let out = renderBox({
    x,
    y,
    width,
    height,
    title: "Command Palette",
    borderFg: "#89b4fa",
    bg: "#1e1e2e",
    fg: "#cdd6f4",
  });

  // Query input
  out += renderText(x + 2, y + 1, `> ${state.query}_`, "#cdd6f4", "#1e1e2e");

  // Command list
  const items = state.filteredCommands
    .slice(0, height - 4)
    .map((c) => `${c.id}  ${c.description}`);

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

  return out;
}
