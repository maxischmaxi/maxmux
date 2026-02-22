import * as ansi from "../renderer/ansi.ts";
import { renderBox, renderText } from "./components.ts";

export interface RenameDialogState {
  title: string;
  value: string;
}

export function createRenameDialogState(
  title: string,
  initialValue: string,
): RenameDialogState {
  return { title, value: initialValue };
}

export function renderRenameDialog(
  state: RenameDialogState,
  cols: number,
  rows: number,
): string {
  const width = Math.min(50, cols - 4);
  const height = 5;
  const x = Math.floor((cols - width) / 2);
  const y = Math.floor((rows - height) / 2);

  let out = renderBox({
    x,
    y,
    width,
    height,
    title: state.title,
    borderFg: "#89b4fa",
    bg: "#1e1e2e",
    fg: "#cdd6f4",
  });

  // Input line with cursor
  const inputDisplay = `> ${state.value}_`;
  const maxLen = width - 4;
  const display =
    inputDisplay.length > maxLen
      ? inputDisplay.slice(inputDisplay.length - maxLen)
      : inputDisplay;
  out += renderText(x + 2, y + 2, display, "#cdd6f4", "#1e1e2e");

  // Hint
  const hint = "Enter: confirm  Esc: cancel";
  out += renderText(
    x + Math.floor((width - hint.length) / 2),
    y + height - 1,
    hint,
    "#585b70",
  );

  return out;
}
