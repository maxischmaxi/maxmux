import { existsSync } from "node:fs";
import { resolve, join } from "node:path";
import { homedir } from "node:os";
import { z } from "zod/v4";
import { ConfigSchema, type MaxMuxConfig } from "./schema.ts";
import { DEFAULT_CONFIG, DEFAULT_KEYBINDINGS } from "./defaults.ts";

const CONFIG_FILENAMES = ["maxmux.config.ts", "maxmux.config.js"];

export class ConfigLoadError extends Error {
  constructor(
    public readonly configPath: string,
    public readonly formattedMessage: string,
    cause?: unknown,
  ) {
    super(formattedMessage);
    this.cause = cause;
  }
}

export function findConfigFile(): string | null {
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

function mergeAndValidate(
  userConfig: Record<string, unknown>,
  configPath: string,
): MaxMuxConfig {
  // Merge user keybindings with defaults (user wins)
  const mergedKeybindings = {
    ...DEFAULT_KEYBINDINGS,
    ...(userConfig.keybindings || {}),
  };

  const merged = {
    ...userConfig,
    keybindings: mergedKeybindings,
  };

  try {
    return ConfigSchema.parse(merged);
  } catch (err) {
    if (err instanceof z.ZodError) {
      const formatted = z.prettifyError(err);
      throw new ConfigLoadError(configPath, formatted, err);
    }
    throw new ConfigLoadError(
      configPath,
      `Config validation failed: ${err}`,
      err,
    );
  }
}

export async function loadConfigFromPath(
  configPath: string,
): Promise<MaxMuxConfig> {
  try {
    const mod = await import(configPath + "?t=" + Date.now());
    const userConfig = mod.default || mod;
    return mergeAndValidate(userConfig, configPath);
  } catch (err) {
    if (err instanceof ConfigLoadError) throw err;
    throw new ConfigLoadError(
      configPath,
      `Failed to load config: ${err instanceof Error ? err.message : String(err)}`,
      err,
    );
  }
}

export async function loadConfig(): Promise<MaxMuxConfig> {
  const configPath = findConfigFile();

  if (!configPath) {
    return DEFAULT_CONFIG;
  }

  return loadConfigFromPath(configPath);
}
