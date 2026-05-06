use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use k8s_openapi::api::apps::v1::DaemonSet;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::batch::v1::CronJob;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::Event;
use k8s_openapi::api::core::v1::Namespace;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
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
use crate::tools::ConditionSummary;
use crate::tools::ContextsResponse;
use crate::tools::CronJobSummary;
use crate::tools::CronJobsResponse;
use crate::tools::CurrentContextResponse;
use crate::tools::EndpointSliceSummary;
use crate::tools::EndpointSummary;
use crate::tools::EndpointTotals;
use crate::tools::EventSummary;
use crate::tools::EventsResponse;
use crate::tools::GetPodInput;
use crate::tools::GetResourceInput;
use crate::tools::GetRolloutStatusInput;
use crate::tools::JobSummary;
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
use crate::tools::RolloutObservation;
use crate::tools::RolloutStatusResponse;
use crate::tools::ServicePortSummary;
use crate::tools::ServiceTraceSummary;
use crate::tools::TraceServiceInput;
use crate::tools::TraceServiceResponse;
use crate::tools::WaitRolloutInput;
use crate::tools::WaitRolloutResponse;

const DEFAULT_LOG_TAIL_LINES: u32 = 200;
const MAX_LOG_TAIL_LINES: u32 = 5_000;
const DEFAULT_ROLLOUT_TIMEOUT_SECONDS: u64 = 300;
const DEFAULT_ROLLOUT_INTERVAL_SECONDS: u64 = 5;
const MIN_ROLLOUT_INTERVAL_SECONDS: u64 = 1;
const MAX_ROLLOUT_TIMEOUT_SECONDS: u64 = 3_600;
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
            Some(path) => Kubeconfig::read_from(path)?,
            None => Kubeconfig::read()?,
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
        let mut kube_config = Config::from_custom_kubeconfig(kubeconfig.clone(), &options).await?;
        let namespace_scope = config.namespace.clone();
        if let Some(namespace) = namespace_scope.clone() {
            kube_config.default_namespace = namespace;
        }
        let namespace = kube_config.default_namespace.clone();
        let client = Client::try_from(kube_config)?;

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
            .map_err(Into::into)
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
        let list = namespaces.list(&ListParams::default()).await?;
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
        let list = pods.list(&params).await?;
        Ok(PodsResponse {
            namespace,
            pods: list.items.into_iter().map(pod_summary).collect(),
        })
    }

    async fn get_pod(&self, input: GetPodInput) -> Result<serde_json::Value, KubeviewError> {
        let namespace = self.namespace_or_default(input.namespace)?;
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &namespace);
        let pod = pods.get(&input.name).await?;
        serde_json::to_value(pod).map_err(|error| KubeviewError::Kubernetes(error.to_string()))
    }

    async fn pod_logs(&self, input: PodLogsInput) -> Result<PodLogsResponse, KubeviewError> {
        let namespace = self.namespace_or_default(input.namespace)?;
        let tail_lines = resolve_log_tail_lines(input.tail_lines)?;
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &namespace);
        let params = LogParams {
            container: input.container.clone(),
            tail_lines: Some(i64::from(tail_lines)),
            ..LogParams::default()
        };
        let logs = pods.logs(&input.pod, &params).await?;

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
        let list = api.list(&params).await?;
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
        let resource = api.get(&input.name).await?;
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
        let events: Api<Event> = match &namespace {
            Some(namespace) => Api::namespaced(self.client.clone(), namespace),
            None => Api::all(self.client.clone()),
        };
        let list = events.list(&ListParams::default()).await?;
        let events = list
            .items
            .into_iter()
            .filter(|event| event_matches_filter(event, &input))
            .map(event_summary)
            .collect();

        Ok(EventsResponse { namespace, events })
    }

    async fn get_rollout_status(
        &self,
        input: GetRolloutStatusInput,
    ) -> Result<RolloutStatusResponse, KubeviewError> {
        let namespace = self.namespace_or_default(input.namespace)?;
        rollout_status_for(&self.client, &namespace, &input.kind, &input.name).await
    }

    async fn wait_rollout(
        &self,
        input: WaitRolloutInput,
    ) -> Result<WaitRolloutResponse, KubeviewError> {
        let timeout = resolve_rollout_timeout(input.timeout_seconds)?;
        let interval = resolve_rollout_interval(input.interval_seconds)?;
        let namespace = self.namespace_or_default(input.namespace)?;
        let started_at = Instant::now();
        let mut observations = Vec::new();

        loop {
            let status =
                rollout_status_for(&self.client, &namespace, &input.kind, &input.name).await?;
            let elapsed_seconds = started_at.elapsed().as_secs();
            observations.push(RolloutObservation {
                elapsed_seconds,
                complete: status.complete,
                message: status.message.clone(),
                updated_replicas: status.updated_replicas,
                ready_replicas: status.ready_replicas,
                available_replicas: status.available_replicas,
            });

            if status.complete {
                return Ok(WaitRolloutResponse {
                    status,
                    completed: true,
                    timed_out: false,
                    elapsed_seconds,
                    observations,
                });
            }

            if elapsed_seconds >= timeout {
                return Ok(WaitRolloutResponse {
                    status,
                    completed: false,
                    timed_out: true,
                    elapsed_seconds,
                    observations,
                });
            }

            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    }

    async fn trace_service(
        &self,
        input: TraceServiceInput,
    ) -> Result<TraceServiceResponse, KubeviewError> {
        let namespace = self.namespace_or_default(input.namespace)?;
        let services: Api<Service> = Api::namespaced(self.client.clone(), &namespace);
        let service = services.get(&input.name).await?;
        let service_summary = service_trace_summary(&service);
        let mut warnings = Vec::new();

        let slices: Api<EndpointSlice> = Api::namespaced(self.client.clone(), &namespace);
        let slice_params =
            ListParams::default().labels(&format!("kubernetes.io/service-name={}", input.name));
        let endpoint_slices = slices
            .list(&slice_params)
            .await?
            .items
            .into_iter()
            .map(endpoint_slice_summary)
            .collect::<Vec<_>>();
        let endpoints = endpoint_totals(&endpoint_slices);
        if endpoints.total == 0 {
            warnings.push("service has no EndpointSlice endpoints".to_string());
        } else if endpoints.ready == 0 {
            warnings.push("service has no ready endpoints".to_string());
        }

        let selected_pods = match service
            .spec
            .as_ref()
            .and_then(|spec| spec.selector.as_ref())
        {
            Some(selector) if !selector.is_empty() => {
                let pods: Api<Pod> = Api::namespaced(self.client.clone(), &namespace);
                let params = ListParams::default().labels(&map_label_selector(selector));
                pods.list(&params)
                    .await?
                    .items
                    .into_iter()
                    .map(pod_summary)
                    .collect()
            }
            _ => {
                warnings
                    .push("service has no selector; endpoints may be managed manually".to_string());
                Vec::new()
            }
        };
        if service_summary.selector.is_some() && selected_pods.is_empty() {
            warnings.push("service selector does not match any pods".to_string());
        }

        Ok(TraceServiceResponse {
            namespace,
            service: service_summary,
            endpoint_slices,
            endpoints,
            selected_pods,
            warnings,
        })
    }

    async fn list_jobs(&self, input: ListJobsInput) -> Result<JobsResponse, KubeviewError> {
        ensure_namespace_filter_not_conflicting(input.all_namespaces, input.namespace.as_deref())?;
        self.ensure_all_namespaces_allowed(input.all_namespaces)?;
        let params = Self::list_params(input.label_selector, input.field_selector);
        let namespace = if input.all_namespaces {
            None
        } else {
            Some(self.namespace_or_default(input.namespace)?)
        };
        let jobs: Api<Job> = match &namespace {
            Some(namespace) => Api::namespaced(self.client.clone(), namespace),
            None => Api::all(self.client.clone()),
        };
        let list = jobs.list(&params).await?;
        Ok(JobsResponse {
            namespace,
            jobs: list.items.into_iter().map(job_summary).collect(),
        })
    }

    async fn list_cronjobs(
        &self,
        input: ListCronJobsInput,
    ) -> Result<CronJobsResponse, KubeviewError> {
        ensure_namespace_filter_not_conflicting(input.all_namespaces, input.namespace.as_deref())?;
        self.ensure_all_namespaces_allowed(input.all_namespaces)?;
        let params = Self::list_params(input.label_selector, input.field_selector);
        let namespace = if input.all_namespaces {
            None
        } else {
            Some(self.namespace_or_default(input.namespace)?)
        };
        let cronjobs: Api<CronJob> = match &namespace {
            Some(namespace) => Api::namespaced(self.client.clone(), namespace),
            None => Api::all(self.client.clone()),
        };
        let list = cronjobs.list(&params).await?;
        Ok(CronJobsResponse {
            namespace,
            cronjobs: list.items.into_iter().map(cronjob_summary).collect(),
        })
    }
}

