import type { Socket } from "node:net";
import type { SystemMetrics } from "../statusbar/types.ts";

export type ServerMessage =
  | { type: "output"; paneId: string; data: string }
  | {
      type: "state";
      sessions: any[];
      activeSession: string;
    }
  | {
      type: "layout";
      layout: any;
      paneRects: Record<string, any>;
    }
  | { type: "pane:exited"; paneId: string; exitCode: number }
  | { type: "metrics"; data: SystemMetrics }
  | {
      type: "cursor-state";
      panes: Record<string, { cursorVisible: boolean; cursorStyle: number }>;
    }
  | { type: "process-info"; panes: Record<string, string> }
  | { type: "error"; message: string }
  | { type: "result"; success: boolean; data?: string; error?: string }
  | { type: "preview-output"; paneId: string; data: string }
  | {
      type: "preview-layout";
      layout: any;
      paneRects: Record<string, any>;
    };

export class Broadcaster {
  private clients: Map<string, Socket> = new Map();
  private clientSessions: Map<string, string> = new Map(); // clientId -> sessionId
  private clientPreviews: Map<string, string> = new Map(); // clientId -> preview sessionId
  private clientPreviewDimensions: Map<string, { cols: number; rows: number }> =
    new Map();

  addClient(id: string, socket: Socket): void {
    this.clients.set(id, socket);
  }

  removeClient(id: string): void {
    this.clients.delete(id);
    this.clientSessions.delete(id);
    this.clientPreviews.delete(id);
    this.clientPreviewDimensions.delete(id);
  }

  setClientSession(clientId: string, sessionId: string): void {
    this.clientSessions.set(clientId, sessionId);
  }

  getClientSession(clientId: string): string | undefined {
    return this.clientSessions.get(clientId);
  }

  setClientPreview(
    clientId: string,
    sessionId: string,
    cols: number,
    rows: number,
  ): void {
    this.clientPreviews.set(clientId, sessionId);
    this.clientPreviewDimensions.set(clientId, { cols, rows });
  }

  clearClientPreview(clientId: string): void {
    this.clientPreviews.delete(clientId);
    this.clientPreviewDimensions.delete(clientId);
  }

  getClientPreview(clientId: string): string | undefined {
    return this.clientPreviews.get(clientId);
  }

  getClientPreviewDimensions(
    clientId: string,
  ): { cols: number; rows: number } | undefined {
    return this.clientPreviewDimensions.get(clientId);
  }

  sendPreviewToSession(sessionId: string, message: ServerMessage): void {
    for (const [clientId, sid] of this.clientPreviews) {
      if (sid === sessionId) {
        this.send(clientId, message);
      }
    }
  }

  send(clientId: string, message: ServerMessage): void {
    const socket = this.clients.get(clientId);
    if (socket && !socket.destroyed) {
      try {
        socket.write(JSON.stringify(message) + "\n");
      } catch {
        // Client disconnected
        this.removeClient(clientId);
      }
    }
  }

  cork(clientId: string): void {
    const socket = this.clients.get(clientId);
    if (socket && !socket.destroyed) {
      socket.cork();
    }
  }

  uncork(clientId: string): void {
    const socket = this.clients.get(clientId);
    if (socket && !socket.destroyed) {
      process.nextTick(() => {
        if (!socket.destroyed) socket.uncork();
      });
    }
  }

  sendToSession(sessionId: string, message: ServerMessage): void {
    for (const [clientId, sid] of this.clientSessions) {
      if (sid === sessionId) {
        this.send(clientId, message);
      }
    }
  }

  broadcast(message: ServerMessage): void {
    // Only send to clients that have attached to a session (skip CLI connections)
    for (const clientId of this.clientSessions.keys()) {
      this.send(clientId, message);
    }
  }

  /** Send to ALL connected sockets (including CLI connections). */
  broadcastAll(message: ServerMessage): void {
    for (const clientId of this.clients.keys()) {
      this.send(clientId, message);
    }
  }

  getClientCount(): number {
    return this.clients.size;
  }

  getSessionClients(sessionId: string): string[] {
    const clients: string[] = [];
    for (const [clientId, sid] of this.clientSessions) {
      if (sid === sessionId) clients.push(clientId);
    }
    return clients;
  }

  notifyShutdown(): void {
    this.broadcastAll({ type: "error", message: "server-shutdown" });
    for (const socket of this.clients.values()) {
      if (!socket.destroyed) {
        socket.end();
      }
    }
    this.clients.clear();
    this.clientSessions.clear();
  }
}
