use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use rmcp::ClientHandler;
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::model::ClientInfo;
use serde_json::json;

use super::*;

#[derive(Default)]
struct MockReader {
    list_events_input: Mutex<Option<ListEventsInput>>,
    list_pods_input: Mutex<Option<ListPodsInput>>,
    fail_namespaces: bool,
}

#[async_trait]
impl KubernetesReader for MockReader {
    async fn list_contexts(&self) -> Result<ContextsResponse, KubeviewError> {
        Ok(ContextsResponse {
            current: Some("dev".to_string()),
            selected: "dev".to_string(),
            contexts: vec!["dev".to_string()],
        })
    }

    async fn current_context(&self) -> Result<CurrentContextResponse, KubeviewError> {
        Ok(CurrentContextResponse {
            context: "dev".to_string(),
            namespace: "default".to_string(),
            cluster: Some("cluster".to_string()),
            user: Some("user".to_string()),
        })
    }

    async fn list_namespaces(&self) -> Result<NamespacesResponse, KubeviewError> {
        if self.fail_namespaces {
            return Err(KubeviewError::Kubernetes("forbidden".to_string()));
        }
        Ok(NamespacesResponse {
            namespaces: vec![NamespaceSummary {
                name: "default".to_string(),
                status: Some("Active".to_string()),
            }],
        })
    }

    async fn list_pods(&self, input: ListPodsInput) -> Result<PodsResponse, KubeviewError> {
        *self.list_pods_input.lock().expect("list_pods_input lock") = Some(input.clone());
        Ok(PodsResponse {
            namespace: input.namespace,
            pods: vec![],
            limit: input.limit.unwrap_or(200),
            continue_token: None,
            truncated: false,
        })
    }

    async fn get_pod(&self, _input: GetPodInput) -> Result<serde_json::Value, KubeviewError> {
        Ok(json!({"kind": "Pod"}))
    }

    async fn pod_logs(&self, input: PodLogsInput) -> Result<PodLogsResponse, KubeviewError> {
        Ok(PodLogsResponse {
            pod: input.pod,
            namespace: input.namespace.unwrap_or_else(|| "default".to_string()),
            container: input.container,
            tail_lines: input.tail_lines.unwrap_or(200),
            logs: "hello".to_string(),
        })
    }

    async fn list_resources(
        &self,
        input: ListResourcesInput,
    ) -> Result<ResourcesResponse, KubeviewError> {
        Ok(ResourcesResponse {
            api_version: input.api_version,
            kind: input.kind,
            namespace: input.namespace,
            items: vec![],
            limit: input.limit.unwrap_or(200),
            continue_token: None,
            truncated: false,
        })
    }

    async fn get_resource(
        &self,
        _input: GetResourceInput,
    ) -> Result<serde_json::Value, KubeviewError> {
        Ok(json!({"kind": "Deployment"}))
    }

    async fn list_events(&self, input: ListEventsInput) -> Result<EventsResponse, KubeviewError> {
        *self
            .list_events_input
            .lock()
            .expect("list_events_input lock") = Some(input.clone());
        Ok(EventsResponse {
            namespace: input.namespace,
            events: vec![],
        })
    }

    async fn get_rollout_status(
        &self,
        input: GetRolloutStatusInput,
    ) -> Result<RolloutStatusResponse, KubeviewError> {
        Ok(RolloutStatusResponse {
            kind: input.kind.as_str().to_string(),
            name: input.name,
            namespace: input.namespace.unwrap_or_else(|| "default".to_string()),
            complete: true,
            message: "rollout complete".to_string(),
            desired_replicas: 1,
            updated_replicas: 1,
            ready_replicas: 1,
            available_replicas: 1,
            unavailable_replicas: 0,
            observed_generation: Some(1),
            generation: Some(1),
            conditions: vec![],
        })
    }

    async fn wait_rollout(
        &self,
        input: WaitRolloutInput,
    ) -> Result<WaitRolloutResponse, KubeviewError> {
        let status = self
            .get_rollout_status(GetRolloutStatusInput {
                kind: input.kind,
                name: input.name,
                namespace: input.namespace,
            })
            .await?;
        Ok(WaitRolloutResponse {
            completed: status.complete,
            timed_out: false,
            elapsed_seconds: 0,
            observations: vec![],
            status,
        })
    }

    async fn trace_service(
        &self,
        input: TraceServiceInput,
    ) -> Result<TraceServiceResponse, KubeviewError> {
        Ok(TraceServiceResponse {
            namespace: input.namespace.unwrap_or_else(|| "default".to_string()),
            service: ServiceTraceSummary {
                name: input.name,
                type_: Some("ClusterIP".to_string()),
                cluster_ip: Some("10.0.0.1".to_string()),
                selector: None,
                ports: vec![],
            },
            endpoint_slices: vec![],
            endpoints: EndpointTotals {
                total: 0,
                ready: 0,
                serving: 0,
                terminating: 0,
            },
            selected_pods: vec![],
            warnings: vec![],
        })
    }

    async fn list_jobs(&self, input: ListJobsInput) -> Result<JobsResponse, KubeviewError> {
        Ok(JobsResponse {
            namespace: input.namespace,
            jobs: vec![],
        })
    }

