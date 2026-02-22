import type { StatusBarThemeDef } from "./types.ts";

export const tokyoNight: StatusBarThemeDef = {
  name: "tokyo-night",
  resolve: () => ({
    bar: { bg: "#1a1b26", fg: "#a9b1d6" },
    accents: [
      "#7aa2f7",
      "#9ece6a",
      "#e0af68",
      "#bb9af7",
      "#f7768e",
      "#ff9e64",
      "#73daca",
      "#7dcfff",
    ],
    modules: {
      session: { fg: "#1a1b26", bg: "#7aa2f7" },
      activeWindow: { fg: "#1a1b26", bg: "#9ece6a" },
      inactiveWindow: { fg: "#a9b1d6", bg: "#24283b" },
      gitClean: { fg: "#1a1b26", bg: "#9ece6a" },
      gitDirty: { fg: "#1a1b26", bg: "#e0af68" },
      cpuLow: { fg: "#1a1b26", bg: "#9ece6a" },
      cpuMed: { fg: "#1a1b26", bg: "#e0af68" },
      cpuHigh: { fg: "#1a1b26", bg: "#f7768e" },
      batteryHigh: { fg: "#1a1b26", bg: "#9ece6a" },
      batteryMed: { fg: "#1a1b26", bg: "#e0af68" },
      batteryLow: { fg: "#1a1b26", bg: "#f7768e" },
      prefix: { fg: "#1a1b26", bg: "#f7768e" },
      prefixInactive: { fg: "#565f89", bg: "#24283b" },
    },
  }),
};
