#[path = "kind_integration/assertions.rs"]
mod assertions;
#[path = "kind_integration/cluster.rs"]
mod cluster;
#[path = "kind_integration/mcp.rs"]
mod mcp;

use assertions::assert_job_logs;
use assertions::assert_namespace_scope;
use assertions::assert_unscoped_observability;
use cluster::NamespaceGuard;
use cluster::apply_fixture;
use cluster::wait_for_workloads;
use mcp::McpClient;
use mcp::TestServer;

const RUN_ENV: &str = "KUBEVIEW_RUN_KIND_INTEGRATION";

#[test]
fn kind_cluster_matches_production_like_read_only_workflows() -> anyhow::Result<()> {
    if std::env::var(RUN_ENV).as_deref() != Ok("1") {
        eprintln!("skipping kind integration test; set {RUN_ENV}=1 to run it");
        return Ok(());
    }

    let namespace = format!("kubeview-prod-smoke-{}", std::process::id());
    let namespace_guard = NamespaceGuard::new(&namespace);
    apply_fixture(&namespace)?;
    wait_for_workloads(&namespace)?;

    let server = TestServer::start(&[])?;
    let client = McpClient::connect(server.url())?;

    assert_unscoped_observability(&client, &namespace)?;
    assert_job_logs(&client, &namespace)?;

    let scoped_server = TestServer::start(&["--namespace", &namespace])?;
    let scoped_client = McpClient::connect(scoped_server.url())?;
    assert_namespace_scope(&scoped_client, &namespace)?;

    drop(scoped_server);
    drop(server);
    drop(namespace_guard);
    Ok(())
}
