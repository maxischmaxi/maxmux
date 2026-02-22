import { connect } from "node:net";
import { existsSync, readFileSync, unlinkSync } from "node:fs";
import {
  getSocketPath,
  getPidPath,
  isServerRunning,
} from "../server/daemon.ts";
import type { ClientMessage } from "../server/handler.ts";
import type { ServerMessage } from "../server/broadcast.ts";

function sendCommand(msg: ClientMessage): Promise<ServerMessage[]> {
  return new Promise(async (resolve, reject) => {
    const running = await isServerRunning();
    if (!running) {
      reject(new Error("Server is not running. Start with: maxmux"));
      return;
    }

    const socketPath = getSocketPath();
    const socket = connect(socketPath);
    const messages: ServerMessage[] = [];
    let buffer = "";

    socket.on("connect", () => {
      socket.write(JSON.stringify(msg) + "\n");
    });

    socket.on("data", (data) => {
      buffer += data.toString();
      const lines = buffer.split("\n");
      buffer = lines.pop() || "";

      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          messages.push(JSON.parse(line));
        } catch {}
      }
    });

    // Wait a bit for responses then close
    setTimeout(() => {
      socket.destroy();
      resolve(messages);
    }, 500);

    socket.on("error", reject);
  });
}

export async function listSessions(): Promise<void> {
  try {
    const messages = await sendCommand({
      type: "attach",
      sessionId: "__list__",
    });

    const stateMsg = messages.find((m) => m.type === "state");
    if (!stateMsg || stateMsg.type !== "state") {
      console.log("No sessions found.");
      return;
    }

    const sessions = stateMsg.sessions as Array<{
      id: string;
      name: string;
      windows: Array<{ name: string }>;
      attached: boolean;
    }>;

    if (sessions.length === 0) {
      console.log("No sessions.");
      return;
    }

    for (const session of sessions) {
      const attached = session.attached ? " (attached)" : "";
      const windowCount = session.windows?.length || 0;
      console.log(
        `${session.name}: ${windowCount} window(s)${attached} [${session.id}]`,
      );
    }
  } catch (err: any) {
    console.error(err.message);
    process.exit(1);
  }
}

export async function killSession(target: string): Promise<void> {
  try {
    await sendCommand({
      type: "command",
      id: "session:kill",
      args: { target },
    });
    console.log(`Session '${target}' killed.`);
  } catch (err: any) {
    console.error(err.message);
    process.exit(1);
  }
}

export async function killServer(): Promise<void> {
  const running = await isServerRunning();
  const pidPath = getPidPath();
  const socketPath = getSocketPath();

  if (!running) {
    // Server not reachable via socket — try PID file as fallback
    if (existsSync(pidPath)) {
      const pid = parseInt(readFileSync(pidPath, "utf-8").trim(), 10);
      if (pid > 0) {
        try {
          process.kill(pid, "SIGTERM");
          console.log(`Server killed (pid ${pid}).`);
        } catch {
          console.log("Server is not running (stale pid file).");
        }
        try {
          unlinkSync(pidPath);
        } catch {}
        try {
          unlinkSync(socketPath);
        } catch {}
      }
    } else {
      console.log("Server is not running.");
    }
    return;
  }

  function killByPid() {
    if (existsSync(pidPath)) {
      const pid = parseInt(readFileSync(pidPath, "utf-8").trim(), 10);
      if (pid > 0) {
        try {
          process.kill(pid, "SIGKILL");
          console.log(`Server killed forcefully (pid ${pid}).`);
        } catch {
          console.log("Server process already gone.");
        }
        try {
          unlinkSync(pidPath);
        } catch {}
      }
    }
    try {
      unlinkSync(socketPath);
    } catch {}
  }

  return new Promise<void>((resolve) => {
    const socketPath = getSocketPath();
    const socket = connect(socketPath);
    let done = false;

    const finish = (msg: string) => {
      if (done) return;
      done = true;
      socket.destroy();
      console.log(msg);
      resolve();
    };

    socket.on("connect", () => {
      socket.write(
        JSON.stringify({ type: "command", id: "server:kill" }) + "\n",
      );
      // Don't wait for close event — give server a moment then exit
      setTimeout(() => finish("Server killed."), 200);
    });

    socket.on("close", () => finish("Server killed."));
    socket.on("end", () => finish("Server killed."));

    socket.on("error", () => {
      killByPid();
      finish("Server killed.");
    });

    // Hard timeout fallback
    setTimeout(() => {
      killByPid();
      finish("Server killed (timeout).");
    }, 2000);
  });
}
