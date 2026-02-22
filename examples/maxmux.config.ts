import { defineConfig } from "../src/config/schema.ts";

export default defineConfig({
  // Prefix Key (like tmux's Ctrl+b, but we use Ctrl+a)
  prefixKey: "C-a",

  // Timeout for prefix mode (ms). 0 = no timeout, stays active until key/Escape
  prefixTimeout: 0,

  // Default shell
  shell: process.env.SHELL || "/bin/bash",

  // Theme (Catppuccin Mocha inspired)
  theme: {
    statusBar: {
      bg: "#1e1e2e",
      fg: "#cdd6f4",
      active: "#89b4fa",
    },
    border: {
      style: "rounded",
      fg: "#585b70",
      activeFg: "#89b4fa",
    },
  },

  // Keybindings (prefix + key)
  // All commands can be bound to any key. Available commands:
  //   window:create, window:next, window:previous, window:rename, window:close
  //   pane:split-horizontal, pane:split-vertical, pane:next, pane:close, pane:zoom
  //   pane:focus-up, pane:focus-down, pane:focus-left, pane:focus-right
  //   session:list, session:find, session:rename, session:detach
  //   server:kill, command-palette, keybindings:show
  keybindings: {
    c: "window:create",
    n: "window:next",
    p: "window:previous",
    ",": "window:rename",
    "&": "window:close",
    "%": "pane:split-horizontal",
    '"': "pane:split-vertical",
    o: "pane:next",
    x: "pane:close",
    z: "pane:zoom",
    // tmux-style: prefix + Arrow
    Up: "pane:focus-up",
    Down: "pane:focus-down",
    Left: "pane:focus-left",
    Right: "pane:focus-right",
    // Vim-style alternative: prefix + hjkl (uncomment to use)
    // h: "pane:focus-left",
    // j: "pane:focus-down",
    // k: "pane:focus-up",
    // l: "pane:focus-right",
    s: "session:list",
    f: "session:find",
    $: "session:rename",
    d: "session:detach",
    Q: "server:kill",
    ":": "command-palette",
    "?": "keybindings:show",
  },

  // Global keybindings (no prefix required, fire immediately)
  // Supports Ctrl combos: "C-h", "C-j", etc.
  // WARNING: Global bindings override normal terminal input!
  globalKeybindings: {
    // Example: Vim-style pane navigation without prefix
    // "C-h": "pane:focus-left",
    // "C-j": "pane:focus-down",
    // "C-k": "pane:focus-up",
    // "C-l": "pane:focus-right",
  },

  // StatusBar configuration
  statusBar: {
    // Theme: catppuccin-mocha | dracula | nord | tokyo-night | gruvbox | one-dark | solarized | custom
    theme: "catppuccin-mocha",

    // Position: top | bottom
    position: "bottom",

    // Separator style: powerline | rounded | flat | arrow | slant
    separator: { style: "powerline" },

    // Nerd Font icons
    icons: true,

    // Modules on the left side
    left: ["session", "windows"],

    // Modules on the right side
    right: ["git", "cwd", "datetime"],

    // Per-module configuration
    modules: {
      windows: { numbering: "index", style: "default" },
      datetime: { format: "HH:mm" },
    },

    // Refresh interval for clock updates (ms)
    refreshInterval: 1000,

    // How often the server pushes system metrics (ms)
    metricsInterval: 5000,
  },

  // Session persistence
  sessions: {
    autoSave: true,
    autoSaveInterval: 30_000,
    autoRestore: true,
    savePath: "~/.maxmux/sessions/",
  },

  // Plugins
  plugins: [
    // Example: gitPlugin()
  ],
});
