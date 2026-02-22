import {
  cpus,
  freemem,
  totalmem,
  hostname,
  userInfo,
  networkInterfaces,
} from "node:os";
import { execSync } from "node:child_process";
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
    this.collectSlow();
    this.notify();

    this.fastTimer = setInterval(() => {
      this.collectFast();
      this.notify();
    }, fastIntervalMs);

    this.slowTimer = setInterval(() => {
      this.collectSlow();
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
      this.collectGit(cwd);
      this.notify();
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

  private collectSlow(): void {
    this.cachedMetrics.battery = this.collectBattery();
    this.cachedMetrics.network = this.collectNetwork();
    if (this.currentCwd) {
      this.collectGit(this.currentCwd);
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

  private collectGit(cwd: string): void {
    try {
      const branch = execSync("git rev-parse --abbrev-ref HEAD", {
        cwd,
        timeout: 2000,
        encoding: "utf-8",
        stdio: ["pipe", "pipe", "pipe"],
      }).trim();

      if (!branch) {
        this.cachedMetrics.git = null;
        return;
      }

      const status = execSync("git status --porcelain", {
        cwd,
        timeout: 2000,
        encoding: "utf-8",
        stdio: ["pipe", "pipe", "pipe"],
      }).trim();

      let ahead = 0;
      let behind = 0;
      try {
        const revList = execSync(
          "git rev-list --left-right --count HEAD...@{upstream}",
          {
            cwd,
            timeout: 2000,
            encoding: "utf-8",
            stdio: ["pipe", "pipe", "pipe"],
          },
        ).trim();
        const parts = revList.split(/\s+/);
        ahead = parseInt(parts[0] || "0", 10);
        behind = parseInt(parts[1] || "0", 10);
      } catch {
        // No upstream
      }

      this.cachedMetrics.git = {
        branch,
        dirty: status.length > 0,
        ahead: isNaN(ahead) ? 0 : ahead,
        behind: isNaN(behind) ? 0 : behind,
      };
    } catch {
      this.cachedMetrics.git = null;
    }
  }
}
