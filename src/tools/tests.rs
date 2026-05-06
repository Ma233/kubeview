use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use poem_mcpserver::McpServer;
use poem_mcpserver::protocol::JSON_RPC_VERSION;
use poem_mcpserver::protocol::rpc::Request;
use poem_mcpserver::protocol::rpc::RequestId;
use poem_mcpserver::protocol::rpc::Requests;
use poem_mcpserver::protocol::tool::ToolsCallRequest;
use poem_mcpserver::protocol::tool::ToolsListRequest;
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
        *self.list_pods_input.lock().unwrap() = Some(input.clone());
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
        *self.list_events_input.lock().unwrap() = Some(input.clone());
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

#[tokio::test]
async fn tools_list_exposes_minimal_core_tools() {
    let reader = Arc::new(MockReader::default());
    let mut server = McpServer::new().tools(KubeTools::new(reader));

    let response = server
        .handle_request(Request {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: Some(RequestId::Int(1)),
            body: Requests::ToolsList {
                params: ToolsListRequest { cursor: None },
            },
        })
        .await;
    let value = serde_json::to_value(response).unwrap();
    let tools = value["result"]["tools"].as_array().unwrap();
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, vec![
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
    ]);
}

#[tokio::test]
async fn list_pods_maps_optional_arguments() {
    let reader = Arc::new(MockReader::default());
    let mut server = McpServer::new().tools(KubeTools::new(reader.clone()));

    let response = server
        .handle_request(Request {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: Some(RequestId::Int(2)),
            body: Requests::ToolsCall {
                params: ToolsCallRequest {
                    name: "list_pods".to_string(),
                    arguments: json!({
                        "namespace": "prod",
                        "all_namespaces": true,
                        "label_selector": "app=web",
                        "field_selector": "status.phase=Running",
                        "limit": 50,
                        "continue_token": "next-page"
                    }),
                },
            },
        })
        .await;

    let value = serde_json::to_value(response).unwrap();
    assert_eq!(value["result"]["isError"], false);
    assert_eq!(
        *reader.list_pods_input.lock().unwrap(),
        Some(ListPodsInput {
            namespace: Some("prod".to_string()),
            all_namespaces: true,
            label_selector: Some("app=web".to_string()),
            field_selector: Some("status.phase=Running".to_string()),
            limit: Some(50),
            continue_token: Some("next-page".to_string()),
        })
    );
}

#[tokio::test]
async fn list_events_maps_optional_arguments() {
    let reader = Arc::new(MockReader::default());
    let mut server = McpServer::new().tools(KubeTools::new(reader.clone()));

    let response = server
        .handle_request(Request {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: Some(RequestId::Int(3)),
            body: Requests::ToolsCall {
                params: ToolsCallRequest {
                    name: "list_events".to_string(),
                    arguments: json!({
                        "namespace": "prod",
                        "all_namespaces": true,
                        "involved_kind": "Pod",
                        "involved_name": "web-0",
                        "type_": "Warning",
                        "limit": 50
                    }),
                },
            },
        })
        .await;

    let value = serde_json::to_value(response).unwrap();
    assert_eq!(value["result"]["isError"], false);
    assert_eq!(
        *reader.list_events_input.lock().unwrap(),
        Some(ListEventsInput {
            namespace: Some("prod".to_string()),
            all_namespaces: true,
            involved_kind: Some("Pod".to_string()),
            involved_name: Some("web-0".to_string()),
            type_: Some("Warning".to_string()),
            limit: Some(50),
        })
    );
}

#[tokio::test]
async fn reader_error_becomes_mcp_tool_error() {
    let reader = Arc::new(MockReader {
        fail_namespaces: true,
        ..MockReader::default()
    });
    let mut server = McpServer::new().tools(KubeTools::new(reader));

    let response = server
        .handle_request(Request {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: Some(RequestId::Int(4)),
            body: Requests::ToolsCall {
                params: ToolsCallRequest {
                    name: "list_namespaces".to_string(),
                    arguments: json!({}),
                },
            },
        })
        .await;

    let value = serde_json::to_value(response).unwrap();
    assert_eq!(value["result"]["isError"], true);
    assert_eq!(value["result"]["content"][0]["text"], "forbidden");
}

#[tokio::test]
async fn invalid_rollout_kind_becomes_mcp_tool_error() {
    let reader = Arc::new(MockReader::default());
    let mut server = McpServer::new().tools(KubeTools::new(reader));

    let response = server
        .handle_request(Request {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id: Some(RequestId::Int(5)),
            body: Requests::ToolsCall {
                params: ToolsCallRequest {
                    name: "get_rollout_status".to_string(),
                    arguments: json!({
                        "kind": "ReplicaSet",
                        "name": "web",
                        "namespace": "prod"
                    }),
                },
            },
        })
        .await;

    let value = serde_json::to_value(response).unwrap();
    assert_eq!(value["result"]["isError"], true);
    assert!(
        value["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unsupported rollout kind")
    );
}
