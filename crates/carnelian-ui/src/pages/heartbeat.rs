//! Heartbeat page — current mantra, status, and recent heartbeat history.

use carnelian_common::types::EventType;
use dioxus::prelude::*;

use crate::store::EventStreamStore;

/// Heartbeat monitoring page.
#[component]
pub fn Heartbeat() -> Element {
    let store = use_context::<EventStreamStore>();
    let mut refresh = use_signal(|| 0_u64);

    // Fetch recent heartbeats
    let heartbeats = use_resource(move || async move {
        let _ = refresh();
        crate::api::get_recent_heartbeats(10)
            .await
            .unwrap_or_default()
    });

    // Fetch heartbeat status
    let status = use_resource(move || async move {
        let _ = refresh();
        crate::api::get_heartbeat_status().await.ok()
    });

    // Auto-refresh every 5 seconds
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            refresh += 1;
        }
    });

    // Trigger refresh on HeartbeatTick events
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            if matches!(last.event_type, EventType::HeartbeatTick) {
                refresh += 1;
            }
        }
    });

    let status_read = status.read();
    let status_data = (*status_read).as_ref().and_then(|o| o.as_ref());

    let mantra_display = status_data
        .and_then(|s| s.current_mantra.as_deref())
        .unwrap_or("No mantra yet");
    let last_time = status_data.and_then(|s| s.last_heartbeat_time).map_or_else(
        || "Never".to_string(),
        |t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    );
    let next_time = status_data.and_then(|s| s.next_heartbeat_time).map_or_else(
        || "Unknown".to_string(),
        |t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    );
    #[allow(clippy::cast_precision_loss)]
    let interval = status_data.map_or_else(
        || "—".to_string(),
        |s| format!("{:.1}s", s.interval_ms as f64 / 1000.0),
    );

    let heartbeats_read = heartbeats.read();
    let hb_list = (*heartbeats_read).as_ref();

    rsx! {
        div { class: "page-panel panel-page",
            // ── Current Status ──────────────────────────────
            div { class: "section-header", h2 { "Current Status" } }
            div { class: "metrics-grid",
                div { class: "metric-card",
                    span { class: "metric-value", "{mantra_display}" }
                    span { class: "metric-label", "Current Mantra" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{last_time}" }
                    span { class: "metric-label", "Last Heartbeat" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{next_time}" }
                    span { class: "metric-label", "Next Heartbeat" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{interval}" }
                    span { class: "metric-label", "Interval" }
                }
            }

            // ── Recent Heartbeats ───────────────────────────
            div { class: "section-header", h2 { "Recent Heartbeats (Last 10)" } }
            match hb_list {
                Some(list) if !list.is_empty() => rsx! {
                    div { class: "data-table",
                        div { class: "table-header",
                            span { class: "col-timestamp", "Timestamp" }
                            span { class: "col-mantra", "Mantra" }
                            span { class: "col-status", "Status" }
                            span { class: "col-tasks", "Tasks" }
                            span { class: "col-duration", "Duration" }
                        }
                        for hb in list.iter() {
                            {
                                let ts = hb.ts.format("%H:%M:%S").to_string();
                                let mantra = hb.mantra.as_deref().map_or_else(|| "—".to_string(), std::string::ToString::to_string);
                                let status_class = match hb.status.as_str() {
                                    "ok" => "status-ok",
                                    "failed" => "status-failed",
                                    _ => "status-skipped",
                                };
                                let duration = hb.duration_ms
                                    .map_or_else(|| "—".to_string(), |d| format!("{d}ms"));
                                let tasks = format!("{}", hb.tasks_queued);
                                rsx! {
                                    div { class: "table-row",
                                        span { class: "col-timestamp", "{ts}" }
                                        span { class: "col-mantra", "{mantra}" }
                                        span { class: "col-status {status_class}", "{hb.status}" }
                                        span { class: "col-tasks", "{tasks}" }
                                        span { class: "col-duration", "{duration}" }
                                    }
                                }
                            }
                        }
                    }
                },
                _ => rsx! {
                    div { class: "state-message",
                        span { "No heartbeat records yet." }
                    }
                },
            }
        }
    }
}
