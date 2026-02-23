#!/usr/bin/env bun
import { loadConfig } from "./config/loader.ts";
import {
  startServer,
  startServerDaemon,
  isServerRunning,
} from "./server/daemon.ts";
import { attachToSession } from "./client/attach.ts";
import {
  listSessions,
  killSession,
  killServer,
  selectPane,
  displayMessage,
  selectWindow,
  splitWindow,
  newWindow,
  remoteCommand,
} from "./client/cli.ts";

const args = process.argv.slice(2);
const command = args[0];

function getTarget(): string | undefined {
  const idx = args.indexOf("-t");
  return idx !== -1 ? args[idx + 1] : undefined;
}

async function main() {
  // Internal server mode (spawned as daemon)
  if (command === "__server__" || process.env.MAXMUX_SERVER === "1") {
    const config = await loadConfig();
    await startServer(config);
    return;
  }

  // Fast path: remote commands that don't need config loading
  const fastCommands = [
    "select-pane",
    "display-message",
    "display",
    "select-window",
    "split-window",
    "new-window",
    "send-command",
  ];
  if (command && fastCommands.includes(command)) {
    await handleFastCommand(command);
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

async function handleFastCommand(cmd: string): Promise<void> {
  const target = getTarget();

  switch (cmd) {
    case "select-pane": {
      const dirMap: Record<string, "up" | "down" | "left" | "right"> = {
        "-U": "up",
        "-D": "down",
        "-L": "left",
        "-R": "right",
      };
      const dirFlag = args.find((a) => dirMap[a]);
      if (!dirFlag || !dirMap[dirFlag]) {
        console.error("Usage: maxmux select-pane -L|-R|-U|-D [-t session]");
        process.exit(1);
      }
      await selectPane(dirMap[dirFlag]!, target);
      break;
    }

    case "display-message":
    case "display": {
      const pIdx = args.indexOf("-p");
      if (pIdx === -1 || !args[pIdx + 1]) {
        console.error(
          "Usage: maxmux display-message -p '<format>' [-t session]",
        );
        process.exit(1);
      }
      await displayMessage(args[pIdx + 1]!, target);
      break;
    }

    case "select-window": {
      let direction: "next" | "previous" | undefined;
      if (args.includes("-n")) direction = "next";
      else if (args.includes("-p")) direction = "previous";
      if (!direction) {
        console.error("Usage: maxmux select-window -n|-p [-t session]");
        process.exit(1);
      }
      await selectWindow(direction, target);
      break;
    }

    case "split-window": {
      let direction: "horizontal" | "vertical" | undefined;
      if (args.includes("-h")) direction = "horizontal";
      else if (args.includes("-v")) direction = "vertical";
      if (!direction) {
        console.error("Usage: maxmux split-window -h|-v [-t session]");
        process.exit(1);
      }
      await splitWindow(direction, target);
      break;
    }

    case "new-window":
      await newWindow(target);
      break;

    case "send-command": {
      const cmdId = args[1];
      if (!cmdId || cmdId.startsWith("-")) {
        console.error("Usage: maxmux send-command <command-id> [-t session]");
        process.exit(1);
      }
      await remoteCommand(cmdId, target);
      break;
    }
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

Remote Control (tmux-compatible):
  maxmux select-pane -L|-R|-U|-D [-t session]      Focus pane in direction
  maxmux display-message -p '<format>' [-t session] Query session info
  maxmux select-window -n|-p [-t session]           Switch window
  maxmux split-window -h|-v [-t session]            Split pane
  maxmux new-window [-t session]                    Create window
  maxmux send-command <id> [-t session]             Execute command

Format variables for display-message:
  #{pane_at_left}, #{pane_at_right}, #{pane_at_top}, #{pane_at_bottom}
  #{session_name}, #{session_id}, #{window_name}, #{window_id}
  #{window_index}, #{pane_id}, #{pane_index}

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
