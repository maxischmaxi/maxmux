import { renderBox, renderText } from "./components.ts";

export interface ConfigErrorDialogState {
  errorMessage: string;
}

export function createConfigErrorDialogState(
  errorMessage: string,
): ConfigErrorDialogState {
  return { errorMessage };
}

export function renderConfigErrorDialog(
  state: ConfigErrorDialogState,
  cols: number,
  rows: number,
): string {
  const maxWidth = Math.min(70, cols - 4);
  const errorLines = wrapText(state.errorMessage, maxWidth - 4);
  // Title + blank + "Config reload failed..." + blank + error lines + blank + hint
  const height = Math.min(errorLines.length + 7, rows - 2);
  const x = Math.floor((cols - maxWidth) / 2);
  const y = Math.floor((rows - height) / 2);

  const borderColor = "#f38ba8"; // Catppuccin Red
  const fg = "#cdd6f4";
  const bg = "#1e1e2e";

  let out = renderBox({
    x,
    y,
    width: maxWidth,
    height,
    title: "Config Error",
    fg,
    bg,
    borderFg: borderColor,
  });

  out += renderText(
    x + 2,
    y + 2,
    "Config reload failed. Using previous config.",
    "#f9e2af", // Catppuccin Yellow (warning)
    bg,
    true,
  );

  const maxErrorLines = height - 7;
  for (let i = 0; i < Math.min(errorLines.length, maxErrorLines); i++) {
    out += renderText(x + 2, y + 4 + i, errorLines[i]!, fg, bg);
  }

  out += renderText(
    x + 2,
    y + height - 2,
    "Press any key to dismiss",
    "#6c7086", // Catppuccin Overlay0 (subtle)
    bg,
  );

  return out;
}

function wrapText(text: string, maxWidth: number): string[] {
  const lines: string[] = [];
  for (const line of text.split("\n")) {
    if (line.length <= maxWidth) {
      lines.push(line);
    } else {
      for (let i = 0; i < line.length; i += maxWidth) {
        lines.push(line.slice(i, i + maxWidth));
      }
    }
  }
  return lines;
}
