import { createServer, type Socket } from "node:net";
import {
  existsSync,
  mkdirSync,
  unlinkSync,
  writeFileSync,
  readFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { homedir } from "node:os";
import { ServerHandler, type ClientMessage } from "./handler.ts";
import { debugLog, debugClear } from "../debug.ts";
import type { MaxMuxConfig } from "../config/schema.ts";

export function getSocketPath(): string {
  const dir = join(homedir(), ".maxmux");
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }
  return join(dir, "server.sock");
}

export function getPidPath(): string {
  return join(homedir(), ".maxmux", "server.pid");
}

export function isServerRunning(): Promise<boolean> {
  return new Promise((resolve) => {
    const { connect } = require("node:net") as typeof import("node:net");
    const socketPath = getSocketPath();

    if (!existsSync(socketPath)) {
      resolve(false);
      return;
    }

    const client = connect(socketPath);
    client.on("connect", () => {
      client.destroy();
      resolve(true);
    });
    client.on("error", () => {
      // Stale socket file
      try {
        unlinkSync(socketPath);
      } catch {}
      resolve(false);
    });
  });
}

export async function startServer(config: MaxMuxConfig): Promise<void> {
  const socketPath = getSocketPath();
  const pidPath = getPidPath();
  const handler = new ServerHandler(config);

  debugClear();
  debugLog(
    "server",
    `starting server, pid=${process.pid} socket=${socketPath}`,
  );

  await handler.init();

  // Clean up stale socket
  if (existsSync(socketPath)) {
    try {
      unlinkSync(socketPath);
    } catch {}
  }

  const dir = dirname(socketPath);
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }

  // Write PID file
  writeFileSync(pidPath, process.pid.toString());

  // Cleanup on exit
  let cleaned = false;
  const cleanup = () => {
    if (cleaned) return;
    cleaned = true;
    debugLog("server", "cleanup: shutting down");
    handler.shutdown();
    server.close();
    try {
      unlinkSync(socketPath);
    } catch {}
    try {
      unlinkSync(pidPath);
    } catch {}
    // Delay exit to let shutdown messages flush to clients
    setTimeout(() => process.exit(0), 100);
  };

  const server = createServer((socket: Socket) => {
    const clientId = handler.handleConnection(socket);
    let buffer = "";

    socket.on("data", (data) => {
      buffer += data.toString();
      const lines = buffer.split("\n");
      buffer = lines.pop() || "";

      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          const msg: ClientMessage = JSON.parse(line);
          // Intercept server:kill before handler
          if (msg.type === "command" && msg.id === "server:kill") {
            debugLog("server", "received server:kill command");
            cleanup();
            return;
          }
          handler.handleMessage(clientId, msg);
        } catch (err) {
          handler.broadcaster.send(clientId, {
            type: "error",
            message: `Invalid message: ${err}`,
          });
        }
      }
    });

    socket.on("close", () => {
      handler.handleDisconnect(clientId);
    });

    socket.on("error", () => {
      handler.handleDisconnect(clientId);
    });
  });

  server.listen(socketPath, () => {
    // Server ready
  });

  process.on("SIGINT", cleanup);
  process.on("SIGTERM", cleanup);
  process.on("SIGHUP", cleanup);
}

export async function startServerDaemon(config: MaxMuxConfig): Promise<void> {
  const running = await isServerRunning();
  if (running) return;

  // Fork server as daemon
  const { spawn } = await import("node:child_process");
  const child = spawn(process.argv[0]!, [process.argv[1]!, "__server__"], {
    detached: true,
    stdio: "ignore",
    env: { ...process.env, MAXMUX_SERVER: "1" },
  });
  child.unref();

  // Wait for server to be ready
  for (let i = 0; i < 50; i++) {
    await new Promise((r) => setTimeout(r, 100));
    const ready = await isServerRunning();
    if (ready) return;
  }

  throw new Error("Server failed to start");
}
