//! Providers page — Ollama connection status and model provider listing.

use carnelian_common::types::EventType;
use dioxus::prelude::*;

use crate::store::EventStreamStore;

/// Model providers monitoring page.
#[component]
pub fn Providers() -> Element {
    let store = use_context::<EventStreamStore>();
    let mut refresh = use_signal(|| 0_u64);

    // Fetch Ollama status
    let ollama_status = use_resource(move || async move {
        let _ = refresh();
        crate::api::get_ollama_status().await.ok()
    });

    // Fetch all providers
    let providers = use_resource(move || async move {
        let _ = refresh();
        crate::api::list_providers().await.ok()
    });

    // Auto-refresh every 10 seconds
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            refresh += 1;
        }
    });

    // Trigger immediate refresh on provider/gateway events
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            if matches!(
                last.event_type,
                EventType::HeartbeatOk
                    | EventType::GatewayRequestEnd
                    | EventType::GatewayRateLimited
            ) {
                refresh += 1;
            }
        }
    });

    let ollama_read = ollama_status.read();
    let ollama = (*ollama_read).as_ref().and_then(|o| o.as_ref());

    let conn_class = if ollama.is_some_and(|o| o.connected) {
        "healthy"
    } else {
        "unhealthy"
    };
    let conn_label = if ollama.is_some_and(|o| o.connected) {
        "Connected"
    } else {
        "Disconnected"
    };
    let gateway_url = ollama.map_or("—", |o| o.url.as_str());
    let model_count = ollama.map_or(0, |o| o.available_models.len());
    let error_msg = ollama.and_then(|o| o.error.as_deref());

    let providers_read = providers.read();
    let provider_list = (*providers_read)
        .as_ref()
        .and_then(|o| o.as_ref())
        .map(|r| &r.providers);

    rsx! {
        div { class: "page-panel panel-page",
            // ── Ollama Status ───────────────────────────────
            div { class: "section-header", h2 { "Ollama Connection" } }
            div { class: "health-row",
                div { class: "health-indicator",
                    span { class: "health-dot {conn_class}" }
                    span { "Gateway: {conn_label}" }
                }
            }
            div { class: "metrics-grid",
                div { class: "metric-card",
                    span { class: "metric-value", "{gateway_url}" }
                    span { class: "metric-label", "Gateway URL" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{model_count}" }
                    span { class: "metric-label", "Available Models" }
                }
            }

            if let Some(err) = error_msg {
                div { class: "state-message",
                    span { class: "state-icon", "\u{26A0}" }
                    span { "{err}" }
                }
            }

            // ── Available Models ────────────────────────────
            if let Some(o) = ollama {
                if !o.available_models.is_empty() {
                    div { class: "section-header", h2 { "Available Models" } }
                    div { class: "data-table",
                        div { class: "table-header",
                            span { class: "col-name", "Model Name" }
                        }
                        for model in o.available_models.iter() {
                            div { class: "table-row",
                                span { class: "col-name", "{model}" }
                            }
                        }
                    }
                }
            }

            // ── Model Providers ─────────────────────────────
            div { class: "section-header", h2 { "Model Providers" } }
            match provider_list {
                Some(list) if !list.is_empty() => rsx! {
                    div { class: "data-table",
                        div { class: "table-header",
                            span { class: "col-name", "Name" }
                            span { class: "col-type", "Type" }
                            span { class: "col-status", "Status" }
                        }
                        for p in list.iter() {
                            {
                                let status_class = if p.enabled { "status-ok" } else { "status-failed" };
                                let status_label = if p.enabled { "Enabled" } else { "Disabled" };
                                rsx! {
                                    div { class: "table-row",
                                        span { class: "col-name", "{p.name}" }
                                        span { class: "col-type", "{p.provider_type}" }
                                        span { class: "col-status {status_class}", "{status_label}" }
                                    }
                                }
                            }
                        }
                    }
                },
                _ => rsx! {
                    div { class: "state-message",
                        span { "No model providers configured." }
                    }
                },
            }
        }
    }
}
