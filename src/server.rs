use std::net::IpAddr;
use std::sync::Arc;

use poem::Route;
use poem::Server;
use poem::listener::TcpListener;
use poem_mcpserver::McpServer;
use poem_mcpserver::streamable_http;

use crate::cli::ServeArgs;
use crate::error::KubeviewError;
use crate::kubernetes::KubeClientReader;
use crate::kubernetes::KubernetesConfig;
use crate::tools::KubeTools;
use crate::tools::KubernetesReader;

pub async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let address = listen_address(&args.host, args.port);
    let reader: Arc<dyn KubernetesReader> = Arc::new(
        KubeClientReader::new(KubernetesConfig {
            kubeconfig: args.kubeconfig,
            context: args.context,
            namespace: args.namespace,
        })
        .await
        .map_err(anyhow::Error::from)?,
    );

    run_http(address, args.path, reader).await
}

pub async fn run_http(
    address: String,
    path: String,
    reader: Arc<dyn KubernetesReader>,
) -> anyhow::Result<()> {
    let normalized_path = normalize_path(&path)?;
    let app = Route::new().at(
        normalized_path.as_str(),
        streamable_http::endpoint(move |_| McpServer::new().tools(KubeTools::new(reader.clone()))),
    );

    tracing::info!(%address, path = %normalized_path, "starting kubeview mcp server");
    Server::new(TcpListener::bind(address)).run(app).await?;
    Ok(())
}

fn listen_address(host: &str, port: u16) -> String {
    if matches!(host.parse::<IpAddr>(), Ok(IpAddr::V6(_))) {
        return format!("[{host}]:{port}");
    }

    format!("{host}:{port}")
}

pub fn normalize_path(path: &str) -> Result<String, KubeviewError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(KubeviewError::InvalidInput(
            "path must not be empty".to_string(),
        ));
    }
    if path.starts_with('/') {
        Ok(path.to_string())
    } else {
        Ok(format!("/{path}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_adds_leading_slash() {
        assert_eq!(normalize_path("mcp").unwrap(), "/mcp");
    }

    #[test]
    fn normalize_path_keeps_leading_slash() {
        assert_eq!(normalize_path("/mcp").unwrap(), "/mcp");
    }

    #[test]
    fn listen_address_preserves_hostnames() {
        assert_eq!(listen_address("localhost", 3000), "localhost:3000");
    }

    #[test]
    fn listen_address_brackets_ipv6_literals() {
        assert_eq!(listen_address("::1", 3000), "[::1]:3000");
    }
}
