// SGR mouse event parser and encoder for terminal mouse tracking
// SGR format: \x1b[<Cb;Cx;CyM (press) or \x1b[<Cb;Cx;Cym (release)
// Coords are 1-based in the protocol, converted to 0-based in MouseEvent

export interface MouseEvent {
  button: number; // raw Cb value (includes modifiers + motion flag)
  x: number; // 0-based column
  y: number; // 0-based row
  isRelease: boolean;
}

export const MOUSE_LEFT = 0;
export const MOUSE_MIDDLE = 1;
export const MOUSE_RIGHT = 2;
export const MOUSE_SCROLL_UP = 64;
export const MOUSE_SCROLL_DOWN = 65;

/** Extract base button (0=left, 1=middle, 2=right, 64+=scroll) */
export function getBaseButton(button: number): number {
  // Bits 0-1 encode button, bit 6 is scroll flag
  // Mask out modifier bits (4=shift, 8=meta, 16=ctrl) and motion bit (32)
  return button & 0b1000011;
}

/** Check if this is a scroll event */
export function isScrollEvent(button: number): boolean {
  return (button & 64) !== 0;
}

/** Check if this is a motion event (mouse drag / mouse move) */
export function isMotionEvent(button: number): boolean {
  return (button & 32) !== 0;
}

export interface ParseResult {
  event: MouseEvent;
  consumed: number; // number of bytes consumed from the buffer
}

/**
 * Parse an SGR mouse sequence from a buffer.
 * Returns null if the buffer doesn't start with a valid SGR mouse sequence.
 * Handles partial sequences by returning null (caller should buffer and retry).
 */
export function parseSgrMouse(data: Buffer, offset = 0): ParseResult | null {
  // Minimum: \x1b[<0;1;1M = 9 bytes
  if (offset + 9 > data.length) return null;
  if (
    data[offset] !== 0x1b ||
    data[offset + 1] !== 0x5b ||
    data[offset + 2] !== 0x3c
  ) {
    return null;
  }

  // Find the terminator (M or m)
  let end = offset + 3;
  while (end < data.length) {
    const byte = data[end]!;
    if (byte === 0x4d || byte === 0x6d) {
      // 'M' or 'm'
      break;
    }
    // Valid chars in the sequence: digits and semicolons
    if ((byte >= 0x30 && byte <= 0x39) || byte === 0x3b) {
      end++;
      continue;
    }
    // Invalid character — not a mouse sequence
    return null;
  }

  if (end >= data.length) {
    // Incomplete sequence
    return null;
  }

  const isRelease = data[end] === 0x6d; // 'm' = release, 'M' = press
  const params = data.subarray(offset + 3, end).toString("ascii");
  const parts = params.split(";");

  if (parts.length !== 3) return null;

  const button = parseInt(parts[0]!, 10);
  const x = parseInt(parts[1]!, 10) - 1; // Convert to 0-based
  const y = parseInt(parts[2]!, 10) - 1; // Convert to 0-based

  if (isNaN(button) || isNaN(x) || isNaN(y)) return null;

  return {
    event: { button, x, y, isRelease },
    consumed: end - offset + 1,
  };
}

/**
 * Encode a mouse event back to SGR format with new (pane-local) coordinates.
 * Coordinates are 0-based, will be converted to 1-based for the protocol.
 */
export function encodeSgrMouse(
  button: number,
  localX: number,
  localY: number,
  isRelease: boolean,
): string {
  const terminator = isRelease ? "m" : "M";
  return `\x1b[<${button};${localX + 1};${localY + 1}${terminator}`;
}
