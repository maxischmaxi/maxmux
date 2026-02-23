import type { KeybindingValue } from "../config/schema.ts";

export interface KeyBinding {
  commandId: string;
  unless?: string[];
}

export class KeybindingRegistry {
  private bindings: Map<string, KeyBinding> = new Map();

  set(key: string, binding: KeyBinding): void {
    this.bindings.set(key, binding);
  }

  get(key: string): string | undefined {
    return this.bindings.get(key)?.commandId;
  }

  getBinding(key: string): KeyBinding | undefined {
    return this.bindings.get(key);
  }

  resolve(key: string, currentProcess?: string): string | undefined {
    const binding = this.bindings.get(key);
    if (!binding) return undefined;

    if (binding.unless && currentProcess) {
      if (binding.unless.includes(currentProcess)) {
        return undefined;
      }
    }

    return binding.commandId;
  }

  remove(key: string): void {
    this.bindings.delete(key);
  }

  loadFromConfig(keybindings: Record<string, KeybindingValue>): void {
    for (const [key, value] of Object.entries(keybindings)) {
      if (typeof value === "string") {
        this.bindings.set(key, { commandId: value });
      } else {
        this.bindings.set(key, {
          commandId: value.command,
          unless: value.unless,
        });
      }
    }
  }

  list(): Array<{ key: string; commandId: string; unless?: string[] }> {
    return [...this.bindings.entries()].map(([key, binding]) => ({
      key,
      commandId: binding.commandId,
      unless: binding.unless,
    }));
  }

  has(key: string): boolean {
    return this.bindings.has(key);
  }

  clear(): void {
    this.bindings.clear();
  }
}
