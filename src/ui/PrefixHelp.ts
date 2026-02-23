import * as ansi from "../renderer/ansi.ts";
import { renderBox } from "./components.ts";

const DESCRIPTIONS: Record<string, { en: string; de: string }> = {
  "window:create": { en: "Create new window", de: "Neues Fenster erstellen" },
  "window:next": { en: "Next window", de: "Nächstes Fenster" },
  "window:previous": { en: "Previous window", de: "Vorheriges Fenster" },
  "window:rename": { en: "Rename window", de: "Fenster umbenennen" },
  "window:close": { en: "Close window", de: "Fenster schließen" },
  "pane:split-horizontal": {
    en: "Split horizontally",
    de: "Horizontal teilen",
  },
  "pane:split-vertical": { en: "Split vertically", de: "Vertikal teilen" },
  "pane:next": { en: "Next pane", de: "Nächster Bereich" },
  "pane:close": { en: "Close pane", de: "Bereich schließen" },
  "pane:zoom": { en: "Toggle zoom", de: "Zoom umschalten" },
  "pane:focus-up": { en: "Focus pane above", de: "Bereich oben fokussieren" },
  "pane:focus-down": {
    en: "Focus pane below",
    de: "Bereich unten fokussieren",
  },
  "pane:focus-left": {
    en: "Focus pane left",
    de: "Bereich links fokussieren",
  },
  "pane:focus-right": {
    en: "Focus pane right",
    de: "Bereich rechts fokussieren",
  },
  "session:list": { en: "List sessions", de: "Sessions auflisten" },
  "session:find": { en: "Find session", de: "Session suchen" },
  "session:create": { en: "Create new session", de: "Neue Session erstellen" },
  "session:rename": { en: "Rename session", de: "Session umbenennen" },
  "session:detach": { en: "Detach", de: "Trennen" },
  "server:kill": { en: "Kill server", de: "Server beenden" },
  "command-palette": { en: "Command palette", de: "Befehlspalette" },
  "keybindings:show": {
    en: "Show all keybindings",
    de: "Alle Keybindings anzeigen",
  },
  "copy-mode:enter": { en: "Enter copy mode", de: "Kopiermodus starten" },
};

const KEY_SYMBOLS: Record<string, string> = {
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
};

export function detectLanguage(): "de" | "en" {
  const lang = process.env.LC_ALL || process.env.LANG || "";
  return lang.toLowerCase().startsWith("de") ? "de" : "en";
}

export function formatKeyDisplay(key: string): string {
  return KEY_SYMBOLS[key] ?? key;
}

export function renderPrefixHelp(
  bindings: Array<{ key: string; commandId: string }>,
  cols: number,
  rows: number,
): string {
  const lang = detectLanguage();

  // Build lines: key + description
  const lines: Array<{ key: string; desc: string }> = [];
  for (const { key, commandId } of bindings) {
    const desc =
      DESCRIPTIONS[commandId]?.[lang] ??
      DESCRIPTIONS[commandId]?.en ??
      commandId;
    lines.push({ key: formatKeyDisplay(key), desc });
  }

  // Calculate dimensions
  const maxKeyLen = Math.max(...lines.map((l) => l.key.length), 1);
  const maxDescLen = Math.max(...lines.map((l) => l.desc.length), 1);
  const contentWidth = maxKeyLen + 4 + maxDescLen; // key + "    " + desc
  const title = " Prefix Mode ";
  const footerText = lang === "de" ? "Esc zum Abbrechen" : "Esc to cancel";
  const minWidth = Math.max(
    contentWidth + 4,
    title.length + 4,
    footerText.length + 4,
  );
  const boxWidth = Math.min(minWidth, cols - 2);
  const boxHeight = Math.min(lines.length + 4, rows - 2); // +4: top border, padding, padding, bottom border
  const maxVisible = boxHeight - 4;

  const x = Math.floor((cols - boxWidth) / 2);
  const y = Math.floor((rows - boxHeight) / 2);

  let out = "";

  // Draw box
  out += renderBox({
    x,
    y,
    width: boxWidth,
    height: boxHeight,
    title: "Prefix Mode",
    fg: "#cdd6f4",
    bg: "#1e1e2e",
    borderFg: "#89b4fa",
  });

  // Draw keybinding lines
  const innerWidth = boxWidth - 4; // 2 border + 2 padding
  for (let i = 0; i < maxVisible && i < lines.length; i++) {
    const line = lines[i]!;
    const keyStr = line.key.padEnd(maxKeyLen);
    const text = `${keyStr}    ${line.desc}`;
    const truncated =
      text.length > innerWidth ? text.slice(0, innerWidth) : text;
    const padded =
      truncated + " ".repeat(Math.max(0, innerWidth - truncated.length));

    out += ansi.moveTo(x + 2, y + 1 + i);
    out += ansi.bgHex("#1e1e2e");
    out += ansi.fgHex("#89b4fa") + ansi.bold();
    out += keyStr;
    out += ansi.resetStyle();
    out += ansi.bgHex("#1e1e2e");
    out += ansi.fgHex("#6c7086");
    out += "    ";
    out += ansi.fgHex("#cdd6f4");
    const descText = line.desc.slice(0, innerWidth - maxKeyLen - 4);
    const descPad = Math.max(0, innerWidth - maxKeyLen - 4 - descText.length);
    out += descText + " ".repeat(descPad);
    out += ansi.resetStyle();
  }

  // Footer line
  const footerY = y + boxHeight - 2;
  out += ansi.moveTo(x + 2, footerY);
  out += ansi.bgHex("#1e1e2e");
  out += ansi.fgHex("#6c7086") + ansi.italic();
  const footerPad = Math.max(0, innerWidth - footerText.length);
  out += footerText + " ".repeat(footerPad);
  out += ansi.resetStyle();

  return out;
}
