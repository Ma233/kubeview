use k8s_openapi::api::batch::v1::CronJob;
use k8s_openapi::api::batch::v1::Job;
use kube::Api;
use kube::Client;
use kube::ResourceExt;
use kube::api::ListParams;

use super::resolve_list_limit;
use super::time_to_string;
use crate::error::KubeviewError;
use crate::models::ConditionSummary;
use crate::models::CronJobSummary;
use crate::models::CronJobsResponse;
use crate::models::JobSummary;
use crate::models::JobsResponse;
use crate::models::ListCronJobsInput;
use crate::models::ListJobsInput;

pub(crate) async fn list_jobs(
    client: &Client,
    namespace: Option<String>,
    input: ListJobsInput,
    params: ListParams,
) -> Result<JobsResponse, KubeviewError> {
    let jobs: Api<Job> = match &namespace {
        Some(namespace) => Api::namespaced(client.clone(), namespace),
        None => Api::all(client.clone()),
    };
    let list = jobs
        .list(&params.limit(resolve_list_limit(input.limit)?))
        .await
        .map_err(|error| KubeviewError::kubernetes_context("list jobs", error))?;
    Ok(JobsResponse {
        namespace,
        jobs: list.items.into_iter().map(job_summary).collect(),
    })
}

pub(crate) async fn list_cronjobs(
    client: &Client,
    namespace: Option<String>,
    input: ListCronJobsInput,
    params: ListParams,
) -> Result<CronJobsResponse, KubeviewError> {
    let cronjobs: Api<CronJob> = match &namespace {
        Some(namespace) => Api::namespaced(client.clone(), namespace),
        None => Api::all(client.clone()),
    };
    let list = cronjobs
        .list(&params.limit(resolve_list_limit(input.limit)?))
        .await
        .map_err(|error| KubeviewError::kubernetes_context("list cronjobs", error))?;
    Ok(CronJobsResponse {
        namespace,
        cronjobs: list.items.into_iter().map(cronjob_summary).collect(),
    })
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
