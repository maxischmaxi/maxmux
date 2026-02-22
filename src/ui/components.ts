import * as ansi from "../renderer/ansi.ts";

export interface BoxOptions {
  x: number;
  y: number;
  width: number;
  height: number;
  title?: string;
  fg?: string;
  bg?: string;
  borderFg?: string;
}

export function renderBox(opts: BoxOptions): string {
  const { x, y, width, height, title, fg, bg, borderFg } = opts;
  let out = "";

  const borderColor = borderFg ? ansi.fgHex(borderFg) : "";
  const textColor = fg ? ansi.fgHex(fg) : "";
  const bgColor = bg ? ansi.bgHex(bg) : "";
  const style = borderColor + bgColor;

  // Top border
  out += ansi.moveTo(x, y) + style;
  out += "╭";
  if (title) {
    const titleStr = ` ${title} `;
    out += titleStr;
    out += "─".repeat(Math.max(0, width - 2 - titleStr.length));
  } else {
    out += "─".repeat(width - 2);
  }
  out += "╮";

  // Sides
  for (let row = 1; row < height - 1; row++) {
    out += ansi.moveTo(x, y + row) + style;
    out += "│" + bgColor + textColor;
    out += " ".repeat(width - 2);
    out += style + "│";
  }

  // Bottom border
  out += ansi.moveTo(x, y + height - 1) + style;
  out += "╰" + "─".repeat(width - 2) + "╯";
  out += ansi.resetStyle();

  return out;
}

export function renderText(
  x: number,
  y: number,
  text: string,
  fg?: string,
  bg?: string,
  isBold?: boolean,
): string {
  let out = ansi.moveTo(x, y);
  if (fg) out += ansi.fgHex(fg);
  if (bg) out += ansi.bgHex(bg);
  if (isBold) out += ansi.bold();
  out += text;
  out += ansi.resetStyle();
  return out;
}

export function renderList(
  x: number,
  y: number,
  items: string[],
  selectedIndex: number,
  fg?: string,
  selectedFg?: string,
  selectedBg?: string,
): string {
  let out = "";
  for (let i = 0; i < items.length; i++) {
    const isSelected = i === selectedIndex;
    out += ansi.moveTo(x, y + i);
    if (isSelected) {
      if (selectedFg) out += ansi.fgHex(selectedFg);
      if (selectedBg) out += ansi.bgHex(selectedBg);
      out += ansi.bold();
      out += `> ${items[i]}`;
    } else {
      if (fg) out += ansi.fgHex(fg);
      out += `  ${items[i]}`;
    }
    out += ansi.resetStyle();
  }
  return out;
}
