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
    let error =
        ensure_cluster_resource_allowed(&scope, &capabilities(Scope::Cluster), "Node").unwrap_err();

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