async fn rollout_status_for(
    client: &Client,
    namespace: &str,
    kind: &str,
    name: &str,
) -> Result<RolloutStatusResponse, KubeviewError> {
    match normalized_workload_kind(kind).as_str() {
        "deployment" => {
            let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
            let deployment = api.get(name).await?;
            Ok(deployment_rollout_status(deployment, namespace))
        }
        "statefulset" => {
            let api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
            let stateful_set = api.get(name).await?;
            Ok(stateful_set_rollout_status(stateful_set, namespace))
        }
        "daemonset" => {
            let api: Api<DaemonSet> = Api::namespaced(client.clone(), namespace);
            let daemon_set = api.get(name).await?;
            Ok(daemon_set_rollout_status(daemon_set, namespace))
        }
        _ => Err(KubeviewError::InvalidInput(format!(
            "unsupported rollout kind '{kind}', expected Deployment, StatefulSet, or DaemonSet"
        ))),
    }
}

fn deployment_rollout_status(deployment: Deployment, namespace: &str) -> RolloutStatusResponse {
    let name = deployment.name_any();
    let desired = deployment
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(1);
    let generation = deployment.metadata.generation;
    let status = deployment.status.unwrap_or_default();
    let observed_generation = status.observed_generation;
    let updated = status.updated_replicas.unwrap_or_default();
    let ready = status.ready_replicas.unwrap_or_default();
    let available = status.available_replicas.unwrap_or_default();
    let unavailable = status.unavailable_replicas.unwrap_or_default();
    let observed = generation_observed(observed_generation, generation);
    let complete = observed && updated >= desired && available >= desired && unavailable == 0;
    let message = if complete {
        format!("deployment '{name}' rollout complete")
    } else if !observed {
        format!(
            "deployment '{}' controller has not observed latest generation",
            name
        )
    } else {
        format!(
            "deployment '{name}' rollout in progress: {available}/{desired} available, {updated}/{desired} updated"
        )
    };

    RolloutStatusResponse {
        kind: "Deployment".to_string(),
        name,
        namespace: namespace.to_string(),
        complete,
        message,
        desired_replicas: desired,
        updated_replicas: updated,
        ready_replicas: ready,
        available_replicas: available,
        unavailable_replicas: unavailable,
        observed_generation,
        generation,
        conditions: status
            .conditions
            .unwrap_or_default()
            .into_iter()
            .map(|condition| ConditionSummary {
                type_: condition.type_,
                status: condition.status,
                reason: condition.reason,
                message: condition.message,
            })
            .collect(),
    }
}

