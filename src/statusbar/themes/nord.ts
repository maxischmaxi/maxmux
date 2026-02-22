import type { StatusBarThemeDef } from "./types.ts";

export const nord: StatusBarThemeDef = {
  name: "nord",
  resolve: () => ({
    bar: { bg: "#2e3440", fg: "#d8dee9" },
    accents: [
      "#88c0d0",
      "#a3be8c",
      "#ebcb8b",
      "#b48ead",
      "#bf616a",
      "#d08770",
      "#8fbcbb",
      "#81a1c1",
    ],
    modules: {
      session: { fg: "#2e3440", bg: "#88c0d0" },
      activeWindow: { fg: "#2e3440", bg: "#a3be8c" },
      inactiveWindow: { fg: "#d8dee9", bg: "#3b4252" },
      gitClean: { fg: "#2e3440", bg: "#a3be8c" },
      gitDirty: { fg: "#2e3440", bg: "#ebcb8b" },
      cpuLow: { fg: "#2e3440", bg: "#a3be8c" },
      cpuMed: { fg: "#2e3440", bg: "#ebcb8b" },
      cpuHigh: { fg: "#2e3440", bg: "#bf616a" },
      batteryHigh: { fg: "#2e3440", bg: "#a3be8c" },
      batteryMed: { fg: "#2e3440", bg: "#ebcb8b" },
      batteryLow: { fg: "#2e3440", bg: "#bf616a" },
      prefix: { fg: "#2e3440", bg: "#bf616a" },
      prefixInactive: { fg: "#4c566a", bg: "#3b4252" },
    },
  }),
};
