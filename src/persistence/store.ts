import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  readdirSync,
} from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";
import type { SessionManager } from "../core/session.ts";

export interface SerializedPane {
  id: string;
  cwd: string;
  command: string;
  title: string;
}

export interface SerializedWindow {
  id: string;
  name: string;
  panes: SerializedPane[];
  layout: any;
  activePane: string;
}

export interface SerializedSession {
  id: string;
  name: string;
  windows: SerializedWindow[];
  activeWindow: string;
  createdAt: number;
}

function resolvePath(savePath: string): string {
  return savePath.replace(/^~/, homedir());
}

export function saveSession(sessions: SessionManager, savePath: string): void {
  const dir = resolvePath(savePath);
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }

  const sessionList = sessions.listSessions();
  const data: SerializedSession[] = sessionList.map((s) => ({
    id: s.id,
    name: s.name,
    windows: s.windows.map((w) => ({
      id: w.id,
      name: w.name,
      panes: w.panes.map((p) => ({
        id: p.id,
        cwd: p.cwd,
        command: p.command,
        title: p.title,
      })),
      layout: w.layout,
      activePane: w.activePane,
    })),
    activeWindow: s.activeWindow,
    createdAt: s.createdAt,
  }));

  const filePath = join(dir, "sessions.json");
  writeFileSync(filePath, JSON.stringify(data, null, 2), "utf-8");
}

export function loadSavedSessions(savePath: string): SerializedSession[] {
  const dir = resolvePath(savePath);
  const filePath = join(dir, "sessions.json");

  if (!existsSync(filePath)) {
    return [];
  }

  try {
    const content = readFileSync(filePath, "utf-8");
    return JSON.parse(content) as SerializedSession[];
  } catch {
    return [];
  }
}