fn stateful_set_rollout_status(
    stateful_set: StatefulSet,
    namespace: &str,
) -> RolloutStatusResponse {
    let name = stateful_set.name_any();
    let desired = stateful_set
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(1);
    let generation = stateful_set.metadata.generation;
    let status = stateful_set.status.unwrap_or_default();
    let observed_generation = status.observed_generation;
    let updated = status.updated_replicas.unwrap_or_default();
    let ready = status.ready_replicas.unwrap_or_default();
    let available = status.available_replicas.unwrap_or_default();
    let unavailable = desired.saturating_sub(ready);
    let observed = generation_observed(observed_generation, generation);
    let complete = observed && updated >= desired && ready >= desired;
    let message = if complete {
        format!("statefulset '{name}' rollout complete")
    } else if !observed {
        format!(
            "statefulset '{}' controller has not observed latest generation",
            name
        )
    } else {
        format!(
            "statefulset '{name}' rollout in progress: {ready}/{desired} ready, {updated}/{desired} updated"
        )
    };

    RolloutStatusResponse {
        kind: "StatefulSet".to_string(),
        name,
        namespace: namespace.to_string(),
        complete,
        message,
        desired_replicas: desired,
        updated_replicas: updated,
        ready_replicas: ready,
        available_replicas: available,
        unavailable_replicas: unavailable,
        observed_generation,
        generation,
        conditions: status
            .conditions
            .unwrap_or_default()
            .into_iter()
            .map(|condition| ConditionSummary {
                type_: condition.type_,
                status: condition.status,
                reason: None,
                message: condition.message,
            })
            .collect(),
    }
}

