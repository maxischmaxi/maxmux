export class KeybindingRegistry {
  private bindings: Map<string, string> = new Map();

  set(key: string, commandId: string): void {
    this.bindings.set(key, commandId);
  }

  get(key: string): string | undefined {
    return this.bindings.get(key);
  }

  remove(key: string): void {
    this.bindings.delete(key);
  }

  loadFromConfig(keybindings: Record<string, string>): void {
    for (const [key, commandId] of Object.entries(keybindings)) {
      this.bindings.set(key, commandId);
    }
  }

  list(): Array<{ key: string; commandId: string }> {
    return [...this.bindings.entries()].map(([key, commandId]) => ({
      key,
      commandId,
    }));
  }

  has(key: string): boolean {
    return this.bindings.has(key);
  }
}
