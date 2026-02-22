export interface CommandContext {
  sessionId: string;
  windowId?: string;
  paneId?: string;
  args?: Record<string, unknown>;
}

export interface Command {
  id: string;
  description: string;
  execute: (ctx: CommandContext) => void | Promise<void>;
}

export class CommandRegistry {
  private commands: Map<string, Command> = new Map();

  register(command: Command): void {
    this.commands.set(command.id, command);
  }

  unregister(id: string): void {
    this.commands.delete(id);
  }

  get(id: string): Command | undefined {
    return this.commands.get(id);
  }

  execute(id: string, ctx: CommandContext): void | Promise<void> {
    const command = this.commands.get(id);
    if (!command) {
      throw new Error(`Unknown command: ${id}`);
    }
    return command.execute(ctx);
  }

  list(): Command[] {
    return [...this.commands.values()];
  }

  has(id: string): boolean {
    return this.commands.has(id);
  }
}
