import { KeybindingRegistry } from "./keybindings.ts";
import { DEFAULT_KEYBINDINGS } from "../config/defaults.ts";

export function createDefaultKeybindings(): KeybindingRegistry {
  const registry = new KeybindingRegistry();
  registry.loadFromConfig(DEFAULT_KEYBINDINGS);
  return registry;
}
