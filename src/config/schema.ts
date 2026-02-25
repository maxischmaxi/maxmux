import { z } from "zod/v4";

export const COMMAND_IDS = [
  "window:create",
  "window:next",
  "window:previous",
  "window:close",
  "window:rename",
  "pane:split-horizontal",
  "pane:split-vertical",
  "pane:next",
  "pane:close",
  "pane:zoom",
  "pane:focus",
  "pane:focus-up",
  "pane:focus-down",
  "pane:focus-left",
  "pane:focus-right",
  "session:detach",
  "session:create",
  "session:list",
  "session:find",
  "session:rename",
  "server:kill",
  "command-palette",
  "keybindings:show",
  "copy-mode:enter",
  "notes:create",
  "notes:list",
] as const;

export const CommandIdSchema = z.enum(COMMAND_IDS);
export type CommandId = z.infer<typeof CommandIdSchema>;

export const KeybindingValueSchema = z.union([
  CommandIdSchema,
  z.object({
    command: CommandIdSchema,
    unless: z.array(z.string()).optional(),
  }),
]);
export type KeybindingValue = z.infer<typeof KeybindingValueSchema>;

export const STATUSBAR_MODULE_IDS = [
  "session",
  "windows",
  "datetime",
  "hostname",
  "user",
  "cwd",
  "git",
  "cpu",
  "ram",
  "battery",
  "network",
  "prefix",
  "pane-info",
] as const;

export const StatusBarModuleIdSchema = z.enum(STATUSBAR_MODULE_IDS);
export type StatusBarModuleId = z.infer<typeof StatusBarModuleIdSchema>;

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
  left: z
    .array(StatusBarModuleIdSchema)
    .default(["session", "windows"] as const),
  right: z
    .array(StatusBarModuleIdSchema)
    .default(["git", "cwd", "datetime"] as const),
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
  historyLimit: z.number().min(0).max(100_000).default(10_000),
  shell: z.string().default(process.env.SHELL || "/bin/bash"),
  newPaneCwd: z.string().default("inherit"),
  switchToNewWindow: z.boolean().default(true),
  automaticRename: z.boolean().default(true),
  automaticRenameInterval: z.number().default(2000),
  mouse: z.boolean().default(true),
  showPrefixHelp: z.boolean().default(true),
  theme: ThemeSchema.default(() => ({
    statusBar: { bg: "#1e1e2e", fg: "#cdd6f4", active: "#89b4fa" },
    border: {
      style: "rounded" as const,
      fg: "#585b70",
      activeFg: "#89b4fa",
    },
  })),
  keybindings: z.record(z.string(), KeybindingValueSchema).default(() => ({})),
  globalKeybindings: z
    .record(z.string(), KeybindingValueSchema)
    .default(() => ({})),
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
    left: ["session", "windows"] satisfies StatusBarModuleId[],
    right: ["git", "cwd", "datetime"] satisfies StatusBarModuleId[],
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
  debug: z.boolean().default(false),
});

export type MaxMuxConfig = z.infer<typeof ConfigSchema>;

// -- Explicit input interfaces with JSDoc for LSP hover documentation --

/** Farbschema der Status Bar. Alle Werte sind Hex-Farbcodes. */
export interface StatusBarThemeInput {
  /** Hintergrundfarbe der Status Bar (Hex).
   * @default "#1e1e2e" */
  bg?: string;
  /** Textfarbe der Status Bar (Hex).
   * @default "#cdd6f4" */
  fg?: string;
  /** Akzentfarbe fuer aktive Elemente wie das aktive Fenster-Tab (Hex).
   * @default "#89b4fa" */
  active?: string;
}

/** Konfiguration fuer Pane-Raender. */
export interface BorderThemeInput {
  /** Stil der Rahmenlinien zwischen Panes.
   * - `"rounded"` — Abgerundete Ecken (Unicode Box-Drawing)
   * - `"sharp"` — Eckige Ecken
   * - `"double"` — Doppelte Linien
   * - `"none"` — Keine sichtbaren Rahmen
   * @default "rounded" */
  style?: "rounded" | "sharp" | "double" | "none";
  /** Farbe der Rahmenlinien fuer inaktive Panes (Hex).
   * @default "#585b70" */
  fg?: string;
  /** Farbe der Rahmenlinien fuer das aktive Pane (Hex).
   * @default "#89b4fa" */
  activeFg?: string;
}

/** Visuelles Gesamtthema — Farben fuer Status Bar und Rahmen. */
export interface ThemeInput {
  /** Farben der Status Bar. */
  statusBar?: StatusBarThemeInput;
  /** Rahmenlinien zwischen Panes. */
  border?: BorderThemeInput;
}

