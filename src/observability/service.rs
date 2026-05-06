use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::Api;
use kube::Client;
use kube::ResourceExt;
use kube::api::ListParams;

use crate::error::KubeviewError;
use crate::kubernetes::pod_summary;
use crate::models::EndpointSliceSummary;
use crate::models::EndpointSummary;
use crate::models::EndpointTotals;
use crate::models::ServicePortSummary;
use crate::models::ServiceTraceSummary;
use crate::models::TraceServiceInput;
use crate::models::TraceServiceResponse;

pub(crate) async fn trace_service(
    client: &Client,
    namespace: String,
    input: TraceServiceInput,
) -> Result<TraceServiceResponse, KubeviewError> {
    let services: Api<Service> = Api::namespaced(client.clone(), &namespace);
    let service = services
        .get(&input.name)
        .await
        .map_err(|error| KubeviewError::kubernetes_context("get service for trace", error))?;
    let service_summary = service_trace_summary(&service);
    let mut warnings = Vec::new();

    let slices: Api<EndpointSlice> = Api::namespaced(client.clone(), &namespace);
    let slice_params =
        ListParams::default().labels(&format!("kubernetes.io/service-name={}", input.name));
    let endpoint_slices = slices
        .list(&slice_params)
        .await
        .map_err(|error| KubeviewError::kubernetes_context("list endpoint slices", error))?
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
            let pods: Api<Pod> = Api::namespaced(client.clone(), &namespace);
            let params = ListParams::default().labels(&map_label_selector(selector));
            pods.list(&params)
                .await
                .map_err(|error| KubeviewError::kubernetes_context("list selected pods", error))?
                .items
                .into_iter()
                .map(pod_summary)
                .collect()
        }
        _ => {
            warnings.push("service has no selector; endpoints may be managed manually".to_string());
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
