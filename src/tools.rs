use std::sync::Arc;

use async_trait::async_trait;
use poem_mcpserver::Tools;
use poem_mcpserver::tool::StructuredContent;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::error::KubeviewError;

#[async_trait]
pub trait KubernetesReader: Send + Sync + 'static {
    async fn list_contexts(&self) -> Result<ContextsResponse, KubeviewError>;

    async fn current_context(&self) -> Result<CurrentContextResponse, KubeviewError>;

    async fn list_namespaces(&self) -> Result<NamespacesResponse, KubeviewError>;

    async fn list_pods(&self, input: ListPodsInput) -> Result<PodsResponse, KubeviewError>;

    async fn get_pod(&self, input: GetPodInput) -> Result<serde_json::Value, KubeviewError>;

    async fn pod_logs(&self, input: PodLogsInput) -> Result<PodLogsResponse, KubeviewError>;

    async fn list_resources(
        &self,
        input: ListResourcesInput,
    ) -> Result<ResourcesResponse, KubeviewError>;

    async fn get_resource(
        &self,
        input: GetResourceInput,
    ) -> Result<serde_json::Value, KubeviewError>;

    async fn list_events(&self, input: ListEventsInput) -> Result<EventsResponse, KubeviewError>;

    async fn get_rollout_status(
        &self,
        input: GetRolloutStatusInput,
    ) -> Result<RolloutStatusResponse, KubeviewError>;

    async fn wait_rollout(
        &self,
        input: WaitRolloutInput,
    ) -> Result<WaitRolloutResponse, KubeviewError>;

    async fn trace_service(
        &self,
        input: TraceServiceInput,
    ) -> Result<TraceServiceResponse, KubeviewError>;

    async fn list_jobs(&self, input: ListJobsInput) -> Result<JobsResponse, KubeviewError>;

    async fn list_cronjobs(
        &self,
        input: ListCronJobsInput,
    ) -> Result<CronJobsResponse, KubeviewError>;
}

#[derive(Clone)]
pub struct KubeTools {
    reader: Arc<dyn KubernetesReader>,
}

impl KubeTools {
    #[must_use]
    pub fn new(reader: Arc<dyn KubernetesReader>) -> Self {
        Self { reader }
    }
}

/// Read-only Kubernetes cluster inspection tools.
#[Tools]
impl KubeTools {
    /// List contexts available in the local kubeconfig.
    async fn list_contexts(&self) -> Result<StructuredContent<ContextsResponse>, KubeviewError> {
        self.reader.list_contexts().await.map(StructuredContent)
    }

    /// Show the Kubernetes context fixed for this MCP server.
    async fn current_context(
        &self,
    ) -> Result<StructuredContent<CurrentContextResponse>, KubeviewError> {
        self.reader.current_context().await.map(StructuredContent)
    }

    /// List namespaces in the current cluster.
    async fn list_namespaces(
        &self,
    ) -> Result<StructuredContent<NamespacesResponse>, KubeviewError> {
        self.reader.list_namespaces().await.map(StructuredContent)
    }

    /// List pods.
    async fn list_pods(
        &self,
        namespace: Option<String>,
        all_namespaces: Option<bool>,
        label_selector: Option<String>,
        field_selector: Option<String>,
    ) -> Result<StructuredContent<PodsResponse>, KubeviewError> {
        self.reader
            .list_pods(ListPodsInput {
                namespace,
                all_namespaces: all_namespaces.unwrap_or(false),
                label_selector,
                field_selector,
            })
            .await
            .map(StructuredContent)
    }

    /// Get a single pod.
    async fn get_pod(
        &self,
        name: String,
        namespace: Option<String>,
    ) -> Result<StructuredContent<serde_json::Value>, KubeviewError> {
        self.reader
            .get_pod(GetPodInput { name, namespace })
            .await
            .map(StructuredContent)
    }

    /// Read pod logs.
    async fn pod_logs(
        &self,
        pod: String,
        namespace: Option<String>,
        container: Option<String>,
        tail_lines: Option<u32>,
    ) -> Result<StructuredContent<PodLogsResponse>, KubeviewError> {
        self.reader
            .pod_logs(PodLogsInput {
                pod,
                namespace,
                container,
                tail_lines,
            })
            .await
            .map(StructuredContent)
    }

    /// List resources by apiVersion and kind.
    async fn list_resources(
        &self,
        api_version: String,
        kind: String,
        namespace: Option<String>,
        all_namespaces: Option<bool>,
        label_selector: Option<String>,
        field_selector: Option<String>,
    ) -> Result<StructuredContent<ResourcesResponse>, KubeviewError> {
        self.reader
            .list_resources(ListResourcesInput {
                api_version,
                kind,
                namespace,
                all_namespaces: all_namespaces.unwrap_or(false),
                label_selector,
                field_selector,
            })
            .await
            .map(StructuredContent)
    }

