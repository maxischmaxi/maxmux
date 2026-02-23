import type { MaxMuxConfig } from "../config/schema.ts";
import type { CommandRegistry } from "../core/command.ts";
import type { Session, Window, Pane } from "../core/session.ts";
import type { KeybindingRegistry } from "../input/keybindings.ts";
import type { SerializedSession } from "../persistence/store.ts";

export interface StatusBarItem {
  text: string;
  fg?: string;
  bg?: string;
  align?: "left" | "right";
  priority?: number;
}

export interface WindowTitleInfo {
  windowId: string;
  title: string;
  processes: Array<{ paneId: string; name: string }>;
}

export interface PluginEvents {
  "session:created": (session: Session) => void;
  "session:closed": (session: Session) => void;
  "window:created": (window: Window) => void;
  "window:closed": (window: Window) => void;
  "pane:created": (pane: Pane) => void;
  "pane:closed": (pane: Pane) => void;
  "render:statusbar": (items: StatusBarItem[]) => StatusBarItem[];
  "window:title": (info: WindowTitleInfo) => WindowTitleInfo;
  "session:saved": (data: { sessions: SerializedSession[] }) => void;
  "config:loaded": (config: MaxMuxConfig) => MaxMuxConfig;
}

export interface PluginContext {
  config: MaxMuxConfig;
  commands: CommandRegistry;
  keybindings: KeybindingRegistry;
  on: <E extends keyof PluginEvents>(
    event: E,
    handler: PluginEvents[E],
  ) => void;
}

export interface MaxMuxPlugin {
  name: string;
  version?: string;
  setup: (ctx: PluginContext) => void | Promise<void>;
}
