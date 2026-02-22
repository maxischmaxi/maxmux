export type SeparatorStyle =
  | "powerline"
  | "rounded"
  | "flat"
  | "arrow"
  | "slant";

export interface SeparatorChars {
  left: string;
  right: string;
}

const SEPARATOR_STYLES: Record<SeparatorStyle, SeparatorChars> = {
  powerline: { left: "\ue0b0", right: "\ue0b2" },
  rounded: { left: "\ue0b4", right: "\ue0b6" },
  flat: { left: " ", right: " " },
  arrow: { left: ">", right: "<" },
  slant: { left: "\ue0b8", right: "\ue0ba" },
};

export function getSeparatorChars(
  style: SeparatorStyle,
  customLeft?: string,
  customRight?: string,
): SeparatorChars {
  const base = SEPARATOR_STYLES[style] || SEPARATOR_STYLES.powerline;
  return {
    left: customLeft || base.left,
    right: customRight || base.right,
  };
}

/**
 * Render a left separator between two segments.
 * fg = previous segment bg, bg = next segment bg
 */
export function renderLeftSeparator(
  char: string,
  prevBg: string,
  nextBg: string,
): { text: string; fg: string; bg: string } {
  return { text: char, fg: prevBg, bg: nextBg };
}

/**
 * Render a right separator between two segments.
 * fg = next segment bg, bg = previous segment bg
 */
export function renderRightSeparator(
  char: string,
  prevBg: string,
  nextBg: string,
): { text: string; fg: string; bg: string } {
  return { text: char, fg: nextBg, bg: prevBg };
}
