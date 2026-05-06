use std::net::IpAddr;
use std::sync::Arc;

use axum::Router;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

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

    run_http(args.allowed_host, address, args.path, reader).await
}

pub async fn run_http(
    allowed_hosts: Vec<String>,
    address: String,
    path: String,
    reader: Arc<dyn KubernetesReader>,
) -> anyhow::Result<()> {
    let normalized_path = normalize_path(&path)?;
    let cancellation_token = CancellationToken::new();
    let config = streamable_http_server_config(&allowed_hosts, cancellation_token.clone());
    let service: StreamableHttpService<KubeTools, LocalSessionManager> = StreamableHttpService::new(
        move || Ok(KubeTools::new(reader.clone())),
        Default::default(),
        config,
    );
    let app = Router::new().nest_service(normalized_path.as_str(), service);
    let listener = TcpListener::bind(&address).await?;

    tracing::info!(%address, path = %normalized_path, "starting kubeview mcp server");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cancellation_token.cancel();
            }
        })
        .await?;
    Ok(())
}

fn streamable_http_server_config(
    allowed_hosts: &[String],
    cancellation_token: CancellationToken,
) -> StreamableHttpServerConfig {
    let config = StreamableHttpServerConfig::default().with_cancellation_token(cancellation_token);
    if allowed_hosts.is_empty() {
        config
    } else {
        config.with_allowed_hosts(allowed_hosts.iter().cloned())
    }
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
    if path == "/" {
        return Err(KubeviewError::InvalidInput(
            "path must not be /; use a non-root path such as /mcp".to_string(),
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
    fn normalize_path_rejects_root() {
        assert_eq!(
            normalize_path("/").unwrap_err().to_string(),
            "invalid input: path must not be /; use a non-root path such as /mcp"
        );
    }

    #[test]
    fn listen_address_preserves_hostnames() {
        assert_eq!(listen_address("localhost", 3000), "localhost:3000");
    }

    #[test]
    fn listen_address_brackets_ipv6_literals() {
        assert_eq!(listen_address("::1", 3000), "[::1]:3000");
    }

    #[test]
    fn empty_allowed_hosts_keep_rmcp_defaults() {
        let config = streamable_http_server_config(&[], CancellationToken::new());

        assert_eq!(config.allowed_hosts, vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string()
        ]);
    }

    #[test]
    fn explicit_allowed_hosts_override_defaults() {
        let config = streamable_http_server_config(
            &["localhost:3000".to_string(), "mcp.example.com".to_string()],
            CancellationToken::new(),
        );

        assert_eq!(config.allowed_hosts, vec![
            "localhost:3000".to_string(),
            "mcp.example.com".to_string()
        ]);
    }
}
