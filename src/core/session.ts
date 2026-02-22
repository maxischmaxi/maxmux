import { randomUUID } from "node:crypto";

export interface Pane {
  id: string;
  pid: number;
  cwd: string;
  command: string;
  title: string;
}

export interface Window {
  id: string;
  name: string;
  panes: Pane[];
  layout: LayoutNode;
  activePane: string;
}

export interface Session {
  id: string;
  name: string;
  windows: Window[];
  activeWindow: string;
  createdAt: number;
  attachedClients: string[];
}

export type LayoutNode =
  | { type: "leaf"; paneId: string }
  | {
      type: "split";
      direction: "horizontal" | "vertical";
      ratio: number;
      children: [LayoutNode, LayoutNode];
    };

export class SessionManager {
  sessions: Map<string, Session> = new Map();

  createSession(name?: string): Session {
    const id = randomUUID().slice(0, 8);
    const session: Session = {
      id,
      name: name || `session-${this.sessions.size}`,
      windows: [],
      activeWindow: "",
      createdAt: Date.now(),
      attachedClients: [],
    };
    this.sessions.set(id, session);
    return session;
  }

  getSession(id: string): Session | undefined {
    return this.sessions.get(id);
  }

  getSessionByName(name: string): Session | undefined {
    for (const session of this.sessions.values()) {
      if (session.name === name) return session;
    }
    return undefined;
  }

  getDefaultSession(): Session | undefined {
    return this.sessions.values().next().value;
  }

  deleteSession(id: string): boolean {
    return this.sessions.delete(id);
  }

  listSessions(): Session[] {
    return [...this.sessions.values()];
  }

  addWindow(sessionId: string, name?: string): Window | null {
    const session = this.sessions.get(sessionId);
    if (!session) return null;

    const windowId = randomUUID().slice(0, 8);
    const paneId = randomUUID().slice(0, 8);

    const window: Window = {
      id: windowId,
      name: name || `${session.windows.length}`,
      panes: [],
      layout: { type: "leaf", paneId },
      activePane: paneId,
    };

    session.windows.push(window);
    if (!session.activeWindow) {
      session.activeWindow = windowId;
    }

    return window;
  }

  getActiveWindow(sessionId: string): Window | undefined {
    const session = this.sessions.get(sessionId);
    if (!session) return undefined;
    return session.windows.find((w) => w.id === session.activeWindow);
  }

  getActivePane(sessionId: string): Pane | undefined {
    const window = this.getActiveWindow(sessionId);
    if (!window) return undefined;
    return window.panes.find((p) => p.id === window.activePane);
  }

  addPaneToWindow(sessionId: string, windowId: string, pane: Pane): void {
    const session = this.sessions.get(sessionId);
    if (!session) return;
    const window = session.windows.find((w) => w.id === windowId);
    if (!window) return;
    window.panes.push(pane);
  }

  removePaneFromWindow(
    sessionId: string,
    windowId: string,
    paneId: string,
  ): void {
    const session = this.sessions.get(sessionId);
    if (!session) return;
    const window = session.windows.find((w) => w.id === windowId);
    if (!window) return;
    window.panes = window.panes.filter((p) => p.id !== paneId);

    if (window.activePane === paneId && window.panes.length > 0) {
      window.activePane = window.panes[0]!.id;
    }
  }

  switchWindow(sessionId: string, direction: "next" | "previous"): void {
    const session = this.sessions.get(sessionId);
    if (!session || session.windows.length <= 1) return;

    const idx = session.windows.findIndex((w) => w.id === session.activeWindow);
    if (idx === -1) return;

    const newIdx =
      direction === "next"
        ? (idx + 1) % session.windows.length
        : (idx - 1 + session.windows.length) % session.windows.length;

    session.activeWindow = session.windows[newIdx]!.id;
  }

  removeWindow(sessionId: string, windowId: string): void {
    const session = this.sessions.get(sessionId);
    if (!session) return;
    session.windows = session.windows.filter((w) => w.id !== windowId);
    if (session.activeWindow === windowId && session.windows.length > 0) {
      session.activeWindow = session.windows[0]!.id;
    }
  }

  switchPane(sessionId: string, direction: "next"): void {
    const window = this.getActiveWindow(sessionId);
    if (!window || window.panes.length <= 1) return;

    const idx = window.panes.findIndex((p) => p.id === window.activePane);
    if (idx === -1) return;

    const newIdx = (idx + 1) % window.panes.length;
    window.activePane = window.panes[newIdx]!.id;
  }

  setActivePane(sessionId: string, paneId: string): void {
    const window = this.getActiveWindow(sessionId);
    if (!window) return;
    if (window.panes.some((p) => p.id === paneId)) {
      window.activePane = paneId;
    }
  }

  toJSON(): object {
    const sessions: Record<string, object> = {};
    for (const [id, session] of this.sessions) {
      sessions[id] = {
        ...session,
        attachedClients: [],
      };
    }
    return sessions;
  }
}
