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

#[derive(Subcommand)]
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
        None => {
            // Default: ensure server, then attach
            server::daemon::ensure_running().await;
            client::attach::run(None).await;
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
mod server;
