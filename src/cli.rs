use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;

#[derive(Debug, Parser)]
#[command(author, version, about = "Read-only Kubernetes MCP server")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Serve(ServeArgs),
}

#[derive(Debug, Clone, Parser)]
pub struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    #[arg(long, default_value_t = 3000)]
    pub port: u16,

    #[arg(long, default_value = "/mcp")]
    pub path: String,

    #[arg(long, value_name = "HOST[:PORT]")]
    pub allowed_host: Vec<String>,

    #[arg(long)]
    pub kubeconfig: Option<PathBuf>,

    #[arg(long)]
    pub context: Option<String>,

    #[arg(long)]
    pub namespace: Option<String>,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn serve_defaults_to_local_http_mcp() {
        let cli = Cli::parse_from(["kubeview", "serve"]);
        let Command::Serve(args) = cli.command;

        assert_eq!(args.host, "127.0.0.1");
        assert_eq!(args.port, 3000);
        assert_eq!(args.path, "/mcp");
        assert!(args.allowed_host.is_empty());
        assert!(args.kubeconfig.is_none());
        assert!(args.context.is_none());
        assert!(args.namespace.is_none());
    }

    #[test]
    fn serve_accepts_repeated_allowed_hosts() {
        let cli = Cli::parse_from([
            "kubeview",
            "serve",
            "--allowed-host",
            "localhost:3000",
            "--allowed-host",
            "mcp.example.com",
        ]);
        let Command::Serve(args) = cli.command;

        assert_eq!(args.allowed_host, vec![
            "localhost:3000".to_string(),
            "mcp.example.com".to_string()
        ]);
    }
}
