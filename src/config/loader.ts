import { existsSync } from "node:fs";
import { resolve, join } from "node:path";
import { homedir } from "node:os";
import { ConfigSchema, type MaxMuxConfig } from "./schema.ts";
import { DEFAULT_CONFIG, DEFAULT_KEYBINDINGS } from "./defaults.ts";

const CONFIG_FILENAMES = ["maxmux.config.ts", "maxmux.config.js"];

function findConfigFile(): string | null {
  // Check CWD first
  for (const name of CONFIG_FILENAMES) {
    const path = resolve(process.cwd(), name);
    if (existsSync(path)) return path;
  }

  // Check ~/.config/maxmux/
  const configDir = join(homedir(), ".config", "maxmux");
  for (const name of CONFIG_FILENAMES) {
    const path = join(configDir, name);
    if (existsSync(path)) return path;
  }

  return null;
}

export async function loadConfig(): Promise<MaxMuxConfig> {
  const configPath = findConfigFile();

  if (!configPath) {
    return DEFAULT_CONFIG;
  }

  try {
    const mod = await import(configPath);
    const userConfig = mod.default || mod;

    // Merge user keybindings with defaults (user wins)
    const mergedKeybindings = {
      ...DEFAULT_KEYBINDINGS,
      ...(userConfig.keybindings || {}),
    };

    const merged = {
      ...userConfig,
      keybindings: mergedKeybindings,
    };

    return ConfigSchema.parse(merged);
  } catch (err) {
    console.error(`Failed to load config from ${configPath}:`, err);
    return DEFAULT_CONFIG;
  }
}
