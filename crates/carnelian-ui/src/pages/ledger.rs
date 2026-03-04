//! Ledger Viewer page — browse and verify ledger events.
//!
//! Features:
//! - Paginated table of ledger events
//! - Filter by action type, actor, date range
//! - Chain integrity verification button
//! - Auto-refresh on `LedgerEvent` WebSocket events

use dioxus::prelude::*;

use crate::api;
use crate::components::{Toast, ToastMessage, ToastType};
use crate::store::EventStreamStore;
use crate::theme::Theme;
use carnelian_common::types::{LedgerEventDetail, LedgerVerifyResponse};

/// Format a ledger event for display.
fn format_event(event: &LedgerEventDetail) -> String {
    format!(
        "{} | {} | {}",
        event.timestamp,
        event.action_type,
        if event.actor_id.is_empty() { "system" } else { &event.actor_id }
    )
}

/// Ledger Viewer page component.
#[component]
pub fn Ledger() -> Element {
    let theme = use_context::<Theme>();
    let event_store = use_context::<EventStreamStore>();
    let toasts = use_signal(Vec::new);

    // Filter state
    let mut action_type_filter = use_signal(String::new);
    let mut actor_filter = use_signal(String::new);
    let mut from_ts_filter = use_signal(String::new);
    let mut to_ts_filter = use_signal(String::new);

    // Data state
    let events = use_signal(Vec::<LedgerEventDetail>::new);
    let total_count = use_signal(|| 0i64);
    let offset = use_signal(|| 0i64);
    let limit = use_signal(|| 50i64);
    let verify_result = use_signal(|| None::<LedgerVerifyResponse>);
    let loading = use_signal(|| false);

    const PAGE_SIZE: i64 = 50;

    // Load events function
    let load_events = {
        let mut events = events.clone();
        let mut total_count = total_count.clone();
        let mut loading = loading.clone();
        let action_type_filter = action_type_filter.clone();
        let actor_filter = actor_filter.clone();
        let from_ts_filter = from_ts_filter.clone();
        let to_ts_filter = to_ts_filter.clone();
        let offset = offset.clone();
        let limit = limit.clone();

        move || {
            loading.set(true);
            let action_type = action_type_filter.read().clone();
            let actor_id = actor_filter.read().clone();
            let from_ts = from_ts_filter.read().clone();
            let to_ts = to_ts_filter.read().clone();
            let current_offset = *offset.read();
            let current_limit = *limit.read();

            spawn(async move {
                match api::list_ledger_events(
                    current_limit,
                    current_offset,
                    if action_type.is_empty() {
                        None
                    } else {
                        Some(action_type)
                    },
                    if actor_id.is_empty() {
                        None
                    } else {
                        Some(actor_id)
                    },
                    if from_ts.is_empty() {
                        None
                    } else {
                        Some(from_ts)
                    },
                    if to_ts.is_empty() { None } else { Some(to_ts) },
                )
                .await
                {
                    Ok(response) => {
                        events.set(response.events);
                        total_count.set(response.total);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to load ledger events");
                    }
                }
                loading.set(false);
            });
        }
    };

    // Initial load
    use_hook({
        // Dioxus Signal<T> requires .clone() for multi-closure capture
        #[allow(clippy::clone_on_copy)]
        let mut load_events = load_events.clone();
        move || {
            load_events();
        }
    });

    // Auto-refresh on ledger events
    use_effect({
        let mut last_check = use_signal(|| 0u64);
        let event_store = event_store;
        // Dioxus Signal<T> requires .clone() for multi-closure capture
        #[allow(clippy::clone_on_copy)]
        let mut load_events = load_events.clone();

        move || {
            let current_count = event_store.event_count.read();
            if *current_count > *last_check.read() {
                last_check.set(*current_count);
                load_events();
            }
        }
    });

    // Verify chain handler
    let verify_chain = {
        // Dioxus Signal<T> requires .clone() for multi-closure capture
        #[allow(clippy::clone_on_copy)]
        let mut verify_result = verify_result.clone();
        #[allow(clippy::clone_on_copy)]
        let mut toasts = toasts.clone();

        move || {
            spawn(async move {
                match api::verify_ledger_chain().await {
                    Ok(result) => {
                        let toast = ToastMessage {
                            id: uuid::Uuid::now_v7().to_string(),
                            message: if result.intact {
                                "✅ Ledger chain integrity verified — all events intact".to_string()
                            } else {
                                "❌ Ledger chain tampered — integrity check failed".to_string()
                            },
                            toast_type: if result.intact {
                                ToastType::Success
                            } else {
                                ToastType::Error
                            },
                            duration_secs: 5,
                        };
                        toasts.push(toast);
                        verify_result.set(Some(result));
                    }
                    Err(e) => {
                        let toast = ToastMessage {
                            id: uuid::Uuid::now_v7().to_string(),
                            message: format!("Failed to verify ledger: {e}"),
                            toast_type: ToastType::Error,
                            duration_secs: 5,
                        };
                        toasts.push(toast);
                    }
                }
            });
        }
    };

    // Pagination handlers
    let mut prev_page = {
        let mut offset = offset.clone();
        let mut load_events = load_events.clone();
        move || {
            let new_offset = (*offset.read() - PAGE_SIZE).max(0);
            offset.set(new_offset);
            load_events();
        }
    };

    let mut next_page = {
        let mut offset = offset.clone();
        let mut load_events = load_events.clone();
        move || {
            let new_offset = *offset.read() + PAGE_SIZE;
            if new_offset < *total_count.read() {
                offset.set(new_offset);
                load_events();
            }
        }
    };

    // Apply filters handler
    let mut apply_filters = {
        let mut offset = offset.clone();
        let mut load_events = load_events.clone();
        move || {
            offset.set(0);
            load_events();
        }
    };

    let theme_class = theme.to_class();

    rsx! {
        div { class: "page ledger-page {theme_class}",
            // Header
            div { class: "page-header",
                h1 { "Ledger" }
                p { class: "subtitle", "Tamper-resistant audit trail of all system events" }
            }

            // Verification banner
            if let Some(ref result) = *verify_result.read() {
                div {
                    class: if result.intact { "verify-banner success" } else { "verify-banner error" },
                    if result.intact {
                        "✅ Chain Intact — {result.event_count} events verified"
                    } else {
                        "❌ Chain Tampered — integrity check failed"
                    }
                }
            }

            // Filter bar
            div { class: "filter-bar",
                div { class: "filter-group",
                    label { "Action Type" }
                    input {
                        r#type: "text",
                        placeholder: "e.g., TaskCreated",
                        value: "{action_type_filter}",
                        oninput: move |e| action_type_filter.set(e.value()),
                    }
                }
                div { class: "filter-group",
                    label { "Actor ID" }
                    input {
                        r#type: "text",
                        placeholder: "Actor UUID",
                        value: "{actor_filter}",
                        oninput: move |e| actor_filter.set(e.value()),
                    }
                }
                div { class: "filter-group",
                    label { "From" }
                    input {
                        r#type: "datetime-local",
                        value: "{from_ts_filter}",
                        oninput: move |e| from_ts_filter.set(e.value()),
                    }
                }
                div { class: "filter-group",
                    label { "To" }
                    input {
                        r#type: "datetime-local",
                        value: "{to_ts_filter}",
                        oninput: move |e| to_ts_filter.set(e.value()),
                    }
                }
                button {
                    class: "btn-primary",
                    onclick: move |_| apply_filters(),
                    "Apply Filters"
                }
                button {
                    class: "btn-secondary",
                    onclick: move |_| verify_chain(),
                    "Verify Chain"
                }
            }

            // Events table
            div { class: "ledger-table-container",
                if *loading.read() {
                    div { class: "loading", "Loading ledger events..." }
                } else if events.read().is_empty() {
                    div { class: "empty-state", "No ledger events found" }
                } else {
                    table { class: "ledger-table",
                        thead {
                            tr {
                                th { "Event ID" }
                                th { "Timestamp" }
                                th { "Actor" }
                                th { "Action" }
                                th { "Hash" }
                                th { "Signature" }
                            }
                        }
                        tbody {
                            for event in events.read().iter() {
                                tr { key: "{event.event_id}",
                                    td { class: "mono", "{event.event_id}" }
                                    td { "{event.timestamp}" }
                                    td { class: "mono truncate", "{event.actor_id}" }
                                    td { "{event.action_type}" }
                                    td { class: "mono truncate", "{event.event_hash}" }
                                    td {
                                        if let Some(ref _sig) = event.signature {
                                            span { class: "badge signed", "Signed" }
                                        } else {
                                            span { class: "badge unsigned", "—" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Pagination
            div { class: "pagination",
                button {
                    class: "btn-icon",
                    disabled: *offset.read() == 0,
                    onclick: move |_| prev_page(),
                    "← Previous"
                }
                span {
                    "Showing {*offset.read() + 1}–{(*offset.read() + PAGE_SIZE).min(*total_count.read())} of {*total_count.read()}"
                }
                button {
                    class: "btn-icon",
                    disabled: *offset.read() + PAGE_SIZE >= *total_count.read(),
                    onclick: move |_| next_page(),
                    "Next →"
                }
            }

            // Toasts
            for toast in toasts.read().iter() {
                Toast { toast: toast.clone() }
            }
        }
    }
}
