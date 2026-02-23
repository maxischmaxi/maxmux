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
import type { ClientMessage } from "./handler.ts";
import { debugLog, setDebugEnabled } from "../debug.ts";
import type { MaxMuxConfig } from "../config/schema.ts";
import { findConfigFile } from "../config/loader.ts";

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
    const pidPath = getPidPath();

    // Check PID file first — if the process is dead, clean up stale files
    if (existsSync(pidPath)) {
      try {
        const pid = parseInt(readFileSync(pidPath, "utf-8").trim(), 10);
        if (!isNaN(pid)) {
          try {
            process.kill(pid, 0); // Check if process exists (signal 0 = no-op)
          } catch {
            // Process is dead — clean up stale files
            debugLog(
              "server",
              `stale server detected (pid=${pid} is dead), cleaning up`,
            );
            try {
              unlinkSync(socketPath);
            } catch {}
            try {
              unlinkSync(pidPath);
            } catch {}
            resolve(false);
            return;
          }
        }
      } catch {}
    }

    if (!existsSync(socketPath)) {
      resolve(false);
      return;
    }

    const client = connect(socketPath);
    client.on("connect", () => {
      clearTimeout(timeout);
      client.destroy();
      resolve(true);
    });
    const timeout = setTimeout(() => {
      client.destroy();
      resolve(false);
    }, 1000);

    client.on("error", () => {
      clearTimeout(timeout);
      // Do NOT delete socket file here — transient errors must not kill the server
      resolve(false);
    });
  });
}

export async function startServer(config: MaxMuxConfig): Promise<void> {
  setDebugEnabled(config.debug);
  const { ServerHandler } = await import("./handler.ts");
  const socketPath = getSocketPath();
  const pidPath = getPidPath();
  const configPath = findConfigFile();
  const handler = new ServerHandler(config, configPath);

  debugLog(
    "server",
    `=== server starting, pid=${process.pid} socket=${socketPath} ===`,
  );

  // Set MAXMUX env so PTY children (e.g. neovim) know they're inside MaxMux
  process.env.MAXMUX = socketPath;

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
    debugLog("server", "=== cleanup: shutting down ===");
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

  server.on("error", (err) => {
    debugLog("server", `server socket error: ${(err as Error).message}`);
  });

  server.listen(socketPath, () => {
    // Server ready
  });

  // Periodically check if socket file still exists — recreate if deleted externally
  const socketWatcher = setInterval(() => {
    if (!existsSync(socketPath)) {
      debugLog("server", "socket file missing — recreating");
      server.close(() => {
        server.listen(socketPath);
      });
    }
  }, 5000);
  socketWatcher.unref();

  process.on("SIGINT", () => {
    debugLog("server", "received SIGINT — shutting down");
    cleanup();
  });
  process.on("SIGTERM", () => {
    debugLog("server", "received SIGTERM — shutting down");
    cleanup();
  });
  process.on("SIGHUP", () => {
    debugLog("server", "received SIGHUP — ignoring (daemon)");
  });
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
