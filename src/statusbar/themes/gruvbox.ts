import type { StatusBarThemeDef } from "./types.ts";

export const gruvbox: StatusBarThemeDef = {
  name: "gruvbox",
  resolve: () => ({
    bar: { bg: "#282828", fg: "#ebdbb2" },
    accents: [
      "#83a598",
      "#b8bb26",
      "#fabd2f",
      "#d3869b",
      "#fb4934",
      "#fe8019",
      "#8ec07c",
      "#458588",
    ],
    modules: {
      session: { fg: "#282828", bg: "#83a598" },
      activeWindow: { fg: "#282828", bg: "#b8bb26" },
      inactiveWindow: { fg: "#ebdbb2", bg: "#3c3836" },
      gitClean: { fg: "#282828", bg: "#b8bb26" },
      gitDirty: { fg: "#282828", bg: "#fabd2f" },
      cpuLow: { fg: "#282828", bg: "#b8bb26" },
      cpuMed: { fg: "#282828", bg: "#fabd2f" },
      cpuHigh: { fg: "#282828", bg: "#fb4934" },
      batteryHigh: { fg: "#282828", bg: "#b8bb26" },
      batteryMed: { fg: "#282828", bg: "#fabd2f" },
      batteryLow: { fg: "#282828", bg: "#fb4934" },
      prefix: { fg: "#282828", bg: "#fb4934" },
      prefixInactive: { fg: "#665c54", bg: "#3c3836" },
    },
  }),
};
