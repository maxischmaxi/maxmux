import type { Rect } from "../core/layout.ts";
import type { VirtualTerminal } from "../core/terminal.ts";
import type { MaxMuxConfig } from "../config/schema.ts";
import type { StatusBarItem } from "../plugins/types.ts";
import { ScreenBuffer } from "./screen.ts";
import { getBorderChars, type BorderStyle } from "./border.ts";
import * as ansi from "./ansi.ts";

export class Compositor {
  private screen: ScreenBuffer;
  private config: MaxMuxConfig;

  constructor(cols: number, rows: number, config: MaxMuxConfig) {
    this.screen = new ScreenBuffer(cols, rows);
    this.config = config;
  }

  resize(cols: number, rows: number): void {
    this.screen.resize(cols, rows);
  }

  /**
   * Compose the full screen from pane buffers, borders, and chrome.
   */
  compose(
    paneTerminals: Map<string, VirtualTerminal>,
    paneRects: Map<string, Rect>,
    activePaneId: string,
    statusBarItems: StatusBarItem[],
    windowList: Array<{ id: string; name: string; active: boolean }>,
    sessionName: string,
    zoomedPaneId: string | null,
  ): string {
    this.screen.snapshot();
    this.screen.clear();

    const borderChars = getBorderChars(
      this.config.theme.border.style as BorderStyle,
    );
    const borderFg = this.config.theme.border.fg;
    const activeBorderFg = this.config.theme.border.activeFg;

    // Reserve last row for status bar
    const contentHeight = this.screen.height - 1;

    if (zoomedPaneId) {
      // Zoomed mode: single pane fills entire content area
      const term = paneTerminals.get(zoomedPaneId);
      if (term) {
        this.renderPane(term, {
          x: 0,
          y: 0,
          width: this.screen.width,
          height: contentHeight,
        });
      }
    } else {
      // Render pane contents
      for (const [paneId, rect] of paneRects) {
        const term = paneTerminals.get(paneId);
        if (!term) continue;

        // Clamp rect to content area
        const clampedRect = {
          ...rect,
          height: Math.min(rect.height, contentHeight - rect.y),
        };

        if (clampedRect.height <= 0) continue;
        this.renderPane(term, clampedRect);
      }

      // Render borders between panes
      if (paneRects.size > 1) {
        this.renderBorders(
          paneRects,
          activePaneId,
          borderChars,
          borderFg,
          activeBorderFg,
          contentHeight,
        );
      }
    }

    // Render status bar
    this.renderStatusBar(statusBarItems, windowList, sessionName, activePaneId);

    // Calculate cursor position, style, and visibility in active pane
    let cursorX = 0;
    let cursorY = 0;
    let cursorStyle = 0;
    let cursorVisible = true;
    const activeTerm = paneTerminals.get(zoomedPaneId || activePaneId);
    const activeRect = zoomedPaneId
      ? { x: 0, y: 0, width: this.screen.width, height: contentHeight }
      : paneRects.get(activePaneId);
    if (activeTerm && activeRect) {
      cursorX = activeRect.x + activeTerm.getCursorX();
      cursorY = activeRect.y + activeTerm.getCursorY();
      cursorStyle = activeTerm.getCursorStyle();
      cursorVisible = activeTerm.isCursorVisible();
    }

    // Generate output
    return this.flush(cursorX, cursorY, cursorStyle, cursorVisible);
  }

  private renderPane(term: VirtualTerminal, rect: Rect): void {
    const lines = term.readLines();
    for (let y = 0; y < rect.height && y < lines.length; y++) {
      const line = lines[y]!;
      for (let x = 0; x < rect.width && x < line.length; x++) {
        this.screen.set(rect.x + x, rect.y + y, line[x]!);
      }
    }
  }

  private renderBorders(
    paneRects: Map<string, Rect>,
    activePaneId: string,
    borderChars: ReturnType<typeof getBorderChars>,
    borderFg: string,
    activeBorderFg: string,
    contentHeight: number,
  ): void {
    // Draw vertical dividers between panes
    const rects = [...paneRects.entries()];

    for (let i = 0; i < rects.length; i++) {
      const [paneId, rect] = rects[i]!;

      // Check if there's a pane to the right that needs a divider
      for (let j = 0; j < rects.length; j++) {
        if (i === j) continue;
        const [, otherRect] = rects[j]!;

        // Vertical divider: this pane's right edge meets other pane's left edge
        if (rect.x + rect.width + 1 === otherRect.x) {
          const dividerX = rect.x + rect.width;
          const fg = paneId === activePaneId ? activeBorderFg : borderFg;
          const startY = Math.max(rect.y, otherRect.y);
          const endY = Math.min(
            rect.y + rect.height,
            otherRect.y + otherRect.height,
            contentHeight,
          );

          for (let y = startY; y < endY; y++) {
            this.screen.set(dividerX, y, borderChars.vertical, fg);
          }
        }

        // Horizontal divider: this pane's bottom edge meets other pane's top edge
        if (rect.y + rect.height + 1 === otherRect.y) {
          const dividerY = rect.y + rect.height;
          if (dividerY >= contentHeight) continue;
          const fg = paneId === activePaneId ? activeBorderFg : borderFg;
          const startX = Math.max(rect.x, otherRect.x);
          const endX = Math.min(
            rect.x + rect.width,
            otherRect.x + otherRect.width,
          );

          for (let x = startX; x < endX; x++) {
            this.screen.set(x, dividerY, borderChars.horizontal, fg);
          }
        }
      }
    }
  }

