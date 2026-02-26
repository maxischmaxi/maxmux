import { readlink } from "node:fs/promises";
import type { Pane } from "../core/session.ts";

export interface ProcessInfo {
  paneId: string;
  name: string;
}

type GetPanesFn = () => Array<{ paneId: string; pid: number; command: string }>;
type OnTitleChangeFn = (paneId: string, processName: string) => void;
type OnCwdChangeFn = (paneId: string, cwd: string) => void;

// Safety timeout: if a tracking tick takes longer than this, force-reset the
// running flag so the tracker doesn't stall permanently.  This guards against
// /proc reads that never resolve (zombie processes, kernel edge cases, etc.).
const TICK_TIMEOUT_MS = 10_000;

export class ProcessTracker {
  private interval: ReturnType<typeof setInterval> | null = null;
  private lastProcess: Map<string, string> = new Map();
  private lastCwd: Map<string, string> = new Map();
  private onCwdChange: OnCwdChangeFn | null = null;
  private running = false;

  start(
    intervalMs: number,
    getPanes: GetPanesFn,
    onTitleChange: OnTitleChangeFn,
    onCwdChange?: OnCwdChangeFn,
  ): void {
    if (onCwdChange) this.onCwdChange = onCwdChange;

    this.interval = setInterval(async () => {
      if (this.running) return;
      this.running = true;
      const safetyTimer = setTimeout(() => {
        this.running = false;
      }, TICK_TIMEOUT_MS);
      try {
        const panes = getPanes();
        for (const { paneId, pid, command } of panes) {
          const name = await this.getForegroundProcess(pid, command);
          const prev = this.lastProcess.get(paneId);
          if (name !== prev) {
            this.lastProcess.set(paneId, name);
            onTitleChange(paneId, name);
          }

          // Track CWD changes via /proc
          if (this.onCwdChange) {
            const cwd = await this.readProcCwd(pid);
            if (cwd) {
              const prevCwd = this.lastCwd.get(paneId);
              if (cwd !== prevCwd) {
                this.lastCwd.set(paneId, cwd);
                this.onCwdChange(paneId, cwd);
              }
            }
          }
        }
      } finally {
        clearTimeout(safetyTimer);
        this.running = false;
      }
    }, intervalMs);
  }

  stop(): void {
    if (this.interval) {
      clearInterval(this.interval);
      this.interval = null;
    }
  }

  getProcessName(paneId: string): string | undefined {
    return this.lastProcess.get(paneId);
  }

  removePanes(ids: string[]): void {
    for (const id of ids) {
      this.lastProcess.delete(id);
      this.lastCwd.delete(id);
    }
  }

  private async getForegroundProcess(
    pid: number,
    fallbackCommand: string,
  ): Promise<string> {
    try {
      // Read /proc/{pid}/stat to get tpgid (field index 7, 0-based)
      const statFile = Bun.file(`/proc/${pid}/stat`);
      const stat = await statFile.text();

      // Fields in /proc/pid/stat are space-separated, but field 2 (comm) is in parens
      // and may contain spaces. Find closing paren, then split the rest.
      const closeParen = stat.lastIndexOf(")");
      if (closeParen === -1) return this.extractName(fallbackCommand);

      const rest = stat.slice(closeParen + 2); // skip ") "
      const fields = rest.split(" ");
      // After (comm), fields are: state(0) ppid(1) pgrp(2) session(3) tty_nr(4) tpgid(5) ...
      const tpgid = parseInt(fields[5]!, 10);

      if (isNaN(tpgid) || tpgid <= 0) return this.extractName(fallbackCommand);

      // Read the comm of the foreground process group leader
      const commFile = Bun.file(`/proc/${tpgid}/comm`);
      const comm = await commFile.text();
      return comm.trim() || this.extractName(fallbackCommand);
    } catch {
      return this.extractName(fallbackCommand);
    }
  }

  private extractName(command: string): string {
    // Extract basename from shell path like /usr/bin/zsh → zsh
    const parts = command.split("/");
    return parts[parts.length - 1] || command;
  }

  private async readProcCwd(pid: number): Promise<string | null> {
    try {
      return await readlink(`/proc/${pid}/cwd`);
    } catch {
      return null;
    }
  }
}
