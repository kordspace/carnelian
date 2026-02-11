//! Dashboard page — task queue summary, system health, and recent events.

use carnelian_common::types::{EventEnvelope, EventLevel};
use dioxus::prelude::*;

use crate::store::EventStreamStore;
use crate::websocket::ConnectionState;

/// Dashboard overview page.
#[component]
pub fn Dashboard() -> Element {
    // Refresh trigger: bump to re-fetch tasks.
    let mut refresh = use_signal(|| 0_u64);

    let tasks = use_resource(move || async move {
        // Read the signal so the resource re-runs when it changes.
        let _ = refresh();
        crate::api::list_tasks()
            .await
            .map(|r| r.tasks)
            .unwrap_or_default()
    });

    // Auto-refresh every 5 seconds.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            refresh += 1;
        }
    });

    let tasks_read = tasks.read();
    let (active, pending, completed, failed) =
        (*tasks_read).as_ref().map_or((0, 0, 0, 0), |list| {
            (
                list.iter().filter(|t| t.state == "running").count(),
                list.iter().filter(|t| t.state == "pending").count(),
                list.iter().filter(|t| t.state == "completed").count(),
                list.iter().filter(|t| t.state == "failed").count(),
            )
        });

    rsx! {
        div { class: "page-panel panel-page",
            // ── Task Queue Metrics ──────────────────────────
            div { class: "section-header", h2 { "Task Queue" } }
            div { class: "metrics-grid",
                MetricCard { value: active, label: "Active", class_mod: "metric-active" }
                MetricCard { value: pending, label: "Pending", class_mod: "metric-pending" }
                MetricCard { value: completed, label: "Completed", class_mod: "metric-completed" }
                MetricCard { value: failed, label: "Failed", class_mod: "metric-failed" }
            }

            SystemHealth {}
            ResourceGauges {}
            RecentEvents {}
        }
    }
}

#[component]
fn MetricCard(value: usize, label: &'static str, class_mod: &'static str) -> Element {
    let card_class = format!("metric-card {class_mod}");
    rsx! {
        div { class: "{card_class}",
            span { class: "metric-value", "{value}" }
            span { class: "metric-label", "{label}" }
        }
    }
}

// ── System Health ───────────────────────────────────────────

#[component]
fn SystemHealth() -> Element {
    let store = use_context::<EventStreamStore>();
    let status = store.system_status.read();
    let conn = store.connection_state.read();

    let worker_class = if status.healthy {
        "healthy"
    } else {
        "unhealthy"
    };
    let worker_label = if status.healthy {
        "Healthy"
    } else {
        "Unhealthy"
    };

    let db_class = match &*conn {
        ConnectionState::Connected => "healthy",
        ConnectionState::Connecting => "unknown",
        _ => "unhealthy",
    };
    let db_label = match &*conn {
        ConnectionState::Connected => "Connected",
        ConnectionState::Connecting => "Connecting",
        _ => "Unavailable",
    };

    rsx! {
        div { class: "section-header", h2 { "System Health" } }
        div { class: "health-row",
            div { class: "health-indicator",
                span { class: "health-dot {worker_class}" }
                span { "Worker: {worker_label}" }
            }
            div { class: "health-indicator",
                span { class: "health-dot {db_class}" }
                span { "Database: {db_label}" }
            }
            div { class: "health-indicator",
                span { class: "health-dot unknown" }
                span { "Model: Standby" }
            }
        }
    }
}

// ── Resource Gauges ─────────────────────────────────────────

#[component]
fn ResourceGauges() -> Element {
    // Placeholder values until a metrics endpoint is available.
    let cpu: f64 = 0.0;
    let mem: f64 = 0.0;

    rsx! {
        div { class: "section-header", h2 { "Resource Usage" } }
        div { class: "gauges-row",
            Gauge { pct: cpu, label: "CPU", color: "#4A90E2" }
            Gauge { pct: mem, label: "Memory", color: "#9B59B6" }
        }
    }
}

#[component]
fn Gauge(pct: f64, label: &'static str, color: &'static str) -> Element {
    let radius: f64 = 40.0;
    let circumference = 2.0 * std::f64::consts::PI * radius;
    let offset = circumference * (1.0 - pct / 100.0);
    let display_pct = format!("{pct:.0}%");

    rsx! {
        div { class: "gauge-container",
            svg {
                width: "100",
                height: "100",
                view_box: "0 0 100 100",
                circle {
                    class: "gauge-background",
                    cx: "50",
                    cy: "50",
                    r: "{radius}",
                    stroke_width: "8",
                }
                circle {
                    class: "gauge-fill",
                    cx: "50",
                    cy: "50",
                    r: "{radius}",
                    stroke_width: "8",
                    stroke: "{color}",
                    stroke_dasharray: "{circumference}",
                    stroke_dashoffset: "{offset}",
                }
                // Percentage text (not rotated — sits on top of the rotated SVG)
            }
            span { class: "gauge-label", "{label}: {display_pct}" }
        }
    }
}

// ── Recent Events ───────────────────────────────────────────

#[component]
fn RecentEvents() -> Element {
    let store = use_context::<EventStreamStore>();
    let events = store.events.read();

    let recent: Vec<&EventEnvelope> = events.iter().rev().take(10).collect();

    rsx! {
        div { class: "section-header", h2 { "Recent Events" } }
        if recent.is_empty() {
            div { class: "state-message",
                span { class: "state-icon", "\u{1F4ED}" }
                span { "No events received yet." }
            }
        } else {
            div { class: "event-list",
                for evt in recent {
                    { render_event_row(evt) }
                }
            }
        }
    }
}

const fn level_class(level: EventLevel) -> &'static str {
    match level {
        EventLevel::Error => "error",
        EventLevel::Warn => "warn",
        EventLevel::Info => "info",
        EventLevel::Debug => "debug",
        EventLevel::Trace => "trace",
    }
}

fn render_event_row(evt: &EventEnvelope) -> Element {
    let lc = level_class(evt.level);
    let row_class = match evt.level {
        EventLevel::Error => "event-row level-error",
        EventLevel::Warn => "event-row level-warn",
        _ => "event-row",
    };
    let ts = evt.timestamp.format("%H:%M:%S%.3f").to_string();
    let etype = format!("{:?}", evt.event_type);
    let msg = evt
        .payload
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(100)
        .collect::<String>();

    rsx! {
        div { class: "{row_class}",
            span { class: "event-timestamp", "{ts}" }
            span { class: "event-level-badge {lc}", "{lc}" }
            span { class: "event-type", "{etype}" }
            if let Some(actor) = &evt.actor_id {
                span { class: "event-actor", "{actor}" }
            }
            span { class: "event-message", "{msg}" }
        }
    }
}