  private renderStatusBar(
    items: StatusBarItem[],
    windowList: Array<{ id: string; name: string; active: boolean }>,
    sessionName: string,
    activePaneId: string,
  ): void {
    const y = this.screen.height - 1;
    const bg = this.config.theme.statusBar.bg;
    const fg = this.config.theme.statusBar.fg;
    const activeFg = this.config.theme.statusBar.active;

    // Fill status bar background
    this.screen.fillRow(y, " ", fg, bg);

    // Left side: session name + window list
    let x = 0;
    const sessionStr = `[${sessionName}] `;
    this.screen.writeString(x, y, sessionStr, activeFg, bg, true);
    x += sessionStr.length;

    for (let i = 0; i < windowList.length; i++) {
      const w = windowList[i]!;
      const marker = w.active ? "*" : "-";
      const windowStr = `${i}:${w.name}${marker} `;
      this.screen.writeString(x, y, windowStr, w.active ? activeFg : fg, bg);
      x += windowStr.length;
    }

    // Right side: plugin items + time
    const rightItems = items.filter((i) => i.align === "right");
    const leftItems = items.filter((i) => i.align !== "right");

    // Plugin left items
    for (const item of leftItems) {
      const str = ` ${item.text} `;
      this.screen.writeString(x, y, str, item.fg || fg, item.bg || bg);
      x += str.length;
    }

    // Right-aligned items
    const now = new Date();
    const timeStr = `${now.getHours().toString().padStart(2, "0")}:${now.getMinutes().toString().padStart(2, "0")}`;

    let rightX = this.screen.width;

    // Time
    rightX -= timeStr.length + 1;
    this.screen.writeString(rightX, y, timeStr, fg, bg);

    // Plugin right items (reverse order)
    for (const item of rightItems.reverse()) {
      const str = ` ${item.text} `;
      rightX -= str.length;
      this.screen.writeString(rightX, y, str, item.fg || fg, item.bg || bg);
    }
  }

  private flush(
    cursorX: number,
    cursorY: number,
    cursorStyle: number = 0,
    cursorVisible: boolean = true,
  ): string {
    let output = ansi.hideCursor();

    const dirty = this.screen.getDirty();

    if (dirty.length > this.screen.width * this.screen.height * 0.5) {
      // Full redraw is more efficient
      output += ansi.moveToOrigin();
      for (let y = 0; y < this.screen.height; y++) {
        output += ansi.moveTo(0, y);
        let lastFg = "";
        let lastBg = "";
        let lastBold = false;
        for (let x = 0; x < this.screen.width; x++) {
          const cell = this.screen.cells[y]![x]!;
          if (
            cell.fg !== lastFg ||
            cell.bg !== lastBg ||
            cell.bold !== lastBold
          ) {
            output += ansi.resetStyle();
            if (cell.fg) output += ansi.fgHex(cell.fg);
            if (cell.bg) output += ansi.bgHex(cell.bg);
            if (cell.bold) output += ansi.bold();
            lastFg = cell.fg;
            lastBg = cell.bg;
            lastBold = cell.bold;
          }
          output += cell.char;
        }
      }
    } else {
      // Diff-based rendering
      for (const { x, y, cell } of dirty) {
        output += ansi.moveTo(x, y);
        output += ansi.resetStyle();
        if (cell.fg) output += ansi.fgHex(cell.fg);
        if (cell.bg) output += ansi.bgHex(cell.bg);
        if (cell.bold) output += ansi.bold();
        output += cell.char;
      }
    }

    output += ansi.resetStyle();
    // Position cursor in active pane with correct style
    output += ansi.setCursorStyle(cursorStyle);
    output += ansi.moveTo(cursorX, cursorY);
    // Only show cursor if the application wants it visible (DECTCEM)
    if (cursorVisible) {
      output += ansi.showCursor();
    }

    this.screen.snapshot();

    return output;
  }
}