/** Konfiguration der Separator-Zeichen zwischen Status-Bar-Modulen. */
export interface StatusBarSeparatorInput {
  /** Visueller Stil der Trennzeichen.
   * - `"powerline"` — Powerline-Pfeile (erfordert Nerd Font)
   * - `"rounded"` — Abgerundete Trennzeichen
   * - `"flat"` — Flache Bloecke ohne Uebergang
   * - `"arrow"` — Schmale Pfeil-Trennzeichen
   * - `"slant"` — Schraege Trennzeichen
   * @default "powerline" */
  style?: "powerline" | "rounded" | "flat" | "arrow" | "slant";
  /** Eigenes Zeichen fuer den linken Separator. Ueberschreibt den Style-Default. */
  left?: string;
  /** Eigenes Zeichen fuer den rechten Separator. Ueberschreibt den Style-Default. */
  right?: string;
}

/** Konfiguration eines einzelnen Status-Bar-Moduls. */
export interface StatusBarModuleConfigInput {
  /** Ob dieses Modul angezeigt wird.
   * @default true */
  enabled?: boolean;
  /** Eigene Textfarbe fuer dieses Modul (Hex). Ueberschreibt die Theme-Farbe. */
  fg?: string;
  /** Eigene Hintergrundfarbe fuer dieses Modul (Hex). Ueberschreibt die Theme-Farbe. */
  bg?: string;
  /** Weitere modulspezifische Optionen (z.B. Format-Strings). */
  [key: string]: unknown;
}

/** Konfiguration der Status Bar am unteren oder oberen Bildschirmrand. */
export interface StatusBarInput {
  /** Ob die Status Bar sichtbar ist.
   * @default true */
  enabled?: boolean;
  /** Position der Status Bar.
   * @default "bottom" */
  position?: "top" | "bottom";
  /** Farbthema-Preset fuer die Status Bar. Bei `"custom"` werden die Farben
   * aus `theme.statusBar` verwendet.
   * @default "catppuccin-mocha" */
  theme?:
    | "catppuccin-mocha"
    | "dracula"
    | "nord"
    | "tokyo-night"
    | "gruvbox"
    | "one-dark"
    | "solarized"
    | "custom";
  /** Trennzeichen zwischen den Status-Bar-Modulen. */
  separator?: StatusBarSeparatorInput;
  /** Nerd-Font-Icons in Status-Bar-Modulen anzeigen. Erfordert einen
   * Nerd Font als Terminal-Schrift.
   * @default true */
  icons?: boolean;
  /** Module auf der linken Seite der Status Bar, in Reihenfolge.
   * @default ["session", "windows"] */
  left?: StatusBarModuleId[];
  /** Module auf der rechten Seite der Status Bar, in Reihenfolge.
   * @default ["git", "cwd", "datetime"] */
  right?: StatusBarModuleId[];
  /** Individuelle Konfiguration pro Modul. Key = Modul-ID. */
  modules?: Record<string, StatusBarModuleConfigInput>;
  /** Aktualisierungsintervall der Status Bar in Millisekunden.
   * @default 1000 */
  refreshInterval?: number;
  /** Intervall in ms fuer das Abfragen von Systemmetriken (CPU, RAM, etc).
   * @default 5000 */
  metricsInterval?: number;
}

/** Session-Persistenz — automatisches Speichern und Wiederherstellen. */
export interface SessionsInput {
  /** Sessions periodisch automatisch speichern.
   * @default true */
  autoSave?: boolean;
  /** Intervall in ms zwischen Auto-Saves.
   * @default 30000 */
  autoSaveInterval?: number;
  /** Gespeicherte Sessions beim Serverstart automatisch wiederherstellen.
   * @default true */
  autoRestore?: boolean;
  /** Verzeichnis fuer Session-Dateien. Wird automatisch erstellt.
   * @default "~/.maxmux/sessions/" */
  savePath?: string;
}

/** Konfiguration der Session-Liste/Picker-Ansicht. */
export interface SessionListInput {
  /** Anzeigemodus der Session-Liste.
   * - `"sidebar"` — Feste Seitenleiste (bleibt sichtbar)
   * - `"overlay"` — Schwebendes Popup ueber dem Terminal
   * @default "sidebar" */
  mode?: "sidebar" | "overlay";
  /** Seite der Seitenleiste (nur bei `mode: "sidebar"`).
   * @default "left" */
  sidebarPosition?: "left" | "right";
  /** Breite der Seitenleiste in Spalten (nur bei `mode: "sidebar"`).
   * Min: 20, Max: 80.
   * @default 30 */
  sidebarWidth?: number;
}

/**
 * MaxMux-Konfiguration. Alle Felder sind optional — nicht angegebene
 * Felder verwenden die Standardwerte. Verwende {@link defineConfig} fuer
 * typsichere Konfiguration mit Validierung.
 *
 * @example
 * ```ts
 * // maxmux.config.ts
 * import { defineConfig } from "maxmux";
 *
 * export default defineConfig({
 *   prefixKey: "C-b",
 *   theme: {
 *     border: { style: "sharp" },
 *   },
 * });
 * ```
 */
