import * as ansi from "../renderer/ansi.ts";
import { getBorderChars, type BorderStyle } from "../renderer/border.ts";

export interface SidebarSessionEntry {
  id: string;
  name: string;
  windowCount: number;
  attached: boolean;
  isActive: boolean;
}

export interface SessionSidebarState {
  selectedIndex: number;
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
  return { selectedIndex, sessions };
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

  // Session list
  const listStartY = 2;
  const maxItems = height - 4; // space for title, separator, bottom hints
  const scrollOffset = Math.max(0, state.selectedIndex - maxItems + 1);
  const visibleSessions = state.sessions.slice(
    scrollOffset,
    scrollOffset + maxItems,
  );

  for (let i = 0; i < visibleSessions.length; i++) {
    const session = visibleSessions[i]!;
    const isSelected = i + scrollOffset === state.selectedIndex;
    const y = listStartY + i;
    const prefix = isSelected ? "▸ " : "  ";
    const activeMarker = session.isActive ? " *" : "";
    const attachedMarker = session.attached ? " ●" : "";
    let label = `${prefix}${session.name}${activeMarker}${attachedMarker}`;

    // Truncate if needed
    if (label.length > contentWidth - 1) {
      label = label.slice(0, contentWidth - 4) + "...";
    }
    // Pad to fill width
    label = label.padEnd(contentWidth);

    out += ansi.moveTo(screenX, y);
    if (isSelected) {
      out += ansi.bgHex("#313244") + activeFgStr + ansi.bold();
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
