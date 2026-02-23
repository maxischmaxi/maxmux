import type { PluginEvents } from "./types.ts";

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

  emitWaterfall<T>(event: string, initial: T): T {
    const handlers = this.handlers.get(event);
    if (!handlers) return initial;
    let result = initial;
    for (const handler of handlers) {
      result = (handler as (val: T) => T)(result) ?? result;
    }
    return result;
  }

  clear(): void {
    this.handlers.clear();
  }
}
