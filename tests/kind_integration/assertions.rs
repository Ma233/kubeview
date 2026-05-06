use serde_json::Value;
use serde_json::json;

use super::mcp::McpClient;
use super::mcp::ToolResult;

pub(crate) fn assert_unscoped_observability(
    client: &McpClient,
    namespace: &str,
) -> anyhow::Result<()> {
    assert_pod_listing(client, namespace)?;
    assert_selector_filtering(client, namespace)?;
    assert_service_tracing(client, namespace)?;
    assert_rollouts(client, namespace)?;
    assert_batch_state(client, namespace)?;
    assert_events(client, namespace)?;
    Ok(())
}

pub(crate) fn assert_job_logs(client: &McpClient, namespace: &str) -> anyhow::Result<()> {
    let pods = client.call_tool("list_pods", json!({ "namespace": namespace }))?;
    let complete_pod = pods.name_with_prefix("pods", "complete-once-");
    let failed_pod = pods.name_with_prefix("pods", "fail-once-");

    let complete_logs = client.call_tool(
        "pod_logs",
        json!({ "namespace": namespace, "pod": complete_pod, "tail_lines": 5 }),
    )?;
    assert_eq!(complete_logs.string_field("logs"), "complete job log\n");

    let failed_logs = client.call_tool(
        "pod_logs",
        json!({ "namespace": namespace, "pod": failed_pod, "tail_lines": 5 }),
    )?;
    assert_eq!(failed_logs.string_field("logs"), "failed job log\n");
    Ok(())
}

pub(crate) fn assert_namespace_scope(client: &McpClient, namespace: &str) -> anyhow::Result<()> {
    let namespaces = client.call_tool("list_namespaces", json!({}))?;
    let namespace_items = namespaces.array("namespaces");
    assert_eq!(namespace_items.len(), 1);
    assert_eq!(namespace_items[0]["name"], namespace);

    let pods = client.call_tool("list_pods", json!({ "label_selector": "app=frontend" }))?;
    assert_eq!(pods.array("pods").len(), 3);

    let broken = client.call_tool(
        "get_rollout_status",
        json!({ "kind": "Deployment", "name": "broken-api" }),
    )?;
    assert!(!broken.bool_field("complete"));

    assert_tool_error(
        client.call_tool("list_pods", json!({ "all_namespaces": true }))?,
        "all_namespaces is not allowed",
    );
    assert_tool_error(
        client.call_tool(
            "trace_service",
            json!({ "namespace": "default", "name": "kubernetes" }),
        )?,
        "outside configured scope",
    );
    assert_tool_error(
        client.call_tool(
            "list_resources",
            json!({ "api_version": "v1", "kind": "Node" }),
        )?,
        "cluster-scoped resource 'Node'",
    );
    Ok(())
}

fn assert_pod_listing(client: &McpClient, namespace: &str) -> anyhow::Result<()> {
    let pods = client.call_tool("list_pods", json!({ "namespace": namespace }))?;
    assert!(!pods.is_error(), "list_pods returned error: {pods:?}");

    assert_eq!(pods.count_with_prefix("pods", "frontend-"), 3);
    assert_eq!(pods.count_with_prefix("pods", "broken-api-"), 2);
    assert_eq!(pods.count_with_prefix("pods", "cache-"), 2);
    assert_eq!(pods.count_with_prefix("pods", "node-probe-"), 1);
    assert_eq!(pods.count_with_prefix("pods", "complete-once-"), 1);
    assert_eq!(pods.count_with_prefix("pods", "fail-once-"), 1);
    assert_eq!(pods.count_with_prefix("pods", "long-running-"), 1);
    assert_eq!(pods.count_by_field("pods", "phase", "Running"), 7);
    assert_eq!(pods.count_by_field("pods", "phase", "Succeeded"), 1);
    assert_eq!(pods.count_by_field("pods", "phase", "Failed"), 1);
    Ok(())
}

fn assert_selector_filtering(client: &McpClient, namespace: &str) -> anyhow::Result<()> {
    let selected = client.call_tool(
        "list_pods",
        json!({ "all_namespaces": true, "label_selector": "app=frontend" }),
    )?;
    assert_eq!(selected.array("pods").len(), 3);

    let deployments = client.call_tool(
        "list_resources",
        json!({
            "api_version": "apps/v1",
            "kind": "Deployment",
            "namespace": namespace,
            "label_selector": "app=frontend",
        }),
    )?;
    assert_eq!(deployments.array("items").len(), 1);
    assert_eq!(
        deployments.array("items")[0]["metadata"]["name"],
        "frontend"
    );
    Ok(())
}

