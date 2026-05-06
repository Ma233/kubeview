mod batch;
mod events;
mod rollout;
mod service;

pub(crate) use batch::list_cronjobs;
pub(crate) use batch::list_jobs;
pub(crate) use events::list_events;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
pub(crate) use rollout::get_rollout_status;
pub(crate) use rollout::wait_rollout;
pub(crate) use service::trace_service;

use crate::error::KubeviewError;

const DEFAULT_ROLLOUT_TIMEOUT_SECONDS: u64 = 300;
const DEFAULT_ROLLOUT_INTERVAL_SECONDS: u64 = 5;
const MIN_ROLLOUT_INTERVAL_SECONDS: u64 = 1;
const MAX_ROLLOUT_TIMEOUT_SECONDS: u64 = 3_600;
const DEFAULT_LIST_LIMIT: u32 = 200;
const MAX_LIST_LIMIT: u32 = 1_000;

fn time_to_string(time: Option<Time>) -> Option<String> {
    time.map(|time| time.0.to_string())
}

pub(crate) fn resolve_rollout_timeout(timeout_seconds: Option<u64>) -> Result<u64, KubeviewError> {
    let timeout = timeout_seconds.unwrap_or(DEFAULT_ROLLOUT_TIMEOUT_SECONDS);
    if timeout > MAX_ROLLOUT_TIMEOUT_SECONDS {
        return Err(KubeviewError::InvalidInput(format!(
            "timeout_seconds must be less than or equal to {MAX_ROLLOUT_TIMEOUT_SECONDS}"
        )));
    }
    Ok(timeout)
}

pub(crate) fn resolve_rollout_interval(
    interval_seconds: Option<u64>,
) -> Result<u64, KubeviewError> {
    let interval = interval_seconds.unwrap_or(DEFAULT_ROLLOUT_INTERVAL_SECONDS);
    if interval < MIN_ROLLOUT_INTERVAL_SECONDS {
        return Err(KubeviewError::InvalidInput(format!(
            "interval_seconds must be greater than or equal to {MIN_ROLLOUT_INTERVAL_SECONDS}"
        )));
    }
    Ok(interval)
}

pub(crate) fn resolve_list_limit(limit: Option<u32>) -> Result<u32, KubeviewError> {
    let limit = limit.unwrap_or(DEFAULT_LIST_LIMIT);
    if limit > MAX_LIST_LIMIT {
        return Err(KubeviewError::InvalidInput(format!(
            "limit must be less than or equal to {MAX_LIST_LIMIT}"
        )));
    }
    Ok(limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollout_timeout_rejects_values_above_limit() {
        let error = resolve_rollout_timeout(Some(MAX_ROLLOUT_TIMEOUT_SECONDS + 1)).unwrap_err();

        assert!(error.to_string().contains("timeout_seconds must be"));
    }

    #[test]
    fn rollout_interval_rejects_zero() {
        let error = resolve_rollout_interval(Some(0)).unwrap_err();

        assert!(error.to_string().contains("interval_seconds must be"));
    }

    #[test]
    fn list_limit_defaults_and_rejects_values_above_limit() {
        assert_eq!(resolve_list_limit(None).unwrap(), DEFAULT_LIST_LIMIT);

        let error = resolve_list_limit(Some(MAX_LIST_LIMIT + 1)).unwrap_err();

        assert!(error.to_string().contains("limit must be"));
    }
}
