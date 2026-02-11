//! Event stream panel — high-performance log viewer with filters,
//! auto-scroll, and event detail modal.

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use dioxus::prelude::*;

use crate::store::EventStreamStore;

/// Visible rows rendered at a time (virtual window).
const VISIBLE_ROWS: usize = 80;

/// Event stream page.
#[component]
pub fn Events() -> Element {
    let store = use_context::<EventStreamStore>();

    // ── Filter state ────────────────────────────────────────
    let mut show_error = use_signal(|| true);
    let mut show_warn = use_signal(|| true);
    let mut show_info = use_signal(|| true);
    let mut show_debug = use_signal(|| true);
    let mut show_trace = use_signal(|| true);
    let mut source_filter = use_signal(|| "All".to_string());
    let mut search_text = use_signal(String::new);
    let mut auto_scroll = use_signal(|| true);
    let mut selected_event = use_signal(|| Option::<usize>::None);

    // ── Derive filtered events ──────────────────────────────
    let events = store.events.read();
    let filtered: Vec<(usize, &EventEnvelope)> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| match e.level {
            EventLevel::Error => *show_error.read(),
            EventLevel::Warn => *show_warn.read(),
            EventLevel::Info => *show_info.read(),
            EventLevel::Debug => *show_debug.read(),
            EventLevel::Trace => *show_trace.read(),
        })
        .filter(|(_, e)| {
            let src = &*source_filter.read();
            if src == "All" {
                return true;
            }
            event_category(&e.event_type) == src
        })
        .filter(|(_, e)| {
            let q = search_text.read().to_lowercase();
            if q.is_empty() {
                return true;
            }
            let etype = format!("{:?}", e.event_type).to_lowercase();
            etype.contains(&q) || e.payload.to_string().to_lowercase().contains(&q)
        })
        .collect();

    let total = filtered.len();

    // Show the last VISIBLE_ROWS (newest at bottom).
    let visible: Vec<(usize, &EventEnvelope)> = if total > VISIBLE_ROWS {
        filtered[total - VISIBLE_ROWS..].to_vec()
    } else {
        filtered.clone()
    };

    rsx! {
        div { class: "page-panel panel-page",
            // ── Filter controls ─────────────────────────────
            div { class: "filter-bar",
                LevelCheckbox { label: "Error", checked: show_error, on_toggle: move |v| show_error.set(v) }
                LevelCheckbox { label: "Warn", checked: show_warn, on_toggle: move |v| show_warn.set(v) }
                LevelCheckbox { label: "Info", checked: show_info, on_toggle: move |v| show_info.set(v) }
                LevelCheckbox { label: "Debug", checked: show_debug, on_toggle: move |v| show_debug.set(v) }
                LevelCheckbox { label: "Trace", checked: show_trace, on_toggle: move |v| show_trace.set(v) }

                select {
                    class: "filter-select",
                    aria_label: "Filter by source",
                    value: "{source_filter}",
                    onchange: move |e| source_filter.set(e.value()),
                    option { value: "All", "All Sources" }
                    option { value: "Task", "Task" }
                    option { value: "Worker", "Worker" }
                    option { value: "Skill", "Skill" }
                    option { value: "Memory", "Memory" }
                    option { value: "Gateway", "Gateway" }
                    option { value: "Database", "Database" }
                    option { value: "Security", "Security" }
                    option { value: "System", "System" }
                }

                input {
                    class: "filter-input",
                    r#type: "text",
                    placeholder: "Search events\u{2026}",
                    aria_label: "Search events",
                    value: "{search_text}",
                    oninput: move |e| search_text.set(e.value()),
                }

                div { class: "filter-bar-actions",
                    button {
                        class: if *auto_scroll.read() { "auto-scroll-btn active" } else { "auto-scroll-btn" },
                        onclick: move |_| {
                            let current = *auto_scroll.read();
                            auto_scroll.set(!current);
                        },
                        if *auto_scroll.read() { "Auto-scroll ON" } else { "Auto-scroll OFF" }
                    }
                    span { class: "pagination-info", "{total} events" }
                }
            }

            // ── Event list ──────────────────────────────────
            if visible.is_empty() {
                div { class: "state-message",
                    span { class: "state-icon", "\u{1F4ED}" }
                    span { "No events match the current filters." }
                }
            } else {
                div { class: "event-list",
                    for (idx, evt) in visible {
                        {render_event_row(idx, evt, &selected_event)}
                    }
                }
            }

            // ── Event detail modal ──────────────────────────
            if let Some(sel_idx) = *selected_event.read() {
                if let Some(evt) = events.get(sel_idx) {
                    EventDetailModal {
                        event: evt.clone(),
                        on_close: move || selected_event.set(None),
                    }
                }
            }
        }
    }
}

