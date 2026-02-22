import { BunPty } from "./bun-pty.ts";

export interface PtyHandle {
  id: string;
  pty: BunPty;
  pid: number;
}

export class PtyManager {
  private ptys: Map<string, PtyHandle> = new Map();

  spawn(
    id: string,
    shell: string,
    cwd: string,
    cols: number,
    rows: number,
    onData: (data: string) => void,
    onExit: (exitCode: number) => void,
  ): PtyHandle {
    const p = BunPty.spawn({
      shell,
      cwd,
      cols: Math.max(1, cols),
      rows: Math.max(1, rows),
      onData,
      onExit: (exitCode) => {
        this.ptys.delete(id);
        onExit(exitCode);
      },
    });

    const handle: PtyHandle = { id, pty: p, pid: p.pid };
    this.ptys.set(id, handle);
    return handle;
  }

  write(id: string, data: string): void {
    const handle = this.ptys.get(id);
    if (handle) {
      handle.pty.write(data);
    }
  }

  resize(id: string, cols: number, rows: number): void {
    const handle = this.ptys.get(id);
    if (handle) {
      handle.pty.resize(Math.max(1, cols), Math.max(1, rows));
    }
  }

  resizeAll(cols: number, rows: number): void {
    for (const handle of this.ptys.values()) {
      handle.pty.resize(Math.max(1, cols), Math.max(1, rows));
    }
  }

  kill(id: string): void {
    const handle = this.ptys.get(id);
    if (handle) {
      handle.pty.destroy();
      this.ptys.delete(id);
    }
  }

  killAll(): void {
    for (const handle of this.ptys.values()) {
      handle.pty.destroy();
    }
    this.ptys.clear();
  }

  get(id: string): PtyHandle | undefined {
    return this.ptys.get(id);
  }

  getPid(id: string): number | undefined {
    return this.ptys.get(id)?.pid;
  }
}
