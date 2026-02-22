import type { StatusBarItem } from "../plugins/types.ts";

export function createDefaultStatusBarItems(): StatusBarItem[] {
  return [];
}

export function formatWindowList(
  windows: Array<{ id: string; name: string; active: boolean }>,
): string {
  return windows
    .map((w, i) => {
      const marker = w.active ? "*" : "-";
      return `${i}:${w.name}${marker}`;
    })
    .join(" ");
}
