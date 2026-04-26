//! OpenPeer client CLI binary.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// OpenPeer P2P file-transfer client.
#[derive(Parser, Debug)]
#[command(name = "opclient", version, author, about)]
struct Args {
    /// Signaling server address.
    #[arg(long, default_value = "127.0.0.1:38901")]
    server: std::net::SocketAddr,

    /// Peer ID to connect to (leave empty to act as receiver/seed).
    #[arg(long)]
    peer_id: Option<String>,

    /// File to send (sender mode).
    #[arg(long)]
    send: Option<std::path::PathBuf>,

    /// Output directory (receiver mode).
    #[arg(long)]
    output: Option<std::path::PathBuf>,

    /// Chunk size in KiB (default: 64).
    #[arg(long, value_name = "KIB")]
    chunk_size: Option<usize>,

    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    let filter: &str = if args.verbose {
        "debug"
    } else {
        &std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!(
        server = %args.server,
        mode = if args.send.is_some() { "sender" } else { "receiver" },
        "opclient starting"
    );

    // TODO: implement client logic in M1+
    tracing::info!("opclient running (placeholder — see M1)");
    std::process::exit(0);
}
