//! Task queue panel with filters, sorting, pagination, and CRUD actions.

use carnelian_common::types::{EventType, TaskDetail};
use dioxus::prelude::*;

use crate::store::EventStreamStore;

/// Page size for client-side pagination.
const PAGE_SIZE: usize = 50;

/// Task queue page.
#[component]
pub fn Tasks() -> Element {
    let store = use_context::<EventStreamStore>();

    // ── Data fetching (signal-driven refresh) ───────────────
    let mut refresh = use_signal(|| 0_u64);

    let tasks_resource = use_resource(move || async move {
        let _ = refresh();
        crate::api::list_tasks()
            .await
            .map(|r| r.tasks)
            .unwrap_or_default()
    });

    // Auto-refresh every 3 seconds.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            refresh += 1;
        }
    });

    // Trigger refresh on relevant task events from WebSocket.
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            match &last.event_type {
                EventType::TaskCreated
                | EventType::TaskStarted
                | EventType::TaskCompleted
                | EventType::TaskFailed
                | EventType::TaskCancelled => {
                    refresh += 1;
                }
                _ => {}
            }
        }
    });

    // ── Local UI state ──────────────────────────────────────
    let mut filter_status = use_signal(|| "All".to_string());
    let mut filter_search = use_signal(String::new);
    let sort_col = use_signal(|| "created_at".to_string());
    let sort_asc = use_signal(|| false);
    let mut current_page = use_signal(|| 1_usize);
    let mut show_create = use_signal(|| false);

    // ── Derived: filtered + sorted + paginated ──────────────
    let tasks_read = tasks_resource.read();
    let all_tasks: Vec<TaskDetail> = (*tasks_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);

    let filtered = filter_tasks(&all_tasks, &filter_status.read(), &filter_search.read());
    let sorted = sort_tasks(filtered, &sort_col.read(), *sort_asc.read());
    let total = sorted.len();
    let total_pages = (total + PAGE_SIZE - 1).max(1) / PAGE_SIZE.max(1);
    let page = (*current_page.read()).min(total_pages).max(1);
    let page_tasks: Vec<&TaskDetail> = sorted
        .into_iter()
        .skip((page - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .collect();

    rsx! {
        div { class: "page-panel panel-page",
            // ── Filter bar ──────────────────────────────────
            div { class: "filter-bar",
                select {
                    class: "filter-select",
                    aria_label: "Filter by status",
                    value: "{filter_status}",
                    onchange: move |e| {
                        filter_status.set(e.value());
                        current_page.set(1);
                    },
                    option { value: "All", "All Statuses" }
                    option { value: "pending", "Pending" }
                    option { value: "running", "Running" }
                    option { value: "completed", "Completed" }
                    option { value: "failed", "Failed" }
                    option { value: "cancelled", "Cancelled" }
                }
                input {
                    class: "filter-input",
                    r#type: "text",
                    placeholder: "Search title / description\u{2026}",
                    aria_label: "Search tasks",
                    value: "{filter_search}",
                    oninput: move |e| {
                        filter_search.set(e.value());
                        current_page.set(1);
                    },
                }
                div { class: "filter-bar-actions",
                    button {
                        class: "btn-secondary btn-sm",
                        onclick: move |_| { refresh += 1; },
                        "\u{21BB} Refresh"
                    }
                    button {
                        class: "btn-primary btn-sm",
                        onclick: move |_| show_create.set(true),
                        "+ Create Task"
                    }
                }
            }

            // ── Table ───────────────────────────────────────
            if tasks_read.is_none() {
                div { class: "state-message",
                    div { class: "spinner" }
                    span { "Loading tasks\u{2026}" }
                }
            } else if page_tasks.is_empty() {
                div { class: "state-message",
                    span { class: "state-icon", "\u{1F4CB}" }
                    span { "No tasks match the current filters." }
                }
            } else {
                div { class: "panel-scroll",
                    table { class: "data-table",
                        thead {
                            tr {
                                { sortable_th("ID", "task_id", &sort_col, &sort_asc) }
                                { sortable_th("Title", "title", &sort_col, &sort_asc) }
                                { sortable_th("Status", "state", &sort_col, &sort_asc) }
                                { sortable_th("Priority", "priority", &sort_col, &sort_asc) }
                                { sortable_th("Created", "created_at", &sort_col, &sort_asc) }
                                th { "Actions" }
                            }
                        }
                        tbody {
                            for task in page_tasks {
                                { render_task_row(task, &refresh) }
                            }
                        }
                    }
                }

                // ── Pagination ──────────────────────────────
                div { class: "pagination",
                    button {
                        class: "pagination-btn",
                        disabled: page <= 1,
                        onclick: move |_| current_page.set(page.saturating_sub(1).max(1)),
                        "\u{25C0} Prev"
                    }
                    span { class: "pagination-info", "Page {page} of {total_pages}" }
                    button {
                        class: "pagination-btn",
                        disabled: page >= total_pages,
                        onclick: move |_| current_page.set((page + 1).min(total_pages)),
                        "Next \u{25B6}"
                    }
                }
            }

            // ── Create Task Modal ───────────────────────────
            if *show_create.read() {
                CreateTaskModal {
                    on_close: move || show_create.set(false),
                    on_created: move || {
                        show_create.set(false);
                        refresh += 1;
                    },
                }
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────

fn filter_tasks<'a>(tasks: &'a [TaskDetail], status: &str, search: &str) -> Vec<&'a TaskDetail> {
    let search_lower = search.to_lowercase();
    tasks
        .iter()
        .filter(|t| status == "All" || t.state == status)
        .filter(|t| {
            if search_lower.is_empty() {
                return true;
            }
            t.title.to_lowercase().contains(&search_lower)
                || t.description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&search_lower)
        })
        .collect()
}

fn sort_tasks<'a>(mut tasks: Vec<&'a TaskDetail>, col: &str, asc: bool) -> Vec<&'a TaskDetail> {
    tasks.sort_by(|a, b| {
        let ord = match col {
            "task_id" => a.task_id.to_string().cmp(&b.task_id.to_string()),
            "title" => a.title.cmp(&b.title),
            "state" => a.state.cmp(&b.state),
            "priority" => a.priority.cmp(&b.priority),
            _ => a.created_at.cmp(&b.created_at),
        };
        if asc { ord } else { ord.reverse() }
    });
    tasks
}

