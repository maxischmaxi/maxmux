import { z } from "zod/v4";

export const StatusBarThemeSchema = z.object({
  bg: z.string().default("#1e1e2e"),
  fg: z.string().default("#cdd6f4"),
  active: z.string().default("#89b4fa"),
});

export const BorderThemeSchema = z.object({
  style: z.enum(["rounded", "sharp", "double", "none"]).default("rounded"),
  fg: z.string().default("#585b70"),
  activeFg: z.string().default("#89b4fa"),
});

export const ThemeSchema = z.object({
  statusBar: StatusBarThemeSchema.default(() => ({
    bg: "#1e1e2e",
    fg: "#cdd6f4",
    active: "#89b4fa",
  })),
  border: BorderThemeSchema.default(() => ({
    style: "rounded" as const,
    fg: "#585b70",
    activeFg: "#89b4fa",
  })),
});

export const StatusBarSeparatorSchema = z.object({
  style: z
    .enum(["powerline", "rounded", "flat", "arrow", "slant"])
    .default("powerline"),
  left: z.string().optional(),
  right: z.string().optional(),
});

export const StatusBarModuleConfigSchema = z
  .object({
    enabled: z.boolean().default(true),
    fg: z.string().optional(),
    bg: z.string().optional(),
  })
  .passthrough();

export const StatusBarConfigSchema = z.object({
  enabled: z.boolean().default(true),
  position: z.enum(["top", "bottom"]).default("bottom"),
  theme: z
    .enum([
      "catppuccin-mocha",
      "dracula",
      "nord",
      "tokyo-night",
      "gruvbox",
      "one-dark",
      "solarized",
      "custom",
    ])
    .default("catppuccin-mocha"),
  separator: StatusBarSeparatorSchema.default(() => ({
    style: "powerline" as const,
  })),
  icons: z.boolean().default(true),
  left: z.array(z.string()).default(() => ["session", "windows"]),
  right: z.array(z.string()).default(() => ["git", "cwd", "datetime"]),
  modules: z
    .record(z.string(), StatusBarModuleConfigSchema)
    .default(() => ({})),
  refreshInterval: z.number().default(1000),
  metricsInterval: z.number().default(5000),
});

export type StatusBarConfig = z.infer<typeof StatusBarConfigSchema>;

export const SessionsConfigSchema = z.object({
  autoSave: z.boolean().default(true),
  autoSaveInterval: z.number().default(30_000),
  autoRestore: z.boolean().default(true),
  savePath: z.string().default("~/.maxmux/sessions/"),
});

export const SessionListConfigSchema = z.object({
  mode: z.enum(["sidebar", "overlay"]).default("sidebar"),
  sidebarPosition: z.enum(["left", "right"]).default("left"),
  sidebarWidth: z.number().min(20).max(80).default(30),
});

export type SessionListConfig = z.infer<typeof SessionListConfigSchema>;

export const ConfigSchema = z.object({
  prefixKey: z.string().default("C-a"),
  prefixTimeout: z.number().default(0),
  shell: z.string().default(process.env.SHELL || "/bin/bash"),
  switchToNewWindow: z.boolean().default(true),
  automaticRename: z.boolean().default(true),
  automaticRenameInterval: z.number().default(500),
  theme: ThemeSchema.default(() => ({
    statusBar: { bg: "#1e1e2e", fg: "#cdd6f4", active: "#89b4fa" },
    border: {
      style: "rounded" as const,
      fg: "#585b70",
      activeFg: "#89b4fa",
    },
  })),
  keybindings: z.record(z.string(), z.string()).default(() => ({})),
  globalKeybindings: z.record(z.string(), z.string()).default(() => ({})),
  sessions: SessionsConfigSchema.default(() => ({
    autoSave: true,
    autoSaveInterval: 30_000,
    autoRestore: true,
    savePath: "~/.maxmux/sessions/",
  })),
  statusBar: StatusBarConfigSchema.default(() => ({
    enabled: true,
    position: "bottom" as const,
    theme: "catppuccin-mocha" as const,
    separator: { style: "powerline" as const },
    icons: true,
    left: ["session", "windows"],
    right: ["git", "cwd", "datetime"],
    modules: {},
    refreshInterval: 1000,
    metricsInterval: 5000,
  })),
  sessionList: SessionListConfigSchema.default(() => ({
    mode: "sidebar" as const,
    sidebarPosition: "left" as const,
    sidebarWidth: 30,
  })),
  plugins: z.array(z.any()).default(() => []),
});

export type MaxMuxConfig = z.infer<typeof ConfigSchema>;
export type MaxMuxConfigInput = z.input<typeof ConfigSchema>;

export function defineConfig(config: MaxMuxConfigInput): MaxMuxConfig {
  return ConfigSchema.parse(config);
}
