use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use k8s_openapi::api::core::v1::Namespace;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::jiff::Timestamp;
use kube::Api;
use kube::Client;
use kube::ResourceExt;
use kube::api::DynamicObject;
use kube::api::ListParams;
use kube::api::LogParams;
use kube::config::Config;
use kube::config::KubeConfigOptions;
use kube::config::Kubeconfig;
use kube::core::GroupVersionKind;
use kube::discovery::ApiCapabilities;
use kube::discovery::Scope;
use kube::discovery::{self};

use crate::error::KubeviewError;
use crate::observability;
use crate::tools::ContextsResponse;
use crate::tools::CronJobsResponse;
use crate::tools::CurrentContextResponse;
use crate::tools::EventsResponse;
use crate::tools::GetPodInput;
use crate::tools::GetResourceInput;
use crate::tools::GetRolloutStatusInput;
use crate::tools::JobsResponse;
use crate::tools::KubernetesReader;
use crate::tools::ListCronJobsInput;
use crate::tools::ListEventsInput;
use crate::tools::ListJobsInput;
use crate::tools::ListPodsInput;
use crate::tools::ListResourcesInput;
use crate::tools::NamespaceSummary;
use crate::tools::NamespacesResponse;
use crate::tools::PodLogsInput;
use crate::tools::PodLogsResponse;
use crate::tools::PodSummary;
use crate::tools::PodsResponse;
use crate::tools::ResourcesResponse;
use crate::tools::RolloutStatusResponse;
use crate::tools::TraceServiceInput;
use crate::tools::TraceServiceResponse;
use crate::tools::WaitRolloutInput;
use crate::tools::WaitRolloutResponse;

const DEFAULT_LOG_TAIL_LINES: u32 = 200;
const MAX_LOG_TAIL_LINES: u32 = 5_000;
const LAST_APPLIED_CONFIGURATION_ANNOTATION: &str =
    "kubectl.kubernetes.io/last-applied-configuration";

#[derive(Debug, Clone)]
pub struct KubernetesConfig {
    pub kubeconfig: Option<PathBuf>,
    pub context: Option<String>,
    pub namespace: Option<String>,
}

#[derive(Clone)]
pub struct KubeClientReader {
    client: Client,
    kubeconfig: Arc<Kubeconfig>,
    selected_context: String,
    namespace: String,
    namespace_scope: Option<String>,
}

impl KubeClientReader {
    pub async fn new(config: KubernetesConfig) -> Result<Self, KubeviewError> {
        let kubeconfig = match &config.kubeconfig {
            Some(path) => Kubeconfig::read_from(path).map_err(|error| {
                KubeviewError::Config(format!("read kubeconfig '{}': {error}", path.display()))
            })?,
            None => Kubeconfig::read()
                .map_err(|error| KubeviewError::Config(format!("read kubeconfig: {error}")))?,
        };
        let selected_context = config
            .context
            .clone()
            .or_else(|| kubeconfig.current_context.clone())
            .ok_or_else(|| KubeviewError::Config("current context is not set".to_string()))?;
        let options = KubeConfigOptions {
            context: Some(selected_context.clone()),
            cluster: None,
            user: None,
        };
        let mut kube_config = Config::from_custom_kubeconfig(kubeconfig.clone(), &options)
            .await
            .map_err(|error| {
                KubeviewError::Config(format!(
                    "build Kubernetes config for context '{selected_context}': {error}"
                ))
            })?;
        let namespace_scope = config.namespace.clone();
        if let Some(namespace) = namespace_scope.clone() {
            kube_config.default_namespace = namespace;
        }
        let namespace = kube_config.default_namespace.clone();
        let client = Client::try_from(kube_config)
            .map_err(|error| KubeviewError::Config(format!("create Kubernetes client: {error}")))?;

        Ok(Self {
            client,
            kubeconfig: Arc::new(kubeconfig),
            selected_context,
            namespace,
            namespace_scope,
        })
    }

    fn namespace_or_default(&self, namespace: Option<String>) -> Result<String, KubeviewError> {
        resolve_namespace(&self.namespace_scope, &self.namespace, namespace)
    }

