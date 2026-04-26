//! OpenPeer signaling / STUN / TURN server binary.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// OpenPeer signaling server entry point.
#[derive(Parser, Debug)]
#[command(name = "opserver", version, author, about)]
struct Args {
    /// Local address to bind for signaling traffic.
    #[arg(long, default_value = "0.0.0.0:38901")]
    bind: std::net::SocketAddr,

    /// Public address peers use to reach this server (host:port).
    #[arg(long)]
    external_addr: Option<std::net::SocketAddr>,

    /// Enable TURN relay on the given port (default: disabled).
    #[arg(long)]
    turn_port: Option<u16>,
}

fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    tracing::info!(
        bind = %args.bind,
        external = %args.external_addr.map_or("dynamic".into(), |a| a.to_string()),
        "opserver starting"
    );

    // TODO: implement server logic in M1+
    tracing::info!("opserver running (placeholder — see M1)");
    std::process::exit(0);
}
