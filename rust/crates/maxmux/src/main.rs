use clap::{Parser, Subcommand};
use std::collections::HashMap;
use tokio::net::UnixStream;

use maxmux_ipc::protocol::{ClientMessage, ServerMessage};
use maxmux_ipc::transport::Connection;

#[derive(Parser)]
#[command(name = "maxmux", version, about = "A modern terminal multiplexer")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Attach to a session
    Attach {
        /// Session name or ID
        session: Option<String>,
    },
    /// Create a new session
    NewSession {
        #[arg(short, long)]
        name: Option<String>,
    },
    /// List sessions
    #[command(alias = "ls")]
    ListSessions,
    /// Kill the server
    KillServer,
    /// (Internal) Run as server daemon
    #[command(hide = true)]
    Server,

    // ---- Remote commands (sent to the running server) ----
    /// Split the current pane horizontally or vertically
    #[command(name = "split-window", disable_help_flag = true)]
    SplitWindow {
        /// Split horizontally (top/bottom)
        #[arg(short = 'h', long)]
        horizontal: bool,
        /// Split vertically (left/right)
        #[arg(short = 'v', long)]
        vertical: bool,
        /// Print help
        #[arg(long, action = clap::ArgAction::Help)]
        help: Option<bool>,
    },

    /// Select a pane by direction
    #[command(name = "select-pane")]
    SelectPane {
        /// Select pane to the left
        #[arg(short = 'L')]
        left: bool,
        /// Select pane to the right
        #[arg(short = 'R')]
        right: bool,
        /// Select pane above
        #[arg(short = 'U')]
        up: bool,
        /// Select pane below
        #[arg(short = 'D')]
        down: bool,
    },

    /// Create a new window
    #[command(name = "new-window")]
    NewWindow,

    /// Select window by index
    #[command(name = "select-window")]
    SelectWindow {
        /// Target window index
        #[arg(short = 't')]
        target: Option<usize>,
    },

    /// Display a message (supports format variables)
    #[command(name = "display-message")]
    DisplayMessage {
        /// Print to stdout instead of status line
        #[arg(short = 'p')]
        print: bool,
        /// Message template with #{var} format variables
        message: Option<String>,
    },

    /// Send a raw command to the server
    #[command(name = "send-command")]
    SendCommand {
        /// The command string to send
        command: String,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Server) => server::daemon::run().await,
        Some(Commands::Attach { session }) => {
            server::daemon::ensure_running().await;
            client::attach::run(session).await;
        }
        Some(Commands::KillServer) => {
            kill_server().await;
        }
        Some(Commands::ListSessions) => {
            list_sessions().await;
        }
        Some(Commands::NewSession { name }) => {
            server::daemon::ensure_running().await;
            client::attach::run(name).await;
        }
        Some(ref cmd @ Commands::SplitWindow { .. })
        | Some(ref cmd @ Commands::SelectPane { .. })
        | Some(ref cmd @ Commands::NewWindow)
        | Some(ref cmd @ Commands::SelectWindow { .. })
        | Some(ref cmd @ Commands::DisplayMessage { .. })
        | Some(ref cmd @ Commands::SendCommand { .. }) => {
            handle_remote_command(cmd).await;
        }
        None => {
            // Default: ensure server, then attach
            server::daemon::ensure_running().await;
            client::attach::run(None).await;
        }
    }
}

/// Send a remote command to the running server and print the result.
async fn handle_remote_command(cmd: &Commands) {
    let socket_path = server::daemon::socket_path();
    let stream = match UnixStream::connect(&socket_path).await {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Server is not running");
            return;
        }
    };
    let mut conn = Connection::new(stream);

    let (command_str, args) = match cmd {
        Commands::SplitWindow {
            horizontal,
            vertical,
            ..
        } => {
            let direction = if *horizontal {
                "horizontal"
            } else if *vertical {
                "vertical"
            } else {
                // Default to vertical split (left/right) like tmux
                "vertical"
            };
            (
                "split-window".to_string(),
                Some(vec![direction.to_string()]),
            )
        }
        Commands::SelectPane {
            left,
            right,
            up,
            down,
        } => {
            let direction = if *left {
                "left"
            } else if *right {
                "right"
            } else if *up {
                "up"
            } else if *down {
                "down"
            } else {
                eprintln!("select-pane requires a direction flag: -L, -R, -U, or -D");
                return;
            };
            ("select-pane".to_string(), Some(vec![direction.to_string()]))
        }
        Commands::NewWindow => ("new-window".to_string(), None),
        Commands::SelectWindow { target } => {
            let args = target.map(|t| vec![t.to_string()]);
            ("select-window".to_string(), args)
        }
        Commands::DisplayMessage { print, message } => {
            let mut args = Vec::new();
            if *print {
                args.push("-p".to_string());
            }
            if let Some(msg) = message {
                // Resolve format variables if the server provides state
                args.push(msg.clone());
            }
            let args_opt = if args.is_empty() { None } else { Some(args) };
            ("display-message".to_string(), args_opt)
        }
        Commands::SendCommand { command } => {
            ("send-command".to_string(), Some(vec![command.clone()]))
        }
        _ => return,
    };

    // Send as a RemoteCommand
    let msg = ClientMessage::RemoteCommand {
        command: command_str,
        args,
        target: None,
    };

    if let Err(e) = conn.send_message(&msg).await {
        eprintln!("Failed to send command: {}", e);
        return;
    }

    // Wait for result
    match conn.read_message::<ServerMessage>().await {
        Ok(ServerMessage::Result {
            success,
            data,
            error,
        }) => {
            if success {
                if let Some(data) = data {
                    println!("{}", data);
                }
            } else if let Some(err) = error {
                eprintln!("Error: {}", err);
            }
        }
        Ok(ServerMessage::Error { message }) => {
            eprintln!("Server error: {}", message);
        }
        Ok(_) => {
            // Other message types are unexpected but not fatal
        }
        Err(e) => {
            eprintln!("Failed to read response: {}", e);
        }
    }
}