    fn ensure_all_namespaces_allowed(&self, all_namespaces: bool) -> Result<(), KubeviewError> {
        ensure_all_namespaces_allowed(&self.namespace_scope, all_namespaces)
    }

    fn ensure_cluster_resource_allowed(
        &self,
        capabilities: &ApiCapabilities,
        kind: &str,
    ) -> Result<(), KubeviewError> {
        ensure_cluster_resource_allowed(&self.namespace_scope, capabilities, kind)
    }

    fn list_params(label_selector: Option<String>, field_selector: Option<String>) -> ListParams {
        let mut params = ListParams::default();
        if let Some(selector) = label_selector {
            params = params.labels(&selector);
        }
        if let Some(selector) = field_selector {
            params = params.fields(&selector);
        }
        params
    }

    async fn api_resource(
        &self,
        api_version: &str,
        kind: &str,
    ) -> Result<(kube::core::ApiResource, ApiCapabilities), KubeviewError> {
        let (group, version) = split_api_version(api_version)?;
        let gvk = GroupVersionKind::gvk(group, version, kind);
        discovery::pinned_kind(&self.client, &gvk)
            .await
            .map_err(|error| {
                KubeviewError::kubernetes_context(
                    format!("discover api resource {api_version}/{kind}"),
                    error,
                )
            })
    }
}

#[async_trait]
impl KubernetesReader for KubeClientReader {
    async fn list_contexts(&self) -> Result<ContextsResponse, KubeviewError> {
        Ok(ContextsResponse {
            current: self.kubeconfig.current_context.clone(),
            selected: self.selected_context.clone(),
            contexts: self
                .kubeconfig
                .contexts
                .iter()
                .map(|context| context.name.clone())
                .collect(),
        })
    }

    async fn current_context(&self) -> Result<CurrentContextResponse, KubeviewError> {
        let named_context = self
            .kubeconfig
            .contexts
            .iter()
            .find(|context| context.name == self.selected_context)
            .ok_or_else(|| {
                KubeviewError::Config(format!("context '{}' not found", self.selected_context))
            })?;
        let context = named_context.context.as_ref().ok_or_else(|| {
            KubeviewError::Config(format!("context '{}' is empty", self.selected_context))
        })?;

        Ok(CurrentContextResponse {
            context: self.selected_context.clone(),
            namespace: self.namespace.clone(),
            cluster: Some(context.cluster.clone()),
            user: context.user.clone(),
        })
    }

    async fn list_namespaces(&self) -> Result<NamespacesResponse, KubeviewError> {
        if let Some(scope) = &self.namespace_scope {
            return Ok(scoped_namespaces_response(scope));
        }

        let namespaces: Api<Namespace> = Api::all(self.client.clone());
        let list = namespaces
            .list(&ListParams::default())
            .await
            .map_err(|error| KubeviewError::kubernetes_context("list namespaces", error))?;
        Ok(NamespacesResponse {
            namespaces: list.items.into_iter().map(namespace_summary).collect(),
        })
    }

    async fn list_pods(&self, input: ListPodsInput) -> Result<PodsResponse, KubeviewError> {
        ensure_namespace_filter_not_conflicting(input.all_namespaces, input.namespace.as_deref())?;
        self.ensure_all_namespaces_allowed(input.all_namespaces)?;
        let params = Self::list_params(input.label_selector, input.field_selector);
        let namespace = if input.all_namespaces {
            None
        } else {
            Some(self.namespace_or_default(input.namespace.clone())?)
        };
        let pods: Api<Pod> = match &namespace {
            Some(namespace) => Api::namespaced(self.client.clone(), namespace),
            None => Api::all(self.client.clone()),
        };
        let list = pods
            .list(&params)
            .await
            .map_err(|error| KubeviewError::kubernetes_context("list pods", error))?;
        Ok(PodsResponse {
            namespace,
            pods: list.items.into_iter().map(pod_summary).collect(),
        })
    }

