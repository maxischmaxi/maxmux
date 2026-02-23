import * as ansi from "../renderer/ansi.ts";
import { getBorderChars, type BorderStyle } from "../renderer/border.ts";

export interface SidebarWindowEntry {
  name: string;
  index: number;
  isActive: boolean;
}

export interface SidebarSessionEntry {
  id: string;
  name: string;
  windowCount: number;
  windows: SidebarWindowEntry[];
  attached: boolean;
  isActive: boolean;
}

export interface SessionSidebarState {
  selectedIndex: number;
  scrollOffset: number;
  sessions: SidebarSessionEntry[];
}

export function createSessionSidebarState(
  sessions: SidebarSessionEntry[],
  activeSessionId: string,
): SessionSidebarState {
  const selectedIndex = Math.max(
    0,
    sessions.findIndex((s) => s.id === activeSessionId),
  );
  return { selectedIndex, scrollOffset: 0, sessions };
}

export function updateSidebarSessions(
  state: SessionSidebarState,
  sessions: SidebarSessionEntry[],
): void {
  const previousSelectedId = state.sessions[state.selectedIndex]?.id;
  state.sessions = sessions;
  if (previousSelectedId) {
    const newIndex = sessions.findIndex((s) => s.id === previousSelectedId);
    state.selectedIndex = newIndex >= 0 ? newIndex : 0;
  }
  // Clamp
  if (state.sessions.length === 0) {
    state.selectedIndex = 0;
  } else if (state.selectedIndex >= state.sessions.length) {
    state.selectedIndex = state.sessions.length - 1;
  }
}

