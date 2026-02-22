import { connect, type Socket } from "node:net";
import { getSocketPath } from "../server/daemon.ts";
import type { ServerMessage } from "../server/broadcast.ts";
import type { ClientMessage } from "../server/handler.ts";

export class ServerConnection {
  private socket: Socket | null = null;
  private buffer = "";
  private onMessage: (msg: ServerMessage) => void;
  private onClose: () => void;

  constructor(onMessage: (msg: ServerMessage) => void, onClose: () => void) {
    this.onMessage = onMessage;
    this.onClose = onClose;
  }

  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const socketPath = getSocketPath();
      this.socket = connect(socketPath);

      let connected = false;
      let closed = false;
      const triggerClose = () => {
        if (closed) return;
        closed = true;
        this.onClose();
      };

      this.socket.on("connect", () => {
        connected = true;
        resolve();
      });

      this.socket.on("data", (data) => {
        this.buffer += data.toString();
        const lines = this.buffer.split("\n");
        this.buffer = lines.pop() || "";

        for (const line of lines) {
          if (!line.trim()) continue;
          try {
            const msg: ServerMessage = JSON.parse(line);
            this.onMessage(msg);
          } catch {
            // Invalid JSON, skip
          }
        }
      });

      this.socket.on("end", triggerClose);
      this.socket.on("close", triggerClose);

      this.socket.on("error", (err) => {
        if (!connected) {
          reject(err);
        } else {
          triggerClose();
        }
      });
    });
  }

  send(msg: ClientMessage): void {
    if (this.socket && !this.socket.destroyed) {
      this.socket.write(JSON.stringify(msg) + "\n");
    }
  }

  disconnect(): void {
    if (this.socket) {
      this.socket.destroy();
      this.socket = null;
    }
  }

  isConnected(): boolean {
    return this.socket !== null && !this.socket.destroyed;
  }
}