fn assert_service_tracing(client: &McpClient, namespace: &str) -> anyhow::Result<()> {
    assert_trace(client, namespace, "frontend", 3, 3, &["frontend-"], &[])?;
    assert_trace(client, namespace, "broken-api", 2, 0, &["broken-api-"], &[
        "service has no ready endpoints",
    ])?;
    assert_trace(client, namespace, "orphan", 0, 0, &[], &[
        "service has no EndpointSlice endpoints",
        "service selector does not match any pods",
    ])?;
    assert_trace(client, namespace, "cache", 2, 2, &["cache-"], &[])?;
    Ok(())
}

fn assert_rollouts(client: &McpClient, namespace: &str) -> anyhow::Result<()> {
    assert_rollout(client, namespace, "Deployment", "frontend", true, 3, 3)?;
    assert_rollout(client, namespace, "Deployment", "broken-api", false, 2, 0)?;
    assert_rollout(client, namespace, "StatefulSet", "cache", true, 2, 2)?;
    assert_rollout(client, namespace, "DaemonSet", "node-probe", true, 1, 1)?;

    let timed_out = client.call_tool(
        "wait_rollout",
        json!({
            "namespace": namespace,
            "kind": "Deployment",
            "name": "broken-api",
            "timeout_seconds": 3,
            "interval_seconds": 1,
        }),
    )?;
    assert!(!timed_out.bool_field("completed"));
    assert!(timed_out.bool_field("timed_out"));
    assert_eq!(timed_out.nested_string(&["status", "name"]), "broken-api");
    Ok(())
}

fn assert_batch_state(client: &McpClient, namespace: &str) -> anyhow::Result<()> {
    let jobs = client.call_tool("list_jobs", json!({ "namespace": namespace, "limit": 20 }))?;
    assert_job(&jobs, "complete-once", 0, 1, 0, "Complete");
    assert_job(&jobs, "fail-once", 0, 0, 1, "Failed");
    assert_job(&jobs, "long-running", 1, 0, 0, "");

    let cronjobs = client.call_tool(
        "list_cronjobs",
        json!({ "namespace": namespace, "limit": 20 }),
    )?;
    let cronjob = cronjobs.item_named("cronjobs", "batch-daily");
    assert_eq!(cronjob["schedule"], "17 3 * * *");
    assert_eq!(cronjob["suspend"], true);
    assert_eq!(cronjob["concurrency_policy"], "Forbid");
    Ok(())
}

fn assert_events(client: &McpClient, namespace: &str) -> anyhow::Result<()> {
    let events = client.call_tool(
        "list_events",
        json!({ "namespace": namespace, "limit": 50 }),
    )?;
    assert!(
        events
            .array("events")
            .iter()
            .any(|event| event["type_"] == "Warning" && event["reason"] == "Failed"),
        "expected image pull warning events, got {:#?}",
        events.array("events")
    );
    assert!(
        events
            .array("events")
            .iter()
            .any(|event| event["type_"] == "Warning" && event["reason"] == "BackoffLimitExceeded"),
        "expected failed job warning event, got {:#?}",
        events.array("events")
    );
    Ok(())
}

fn assert_trace(
    client: &McpClient,
    namespace: &str,
    service: &str,
    total: i64,
    ready: i64,
    selected_prefixes: &[&str],
    expected_warnings: &[&str],
) -> anyhow::Result<()> {
    let trace = client.call_tool(
        "trace_service",
        json!({ "namespace": namespace, "name": service }),
    )?;
    assert_eq!(trace.nested_i64(&["endpoints", "total"]), total);
    assert_eq!(trace.nested_i64(&["endpoints", "ready"]), ready);

    for prefix in selected_prefixes {
        assert!(
            trace.has_name_with_prefix("selected_pods", prefix),
            "expected selected pod with prefix {prefix}, got {:#?}",
            trace.array("selected_pods")
        );
    }

    let warnings = trace.array("warnings");
    for expected in expected_warnings {
        assert!(
            warnings.iter().any(|warning| warning == expected),
            "expected warning {expected:?}, got {warnings:#?}"
        );
    }

    if expected_warnings.is_empty() {
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:#?}");
    }
    Ok(())
}

fn assert_rollout(
    client: &McpClient,
    namespace: &str,
    kind: &str,
    name: &str,
    complete: bool,
    desired: i64,
    ready: i64,
) -> anyhow::Result<()> {
    let rollout = client.call_tool(
        "get_rollout_status",
        json!({ "namespace": namespace, "kind": kind, "name": name }),
    )?;
    assert_eq!(rollout.string_field("kind"), kind);
    assert_eq!(rollout.string_field("name"), name);
    assert_eq!(rollout.bool_field("complete"), complete);
    assert_eq!(rollout.i64_field("desired_replicas"), desired);
    assert_eq!(rollout.i64_field("ready_replicas"), ready);
    Ok(())
}

