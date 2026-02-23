// ANSI escape code utilities

export const ESC = "\x1b";
export const CSI = `${ESC}[`;

// Cursor movement
export const moveTo = (x: number, y: number) => `${CSI}${y + 1};${x + 1}H`;
export const moveToOrigin = () => `${CSI}H`;
export const hideCursor = () => `${CSI}?25l`;
export const showCursor = () => `${CSI}?25h`;
export const saveCursor = () => `${ESC}7`;
export const restoreCursor = () => `${ESC}8`;

// Cursor shape (DECSCUSR)
// 0 = default, 1 = blinking block, 2 = steady block,
// 3 = blinking underline, 4 = steady underline,
// 5 = blinking bar, 6 = steady bar
export const setCursorStyle = (style: number) => `${CSI}${style} q`;

// Screen
export const clearScreen = () => `${CSI}2J`;
export const clearLine = () => `${CSI}2K`;
export const clearToEnd = () => `${CSI}J`;

// Colors (256-color)
export const fgColor = (n: number) => `${CSI}38;5;${n}m`;
export const bgColor = (n: number) => `${CSI}48;5;${n}m`;
export const resetStyle = () => `${CSI}0m`;

// RGB colors
export const fgRgb = (r: number, g: number, b: number) =>
  `${CSI}38;2;${r};${g};${b}m`;
export const bgRgb = (r: number, g: number, b: number) =>
  `${CSI}48;2;${r};${g};${b}m`;

// Styles
export const bold = () => `${CSI}1m`;
export const dim = () => `${CSI}2m`;
export const italic = () => `${CSI}3m`;
export const underline = () => `${CSI}4m`;
export const inverse = () => `${CSI}7m`;

// Alternative screen buffer
export const enterAltScreen = () => `${CSI}?1049h`;
export const exitAltScreen = () => `${CSI}?1049l`;

// Mouse
export const enableMouse = () => `${CSI}?1000h${CSI}?1006h`;
export const disableMouse = () => `${CSI}?1000l${CSI}?1006l`;

/**
 * Parse a hex color string to RGB values.
 */
export function hexToRgb(hex: string): [number, number, number] {
  const clean = hex.replace("#", "");
  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);
  return [r, g, b];
}

/**
 * Apply fg color from hex string.
 */
export function fgHex(hex: string): string {
  const [r, g, b] = hexToRgb(hex);
  return fgRgb(r, g, b);
}

/**
 * Apply bg color from hex string.
 */
export function bgHex(hex: string): string {
  const [r, g, b] = hexToRgb(hex);
  return bgRgb(r, g, b);
}
