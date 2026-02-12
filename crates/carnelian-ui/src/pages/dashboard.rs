//! Dashboard page — task queue summary, system health, and recent events.

use carnelian_common::types::{EventEnvelope, EventLevel, MetricsSnapshot};
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
            PerformanceMetrics {}
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
    let mut refresh = use_signal(|| 0_u64);

    let metrics = use_resource(move || async move {
        let _ = refresh();
        crate::api::get_metrics().await.ok()
    });

    // Auto-refresh every 5 seconds.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            refresh += 1;
        }
    });

    let metrics_read = metrics.read();
    let snapshot: Option<&MetricsSnapshot> = (*metrics_read).as_ref().and_then(|o| o.as_ref());

    // Event throughput gauge: scale to 0-100% based on 1000 events/sec = 100%
    let throughput_pct = snapshot.map_or(0.0, |m| {
        (m.event_throughput_per_sec / 1000.0 * 100.0).min(100.0)
    });
    // Task latency P95 gauge: scale to 0-100% based on 2000ms = 100%
    let latency_pct = snapshot.map_or(0.0, |m| (m.task_latency.p95_ms / 2000.0 * 100.0).min(100.0));

    rsx! {
        div { class: "section-header", h2 { "Resource Usage" } }
        div { class: "gauges-row",
            Gauge { pct: throughput_pct, label: "Throughput", color: "#4A90E2" }
            Gauge { pct: latency_pct, label: "Latency P95", color: "#9B59B6" }
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

// ── Performance Metrics ─────────────────────────────────────

#[component]
fn PerformanceMetrics() -> Element {
    let mut refresh = use_signal(|| 0_u64);
    let mut render_time = use_signal(|| 0.0_f64);

    // Measure render duration: capture start time, then update signal in use_effect.
    let render_start = std::time::Instant::now();
    use_effect(move || {
        let elapsed = render_start.elapsed().as_secs_f64() * 1000.0;
        render_time.set(elapsed);
    });

    let metrics = use_resource(move || async move {
        let _ = refresh();
        crate::api::get_metrics().await.ok()
    });

    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            refresh += 1;
        }
    });

    let metrics_read = metrics.read();
    let snapshot: Option<&MetricsSnapshot> = (*metrics_read).as_ref().and_then(|o| o.as_ref());

    let throughput_str = snapshot.map_or_else(String::new, |m| {
        format!("{:.1}", m.event_throughput_per_sec)
    });
    let samples_str =
        snapshot.map_or_else(String::new, |m| format!("{}", m.task_latency.sample_count));
    let received_str = snapshot.map_or_else(String::new, |m| {
        format!("{}", m.event_stream_total_received)
    });
    let subs_str = snapshot.map_or_else(String::new, |m| {
        format!("{}", m.event_stream_subscriber_count)
    });
    let fill_str = snapshot.map_or_else(String::new, |m| {
        format!("{:.1}%", m.event_stream_fill_percentage * 100.0)
    });
    let render_str = format!("{:.1}ms", render_time());

    rsx! {
        div { class: "section-header", h2 { "Performance Metrics" } }
        if let Some(m) = snapshot {
            div { class: "metrics-grid",
                LatencyCard { value: m.task_latency.p50_ms, label: "P50 Latency" }
                LatencyCard { value: m.task_latency.p95_ms, label: "P95 Latency" }
                LatencyCard { value: m.task_latency.p99_ms, label: "P99 Latency" }
                div { class: "metric-card",
                    span { class: "metric-value", "{throughput_str}" }
                    span { class: "metric-label", "Events/sec" }
                }
            }
            div { class: "metrics-grid",
                div { class: "metric-card",
                    span { class: "metric-value", "{samples_str}" }
                    span { class: "metric-label", "Samples" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{received_str}" }
                    span { class: "metric-label", "Events Received" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{subs_str}" }
                    span { class: "metric-label", "Subscribers" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{fill_str}" }
                    span { class: "metric-label", "Buffer Fill" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{render_str}" }
                    span { class: "metric-label", "Render Time" }
                }
            }
        } else {
            div { class: "state-message",
                span { "Metrics unavailable — server may be offline." }
            }
        }
    }
}

#[component]
fn LatencyCard(value: f64, label: &'static str) -> Element {
    let color_class = if value < 500.0 {
        "metric-completed"
    } else if value < 1000.0 {
        "metric-pending"
    } else {
        "metric-failed"
    };
    let card_class = format!("metric-card {color_class}");
    rsx! {
        div { class: "{card_class}",
            span { class: "metric-value", "{value:.0}ms" }
            span { class: "metric-label", "{label}" }
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