    async fn get_pod(&self, input: GetPodInput) -> Result<serde_json::Value, KubeviewError> {
        let namespace = self.namespace_or_default(input.namespace.clone())?;
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &namespace);
        let pod = pods
            .get(&input.name)
            .await
            .map_err(|error| KubeviewError::kubernetes_context("get pod", error))?;
        serde_json::to_value(pod).map_err(|error| KubeviewError::Kubernetes(error.to_string()))
    }

    async fn pod_logs(&self, input: PodLogsInput) -> Result<PodLogsResponse, KubeviewError> {
        let namespace = self.namespace_or_default(input.namespace.clone())?;
        let tail_lines = resolve_log_tail_lines(input.tail_lines)?;
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &namespace);
        let params = LogParams {
            container: input.container.clone(),
            tail_lines: Some(i64::from(tail_lines)),
            ..LogParams::default()
        };
        let logs = pods
            .logs(&input.pod, &params)
            .await
            .map_err(|error| KubeviewError::kubernetes_context("read pod logs", error))?;

        Ok(PodLogsResponse {
            pod: input.pod,
            namespace,
            container: input.container,
            tail_lines,
            logs,
        })
    }

    async fn list_resources(
        &self,
        input: ListResourcesInput,
    ) -> Result<ResourcesResponse, KubeviewError> {
        let (api_resource, capabilities) =
            self.api_resource(&input.api_version, &input.kind).await?;
        ensure_namespace_filter_not_conflicting(input.all_namespaces, input.namespace.as_deref())?;
        self.ensure_all_namespaces_allowed(input.all_namespaces)?;
        self.ensure_cluster_resource_allowed(&capabilities, &input.kind)?;
        ensure_cluster_resource_namespace_absent(
            &capabilities,
            &input.kind,
            input.namespace.as_deref(),
        )?;
        let params = Self::list_params(input.label_selector, input.field_selector);
        let namespace = if input.all_namespaces || capabilities.scope == Scope::Cluster {
            None
        } else {
            Some(self.namespace_or_default(input.namespace)?)
        };
        let api: Api<DynamicObject> = match &namespace {
            Some(namespace) => Api::namespaced_with(self.client.clone(), namespace, &api_resource),
            None => Api::all_with(self.client.clone(), &api_resource),
        };
        let list = api
            .list(&params)
            .await
            .map_err(|error| KubeviewError::kubernetes_context("list resources", error))?;
        let items = list
            .items
            .into_iter()
            .map(|resource| serialize_dynamic_resource(resource, &api_resource))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| KubeviewError::Kubernetes(error.to_string()))?;

        Ok(ResourcesResponse {
            api_version: input.api_version,
            kind: input.kind,
            namespace,
            items,
        })
    }

    async fn get_resource(
        &self,
        input: GetResourceInput,
    ) -> Result<serde_json::Value, KubeviewError> {
        let (api_resource, capabilities) =
            self.api_resource(&input.api_version, &input.kind).await?;
        self.ensure_cluster_resource_allowed(&capabilities, &input.kind)?;
        ensure_cluster_resource_namespace_absent(
            &capabilities,
            &input.kind,
            input.namespace.as_deref(),
        )?;
        let namespace = if capabilities.scope == Scope::Cluster {
            None
        } else {
            Some(self.namespace_or_default(input.namespace)?)
        };
        let api: Api<DynamicObject> = match &namespace {
            Some(namespace) => Api::namespaced_with(self.client.clone(), namespace, &api_resource),
            None => Api::all_with(self.client.clone(), &api_resource),
        };
        let resource = api
            .get(&input.name)
            .await
            .map_err(|error| KubeviewError::kubernetes_context("get resource", error))?;
        serialize_dynamic_resource(resource, &api_resource)
            .map_err(|error| KubeviewError::Kubernetes(error.to_string()))
    }

    async fn list_events(&self, input: ListEventsInput) -> Result<EventsResponse, KubeviewError> {
        ensure_namespace_filter_not_conflicting(input.all_namespaces, input.namespace.as_deref())?;
        self.ensure_all_namespaces_allowed(input.all_namespaces)?;
        let namespace = if input.all_namespaces {
            None
        } else {
            Some(self.namespace_or_default(input.namespace.clone())?)
        };
        observability::list_events(&self.client, namespace, input).await
    }

    async fn get_rollout_status(
        &self,
        input: GetRolloutStatusInput,
    ) -> Result<RolloutStatusResponse, KubeviewError> {
        let namespace = self.namespace_or_default(input.namespace.clone())?;
        observability::get_rollout_status(&self.client, namespace, input).await
    }

    async fn wait_rollout(
        &self,
        input: WaitRolloutInput,
    ) -> Result<WaitRolloutResponse, KubeviewError> {
        let namespace = self.namespace_or_default(input.namespace.clone())?;
        observability::wait_rollout(&self.client, namespace, input).await
    }

    async fn trace_service(
        &self,
        input: TraceServiceInput,
    ) -> Result<TraceServiceResponse, KubeviewError> {
        let namespace = self.namespace_or_default(input.namespace.clone())?;
        observability::trace_service(&self.client, namespace, input).await
    }

    async fn list_jobs(&self, input: ListJobsInput) -> Result<JobsResponse, KubeviewError> {
        ensure_namespace_filter_not_conflicting(input.all_namespaces, input.namespace.as_deref())?;
        self.ensure_all_namespaces_allowed(input.all_namespaces)?;
        let params = Self::list_params(input.label_selector.clone(), input.field_selector.clone());
        let namespace = if input.all_namespaces {
            None
        } else {
            Some(self.namespace_or_default(input.namespace.clone())?)
        };
        observability::list_jobs(&self.client, namespace, input, params).await
    }

    async fn list_cronjobs(
        &self,
        input: ListCronJobsInput,
    ) -> Result<CronJobsResponse, KubeviewError> {
        ensure_namespace_filter_not_conflicting(input.all_namespaces, input.namespace.as_deref())?;
        self.ensure_all_namespaces_allowed(input.all_namespaces)?;
        let params = Self::list_params(input.label_selector.clone(), input.field_selector.clone());
        let namespace = if input.all_namespaces {
            None
        } else {
            Some(self.namespace_or_default(input.namespace.clone())?)
        };
        observability::list_cronjobs(&self.client, namespace, input, params).await
    }
}