    /// Get a resource by apiVersion, kind, name, and optional namespace.
    async fn get_resource(
        &self,
        api_version: String,
        kind: String,
        name: String,
        namespace: Option<String>,
    ) -> Result<StructuredContent<serde_json::Value>, KubeviewError> {
        self.reader
            .get_resource(GetResourceInput {
                api_version,
                kind,
                name,
                namespace,
            })
            .await
            .map(StructuredContent)
    }

    /// List Kubernetes events for rollout and runtime diagnosis.
    async fn list_events(
        &self,
        namespace: Option<String>,
        all_namespaces: Option<bool>,
        involved_kind: Option<String>,
        involved_name: Option<String>,
        type_: Option<String>,
    ) -> Result<StructuredContent<EventsResponse>, KubeviewError> {
        self.reader
            .list_events(ListEventsInput {
                namespace,
                all_namespaces: all_namespaces.unwrap_or(false),
                involved_kind,
                involved_name,
                type_,
            })
            .await
            .map(StructuredContent)
    }

    /// Get rollout status for a Deployment, StatefulSet, or DaemonSet.
    async fn get_rollout_status(
        &self,
        kind: String,
        name: String,
        namespace: Option<String>,
    ) -> Result<StructuredContent<RolloutStatusResponse>, KubeviewError> {
        self.reader
            .get_rollout_status(GetRolloutStatusInput {
                kind,
                name,
                namespace,
            })
            .await
            .map(StructuredContent)
    }

    /// Wait for a Deployment, StatefulSet, or DaemonSet rollout to complete.
    async fn wait_rollout(
        &self,
        kind: String,
        name: String,
        namespace: Option<String>,
        timeout_seconds: Option<u64>,
        interval_seconds: Option<u64>,
    ) -> Result<StructuredContent<WaitRolloutResponse>, KubeviewError> {
        self.reader
            .wait_rollout(WaitRolloutInput {
                kind,
                name,
                namespace,
                timeout_seconds,
                interval_seconds,
            })
            .await
            .map(StructuredContent)
    }

    /// Trace a Service to EndpointSlices and selected Pods.
    async fn trace_service(
        &self,
        name: String,
        namespace: Option<String>,
    ) -> Result<StructuredContent<TraceServiceResponse>, KubeviewError> {
        self.reader
            .trace_service(TraceServiceInput { name, namespace })
            .await
            .map(StructuredContent)
    }

    /// List Jobs and their completion state.
    async fn list_jobs(
        &self,
        namespace: Option<String>,
        all_namespaces: Option<bool>,
        label_selector: Option<String>,
        field_selector: Option<String>,
    ) -> Result<StructuredContent<JobsResponse>, KubeviewError> {
        self.reader
            .list_jobs(ListJobsInput {
                namespace,
                all_namespaces: all_namespaces.unwrap_or(false),
                label_selector,
                field_selector,
            })
            .await
            .map(StructuredContent)
    }

