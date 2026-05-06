use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPodsInput {
    pub namespace: Option<String>,
    pub all_namespaces: bool,
    pub label_selector: Option<String>,
    pub field_selector: Option<String>,
    pub limit: Option<u32>,
    pub continue_token: Option<String>,
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
    pub limit: Option<u32>,
    pub continue_token: Option<String>,
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
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRolloutStatusInput {
    pub kind: RolloutKind,
    pub name: String,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitRolloutInput {
    pub kind: RolloutKind,
    pub name: String,
    pub namespace: Option<String>,
    pub timeout_seconds: Option<u64>,
    pub interval_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolloutKind {
    Deployment,
    StatefulSet,
    DaemonSet,
}

impl RolloutKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().replace('-', "").as_str() {
            "deployment" => Some(Self::Deployment),
            "statefulset" => Some(Self::StatefulSet),
            "daemonset" => Some(Self::DaemonSet),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deployment => "Deployment",
            Self::StatefulSet => "StatefulSet",
            Self::DaemonSet => "DaemonSet",
        }
    }
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
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListCronJobsInput {
    pub namespace: Option<String>,
    pub all_namespaces: bool,
    pub label_selector: Option<String>,
    pub field_selector: Option<String>,
    pub limit: Option<u32>,
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
    pub limit: u32,
    pub continue_token: Option<String>,
    pub truncated: bool,
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
    pub limit: u32,
    pub continue_token: Option<String>,
    pub truncated: bool,
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
