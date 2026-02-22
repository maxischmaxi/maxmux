import type { MaxMuxConfig } from "../config/schema.ts";
import type { CommandRegistry } from "../core/command.ts";
import type { KeybindingRegistry } from "../input/keybindings.ts";
import type { MaxMuxPlugin, PluginContext } from "./types.ts";
import { HookRegistry } from "./hooks.ts";

export async function loadPlugins(
  config: MaxMuxConfig,
  commands: CommandRegistry,
  keybindings: KeybindingRegistry,
  hooks: HookRegistry,
): Promise<void> {
  for (const plugin of config.plugins) {
    if (!plugin || typeof plugin !== "object" || !plugin.name) {
      continue;
    }

    const p = plugin as MaxMuxPlugin;
    const ctx: PluginContext = {
      config,
      commands,
      keybindings,
      on: (event, handler) => hooks.on(event, handler),
    };

    try {
      await p.setup(ctx);
    } catch (err) {
      console.error(`Plugin '${p.name}' failed to load:`, err);
    }
  }
}
