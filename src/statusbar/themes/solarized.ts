import type { StatusBarThemeDef } from "./types.ts";

export const solarized: StatusBarThemeDef = {
  name: "solarized",
  resolve: () => ({
    bar: { bg: "#002b36", fg: "#839496" },
    accents: [
      "#268bd2",
      "#859900",
      "#b58900",
      "#6c71c4",
      "#dc322f",
      "#cb4b16",
      "#2aa198",
      "#93a1a1",
    ],
    modules: {
      session: { fg: "#002b36", bg: "#268bd2" },
      activeWindow: { fg: "#002b36", bg: "#859900" },
      inactiveWindow: { fg: "#839496", bg: "#073642" },
      gitClean: { fg: "#002b36", bg: "#859900" },
      gitDirty: { fg: "#002b36", bg: "#b58900" },
      cpuLow: { fg: "#002b36", bg: "#859900" },
      cpuMed: { fg: "#002b36", bg: "#b58900" },
      cpuHigh: { fg: "#002b36", bg: "#dc322f" },
      batteryHigh: { fg: "#002b36", bg: "#859900" },
      batteryMed: { fg: "#002b36", bg: "#b58900" },
      batteryLow: { fg: "#002b36", bg: "#dc322f" },
      prefix: { fg: "#002b36", bg: "#dc322f" },
      prefixInactive: { fg: "#586e75", bg: "#073642" },
    },
  }),
};
