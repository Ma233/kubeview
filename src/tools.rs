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
