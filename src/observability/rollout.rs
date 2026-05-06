use std::time::Duration;
use std::time::Instant;

use k8s_openapi::api::apps::v1::DaemonSet;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::apps::v1::StatefulSet;
use kube::Api;
use kube::Client;
use kube::ResourceExt;

use super::resolve_rollout_interval;
use super::resolve_rollout_timeout;
use crate::error::KubeviewError;
use crate::models::ConditionSummary;
use crate::models::GetRolloutStatusInput;
use crate::models::RolloutKind;
use crate::models::RolloutObservation;
use crate::models::RolloutStatusResponse;
use crate::models::WaitRolloutInput;
use crate::models::WaitRolloutResponse;

pub(crate) async fn get_rollout_status(
    client: &Client,
    namespace: String,
    input: GetRolloutStatusInput,
) -> Result<RolloutStatusResponse, KubeviewError> {
    rollout_status_for(client, &namespace, input.kind, &input.name).await
}

pub(crate) async fn wait_rollout(
    client: &Client,
    namespace: String,
    input: WaitRolloutInput,
) -> Result<WaitRolloutResponse, KubeviewError> {
    let timeout = resolve_rollout_timeout(input.timeout_seconds)?;
    let interval = resolve_rollout_interval(input.interval_seconds)?;
    let started_at = Instant::now();
    let mut observations = Vec::new();

    loop {
        let status = rollout_status_for(client, &namespace, input.kind, &input.name).await?;
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

async fn rollout_status_for(
    client: &Client,
    namespace: &str,
    kind: RolloutKind,
    name: &str,
) -> Result<RolloutStatusResponse, KubeviewError> {
    match kind {
        RolloutKind::Deployment => {
            let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
            let deployment = api.get(name).await.map_err(|error| {
                KubeviewError::kubernetes_context("get deployment rollout status", error)
            })?;
            Ok(deployment_rollout_status(deployment, namespace))
        }
        RolloutKind::StatefulSet => {
            let api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
            let stateful_set = api.get(name).await.map_err(|error| {
                KubeviewError::kubernetes_context("get statefulset rollout status", error)
            })?;
            Ok(stateful_set_rollout_status(stateful_set, namespace))
        }
        RolloutKind::DaemonSet => {
            let api: Api<DaemonSet> = Api::namespaced(client.clone(), namespace);
            let daemon_set = api.get(name).await.map_err(|error| {
                KubeviewError::kubernetes_context("get daemonset rollout status", error)
            })?;
            Ok(daemon_set_rollout_status(daemon_set, namespace))
        }
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
        format!("deployment '{name}' controller has not observed latest generation")
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
        format!("statefulset '{name}' controller has not observed latest generation")
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
        format!("daemonset '{name}' controller has not observed latest generation")
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

fn generation_observed(observed_generation: Option<i64>, generation: Option<i64>) -> bool {
    match (observed_generation, generation) {
        (Some(observed), Some(generation)) => observed >= generation,
        _ => true,
    }
}