async fn kill_server() {
    let path = server::daemon::socket_path();
    if let Ok(stream) = UnixStream::connect(&path).await {
        let mut conn = Connection::new(stream);
        conn.send_message(&ClientMessage::Command {
            id: "server:kill".to_string(),
            args: HashMap::new(),
        })
        .await
        .ok();
        println!("Kill signal sent to server");
    } else {
        eprintln!("Server is not running");
    }
}

async fn list_sessions() {
    let path = server::daemon::socket_path();
    if let Ok(stream) = UnixStream::connect(&path).await {
        let mut conn = Connection::new(stream);
        // Send attach with no session to get state back
        conn.send_message(&ClientMessage::Attach {
            session_id: None,
            cwd: None,
        })
        .await
        .ok();
        // Read state response
        if let Ok(msg) = conn.read_message::<ServerMessage>().await {
            if let ServerMessage::State { sessions, .. } = msg {
                if sessions.is_empty() {
                    println!("No sessions");
                } else {
                    for s in &sessions {
                        println!(
                            "{}: {} ({} windows, {} clients)",
                            s.id,
                            s.name,
                            s.windows.len(),
                            s.attached_clients.len()
                        );
                    }
                }
            }
        }
        // Detach cleanly
        conn.send_message(&ClientMessage::Detach).await.ok();
    } else {
        eprintln!("Server is not running");
    }
}

mod client;
mod remote;
mod server;

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_split_window_horizontal() {
        let cli = Cli::parse_from(["maxmux", "split-window", "-h"]);
        match cli.command {
            Some(Commands::SplitWindow {
                horizontal,
                vertical,
                ..
            }) => {
                assert!(horizontal);
                assert!(!vertical);
            }
            _ => panic!("Expected SplitWindow command"),
        }
    }

    #[test]
    fn test_cli_split_window_vertical() {
        let cli = Cli::parse_from(["maxmux", "split-window", "-v"]);
        match cli.command {
            Some(Commands::SplitWindow {
                horizontal,
                vertical,
                ..
            }) => {
                assert!(!horizontal);
                assert!(vertical);
            }
            _ => panic!("Expected SplitWindow command"),
        }
    }

    #[test]
    fn test_cli_select_pane_left() {
        let cli = Cli::parse_from(["maxmux", "select-pane", "-L"]);
        match cli.command {
            Some(Commands::SelectPane {
                left,
                right,
                up,
                down,
            }) => {
                assert!(left);
                assert!(!right);
                assert!(!up);
                assert!(!down);
            }
            _ => panic!("Expected SelectPane command"),
        }
    }

    #[test]
    fn test_cli_select_pane_down() {
        let cli = Cli::parse_from(["maxmux", "select-pane", "-D"]);
        match cli.command {
            Some(Commands::SelectPane {
                left,
                right,
                up,
                down,
            }) => {
                assert!(!left);
                assert!(!right);
                assert!(!up);
                assert!(down);
            }
            _ => panic!("Expected SelectPane command"),
        }
    }

    #[test]
    fn test_cli_new_window() {
        let cli = Cli::parse_from(["maxmux", "new-window"]);
        assert!(matches!(cli.command, Some(Commands::NewWindow)));
    }

    #[test]
    fn test_cli_select_window_with_target() {
        let cli = Cli::parse_from(["maxmux", "select-window", "-t", "3"]);
        match cli.command {
            Some(Commands::SelectWindow { target }) => {
                assert_eq!(target, Some(3));
            }
            _ => panic!("Expected SelectWindow command"),
        }
    }

    #[test]
    fn test_cli_select_window_no_target() {
        let cli = Cli::parse_from(["maxmux", "select-window"]);
        match cli.command {
            Some(Commands::SelectWindow { target }) => {
                assert_eq!(target, None);
            }
            _ => panic!("Expected SelectWindow command"),
        }
    }

    #[test]
    fn test_cli_display_message_print() {
        let cli = Cli::parse_from(["maxmux", "display-message", "-p", "#{session_name}"]);
        match cli.command {
            Some(Commands::DisplayMessage { print, message }) => {
                assert!(print);
                assert_eq!(message, Some("#{session_name}".to_string()));
            }
            _ => panic!("Expected DisplayMessage command"),
        }
    }

    #[test]
    fn test_cli_send_command() {
        let cli = Cli::parse_from(["maxmux", "send-command", "resize-pane"]);
        match cli.command {
            Some(Commands::SendCommand { command }) => {
                assert_eq!(command, "resize-pane");
            }
            _ => panic!("Expected SendCommand command"),
        }
    }

    #[test]
    fn test_cli_no_command_is_none() {
        let cli = Cli::parse_from(["maxmux"]);
        assert!(cli.command.is_none());
    }
}
