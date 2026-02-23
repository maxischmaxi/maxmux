#!/usr/bin/env bun
// Minimal debug script: connect to server, send select-pane -L, log everything
import { connect } from "node:net";
import { join } from "node:path";
import { homedir } from "node:os";

const socketPath = join(homedir(), ".maxmux", "server.sock");
console.error("[debug] connecting to", socketPath);

const socket = connect(socketPath);
let buffer = "";

socket.on("connect", () => {
  console.error("[debug] connected, sending remote-command");
  const msg = JSON.stringify({
    type: "remote-command",
    command: "select-pane",
    args: { direction: "left" },
  });
  socket.write(msg + "\n");
  console.error("[debug] sent:", msg);
});

socket.on("data", (data) => {
  const chunk = data.toString();
  console.error("[debug] received data chunk:", JSON.stringify(chunk));
  buffer += chunk;
  const lines = buffer.split("\n");
  buffer = lines.pop() || "";
  for (const line of lines) {
    if (!line.trim()) continue;
    try {
      const parsed = JSON.parse(line);
      console.error("[debug] parsed message:", JSON.stringify(parsed));
      if (parsed.type === "result") {
        console.error("[debug] GOT RESULT, destroying socket");
        socket.destroy();
        process.exit(parsed.success ? 0 : 1);
      }
    } catch (e) {
      console.error("[debug] parse error:", e);
    }
  }
});

socket.on("error", (err) => {
  console.error("[debug] socket error:", err.message);
  process.exit(1);
});

socket.on("close", () => {
  console.error("[debug] socket closed");
  process.exit(0);
});

socket.on("end", () => {
  console.error("[debug] socket end");
});

setTimeout(() => {
  console.error("[debug] TIMEOUT after 5s, forcing exit");
  process.exit(1);
}, 5000);