    async fn list_cronjobs(
        &self,
        input: ListCronJobsInput,
    ) -> Result<CronJobsResponse, KubeviewError> {
        Ok(CronJobsResponse {
            namespace: input.namespace,
            cronjobs: vec![],
        })
    }
}

struct DummyClientHandler;

impl ClientHandler for DummyClientHandler {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

#[tokio::test]
async fn tools_list_exposes_minimal_core_tools() -> anyhow::Result<()> {
    let reader = Arc::new(MockReader::default());
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn(async move {
        KubeTools::new(reader)
            .serve(server_transport)
            .await?
            .waiting()
            .await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;
    let tools = client.list_tools(None).await?.tools;
    let names = tools
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect::<Vec<_>>();
    let mut expected = vec![
        "list_contexts",
        "current_context",
        "list_namespaces",
        "list_pods",
        "get_pod",
        "pod_logs",
        "list_resources",
        "get_resource",
        "list_events",
        "get_rollout_status",
        "wait_rollout",
        "trace_service",
        "list_jobs",
        "list_cronjobs",
    ];
    expected.sort_unstable();

    assert_eq!(names, expected);

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn list_pods_maps_optional_arguments() -> anyhow::Result<()> {
    let reader = Arc::new(MockReader::default());
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn({
        let reader = reader.clone();
        async move {
            KubeTools::new(reader)
                .serve(server_transport)
                .await?
                .waiting()
                .await?;
            anyhow::Ok(())
        }
    });

    let client = DummyClientHandler.serve(client_transport).await?;
    let response = client
        .call_tool(
            CallToolRequestParams::new("list_pods").with_arguments(
                json!({
                    "namespace": "prod",
                    "all_namespaces": true,
                    "label_selector": "app=web",
                    "field_selector": "status.phase=Running",
                    "limit": 50,
                    "continue_token": "next-page"
                })
                .as_object()
                .expect("list_pods arguments object")
                .clone(),
            ),
        )
        .await?;

    assert_eq!(response.is_error, Some(false));
    assert_eq!(
        *reader.list_pods_input.lock().expect("list_pods_input lock"),
        Some(ListPodsInput {
            namespace: Some("prod".to_string()),
            all_namespaces: true,
            label_selector: Some("app=web".to_string()),
            field_selector: Some("status.phase=Running".to_string()),
            limit: Some(50),
            continue_token: Some("next-page".to_string()),
        })
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn list_events_maps_optional_arguments() -> anyhow::Result<()> {
    let reader = Arc::new(MockReader::default());
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn({
        let reader = reader.clone();
        async move {
            KubeTools::new(reader)
                .serve(server_transport)
                .await?
                .waiting()
                .await?;
            anyhow::Ok(())
        }
    });

    let client = DummyClientHandler.serve(client_transport).await?;
    let response = client
        .call_tool(
            CallToolRequestParams::new("list_events").with_arguments(
                json!({
                    "namespace": "prod",
                    "all_namespaces": true,
                    "involved_kind": "Pod",
                    "involved_name": "web-0",
                    "type_": "Warning",
                    "limit": 50
                })
                .as_object()
                .expect("list_events arguments object")
                .clone(),
            ),
        )
        .await?;

    assert_eq!(response.is_error, Some(false));
    assert_eq!(
        *reader
            .list_events_input
            .lock()
            .expect("list_events_input lock"),
        Some(ListEventsInput {
            namespace: Some("prod".to_string()),
            all_namespaces: true,
            involved_kind: Some("Pod".to_string()),
            involved_name: Some("web-0".to_string()),
            type_: Some("Warning".to_string()),
            limit: Some(50),
        })
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn reader_error_becomes_mcp_tool_error() -> anyhow::Result<()> {
    let reader = Arc::new(MockReader {
        fail_namespaces: true,
        ..MockReader::default()
    });
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn(async move {
        KubeTools::new(reader)
            .serve(server_transport)
            .await?
            .waiting()
            .await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;
    let response = client
        .call_tool(CallToolRequestParams::new("list_namespaces"))
        .await?;

    assert_eq!(response.is_error, Some(true));
    assert_eq!(
        response
            .content
            .first()
            .and_then(|content| content.raw.as_text())
            .map(|text| text.text.as_str()),
        Some("forbidden")
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn invalid_rollout_kind_becomes_mcp_tool_error() -> anyhow::Result<()> {
    let reader = Arc::new(MockReader::default());
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn(async move {
        KubeTools::new(reader)
            .serve(server_transport)
            .await?
            .waiting()
            .await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;
    let response = client
        .call_tool(
            CallToolRequestParams::new("get_rollout_status").with_arguments(
                json!({
                    "kind": "ReplicaSet",
                    "name": "web",
                    "namespace": "prod"
                })
                .as_object()
                .expect("get_rollout_status arguments object")
                .clone(),
            ),
        )
        .await?;

    assert_eq!(response.is_error, Some(true));
    assert!(
        response
            .content
            .first()
            .and_then(|content| content.raw.as_text())
            .map(|text| text.text.contains("unsupported rollout kind"))
            .unwrap_or(false)
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
