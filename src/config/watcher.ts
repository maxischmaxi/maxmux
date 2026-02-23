import { watch, type FSWatcher } from "node:fs";
import { dirname, basename } from "node:path";
import { loadConfigFromPath } from "./loader.ts";
import type { MaxMuxConfig } from "./schema.ts";

export class ConfigWatcher {
  private watcher: FSWatcher | null = null;
  private debounceTimer: ReturnType<typeof setTimeout> | null = null;
  private restartTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private configPath: string | null,
    private onReload: (config: MaxMuxConfig) => void,
    private onError: (error: string) => void,
    private debounceMs = 300,
  ) {}

  start(): void {
    if (!this.configPath) return;

    const dir = dirname(this.configPath);
    const filename = basename(this.configPath);

    try {
      this.watcher = watch(dir, (eventType, changedFile) => {
        if (changedFile !== filename) return;
        this.scheduleReload();
      });

      this.watcher.on("error", () => {
        this.watcher?.close();
        this.watcher = null;
        // Restart after delay (directory may have been briefly unavailable)
        this.restartTimer = setTimeout(() => this.start(), 1000);
      });
    } catch {
      // Directory doesn't exist or can't be watched — silently ignore
    }
  }

  stop(): void {
    if (this.watcher) {
      this.watcher.close();
      this.watcher = null;
    }
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
      this.debounceTimer = null;
    }
    if (this.restartTimer) {
      clearTimeout(this.restartTimer);
      this.restartTimer = null;
    }
  }

  private scheduleReload(): void {
    if (this.debounceTimer) clearTimeout(this.debounceTimer);
    this.debounceTimer = setTimeout(() => {
      this.debounceTimer = null;
      this.reload();
    }, this.debounceMs);
  }

  private async reload(): Promise<void> {
    if (!this.configPath) return;

    try {
      const config = await loadConfigFromPath(this.configPath);
      this.onReload(config);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      this.onError(message);
    }
  }
}
