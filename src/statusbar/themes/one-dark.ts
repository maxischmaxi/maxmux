import type { StatusBarThemeDef } from "./types.ts";

export const oneDark: StatusBarThemeDef = {
  name: "one-dark",
  resolve: () => ({
    bar: { bg: "#282c34", fg: "#abb2bf" },
    accents: [
      "#61afef",
      "#98c379",
      "#e5c07b",
      "#c678dd",
      "#e06c75",
      "#d19a66",
      "#56b6c2",
      "#528bff",
    ],
    modules: {
      session: { fg: "#282c34", bg: "#61afef" },
      activeWindow: { fg: "#282c34", bg: "#98c379" },
      inactiveWindow: { fg: "#abb2bf", bg: "#3e4452" },
      gitClean: { fg: "#282c34", bg: "#98c379" },
      gitDirty: { fg: "#282c34", bg: "#e5c07b" },
      cpuLow: { fg: "#282c34", bg: "#98c379" },
      cpuMed: { fg: "#282c34", bg: "#e5c07b" },
      cpuHigh: { fg: "#282c34", bg: "#e06c75" },
      batteryHigh: { fg: "#282c34", bg: "#98c379" },
      batteryMed: { fg: "#282c34", bg: "#e5c07b" },
      batteryLow: { fg: "#282c34", bg: "#e06c75" },
      prefix: { fg: "#282c34", bg: "#e06c75" },
      prefixInactive: { fg: "#5c6370", bg: "#3e4452" },
    },
  }),
};
