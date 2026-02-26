export type BorderStyle = "rounded" | "sharp" | "double" | "none";
export type LineStyle = "solid" | "dashed" | "dotted";

export interface BorderChars {
  topLeft: string;
  topRight: string;
  bottomLeft: string;
  bottomRight: string;
  horizontal: string;
  vertical: string;
  teeLeft: string;
  teeRight: string;
  teeTop: string;
  teeBottom: string;
  cross: string;
}

const BORDER_STYLES: Record<BorderStyle, BorderChars> = {
  rounded: {
    topLeft: "╭",
    topRight: "╮",
    bottomLeft: "╰",
    bottomRight: "╯",
    horizontal: "─",
    vertical: "│",
    teeLeft: "├",
    teeRight: "┤",
    teeTop: "┬",
    teeBottom: "┴",
    cross: "┼",
  },
  sharp: {
    topLeft: "┌",
    topRight: "┐",
    bottomLeft: "└",
    bottomRight: "┘",
    horizontal: "─",
    vertical: "│",
    teeLeft: "├",
    teeRight: "┤",
    teeTop: "┬",
    teeBottom: "┴",
    cross: "┼",
  },
  double: {
    topLeft: "╔",
    topRight: "╗",
    bottomLeft: "╚",
    bottomRight: "╝",
    horizontal: "═",
    vertical: "║",
    teeLeft: "╠",
    teeRight: "╣",
    teeTop: "╦",
    teeBottom: "╩",
    cross: "╬",
  },
  none: {
    topLeft: " ",
    topRight: " ",
    bottomLeft: " ",
    bottomRight: " ",
    horizontal: " ",
    vertical: " ",
    teeLeft: " ",
    teeRight: " ",
    teeTop: " ",
    teeBottom: " ",
    cross: " ",
  },
};

export function getBorderChars(style: BorderStyle): BorderChars {
  return BORDER_STYLES[style];
}

const LINE_STYLE_CHARS: Record<
  LineStyle,
  { horizontal: string; vertical: string }
> = {
  solid: { horizontal: "─", vertical: "│" },
  dashed: { horizontal: "┄", vertical: "┆" },
  dotted: { horizontal: "┈", vertical: "┊" },
};

export function getLineChars(lineStyle: LineStyle): {
  horizontal: string;
  vertical: string;
} {
  return LINE_STYLE_CHARS[lineStyle];
}

export interface BorderSegment {
  orientation: "v" | "h";
  fixedCoord: number;
  start: number;
  end: number;
  firstChildPanes: string[];
  secondChildPanes: string[];
}

export interface BorderCellMeta {
  x: number;
  y: number;
  segments: BorderSegment[];
}
