import type { StatusBarThemeDef } from "./types.ts";

export const dracula: StatusBarThemeDef = {
  name: "dracula",
  resolve: () => ({
    bar: { bg: "#282a36", fg: "#f8f8f2" },
    accents: [
      "#bd93f9",
      "#50fa7b",
      "#f1fa8c",
      "#ff79c6",
      "#ff5555",
      "#ffb86c",
      "#8be9fd",
      "#6272a4",
    ],
    modules: {
      session: { fg: "#282a36", bg: "#bd93f9" },
      activeWindow: { fg: "#282a36", bg: "#50fa7b" },
      inactiveWindow: { fg: "#f8f8f2", bg: "#44475a" },
      gitClean: { fg: "#282a36", bg: "#50fa7b" },
      gitDirty: { fg: "#282a36", bg: "#f1fa8c" },
      cpuLow: { fg: "#282a36", bg: "#50fa7b" },
      cpuMed: { fg: "#282a36", bg: "#f1fa8c" },
      cpuHigh: { fg: "#282a36", bg: "#ff5555" },
      batteryHigh: { fg: "#282a36", bg: "#50fa7b" },
      batteryMed: { fg: "#282a36", bg: "#f1fa8c" },
      batteryLow: { fg: "#282a36", bg: "#ff5555" },
      prefix: { fg: "#282a36", bg: "#ff79c6" },
      prefixInactive: { fg: "#6272a4", bg: "#44475a" },
    },
  }),
};