fn status_badge_class(state: &str) -> &'static str {
    match state {
        "pending" => "badge-status badge-pending",
        "running" => "badge-status badge-running",
        "completed" => "badge-status badge-completed",
        "failed" => "badge-status badge-failed",
        "cancelled" => "badge-status badge-cancelled",
        _ => "badge-status",
    }
}

fn sortable_th(
    label: &'static str,
    col: &'static str,
    sort_col: &Signal<String>,
    sort_asc: &Signal<bool>,
) -> Element {
    let mut sc = *sort_col;
    let mut sa = *sort_asc;
    let current_col = sort_col.read().clone();
    let current_asc = *sort_asc.read();
    let indicator = if current_col == col {
        if current_asc { "\u{25B2}" } else { "\u{25BC}" }
    } else {
        ""
    };
    rsx! {
        th {
            onclick: move |_| {
                if *sc.read() == col {
                    let old = *sa.read();
                    sa.set(!old);
                } else {
                    sc.set(col.to_string());
                    sa.set(true);
                }
            },
            "{label} "
            span { class: "sort-indicator", "{indicator}" }
        }
    }
}

fn render_task_row(task: &TaskDetail, refresh: &Signal<u64>) -> Element {
    let badge = status_badge_class(&task.state);
    let id_short = task.task_id.to_string();
    let id_display = &id_short[..8.min(id_short.len())];
    let created = task.created_at.format("%Y-%m-%d %H:%M").to_string();
    let can_cancel = task.state == "pending" || task.state == "running";
    let can_retry = task.state == "failed";
    let task_id = task.task_id;
    let title_clone = task.title.clone();
    let desc_clone = task.description.clone();
    let skill_clone = task.skill_id;
    let priority = task.priority;
    let mut refresh = *refresh;

    rsx! {
        tr {
            td { class: "cell-mono", "{id_display}" }
            td { "{task.title}" }
            td { span { class: "{badge}", "{task.state}" } }
            td { "{task.priority}" }
            td { "{created}" }
            td {
                if can_cancel {
                    button {
                        class: "btn-danger btn-sm",
                        onclick: move |_| {
                            let tid = task_id;
                            spawn(async move {
                                let _ = crate::api::cancel_task(tid, "User cancelled".to_string()).await;
                                refresh += 1;
                            });
                        },
                        "Cancel"
                    }
                }
                if can_retry {
                    button {
                        class: "btn-secondary btn-sm",
                        onclick: {
                            move |_| {
                                let t = title_clone.clone();
                                let d = desc_clone.clone();
                                let s = skill_clone;
                                let p = priority;
                                spawn(async move {
                                    let _ = crate::api::create_task(t, d, s, p).await;
                                    refresh += 1;
                                });
                            }
                        },
                        "Retry"
                    }
                }
            }
        }
    }
}

// ── Create Task Modal ───────────────────────────────────────

#[component]
fn CreateTaskModal(on_close: EventHandler, on_created: EventHandler) -> Element {
    let mut title = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut priority = use_signal(|| 0_i32);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut submitting = use_signal(|| false);

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Create Task",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Create Task" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    div { class: "form-group",
                        label { class: "form-label", "Title" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "Task title",
                            value: "{title}",
                            oninput: move |e| title.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Description" }
                        textarea {
                            class: "form-textarea",
                            placeholder: "Optional description\u{2026}",
                            value: "{description}",
                            oninput: move |e| description.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Priority" }
                        input {
                            class: "form-input",
                            r#type: "number",
                            value: "{priority}",
                            oninput: move |e| {
                                if let Ok(v) = e.value().parse::<i32>() {
                                    priority.set(v);
                                }
                            },
                        }
                    }
                    if let Some(err) = &*error_msg.read() {
                        p { style: "color: #E74C3C; font-size: 13px;", "{err}" }
                    }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn-secondary",
                        onclick: move |_| on_close.call(()),
                        "Cancel"
                    }
                    button {
                        class: "btn-primary",
                        disabled: *submitting.read(),
                        onclick: move |_| {
                            let t = title.read().clone();
                            if t.trim().is_empty() {
                                error_msg.set(Some("Title is required.".to_string()));
                                return;
                            }
                            submitting.set(true);
                            let d = {
                                let v = description.read().clone();
                                if v.trim().is_empty() { None } else { Some(v) }
                            };
                            let p = *priority.read();
                            spawn(async move {
                                match crate::api::create_task(t, d, None, p).await {
                                    Ok(_) => on_created.call(()),
                                    Err(e) => {
                                        error_msg.set(Some(e));
                                        submitting.set(false);
                                    }
                                }
                            });
                        },
                        if *submitting.read() { "Creating\u{2026}" } else { "Create" }
                    }
                }
            }
        }
    }
}
