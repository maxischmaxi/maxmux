use clap::{Parser, Subcommand};

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
        Some(Commands::Attach { session }) => client::attach::run(session).await,
        Some(Commands::KillServer) => {
            // Connect and send kill - placeholder
            tracing::info!("Kill server not yet fully implemented");
        }
        Some(Commands::ListSessions) => {
            // Connect and list - placeholder
            tracing::info!("List sessions not yet fully implemented");
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

mod server;
mod client;
