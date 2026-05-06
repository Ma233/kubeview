use std::sync::Arc;

use async_trait::async_trait;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Json;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::schemars::JsonSchema;
use rmcp::tool;
use rmcp::tool_handler;
use rmcp::tool_router;
use serde::Deserialize;

use crate::error::KubeviewError;
pub use crate::models::*;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct GetRolloutStatusRequest {
    kind: String,
    name: String,
    namespace: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct WaitRolloutRequest {
    kind: String,
    name: String,
    namespace: Option<String>,
    timeout_seconds: Option<u64>,
    interval_seconds: Option<u64>,
}

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
    tool_router: ToolRouter<Self>,
}

impl KubeTools {
    #[must_use]
    pub fn new(reader: Arc<dyn KubernetesReader>) -> Self {
        Self {
            reader,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for KubeTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions("Read-only Kubernetes cluster inspection tools.")
    }
}

/// Read-only Kubernetes cluster inspection tools.
#[tool_router(router = tool_router)]
impl KubeTools {
    /// List contexts available in the local kubeconfig.
    #[tool(
        name = "list_contexts",
        description = "List contexts available in the local kubeconfig."
    )]
    async fn list_contexts(&self) -> Result<Json<ContextsResponse>, KubeviewError> {
        self.reader.list_contexts().await.map(Json)
    }

    /// Show the Kubernetes context fixed for this MCP server.
    #[tool(
        name = "current_context",
        description = "Show the Kubernetes context fixed for this MCP server."
    )]
    async fn current_context(&self) -> Result<Json<CurrentContextResponse>, KubeviewError> {
        self.reader.current_context().await.map(Json)
    }

    /// List namespaces in the current cluster.
    #[tool(
        name = "list_namespaces",
        description = "List namespaces in the current cluster."
    )]
    async fn list_namespaces(&self) -> Result<Json<NamespacesResponse>, KubeviewError> {
        self.reader.list_namespaces().await.map(Json)
    }

    /// List pods.
    #[tool(name = "list_pods", description = "List pods.")]
    async fn list_pods(
        &self,
        Parameters(input): Parameters<ListPodsInput>,
    ) -> Result<Json<PodsResponse>, KubeviewError> {
        self.reader.list_pods(input).await.map(Json)
    }

    /// Get a single pod.
    #[tool(name = "get_pod", description = "Get a single pod.")]
    async fn get_pod(
        &self,
        Parameters(input): Parameters<GetPodInput>,
    ) -> Result<CallToolResult, KubeviewError> {
        self.reader
            .get_pod(input)
            .await
            .map(CallToolResult::structured)
    }

    /// Read pod logs.
    #[tool(name = "pod_logs", description = "Read pod logs.")]
    async fn pod_logs(
        &self,
        Parameters(input): Parameters<PodLogsInput>,
    ) -> Result<Json<PodLogsResponse>, KubeviewError> {
        self.reader.pod_logs(input).await.map(Json)
    }

    /// List resources by apiVersion and kind.
    #[tool(
        name = "list_resources",
        description = "List resources by apiVersion and kind."
    )]
    async fn list_resources(
        &self,
        Parameters(input): Parameters<ListResourcesInput>,
    ) -> Result<Json<ResourcesResponse>, KubeviewError> {
        self.reader.list_resources(input).await.map(Json)
    }

    /// Get a resource by apiVersion, kind, name, and optional namespace.
    #[tool(
        name = "get_resource",
        description = "Get a resource by apiVersion, kind, name, and optional namespace."
    )]
    async fn get_resource(
        &self,
        Parameters(input): Parameters<GetResourceInput>,
    ) -> Result<CallToolResult, KubeviewError> {
        self.reader
            .get_resource(input)
            .await
            .map(CallToolResult::structured)
    }

    /// List Kubernetes events for rollout and runtime diagnosis.
    #[tool(
        name = "list_events",
        description = "List Kubernetes events for rollout and runtime diagnosis."
    )]
    async fn list_events(
        &self,
        Parameters(input): Parameters<ListEventsInput>,
    ) -> Result<Json<EventsResponse>, KubeviewError> {
        self.reader.list_events(input).await.map(Json)
    }

    /// Get rollout status for a Deployment, StatefulSet, or DaemonSet.
    #[tool(
        name = "get_rollout_status",
        description = "Get rollout status for a Deployment, StatefulSet, or DaemonSet."
    )]
    async fn get_rollout_status(
        &self,
        Parameters(request): Parameters<GetRolloutStatusRequest>,
    ) -> Result<Json<RolloutStatusResponse>, KubeviewError> {
        self.reader
            .get_rollout_status(GetRolloutStatusInput {
                kind: parse_rollout_kind(&request.kind)?,
                name: request.name,
                namespace: request.namespace,
            })
            .await
            .map(Json)
    }

    /// Wait for a Deployment, StatefulSet, or DaemonSet rollout to complete.
    #[tool(
        name = "wait_rollout",
        description = "Wait for a Deployment, StatefulSet, or DaemonSet rollout to complete."
    )]
    async fn wait_rollout(
        &self,
        Parameters(request): Parameters<WaitRolloutRequest>,
    ) -> Result<Json<WaitRolloutResponse>, KubeviewError> {
        self.reader
            .wait_rollout(WaitRolloutInput {
                kind: parse_rollout_kind(&request.kind)?,
                name: request.name,
                namespace: request.namespace,
                timeout_seconds: request.timeout_seconds,
                interval_seconds: request.interval_seconds,
            })
            .await
            .map(Json)
    }

    /// Trace a Service to EndpointSlices and selected Pods.
    #[tool(
        name = "trace_service",
        description = "Trace a Service to EndpointSlices and selected Pods."
    )]
    async fn trace_service(
        &self,
        Parameters(input): Parameters<TraceServiceInput>,
    ) -> Result<Json<TraceServiceResponse>, KubeviewError> {
        self.reader.trace_service(input).await.map(Json)
    }

    /// List Jobs and their completion state.
    #[tool(
        name = "list_jobs",
        description = "List Jobs and their completion state."
    )]
    async fn list_jobs(
        &self,
        Parameters(input): Parameters<ListJobsInput>,
    ) -> Result<Json<JobsResponse>, KubeviewError> {
        self.reader.list_jobs(input).await.map(Json)
    }

    /// List CronJobs and their recent scheduling state.
    #[tool(
        name = "list_cronjobs",
        description = "List CronJobs and their recent scheduling state."
    )]
    async fn list_cronjobs(
        &self,
        Parameters(input): Parameters<ListCronJobsInput>,
    ) -> Result<Json<CronJobsResponse>, KubeviewError> {
        self.reader.list_cronjobs(input).await.map(Json)
    }
}

fn parse_rollout_kind(kind: &str) -> Result<RolloutKind, KubeviewError> {
    RolloutKind::parse(kind).ok_or_else(|| {
        KubeviewError::InvalidInput(format!(
            "unsupported rollout kind '{kind}', expected Deployment, StatefulSet, or DaemonSet"
        ))
    })
}

#[cfg(test)]
mod tests;