fn daemon_set_rollout_status(daemon_set: DaemonSet, namespace: &str) -> RolloutStatusResponse {
    let name = daemon_set.name_any();
    let generation = daemon_set.metadata.generation;
    let status = daemon_set.status.unwrap_or_default();
    let observed_generation = status.observed_generation;
    let desired = status.desired_number_scheduled;
    let updated = status.updated_number_scheduled.unwrap_or_default();
    let ready = status.number_ready;
    let available = status.number_available.unwrap_or(ready);
    let unavailable = status
        .number_unavailable
        .unwrap_or_else(|| desired.saturating_sub(ready));
    let observed = generation_observed(observed_generation, generation);
    let complete = observed && updated >= desired && ready >= desired && unavailable == 0;
    let message = if complete {
        format!("daemonset '{name}' rollout complete")
    } else if !observed {
        format!(
            "daemonset '{}' controller has not observed latest generation",
            name
        )
    } else {
        format!(
            "daemonset '{name}' rollout in progress: {ready}/{desired} ready, {updated}/{desired} updated"
        )
    };

    RolloutStatusResponse {
        kind: "DaemonSet".to_string(),
        name,
        namespace: namespace.to_string(),
        complete,
        message,
        desired_replicas: desired,
        updated_replicas: updated,
        ready_replicas: ready,
        available_replicas: available,
        unavailable_replicas: unavailable,
        observed_generation,
        generation,
        conditions: status
            .conditions
            .unwrap_or_default()
            .into_iter()
            .map(|condition| ConditionSummary {
                type_: condition.type_,
                status: condition.status,
                reason: None,
                message: condition.message,
            })
            .collect(),
    }
}

fn normalized_workload_kind(kind: &str) -> String {
    kind.to_ascii_lowercase().replace('-', "")
}

fn generation_observed(observed_generation: Option<i64>, generation: Option<i64>) -> bool {
    match (observed_generation, generation) {
        (Some(observed), Some(generation)) => observed >= generation,
        _ => true,
    }
}

fn event_matches_filter(event: &Event, input: &ListEventsInput) -> bool {
    if let Some(expected) = &input.involved_kind
        && event.involved_object.kind.as_deref() != Some(expected.as_str())
    {
        return false;
    }
    if let Some(expected) = &input.involved_name
        && event.involved_object.name.as_deref() != Some(expected.as_str())
    {
        return false;
    }
    if let Some(expected) = &input.type_
        && event.type_.as_deref() != Some(expected.as_str())
    {
        return false;
    }
    true
}

fn event_summary(event: Event) -> EventSummary {
    EventSummary {
        name: event.name_any(),
        namespace: event.namespace(),
        type_: event.type_,
        reason: event.reason,
        message: event.message,
        count: event.count,
        first_timestamp: time_to_string(event.first_timestamp),
        last_timestamp: time_to_string(event.last_timestamp),
        involved_kind: event.involved_object.kind,
        involved_name: event.involved_object.name,
        reporting_component: event.reporting_component.or_else(|| {
            event
                .source
                .and_then(|source| source.component.or(source.host))
        }),
    }
}