export interface MaxMuxConfigInput {
  /** Prefix-Taste, die den Keybinding-Modus aktiviert.
   * Format: `"C-x"` fuer Ctrl+x. Nach dem Druecken dieser Taste wird
   * der naechste Tastendruck als Keybinding interpretiert.
   * @default "C-a" */
  prefixKey?: string;
  /** Timeout in ms nach dem Druecken der Prefix-Taste, bevor der
   * Prefix-Modus automatisch zurueckgesetzt wird.
   * `0` = kein Timeout (wartet unbegrenzt auf den naechsten Tastendruck).
   * @default 0 */
  prefixTimeout?: number;
  /** Maximale Anzahl an Zeilen im Scrollback-Buffer pro Pane.
   * Hoehere Werte verbrauchen mehr Speicher.
   * Min: 0, Max: 100.000.
   * @default 10000 */
  historyLimit?: number;
  /** Shell-Programm, das fuer neue Panes gestartet wird.
   * Verwendet standardmaessig die `$SHELL`-Umgebungsvariable.
   * @default $SHELL oder "/bin/bash" */
  shell?: string;
  /** Arbeitsverzeichnis fuer neue Panes.
   * - `"inherit"` — Verzeichnis des aktiven Panes uebernehmen
   * - Ein absoluter Pfad (z.B. `"/home/user/projects"`) — immer dieses Verzeichnis verwenden
   * @default "inherit" */
  newPaneCwd?: string;
  /** Automatisch zum neu erstellten Fenster wechseln.
   * @default true */
  switchToNewWindow?: boolean;
  /** Fenster automatisch nach dem laufenden Prozess umbenennen.
   * @default true */
  automaticRename?: boolean;
  /** Intervall in ms fuer die Prozessnamens-Pruefung bei automatischer Umbenennung.
   * @default 2000 */
  automaticRenameInterval?: number;
  /** Maus-Unterstuetzung aktivieren (Klick zum Fokussieren, Scrollen).
   * @default true */
  mouse?: boolean;
  /** Keybinding-Hilfe-Popup nach Druecken der Prefix-Taste anzeigen.
   * Zeigt eine kurze Uebersicht der verfuegbaren Tastenkuerzel.
   * @default true */
  showPrefixHelp?: boolean;
  /** Visuelles Theme — Farben fuer Rahmen und Status Bar. */
  theme?: ThemeInput;
  /** Keybindings im Prefix-Modus. Tasten, die nach der Prefix-Taste
   * gedrueckt werden. Werden mit den Standard-Keybindings gemerged
   * (eigene Eintraege ueberschreiben Defaults).
   *
   * Keys: einzelne Zeichen (`"c"`, `"%"`) oder Spezialtasten (`"Up"`, `"Down"`).
   * Values: Command-ID (`"window:create"`) oder Objekt mit `command` und `unless`.
   *
   * @example
   * ```ts
   * keybindings: {
   *   "c": "window:create",
   *   "|": "pane:split-horizontal",
   *   "-": "pane:split-vertical",
   * }
   * ``` */
  keybindings?: Record<string, KeybindingValue>;
  /** Globale Keybindings — feuern sofort ohne Prefix-Taste.
   *
   * **Achtung:** Globale Bindings fangen den Tastendruck ab, bevor er an
   * die Shell/das laufende Programm weitergeleitet wird. Verwende nur
   * Tastenkombinationen, die du nicht im Terminal brauchst.
   *
   * Keys: Ctrl-Kombinationen (`"C-h"`, `"C-j"`), Spezialtasten, oder Zeichen.
   * Values: Command-ID oder Objekt mit `command` und `unless`.
   *
   * @default {} (keine globalen Bindings)
   * @example
   * ```ts
   * globalKeybindings: {
   *   "C-h": "pane:focus-left",
   *   "C-l": "pane:focus-right",
   * }
   * ``` */
  globalKeybindings?: Record<string, KeybindingValue>;
  /** Session-Persistenz — automatisches Speichern und Wiederherstellen. */
  sessions?: SessionsInput;
  /** Status Bar am Bildschirmrand — Module, Position und Farbthema. */
  statusBar?: StatusBarInput;
  /** Session-Liste/Picker — Darstellungsmodus und Seitenleisten-Optionen. */
  sessionList?: SessionListInput;
  /** Plugins, die beim Start geladen werden. Jedes Plugin stellt eine
   * `setup(ctx)`-Funktion bereit, die Zugriff auf Commands, Keybindings
   * und Event-Hooks erhaelt. */
  plugins?: unknown[];
  /** Debug-Logging in Datei aktivieren. Wenn `true`, werden interne
   * Ereignisse (Server, Client, PTY) in `~/.maxmux/debug.log` protokolliert.
   * Nuetzlich zur Fehleranalyse. Im Normalbetrieb deaktiviert lassen.
   * @default false */
  debug?: boolean;
}

/**
 * Validiert und vervollstaendigt die MaxMux-Konfiguration.
 * Fehlende Felder werden mit Standardwerten aufgefuellt.
 * Wirft einen Fehler bei ungueltigen Werten.
 *
 * @example
 * ```ts
 * // maxmux.config.ts
 * import { defineConfig } from "maxmux";
 *
 * export default defineConfig({
 *   prefixKey: "C-b",
 *   mouse: true,
 *   theme: {
 *     border: { style: "sharp" },
 *   },
 * });
 * ```
 */
export function defineConfig(config: MaxMuxConfigInput): MaxMuxConfig {
  return ConfigSchema.parse(config);
}