fn namespace_summary(namespace: Namespace) -> NamespaceSummary {
    NamespaceSummary {
        name: namespace.name_any(),
        status: namespace.status.and_then(|status| status.phase),
    }
}

fn scoped_namespaces_response(scope: &str) -> NamespacesResponse {
    NamespacesResponse {
        namespaces: vec![NamespaceSummary {
            name: scope.to_string(),
            status: None,
        }],
    }
}

fn resolve_namespace(
    namespace_scope: &Option<String>,
    default_namespace: &str,
    requested: Option<String>,
) -> Result<String, KubeviewError> {
    match (namespace_scope, requested) {
        (Some(scope), Some(requested)) if requested != *scope => Err(KubeviewError::InvalidInput(
            format!("namespace '{requested}' is outside configured scope '{scope}'"),
        )),
        (Some(scope), _) => Ok(scope.clone()),
        (None, Some(namespace)) => Ok(namespace),
        (None, None) => Ok(default_namespace.to_string()),
    }
}

fn ensure_all_namespaces_allowed(
    namespace_scope: &Option<String>,
    all_namespaces: bool,
) -> Result<(), KubeviewError> {
    if all_namespaces && let Some(scope) = namespace_scope {
        return Err(KubeviewError::InvalidInput(format!(
            "all_namespaces is not allowed when namespace scope is '{scope}'"
        )));
    }
    Ok(())
}

fn ensure_namespace_filter_not_conflicting(
    all_namespaces: bool,
    namespace: Option<&str>,
) -> Result<(), KubeviewError> {
    if all_namespaces && let Some(namespace) = namespace {
        return Err(KubeviewError::InvalidInput(format!(
            "namespace '{namespace}' cannot be combined with all_namespaces"
        )));
    }
    Ok(())
}