fn service_trace_summary(service: &Service) -> ServiceTraceSummary {
    let spec = service.spec.as_ref();
    ServiceTraceSummary {
        name: service.name_any(),
        type_: spec.and_then(|spec| spec.type_.clone()),
        cluster_ip: spec.and_then(|spec| spec.cluster_ip.clone()),
        selector: spec.and_then(|spec| spec.selector.clone()),
        ports: spec
            .and_then(|spec| spec.ports.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|port| ServicePortSummary {
                name: port.name,
                port: port.port,
                target_port: port.target_port.map(int_or_string),
                protocol: port.protocol,
            })
            .collect(),
    }
}

fn endpoint_slice_summary(slice: EndpointSlice) -> EndpointSliceSummary {
    EndpointSliceSummary {
        name: slice.name_any(),
        address_type: slice.address_type,
        endpoints: slice.endpoints.into_iter().map(endpoint_summary).collect(),
    }
}

fn endpoint_summary(endpoint: k8s_openapi::api::discovery::v1::Endpoint) -> EndpointSummary {
    let ready = endpoint
        .conditions
        .as_ref()
        .and_then(|conditions| conditions.ready)
        .unwrap_or(true);
    let serving = endpoint
        .conditions
        .as_ref()
        .and_then(|conditions| conditions.serving)
        .unwrap_or(true);
    let terminating = endpoint
        .conditions
        .as_ref()
        .and_then(|conditions| conditions.terminating)
        .unwrap_or(false);

    EndpointSummary {
        addresses: endpoint.addresses,
        ready,
        serving,
        terminating,
        target_kind: endpoint
            .target_ref
            .as_ref()
            .and_then(|target| target.kind.clone()),
        target_name: endpoint
            .target_ref
            .as_ref()
            .and_then(|target| target.name.clone()),
        node_name: endpoint.node_name,
    }
}

fn endpoint_totals(slices: &[EndpointSliceSummary]) -> EndpointTotals {
    let mut totals = EndpointTotals {
        total: 0,
        ready: 0,
        serving: 0,
        terminating: 0,
    };
    for endpoint in slices.iter().flat_map(|slice| slice.endpoints.iter()) {
        totals.total += 1;
        totals.ready += usize::from(endpoint.ready);
        totals.serving += usize::from(endpoint.serving);
        totals.terminating += usize::from(endpoint.terminating);
    }
    totals
}

fn job_summary(job: Job) -> JobSummary {
    let spec = job.spec.as_ref();
    let status = job.status.as_ref();
    JobSummary {
        name: job.name_any(),
        namespace: job.namespace(),
        active: status.and_then(|status| status.active).unwrap_or_default(),
        ready: status.and_then(|status| status.ready).unwrap_or_default(),
        succeeded: status
            .and_then(|status| status.succeeded)
            .unwrap_or_default(),
        failed: status.and_then(|status| status.failed).unwrap_or_default(),
        terminating: status
            .and_then(|status| status.terminating)
            .unwrap_or_default(),
        completions: spec.and_then(|spec| spec.completions),
        parallelism: spec.and_then(|spec| spec.parallelism),
        suspend: spec.and_then(|spec| spec.suspend),
        completion_time: status.and_then(|status| time_to_string(status.completion_time.clone())),
        start_time: status.and_then(|status| time_to_string(status.start_time.clone())),
        conditions: status
            .and_then(|status| status.conditions.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|condition| ConditionSummary {
                type_: condition.type_,
                status: condition.status,
                reason: condition.reason,
                message: condition.message,
            })
            .collect(),
        owner_names: job
            .metadata
            .owner_references
            .unwrap_or_default()
            .into_iter()
            .map(|owner| owner.name)
            .collect(),
    }
}

