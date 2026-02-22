import type { StatusBarThemeDef } from "./types.ts";

export const catppuccinMocha: StatusBarThemeDef = {
  name: "catppuccin-mocha",
  resolve: () => ({
    bar: { bg: "#1e1e2e", fg: "#cdd6f4" },
    accents: [
      "#89b4fa",
      "#a6e3a1",
      "#f9e2af",
      "#cba6f7",
      "#f38ba8",
      "#fab387",
      "#94e2d5",
      "#74c7ec",
    ],
    modules: {
      session: { fg: "#1e1e2e", bg: "#89b4fa" },
      activeWindow: { fg: "#1e1e2e", bg: "#a6e3a1" },
      inactiveWindow: { fg: "#bac2de", bg: "#313244" },
      gitClean: { fg: "#1e1e2e", bg: "#a6e3a1" },
      gitDirty: { fg: "#1e1e2e", bg: "#f9e2af" },
      cpuLow: { fg: "#1e1e2e", bg: "#a6e3a1" },
      cpuMed: { fg: "#1e1e2e", bg: "#f9e2af" },
      cpuHigh: { fg: "#1e1e2e", bg: "#f38ba8" },
      batteryHigh: { fg: "#1e1e2e", bg: "#a6e3a1" },
      batteryMed: { fg: "#1e1e2e", bg: "#f9e2af" },
      batteryLow: { fg: "#1e1e2e", bg: "#f38ba8" },
      prefix: { fg: "#1e1e2e", bg: "#f38ba8" },
      prefixInactive: { fg: "#585b70", bg: "#313244" },
    },
  }),
};
