use k8s_openapi::api::core::v1::Event;
use kube::Api;
use kube::Client;
use kube::ResourceExt;
use kube::api::ListParams;

use super::resolve_list_limit;
use super::time_to_string;
use crate::error::KubeviewError;
use crate::models::EventSummary;
use crate::models::EventsResponse;
use crate::models::ListEventsInput;

pub(crate) async fn list_events(
    client: &Client,
    namespace: Option<String>,
    input: ListEventsInput,
) -> Result<EventsResponse, KubeviewError> {
    let limit = resolve_list_limit(input.limit)?;
    let events: Api<Event> = match &namespace {
        Some(namespace) => Api::namespaced(client.clone(), namespace),
        None => Api::all(client.clone()),
    };
    let list = events
        .list(&ListParams::default().limit(limit))
        .await
        .map_err(|error| KubeviewError::kubernetes_context("list events", error))?;
    let events = list
        .items
        .into_iter()
        .filter(|event| event_matches_filter(event, &input))
        .map(event_summary)
        .collect();

    Ok(EventsResponse { namespace, events })
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
