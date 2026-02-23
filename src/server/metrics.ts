import {
  cpus,
  freemem,
  totalmem,
  hostname,
  userInfo,
  networkInterfaces,
} from "node:os";
import { existsSync, readFileSync } from "node:fs";
import type { SystemMetrics } from "../statusbar/types.ts";

export class MetricsCollector {
  private prevCpuTimes: { idle: number; total: number } | null = null;
  private cachedMetrics: SystemMetrics;
  private fastTimer: ReturnType<typeof setInterval> | null = null;
  private slowTimer: ReturnType<typeof setInterval> | null = null;
  private onUpdate: ((metrics: SystemMetrics) => void) | null = null;
  private currentCwd = "";

  constructor() {
    this.cachedMetrics = {
      hostname: hostname(),
      username: userInfo().username,
      cpu: { usage: 0, count: cpus().length },
      memory: this.collectMemory(),
      battery: null,
      network: null,
      git: null,
      cwd: "",
      paneTitle: "",
      paneCount: 0,
    };
  }

  start(
    fastIntervalMs: number,
    slowIntervalMs: number,
    onUpdate: (metrics: SystemMetrics) => void,
  ): void {
    this.onUpdate = onUpdate;

    // Initial collection
    this.collectFast();
    this.notify();
    this.collectSlow().then(() => this.notify());

    // Single fast timer for CPU/memory
    this.fastTimer = setInterval(() => {
      this.collectFast();
      this.notify();
    }, fastIntervalMs);

    // Slow timer for git etc. — only runs when there are changes to detect
    this.slowTimer = setInterval(async () => {
      await this.collectSlow();
      this.notify();
    }, slowIntervalMs);
  }

  stop(): void {
    if (this.fastTimer) clearInterval(this.fastTimer);
    if (this.slowTimer) clearInterval(this.slowTimer);
    this.fastTimer = null;
    this.slowTimer = null;
  }

  setCwd(cwd: string): void {
    if (cwd !== this.currentCwd) {
      this.currentCwd = cwd;
      this.cachedMetrics.cwd = cwd;
      this.collectGit(cwd).then(() => this.notify());
    }
  }

  setPaneInfo(paneTitle: string, paneCount: number): void {
    this.cachedMetrics.paneTitle = paneTitle;
    this.cachedMetrics.paneCount = paneCount;
  }

  getMetrics(): SystemMetrics {
    return { ...this.cachedMetrics };
  }

  private notify(): void {
    this.onUpdate?.({ ...this.cachedMetrics });
  }

  private collectFast(): void {
    this.cachedMetrics.cpu = this.collectCpu();
    this.cachedMetrics.memory = this.collectMemory();
  }

  private async collectSlow(): Promise<void> {
    this.cachedMetrics.battery = this.collectBattery();
    this.cachedMetrics.network = this.collectNetwork();
    if (this.currentCwd) {
      await this.collectGit(this.currentCwd);
    }
  }

  private collectCpu(): { usage: number; count: number } {
    const cores = cpus();
    let idle = 0;
    let total = 0;
    for (const core of cores) {
      idle += core.times.idle;
      total +=
        core.times.user +
        core.times.nice +
        core.times.sys +
        core.times.idle +
        core.times.irq;
    }

    let usage = 0;
    if (this.prevCpuTimes) {
      const idleDelta = idle - this.prevCpuTimes.idle;
      const totalDelta = total - this.prevCpuTimes.total;
      if (totalDelta > 0) {
        usage = ((totalDelta - idleDelta) / totalDelta) * 100;
      }
    }
    this.prevCpuTimes = { idle, total };

    return { usage, count: cores.length };
  }

  private collectMemory(): {
    totalMB: number;
    usedMB: number;
    percentage: number;
  } {
    const totalBytes = totalmem();
    const freeBytes = freemem();
    const usedBytes = totalBytes - freeBytes;
    const totalMB = totalBytes / (1024 * 1024);
    const usedMB = usedBytes / (1024 * 1024);
    return {
      totalMB: Math.round(totalMB),
      usedMB: Math.round(usedMB),
      percentage: (usedBytes / totalBytes) * 100,
    };
  }

  private collectBattery(): {
    present: boolean;
    level: number;
    charging: boolean;
  } | null {
    try {
      const batPath = "/sys/class/power_supply/BAT0";
      if (!existsSync(batPath)) return null;

      const capacity = parseInt(
        readFileSync(`${batPath}/capacity`, "utf-8").trim(),
        10,
      );
      const status = readFileSync(`${batPath}/status`, "utf-8").trim();
      return {
        present: true,
        level: isNaN(capacity) ? 0 : capacity,
        charging: status === "Charging" || status === "Full",
      };
    } catch {
      return null;
    }
  }

  private collectNetwork(): { interface: string; ip: string } | null {
    try {
      const ifaces = networkInterfaces();
      for (const [name, addrs] of Object.entries(ifaces)) {
        if (name === "lo" || !addrs) continue;
        for (const addr of addrs) {
          if (addr.family === "IPv4" && !addr.internal) {
            return { interface: name, ip: addr.address };
          }
        }
      }
      return null;
    } catch {
      return null;
    }
  }

  private async runGitCommand(
    args: string[],
    cwd: string,
    timeoutMs = 2000,
  ): Promise<string | null> {
    const proc = Bun.spawn(["git", ...args], {
      cwd,
      stdout: "pipe",
      stderr: "pipe",
    });
    const timer = setTimeout(() => proc.kill(), timeoutMs);
    try {
      const output = await proc.stdout.text();
      const exitCode = await proc.exited;
      if (exitCode !== 0) return null;
      return output.trim();
    } catch {
      return null;
    } finally {
      clearTimeout(timer);
    }
  }

  private async collectGit(cwd: string): Promise<void> {
    try {
      const branch = await this.runGitCommand(
        ["rev-parse", "--abbrev-ref", "HEAD"],
        cwd,
      );

      if (!branch) {
        this.cachedMetrics.git = null;
        return;
      }

      const status = await this.runGitCommand(["status", "--porcelain"], cwd);

      let ahead = 0;
      let behind = 0;
      const revList = await this.runGitCommand(
        ["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        cwd,
      );
      if (revList) {
        const parts = revList.split(/\s+/);
        ahead = parseInt(parts[0] || "0", 10);
        behind = parseInt(parts[1] || "0", 10);
      }

      this.cachedMetrics.git = {
        branch,
        dirty: (status?.length ?? 0) > 0,
        ahead: isNaN(ahead) ? 0 : ahead,
        behind: isNaN(behind) ? 0 : behind,
      };
    } catch {
      this.cachedMetrics.git = null;
    }
  }
}