export function renderSessionSidebar(
  state: SessionSidebarState,
  width: number,
  height: number,
  screenX: number,
  position: "left" | "right",
  borderStyle: BorderStyle,
  theme: { fg: string; bg: string; activeFg: string; borderFg: string },
): string {
  const borderChars = getBorderChars(borderStyle);
  const contentWidth = width;
  let out = "";

  // Background fill
  const bgStr = ansi.bgHex(theme.bg);
  const fgStr = ansi.fgHex(theme.fg);
  const activeFgStr = ansi.fgHex(theme.activeFg);
  const borderFgStr = ansi.fgHex(theme.borderFg);

  for (let y = 0; y < height; y++) {
    out += ansi.moveTo(screenX, y) + bgStr + fgStr;
    out += " ".repeat(contentWidth);
    out += ansi.resetStyle();
  }

  // Title
  const title = " Sessions ";
  const titleX =
    screenX + Math.max(0, Math.floor((contentWidth - title.length) / 2));
  out += ansi.moveTo(titleX, 0) + bgStr + activeFgStr + ansi.bold();
  out += title;
  out += ansi.resetStyle();

  // Separator under title
  out += ansi.moveTo(screenX, 1) + bgStr + borderFgStr;
  out += "─".repeat(contentWidth);
  out += ansi.resetStyle();

  // Build display rows: each session is 1 row, selected session also shows windows
  interface DisplayRow {
    type: "session" | "window";
    sessionIndex: number;
    label: string;
    isSelected: boolean;
    isActiveWindow?: boolean;
  }

  const displayRows: DisplayRow[] = [];
  for (let si = 0; si < state.sessions.length; si++) {
    const session = state.sessions[si]!;
    const isSelected = si === state.selectedIndex;
    const prefix = isSelected ? "▸ " : "  ";
    const activeMarker = session.isActive ? " *" : "";
    const attachedMarker = session.attached ? " ●" : "";
    const label = `${prefix}${session.name}${activeMarker}${attachedMarker}`;
    displayRows.push({ type: "session", sessionIndex: si, label, isSelected });

    // Show windows only for selected session
    if (isSelected && session.windows.length > 0) {
      for (let wi = 0; wi < session.windows.length; wi++) {
        const w = session.windows[wi]!;
        const isLast = wi === session.windows.length - 1;
        const treeChar = isLast ? "└" : "├";
        const activeW = w.isActive ? " *" : "";
        const wLabel = `  ${treeChar} ${w.index}:${w.name}${activeW}`;
        displayRows.push({
          type: "window",
          sessionIndex: si,
          label: wLabel,
          isSelected: true,
          isActiveWindow: w.isActive,
        });
      }
    }
  }

  // Session list with scroll
  const listStartY = 2;
  const maxItems = height - 4; // space for title, separator, bottom hints

  // Ensure selected session header + all its windows are visible
  const selectedFirstRow = displayRows.findIndex(
    (r) => r.type === "session" && r.sessionIndex === state.selectedIndex,
  );
  const selectedLastRow = displayRows.findLastIndex(
    (r) => r.sessionIndex === state.selectedIndex,
  );

  // Adjust scroll offset to keep selected block visible
  if (selectedFirstRow >= 0) {
    const blockSize = selectedLastRow - selectedFirstRow + 1;
    if (selectedFirstRow < state.scrollOffset) {
      state.scrollOffset = selectedFirstRow;
    } else if (selectedLastRow >= state.scrollOffset + maxItems) {
      // Try to fit the whole block; if block > maxItems, show from the start
      if (blockSize <= maxItems) {
        state.scrollOffset = selectedLastRow - maxItems + 1;
      } else {
        state.scrollOffset = selectedFirstRow;
      }
    }
  }
  state.scrollOffset = Math.max(
    0,
    Math.min(state.scrollOffset, displayRows.length - maxItems),
  );
  if (state.scrollOffset < 0) state.scrollOffset = 0;

  const visibleRows = displayRows.slice(
    state.scrollOffset,
    state.scrollOffset + maxItems,
  );

  const selectedBg = ansi.bgHex("#313244");

  for (let i = 0; i < visibleRows.length; i++) {
    const row = visibleRows[i]!;
    const y = listStartY + i;

    let label = row.label;
    // Truncate if needed
    if (label.length > contentWidth - 1) {
      label = label.slice(0, contentWidth - 4) + "...";
    }
    // Pad to fill width
    label = label.padEnd(contentWidth);

    out += ansi.moveTo(screenX, y);
    if (row.isSelected && row.type === "session") {
      out += selectedBg + activeFgStr + ansi.bold();
    } else if (row.isSelected && row.type === "window") {
      out += selectedBg;
      if (row.isActiveWindow) {
        out += activeFgStr + ansi.bold();
      } else {
        out += fgStr;
      }
    } else {
      out += bgStr + fgStr;
    }
    out += label;
    out += ansi.resetStyle();
  }

  // Bottom hints
  const hintsY = height - 1;
  const hints = "j/k:nav  ⏎:switch  esc:close";
  const hintsStr =
    hints.length > contentWidth ? hints.slice(0, contentWidth) : hints;
  out += ansi.moveTo(screenX + 1, hintsY) + bgStr + ansi.fgHex("#585b70");
  out += hintsStr;
  out += ansi.resetStyle();

  // Border line (vertical separator)
  const borderX = position === "left" ? screenX + contentWidth : screenX - 1;
  out += borderFgStr;
  for (let y = 0; y < height; y++) {
    out += ansi.moveTo(borderX, y) + borderChars.vertical;
  }
  out += ansi.resetStyle();

  return out;
}

export function renderPreviewBar(
  sessionName: string,
  windows: SidebarWindowEntry[],
  startX: number,
  width: number,
  row: number,
  theme: { fg: string; bg: string; activeFg: string; borderFg: string },
): string {
  const bgStr = ansi.bgHex(theme.bg);
  const borderFgStr = ansi.fgHex(theme.borderFg);
  const activeFgStr = ansi.fgHex(theme.activeFg);

  let out = "";

  // Fill background
  out +=
    ansi.moveTo(startX, row) + bgStr + " ".repeat(width) + ansi.resetStyle();

  // Label: " ◆ Preview: {name} "
  const label = ` \u25C6 Preview: ${sessionName} `;
  out += ansi.moveTo(startX, row) + bgStr + borderFgStr + ansi.italic();
  out += label.length > width ? label.slice(0, width) : label;
  out += ansi.resetStyle();

  // Window tabs after the label
  let x = startX + label.length;
  const maxX = startX + width;

  for (const w of windows) {
    const tab = ` ${w.index}:${w.name} `;
    if (x + tab.length > maxX) break;

    out += ansi.moveTo(x, row) + bgStr;
    if (w.isActive) {
      out += activeFgStr + ansi.bold();
    } else {
      out += borderFgStr;
    }
    out += tab;
    out += ansi.resetStyle();
    x += tab.length;
  }

  return out;
}
