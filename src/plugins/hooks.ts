import type { PluginEvents, StatusBarItem } from "./types.ts";

type EventHandler = (...args: any[]) => any;

export class HookRegistry {
  private handlers: Map<string, EventHandler[]> = new Map();

  on<E extends keyof PluginEvents>(event: E, handler: PluginEvents[E]): void {
    const existing = this.handlers.get(event) || [];
    existing.push(handler as EventHandler);
    this.handlers.set(event, existing);
  }

  emit<E extends keyof PluginEvents>(
    event: E,
    ...args: Parameters<PluginEvents[E]>
  ): void {
    const handlers = this.handlers.get(event);
    if (!handlers) return;
    for (const handler of handlers) {
      handler(...args);
    }
  }

  emitWaterfall(
    event: "render:statusbar",
    items: StatusBarItem[],
  ): StatusBarItem[] {
    const handlers = this.handlers.get(event);
    if (!handlers) return items;
    let result = items;
    for (const handler of handlers) {
      result = handler(result) || result;
    }
    return result;
  }

  clear(): void {
    this.handlers.clear();
  }
}
