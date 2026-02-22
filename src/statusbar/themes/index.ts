import type { ResolvedStatusBarTheme } from "../types.ts";
import type { StatusBarThemeDef } from "./types.ts";
import { catppuccinMocha } from "./catppuccin.ts";
import { dracula } from "./dracula.ts";
import { nord } from "./nord.ts";
import { tokyoNight } from "./tokyo-night.ts";
import { gruvbox } from "./gruvbox.ts";
import { oneDark } from "./one-dark.ts";
import { solarized } from "./solarized.ts";

const themes: Map<string, StatusBarThemeDef> = new Map([
  ["catppuccin-mocha", catppuccinMocha],
  ["dracula", dracula],
  ["nord", nord],
  ["tokyo-night", tokyoNight],
  ["gruvbox", gruvbox],
  ["one-dark", oneDark],
  ["solarized", solarized],
]);

export function resolveTheme(
  themeName: string,
  fallbackColors?: { bg: string; fg: string; active: string },
): ResolvedStatusBarTheme {
  if (themeName === "custom" && fallbackColors) {
    return {
      bar: { bg: fallbackColors.bg, fg: fallbackColors.fg },
      accents: [fallbackColors.active],
      modules: {
        session: { fg: fallbackColors.bg, bg: fallbackColors.active },
        activeWindow: { fg: fallbackColors.bg, bg: fallbackColors.active },
        inactiveWindow: { fg: fallbackColors.fg, bg: fallbackColors.bg },
        gitClean: { fg: fallbackColors.bg, bg: fallbackColors.active },
        gitDirty: { fg: fallbackColors.bg, bg: fallbackColors.active },
        cpuLow: { fg: fallbackColors.bg, bg: fallbackColors.active },
        cpuMed: { fg: fallbackColors.bg, bg: fallbackColors.active },
        cpuHigh: { fg: fallbackColors.bg, bg: fallbackColors.active },
        batteryHigh: { fg: fallbackColors.bg, bg: fallbackColors.active },
        batteryMed: { fg: fallbackColors.bg, bg: fallbackColors.active },
        batteryLow: { fg: fallbackColors.bg, bg: fallbackColors.active },
        prefix: { fg: fallbackColors.bg, bg: fallbackColors.active },
        prefixInactive: { fg: fallbackColors.fg, bg: fallbackColors.bg },
      },
    };
  }

  const theme = themes.get(themeName);
  if (theme) return theme.resolve();

  // Fallback to catppuccin
  return catppuccinMocha.resolve();
}

export { type StatusBarThemeDef } from "./types.ts";