fn assert_job(
    result: &ToolResult,
    name: &str,
    active: i64,
    succeeded: i64,
    failed: i64,
    condition: &str,
) {
    let job = result.item_named("jobs", name);
    assert_eq!(job["active"], active);
    assert_eq!(job["succeeded"], succeeded);
    assert_eq!(job["failed"], failed);

    if !condition.is_empty() {
        let conditions = job["conditions"].as_array().unwrap();
        assert!(
            conditions.iter().any(|item| item["type_"] == condition),
            "expected condition {condition} in {conditions:#?}"
        );
    }
}

fn assert_tool_error(result: ToolResult, expected_text: &str) {
    assert!(result.is_error(), "expected tool error, got {result:?}");
    assert!(
        result.content_text().contains(expected_text),
        "expected error text containing {expected_text:?}, got {:?}",
        result.content_text()
    );
}

trait ToolResultExt {
    fn array(&self, field: &str) -> &[Value];
    fn bool_field(&self, field: &str) -> bool;
    fn i64_field(&self, field: &str) -> i64;
    fn string_field(&self, field: &str) -> &str;
    fn nested_i64(&self, path: &[&str]) -> i64;
    fn nested_string(&self, path: &[&str]) -> &str;
    fn item_named(&self, array_field: &str, name: &str) -> &Value;
    fn count_with_prefix(&self, array_field: &str, prefix: &str) -> usize;
    fn count_by_field(&self, array_field: &str, field: &str, value: &str) -> usize;
    fn has_name_with_prefix(&self, array_field: &str, prefix: &str) -> bool;
    fn name_with_prefix(&self, array_field: &str, prefix: &str) -> &str;
}

impl ToolResultExt for ToolResult {
    fn array(&self, field: &str) -> &[Value] {
        self.structured()[field].as_array().unwrap_or_else(|| {
            panic!(
                "structuredContent.{field} is not an array: {:#?}",
                self.structured()
            )
        })
    }

    fn bool_field(&self, field: &str) -> bool {
        self.structured()[field].as_bool().unwrap_or_else(|| {
            panic!(
                "structuredContent.{field} is not a bool: {:#?}",
                self.structured()
            )
        })
    }

    fn i64_field(&self, field: &str) -> i64 {
        self.structured()[field].as_i64().unwrap_or_else(|| {
            panic!(
                "structuredContent.{field} is not an integer: {:#?}",
                self.structured()
            )
        })
    }

    fn string_field(&self, field: &str) -> &str {
        self.structured()[field].as_str().unwrap_or_else(|| {
            panic!(
                "structuredContent.{field} is not a string: {:#?}",
                self.structured()
            )
        })
    }

    fn nested_i64(&self, path: &[&str]) -> i64 {
        self.nested(path).as_i64().unwrap_or_else(|| {
            panic!(
                "structuredContent.{} is not an integer: {:#?}",
                path.join("."),
                self.structured()
            )
        })
    }

    fn nested_string(&self, path: &[&str]) -> &str {
        self.nested(path).as_str().unwrap_or_else(|| {
            panic!(
                "structuredContent.{} is not a string: {:#?}",
                path.join("."),
                self.structured()
            )
        })
    }

    fn item_named(&self, array_field: &str, name: &str) -> &Value {
        self.array(array_field)
            .iter()
            .find(|item| item["name"] == name || item["metadata"]["name"] == name)
            .unwrap_or_else(|| {
                panic!(
                    "{array_field} item {name} not found in {:#?}",
                    self.array(array_field)
                )
            })
    }

    fn count_with_prefix(&self, array_field: &str, prefix: &str) -> usize {
        self.array(array_field)
            .iter()
            .filter(|item| item["name"].as_str().unwrap().starts_with(prefix))
            .count()
    }

    fn count_by_field(&self, array_field: &str, field: &str, value: &str) -> usize {
        self.array(array_field)
            .iter()
            .filter(|item| item[field] == value)
            .count()
    }

    fn has_name_with_prefix(&self, array_field: &str, prefix: &str) -> bool {
        self.array(array_field)
            .iter()
            .any(|item| item["name"].as_str().unwrap().starts_with(prefix))
    }

    fn name_with_prefix(&self, array_field: &str, prefix: &str) -> &str {
        self.array(array_field)
            .iter()
            .find_map(|item| {
                item["name"]
                    .as_str()
                    .filter(|name| name.starts_with(prefix))
            })
            .unwrap_or_else(|| {
                panic!(
                    "{array_field} item with prefix {prefix} not found in {:#?}",
                    self.array(array_field)
                )
            })
    }
}

impl ToolResult {
    fn nested(&self, path: &[&str]) -> &Value {
        path.iter()
            .fold(self.structured(), |value, segment| &value[*segment])
    }
}
