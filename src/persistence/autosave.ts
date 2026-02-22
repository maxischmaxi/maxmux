import type { SessionManager } from "../core/session.ts";
import { saveSession } from "./store.ts";

export class AutoSaver {
  private timer: ReturnType<typeof setInterval> | null = null;
  private sessions: SessionManager;
  private interval: number;
  private savePath: string;

  constructor(sessions: SessionManager, interval: number, savePath: string) {
    this.sessions = sessions;
    this.interval = interval;
    this.savePath = savePath;
  }

  start(): void {
    if (this.timer) return;
    this.timer = setInterval(() => {
      this.saveNow();
    }, this.interval);
  }

  stop(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  saveNow(): void {
    try {
      saveSession(this.sessions, this.savePath);
    } catch (err) {
      console.error("Auto-save failed:", err);
    }
  }
}
