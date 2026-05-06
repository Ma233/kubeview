use std::sync::Arc;

use async_trait::async_trait;
use poem_mcpserver::Tools;
use poem_mcpserver::tool::StructuredContent;

use crate::error::KubeviewError;
pub use crate::models::*;

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
        limit: Option<u32>,
    ) -> Result<StructuredContent<EventsResponse>, KubeviewError> {
        self.reader
            .list_events(ListEventsInput {
                namespace,
                all_namespaces: all_namespaces.unwrap_or(false),
                involved_kind,
                involved_name,
                type_,
                limit,
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
                kind: parse_rollout_kind(&kind)?,
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
                kind: parse_rollout_kind(&kind)?,
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
        limit: Option<u32>,
    ) -> Result<StructuredContent<JobsResponse>, KubeviewError> {
        self.reader
            .list_jobs(ListJobsInput {
                namespace,
                all_namespaces: all_namespaces.unwrap_or(false),
                label_selector,
                field_selector,
                limit,
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
        limit: Option<u32>,
    ) -> Result<StructuredContent<CronJobsResponse>, KubeviewError> {
        self.reader
            .list_cronjobs(ListCronJobsInput {
                namespace,
                all_namespaces: all_namespaces.unwrap_or(false),
                label_selector,
                field_selector,
                limit,
            })
            .await
            .map(StructuredContent)
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