fn ensure_cluster_resource_allowed(
    namespace_scope: &Option<String>,
    capabilities: &ApiCapabilities,
    kind: &str,
) -> Result<(), KubeviewError> {
    if capabilities.scope == Scope::Cluster
        && let Some(scope) = namespace_scope
    {
        return Err(KubeviewError::InvalidInput(format!(
            "cluster-scoped resource '{kind}' is outside configured namespace scope '{scope}'"
        )));
    }
    Ok(())
}

fn ensure_cluster_resource_namespace_absent(
    capabilities: &ApiCapabilities,
    kind: &str,
    namespace: Option<&str>,
) -> Result<(), KubeviewError> {
    if capabilities.scope == Scope::Cluster
        && let Some(namespace) = namespace
    {
        return Err(KubeviewError::InvalidInput(format!(
            "namespace '{namespace}' is not valid for cluster-scoped resource '{kind}'"
        )));
    }
    Ok(())
}

fn serialize_dynamic_resource(
    resource: DynamicObject,
    api_resource: &kube::core::ApiResource,
) -> Result<serde_json::Value, serde_json::Error> {
    let mut value = serde_json::to_value(resource)?;
    if is_secret_resource(api_resource) {
        redact_secret_value(&mut value);
    }
    Ok(value)
}

fn is_secret_resource(api_resource: &kube::core::ApiResource) -> bool {
    api_resource.group.is_empty() && api_resource.kind.eq_ignore_ascii_case("secret")
}

fn redact_secret_value(value: &mut serde_json::Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("data");
        object.remove("stringData");
        remove_secret_annotations(object);
        object.insert(
            "redacted".to_string(),
            serde_json::Value::String("secret data omitted".to_string()),
        );
    }
}

fn remove_secret_annotations(object: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(annotations) = object
        .get_mut("metadata")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|metadata| metadata.get_mut("annotations"))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };

    annotations.remove(LAST_APPLIED_CONFIGURATION_ANNOTATION);
}

fn split_api_version(api_version: &str) -> Result<(&str, &str), KubeviewError> {
    if api_version.trim().is_empty() {
        return Err(KubeviewError::InvalidInput(
            "api_version must not be empty".to_string(),
        ));
    }
    Ok(api_version.split_once('/').unwrap_or(("", api_version)))
}

fn resolve_log_tail_lines(tail_lines: Option<u32>) -> Result<u32, KubeviewError> {
    let tail_lines = tail_lines.unwrap_or(DEFAULT_LOG_TAIL_LINES);
    if tail_lines > MAX_LOG_TAIL_LINES {
        return Err(KubeviewError::InvalidInput(format!(
            "tail_lines must be less than or equal to {MAX_LOG_TAIL_LINES}"
        )));
    }
    Ok(tail_lines)
}

fn format_age(created_at: Timestamp, now: Timestamp) -> String {
    let elapsed = now.duration_since(created_at);
    if elapsed.is_negative() {
        return "0s".to_string();
    }

    let seconds = elapsed.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h", seconds / 3_600)
    } else {
        format!("{}d", seconds / 86_400)
    }
}

pub(crate) fn pod_summary(pod: Pod) -> PodSummary {
    let status = pod.status.clone();
    let container_statuses = status
        .as_ref()
        .and_then(|status| status.container_statuses.as_ref())
        .cloned()
        .unwrap_or_default();
    let ready = container_statuses
        .iter()
        .filter(|status| status.ready)
        .count();
    let restart_count = container_statuses
        .iter()
        .map(|status| status.restart_count)
        .sum();
    let total = pod
        .spec
        .as_ref()
        .map_or(container_statuses.len(), |spec| spec.containers.len());

    PodSummary {
        name: pod.name_any(),
        namespace: pod.namespace(),
        phase: status.as_ref().and_then(|status| status.phase.clone()),
        node_name: pod.spec.as_ref().and_then(|spec| spec.node_name.clone()),
        pod_ip: status.as_ref().and_then(|status| status.pod_ip.clone()),
        host_ip: status.and_then(|status| status.host_ip),
        restart_count,
        containers_ready: format!("{ready}/{total}"),
        age: pod
            .metadata
            .creation_timestamp
            .map(|timestamp| format_age(timestamp.0, Timestamp::now())),
    }
}

#[cfg(test)]
mod tests;