    /// List CronJobs and their recent scheduling state.
    async fn list_cronjobs(
        &self,
        namespace: Option<String>,
        all_namespaces: Option<bool>,
        label_selector: Option<String>,
        field_selector: Option<String>,
    ) -> Result<StructuredContent<CronJobsResponse>, KubeviewError> {
        self.reader
            .list_cronjobs(ListCronJobsInput {
                namespace,
                all_namespaces: all_namespaces.unwrap_or(false),
                label_selector,
                field_selector,
            })
            .await
            .map(StructuredContent)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPodsInput {
    pub namespace: Option<String>,
    pub all_namespaces: bool,
    pub label_selector: Option<String>,
    pub field_selector: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetPodInput {
    pub name: String,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodLogsInput {
    pub pod: String,
    pub namespace: Option<String>,
    pub container: Option<String>,
    pub tail_lines: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListResourcesInput {
    pub api_version: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub all_namespaces: bool,
    pub label_selector: Option<String>,
    pub field_selector: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetResourceInput {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEventsInput {
    pub namespace: Option<String>,
    pub all_namespaces: bool,
    pub involved_kind: Option<String>,
    pub involved_name: Option<String>,
    pub type_: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRolloutStatusInput {
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitRolloutInput {
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub timeout_seconds: Option<u64>,
    pub interval_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceServiceInput {
    pub name: String,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListJobsInput {
    pub namespace: Option<String>,
    pub all_namespaces: bool,
    pub label_selector: Option<String>,
    pub field_selector: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListCronJobsInput {
    pub namespace: Option<String>,
    pub all_namespaces: bool,
    pub label_selector: Option<String>,
    pub field_selector: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContextsResponse {
    pub current: Option<String>,
    pub selected: String,
    pub contexts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CurrentContextResponse {
    pub context: String,
    pub namespace: String,
    pub cluster: Option<String>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NamespacesResponse {
    pub namespaces: Vec<NamespaceSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NamespaceSummary {
    pub name: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PodsResponse {
    pub namespace: Option<String>,
    pub pods: Vec<PodSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PodSummary {
    pub name: String,
    pub namespace: Option<String>,
    pub phase: Option<String>,
    pub node_name: Option<String>,
    pub pod_ip: Option<String>,
    pub host_ip: Option<String>,
    pub restart_count: i32,
    pub containers_ready: String,
    pub age: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PodLogsResponse {
    pub pod: String,
    pub namespace: String,
    pub container: Option<String>,
    pub tail_lines: u32,
    pub logs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResourcesResponse {
    pub api_version: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub items: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EventsResponse {
    pub namespace: Option<String>,
    pub events: Vec<EventSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EventSummary {
    pub name: String,
    pub namespace: Option<String>,
    pub type_: Option<String>,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub count: Option<i32>,
    pub first_timestamp: Option<String>,
    pub last_timestamp: Option<String>,
    pub involved_kind: Option<String>,
    pub involved_name: Option<String>,
    pub reporting_component: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RolloutStatusResponse {
    pub kind: String,
    pub name: String,
    pub namespace: String,
    pub complete: bool,
    pub message: String,
    pub desired_replicas: i32,
    pub updated_replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub unavailable_replicas: i32,
    pub observed_generation: Option<i64>,
    pub generation: Option<i64>,
    pub conditions: Vec<ConditionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConditionSummary {
    pub type_: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WaitRolloutResponse {
    pub status: RolloutStatusResponse,
    pub completed: bool,
    pub timed_out: bool,
    pub elapsed_seconds: u64,
    pub observations: Vec<RolloutObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RolloutObservation {
    pub elapsed_seconds: u64,
    pub complete: bool,
    pub message: String,
    pub updated_replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TraceServiceResponse {
    pub namespace: String,
    pub service: ServiceTraceSummary,
    pub endpoint_slices: Vec<EndpointSliceSummary>,
    pub endpoints: EndpointTotals,
    pub selected_pods: Vec<PodSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceTraceSummary {
    pub name: String,
    pub type_: Option<String>,
    pub cluster_ip: Option<String>,
    pub selector: Option<std::collections::BTreeMap<String, String>>,
    pub ports: Vec<ServicePortSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServicePortSummary {
    pub name: Option<String>,
    pub port: i32,
    pub target_port: Option<String>,
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EndpointSliceSummary {
    pub name: String,
    pub address_type: String,
    pub endpoints: Vec<EndpointSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EndpointSummary {
    pub addresses: Vec<String>,
    pub ready: bool,
    pub serving: bool,
    pub terminating: bool,
    pub target_kind: Option<String>,
    pub target_name: Option<String>,
    pub node_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EndpointTotals {
    pub total: usize,
    pub ready: usize,
    pub serving: usize,
    pub terminating: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobsResponse {
    pub namespace: Option<String>,
    pub jobs: Vec<JobSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobSummary {
    pub name: String,
    pub namespace: Option<String>,
    pub active: i32,
    pub ready: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub terminating: i32,
    pub completions: Option<i32>,
    pub parallelism: Option<i32>,
    pub suspend: Option<bool>,
    pub completion_time: Option<String>,
    pub start_time: Option<String>,
    pub conditions: Vec<ConditionSummary>,
    pub owner_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CronJobsResponse {
    pub namespace: Option<String>,
    pub cronjobs: Vec<CronJobSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CronJobSummary {
    pub name: String,
    pub namespace: Option<String>,
    pub schedule: String,
    pub suspend: Option<bool>,
    pub active: usize,
    pub active_jobs: Vec<String>,
    pub last_schedule_time: Option<String>,
    pub last_successful_time: Option<String>,
    pub concurrency_policy: Option<String>,
    pub time_zone: Option<String>,
}

#[cfg(test)]
mod tests {
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
            })
        }

        async fn get_resource(
            &self,
            _input: GetResourceInput,
        ) -> Result<serde_json::Value, KubeviewError> {
            Ok(json!({"kind": "Deployment"}))
        }

        async fn list_events(
            &self,
            input: ListEventsInput,
        ) -> Result<EventsResponse, KubeviewError> {
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
                kind: input.kind,
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
                            "field_selector": "status.phase=Running"
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
                id: Some(RequestId::Int(3)),
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
}