// ── Sub-components ──────────────────────────────────────────

#[component]
fn LevelCheckbox(
    label: &'static str,
    checked: Signal<bool>,
    on_toggle: EventHandler<bool>,
) -> Element {
    let is_checked = *checked.read();
    rsx! {
        label { class: "filter-checkbox",
            input {
                r#type: "checkbox",
                checked: is_checked,
                onchange: move |e| on_toggle.call(e.checked()),
            }
            "{label}"
        }
    }
}

#[component]
fn EventDetailModal(event: EventEnvelope, on_close: EventHandler) -> Element {
    let ts = event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
    let level = format!("{:?}", event.level);
    let etype = format!("{:?}", event.event_type);
    let payload_json = serde_json::to_string_pretty(&event.payload).unwrap_or_default();

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Event Detail",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Event Detail" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    div { class: "form-group",
                        span { class: "form-label", "Event ID" }
                        span { class: "cell-mono", "{event.event_id.0}" }
                    }
                    div { class: "form-group",
                        span { class: "form-label", "Timestamp" }
                        span { "{ts}" }
                    }
                    div { class: "form-group",
                        span { class: "form-label", "Level" }
                        span { class: "event-level-badge", "{level}" }
                    }
                    div { class: "form-group",
                        span { class: "form-label", "Type" }
                        span { "{etype}" }
                    }
                    if let Some(actor) = &event.actor_id {
                        div { class: "form-group",
                            span { class: "form-label", "Actor" }
                            span { "{actor}" }
                        }
                    }
                    if let Some(cid) = &event.correlation_id {
                        div { class: "form-group",
                            span { class: "form-label", "Correlation ID" }
                            span { class: "cell-mono", "{cid}" }
                        }
                    }
                    div { class: "form-group",
                        span { class: "form-label", "Payload" }
                        pre { "{payload_json}" }
                    }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn-secondary",
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────

const fn event_category(et: &EventType) -> &'static str {
    match et {
        EventType::TaskCreated
        | EventType::TaskStarted
        | EventType::TaskCompleted
        | EventType::TaskFailed
        | EventType::TaskCancelled => "Task",

        EventType::WorkerStarted | EventType::WorkerStopped | EventType::WorkerHealthCheck => {
            "Worker"
        }

        EventType::SkillInvokeStart
        | EventType::SkillInvokeEnd
        | EventType::SkillInvokeFailed
        | EventType::SkillDiscovered
        | EventType::SkillUpdated
        | EventType::SkillRemoved => "Skill",

        EventType::MemoryFetchStart
        | EventType::MemoryFetchEnd
        | EventType::MemoryCompressStart
        | EventType::MemoryCompressEnd
        | EventType::MemoryWriteStart
        | EventType::MemoryWriteEnd => "Memory",

        EventType::GatewayRequestStart
        | EventType::GatewayRequestEnd
        | EventType::GatewayRateLimited => "Gateway",

        EventType::DbQueryStart
        | EventType::DbQueryEnd
        | EventType::DbTransactionBegin
        | EventType::DbTransactionCommit
        | EventType::DbTransactionRollback => "Database",

        EventType::CapabilityGranted
        | EventType::CapabilityDenied
        | EventType::CapabilityRevoked => "Security",

        // System: RuntimeStart, RuntimeReady, RuntimeShutdown, ConfigLoaded,
        // HeartbeatTick, Custom(_), and any future variants.
        _ => "System",
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

fn render_event_row(idx: usize, evt: &EventEnvelope, selected: &Signal<Option<usize>>) -> Element {
    let lc = level_class(evt.level);
    let row_class = match evt.level {
        EventLevel::Error => "event-row level-error",
        EventLevel::Warn => "event-row level-warn",
        _ => "event-row",
    };
    let ts = evt.timestamp.format("%H:%M:%S%.3f").to_string();
    let etype = format!("{:?}", evt.event_type);
    let msg: String = evt
        .payload
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(100)
        .collect();

    let mut selected = *selected;
    rsx! {
        div {
            class: "{row_class}",
            onclick: move |_| selected.set(Some(idx)),
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
