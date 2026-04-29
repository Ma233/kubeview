use clap::Parser;
use kubeview::cli::Cli;
use kubeview::cli::Command;
use kubeview::server;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    match Cli::parse().command {
        Command::Serve(args) => server::serve(args).await,
    }
}
