#!/usr/bin/env bun
import { loadConfig } from "./config/loader.ts";
import {
  startServer,
  startServerDaemon,
  isServerRunning,
} from "./server/daemon.ts";
import { attachToSession } from "./client/attach.ts";
import { listSessions, killSession, killServer } from "./client/cli.ts";

const args = process.argv.slice(2);
const command = args[0];

async function main() {
  // Internal server mode (spawned as daemon)
  if (command === "__server__" || process.env.MAXMUX_SERVER === "1") {
    const config = await loadConfig();
    await startServer(config);
    return;
  }

  const config = await loadConfig();

  switch (command) {
    case "ls":
    case "list-sessions":
      await listSessions();
      break;

    case "new-session":
    case "new": {
      const nameIdx = args.indexOf("-s");
      const name = nameIdx !== -1 ? args[nameIdx + 1] : undefined;
      await startServerDaemon(config);
      await attachToSession(config, name ? `__new__:${name}` : "__new__");
      break;
    }

    case "attach":
    case "a": {
      const targetIdx = args.indexOf("-t");
      const target = targetIdx !== -1 ? args[targetIdx + 1] : undefined;
      const running = await isServerRunning();
      if (!running) {
        console.error("No server running. Start with: maxmux");
        process.exit(1);
      }
      await attachToSession(config, target);
      break;
    }

    case "detach":
      console.log("Use prefix + d to detach from within a session.");
      break;

    case "kill-session": {
      const targetIdx = args.indexOf("-t");
      const target = targetIdx !== -1 ? args[targetIdx + 1] : undefined;
      if (!target) {
        console.error("Usage: maxmux kill-session -t <name>");
        process.exit(1);
      }
      await killSession(target);
      break;
    }

    case "kill-server":
      await killServer();
      break;

    case "help":
    case "--help":
    case "-h":
      printHelp();
      break;

    case "version":
    case "--version":
    case "-v":
      console.log("maxmux v0.1.0");
      break;

    default:
      // Default: start server (if needed) and attach
      await startServerDaemon(config);
      await attachToSession(config);
      break;
  }
}

function printHelp() {
  console.log(`
maxmux - Modern Terminal Session Manager

Usage:
  maxmux                          Start server + attach to default session
  maxmux new-session [-s name]    Create a new session
  maxmux attach [-t session]      Attach to an existing session
  maxmux ls                       List sessions
  maxmux kill-session -t <name>   Kill a session
  maxmux kill-server              Stop the server
  maxmux help                     Show this help

Default prefix key: Ctrl+a

Keybindings (after prefix):
  c       Create new window
  n       Next window
  p       Previous window
  %       Split pane horizontally
  "       Split pane vertically
  o       Next pane
  x       Close pane
  z       Zoom pane
  d       Detach
  s       Session list
  :       Command palette
  ?       Show keybindings

Config: maxmux.config.ts or ~/.config/maxmux/maxmux.config.ts
`);
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