fn cronjob_summary(cronjob: CronJob) -> CronJobSummary {
    let spec = cronjob.spec.as_ref();
    let status = cronjob.status.as_ref();
    let active_jobs = status
        .and_then(|status| status.active.clone())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|reference| reference.name)
        .collect::<Vec<_>>();

    CronJobSummary {
        name: cronjob.name_any(),
        namespace: cronjob.namespace(),
        schedule: spec.map_or_else(String::new, |spec| spec.schedule.clone()),
        suspend: spec.and_then(|spec| spec.suspend),
        active: active_jobs.len(),
        active_jobs,
        last_schedule_time: status
            .and_then(|status| time_to_string(status.last_schedule_time.clone())),
        last_successful_time: status
            .and_then(|status| time_to_string(status.last_successful_time.clone())),
        concurrency_policy: spec.and_then(|spec| spec.concurrency_policy.clone()),
        time_zone: spec.and_then(|spec| spec.time_zone.clone()),
    }
}

fn map_label_selector(selector: &std::collections::BTreeMap<String, String>) -> String {
    selector
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn int_or_string(value: IntOrString) -> String {
    match value {
        IntOrString::Int(value) => value.to_string(),
        IntOrString::String(value) => value,
    }
}

fn time_to_string(time: Option<Time>) -> Option<String> {
    time.map(|time| time.0.to_string())
}

fn resolve_rollout_timeout(timeout_seconds: Option<u64>) -> Result<u64, KubeviewError> {
    let timeout = timeout_seconds.unwrap_or(DEFAULT_ROLLOUT_TIMEOUT_SECONDS);
    if timeout > MAX_ROLLOUT_TIMEOUT_SECONDS {
        return Err(KubeviewError::InvalidInput(format!(
            "timeout_seconds must be less than or equal to {MAX_ROLLOUT_TIMEOUT_SECONDS}"
        )));
    }
    Ok(timeout)
}

fn resolve_rollout_interval(interval_seconds: Option<u64>) -> Result<u64, KubeviewError> {
    let interval = interval_seconds.unwrap_or(DEFAULT_ROLLOUT_INTERVAL_SECONDS);
    if interval < MIN_ROLLOUT_INTERVAL_SECONDS {
        return Err(KubeviewError::InvalidInput(format!(
            "interval_seconds must be greater than or equal to {MIN_ROLLOUT_INTERVAL_SECONDS}"
        )));
    }
    Ok(interval)
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

fn pod_summary(pod: Pod) -> PodSummary {
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
mod tests {
    use kube::core::ApiResource;

    use super::*;

    fn capabilities(scope: Scope) -> ApiCapabilities {
        ApiCapabilities {
            scope,
            subresources: vec![],
            operations: vec![],
        }
    }

    #[test]
    fn split_core_api_version() {
        assert_eq!(split_api_version("v1").unwrap(), ("", "v1"));
    }

    #[test]
    fn split_group_api_version() {
        assert_eq!(split_api_version("apps/v1").unwrap(), ("apps", "v1"));
    }

    #[test]
    fn scoped_namespace_rejects_other_namespace() {
        let scope = Some("prod".to_string());
        let error = resolve_namespace(&scope, "default", Some("dev".to_string())).unwrap_err();

        assert!(error.to_string().contains("outside configured scope"));
    }

    #[test]
    fn scoped_namespace_uses_scope_when_request_omits_namespace() {
        let scope = Some("prod".to_string());

        assert_eq!(resolve_namespace(&scope, "default", None).unwrap(), "prod");
    }

    #[test]
    fn scoped_namespace_rejects_all_namespaces() {
        let scope = Some("prod".to_string());
        let error = ensure_all_namespaces_allowed(&scope, true).unwrap_err();

        assert!(error.to_string().contains("all_namespaces is not allowed"));
    }

    #[test]
    fn namespace_filter_rejects_all_namespaces_with_namespace() {
        let error = ensure_namespace_filter_not_conflicting(true, Some("prod")).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("cannot be combined with all_namespaces")
        );
    }

    #[test]
    fn namespace_filter_accepts_single_namespace() {
        let result = ensure_namespace_filter_not_conflicting(false, Some("prod"));

        assert!(result.is_ok());
    }

    #[test]
    fn namespace_filter_accepts_all_namespaces_without_namespace() {
        let result = ensure_namespace_filter_not_conflicting(true, None);

        assert!(result.is_ok());
    }

    #[test]
    fn scoped_namespace_rejects_cluster_scoped_generic_resource() {
        let scope = Some("prod".to_string());
        let error = ensure_cluster_resource_allowed(&scope, &capabilities(Scope::Cluster), "Node")
            .unwrap_err();

        assert!(error.to_string().contains("cluster-scoped resource"));
    }

    #[test]
    fn cluster_scoped_resource_rejects_namespace() {
        let error = ensure_cluster_resource_namespace_absent(
            &capabilities(Scope::Cluster),
            "Node",
            Some("prod"),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("not valid for cluster-scoped resource")
        );
    }

    #[test]
    fn namespaced_resource_accepts_namespace() {
        let result = ensure_cluster_resource_namespace_absent(
            &capabilities(Scope::Namespaced),
            "Deployment",
            Some("prod"),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn log_tail_lines_default_when_omitted() {
        assert_eq!(
            resolve_log_tail_lines(None).unwrap(),
            DEFAULT_LOG_TAIL_LINES
        );
    }

    #[test]
    fn log_tail_lines_rejects_values_above_limit() {
        let error = resolve_log_tail_lines(Some(MAX_LOG_TAIL_LINES + 1)).unwrap_err();

        assert!(error.to_string().contains("tail_lines must be"));
    }

    #[test]
    fn format_age_uses_largest_whole_time_unit() {
        let created_at = Timestamp::from_second(1_000).unwrap();

        assert_eq!(
            format_age(created_at, Timestamp::from_second(1_045).unwrap()),
            "45s"
        );
        assert_eq!(
            format_age(created_at, Timestamp::from_second(1_180).unwrap()),
            "3m"
        );
        assert_eq!(
            format_age(created_at, Timestamp::from_second(8_200).unwrap()),
            "2h"
        );
        assert_eq!(
            format_age(created_at, Timestamp::from_second(173_800).unwrap()),
            "2d"
        );
    }

    #[test]
    fn format_age_clamps_future_timestamps_to_zero() {
        assert_eq!(
            format_age(
                Timestamp::from_second(2_000).unwrap(),
                Timestamp::from_second(1_000).unwrap()
            ),
            "0s"
        );
    }

    #[test]
    fn scoped_namespaces_response_uses_scope_without_status() {
        let response = scoped_namespaces_response("prod");

        assert_eq!(response.namespaces.len(), 1);
        assert_eq!(response.namespaces[0].name, "prod");
        assert_eq!(response.namespaces[0].status, None);
    }

    #[test]
    fn secret_redaction_removes_secret_payload_fields() {
        let api_resource = ApiResource {
            group: String::new(),
            version: "v1".to_string(),
            api_version: "v1".to_string(),
            kind: "Secret".to_string(),
            plural: "secrets".to_string(),
        };
        let mut value = serde_json::json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "data": {"token": "c2VjcmV0"},
            "stringData": {"token": "secret"},
            "metadata": {
                "name": "example",
                "annotations": {
                    "kubectl.kubernetes.io/last-applied-configuration": "{\"data\":{\"token\":\"c2VjcmV0\"}}",
                    "example.com/owner": "platform"
                }
            }
        });

        if is_secret_resource(&api_resource) {
            redact_secret_value(&mut value);
        }

        assert!(value.get("data").is_none());
        assert!(value.get("stringData").is_none());
        assert!(
            value["metadata"]["annotations"]
                .get("kubectl.kubernetes.io/last-applied-configuration")
                .is_none()
        );
        assert_eq!(
            value["metadata"]["annotations"]["example.com/owner"],
            "platform"
        );
        assert_eq!(value["redacted"], "secret data omitted");
    }
}
