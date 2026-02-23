import { BunPty } from "./bun-pty.ts";

export interface PtyHandle {
  id: string;
  pty: BunPty;
  pid: number;
}

export class PtyManager {
  private ptys: Map<string, PtyHandle> = new Map();
  private ptySizes: Map<string, { cols: number; rows: number }> = new Map();

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
        this.ptySizes.delete(id);
        onExit(exitCode);
      },
    });

    const handle: PtyHandle = { id, pty: p, pid: p.pid };
    this.ptys.set(id, handle);
    this.ptySizes.set(id, { cols: Math.max(1, cols), rows: Math.max(1, rows) });
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
    if (!handle) return;
    const safeCols = Math.max(1, cols);
    const safeRows = Math.max(1, rows);
    // Skip resize if dimensions are unchanged to avoid unnecessary SIGWINCH
    const current = this.ptySizes.get(id);
    if (current && current.cols === safeCols && current.rows === safeRows)
      return;
    this.ptySizes.set(id, { cols: safeCols, rows: safeRows });
    handle.pty.resize(safeCols, safeRows);
  }

  /** Resize bypassing the dedup cache — always sends SIGWINCH. */
  forceResize(id: string, cols: number, rows: number): void {
    const handle = this.ptys.get(id);
    if (!handle) return;
    const safeCols = Math.max(1, cols);
    const safeRows = Math.max(1, rows);
    const current = this.ptySizes.get(id);
    if (current && current.cols === safeCols && current.rows === safeRows) {
      // Kernel sends no SIGWINCH when size is unchanged.
      // Resize to a different size first to force SIGWINCH delivery.
      // The second resize is delayed so the kernel doesn't coalesce
      // the two SIGWINCHs into one (same signal pending = dropped).
      const tempCols = safeCols > 1 ? safeCols - 1 : safeCols + 1;
      handle.pty.resize(tempCols, safeRows);
      setTimeout(() => {
        if (this.ptys.has(id)) {
          handle.pty.resize(safeCols, safeRows);
        }
      }, 16);
    } else {
      handle.pty.resize(safeCols, safeRows);
    }
    this.ptySizes.set(id, { cols: safeCols, rows: safeRows });
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
      this.ptySizes.delete(id);
    }
  }

  killAll(): void {
    for (const handle of this.ptys.values()) {
      handle.pty.destroy();
    }
    this.ptys.clear();
    this.ptySizes.clear();
  }

  get(id: string): PtyHandle | undefined {
    return this.ptys.get(id);
  }

  getPid(id: string): number | undefined {
    return this.ptys.get(id)?.pid;
  }
}
