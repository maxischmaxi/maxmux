import { appendFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";

const LOG_PATH = join(homedir(), ".maxmux", "debug.log");

let enabled = true;

export function debugLog(tag: string, msg: string): void {
  if (!enabled) return;
  const ts = new Date().toISOString().slice(11, 23);
  try {
    appendFileSync(LOG_PATH, `[${ts}] [${tag}] ${msg}\n`);
  } catch {}
}

export function debugClear(): void {
  try {
    writeFileSync(LOG_PATH, "");
  } catch {}
}
