import { existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";
import type { SessionManager } from "../core/session.ts";
import type { LayoutNode } from "../core/session.ts";

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

export function serializeSessions(
  sessions: SessionManager,
): SerializedSession[] {
  return sessions.listSessions().map((s) => ({
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
}

export async function saveSession(
  sessions: SessionManager,
  savePath: string,
): Promise<void> {
  const dir = resolvePath(savePath);
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }

  const data = serializeSessions(sessions);
  const filePath = join(dir, "sessions.json");
  await Bun.write(filePath, JSON.stringify(data, null, 2));
}

export async function loadSavedSessions(
  savePath: string,
): Promise<SerializedSession[]> {
  const dir = resolvePath(savePath);
  const filePath = join(dir, "sessions.json");

  const file = Bun.file(filePath);
  if (!(await file.exists())) {
    return [];
  }

  try {
    const content = await file.text();
    return JSON.parse(content) as SerializedSession[];
  } catch {
    return [];
  }
}

export function remapLayoutIds(
  layout: LayoutNode,
  mapping: Map<string, string>,
): LayoutNode {
  if (layout.type === "leaf") {
    return {
      type: "leaf",
      paneId: mapping.get(layout.paneId) || layout.paneId,
    };
  }
  return {
    type: "split",
    direction: layout.direction,
    ratio: layout.ratio,
    children: [
      remapLayoutIds(layout.children[0], mapping),
      remapLayoutIds(layout.children[1], mapping),
    ],
  };
}

export function getAllPaneIdsFromSerialized(layout: any): string[] {
  if (layout.type === "leaf") return [layout.paneId];
  return [
    ...getAllPaneIdsFromSerialized(layout.children[0]),
    ...getAllPaneIdsFromSerialized(layout.children[1]),
  ];
}
