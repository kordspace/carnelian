//! Elixirs page — manage skill configurations with filtering, sorting, CRUD, and drafts.

#![allow(clippy::nonminimal_bool)]

use crate::api;
use crate::components::{Toast, ToastMessage, ToastType};
use crate::theme::Theme;
use carnelian_common::types::{ElixirDetail, ElixirDraft, ListElixirsQuery};
use dioxus::prelude::*;
use uuid::Uuid;

#[component]
pub fn Elixirs() -> Element {
    let theme = use_context::<Theme>();
    let mut toasts = use_signal(Vec::<ToastMessage>::new);
    let mut active_tab = use_signal(|| "library".to_string());
    let mut elixirs = use_signal(Vec::<ElixirDetail>::new);
    let mut elixirs_total = use_signal(|| 0i64);
    let mut loading_elixirs = use_signal(|| false);
    let mut drafts = use_signal(Vec::<ElixirDraft>::new);
    let mut loading_drafts = use_signal(|| false);
    let mut filter_type = use_signal(String::new);
    let mut search_query = use_signal(String::new);
    let mut sort_field = use_signal(|| "name".to_string());
    let mut sort_asc = use_signal(|| true);
    let mut selected_elixir = use_signal(|| None::<ElixirDetail>);
    let mut show_detail = use_signal(|| false);
    let mut selected_draft_ids = use_signal(Vec::<Uuid>::new);

    let load_elixirs = move || {
        spawn(async move {
            loading_elixirs.set(true);
            let query_str = search_query.read().clone();

            let result = if !query_str.is_empty() {
                api::elixirs_search(query_str, 50)
                    .await
                    .map(|resp| (resp.results, resp.total))
            } else {
                let filter_type_val = filter_type.read().clone();
                let query = ListElixirsQuery {
                    elixir_type: if filter_type_val.is_empty() {
                        None
                    } else {
                        Some(filter_type_val)
                    },
                    skill_id: None,
                    active: None,
                    page: 1,
                    page_size: 50,
                };
                api::elixirs_list(query)
                    .await
                    .map(|resp| (resp.elixirs, resp.total))
            };

            match result {
                Ok((elixirs_data, total)) => {
                    elixirs.set(elixirs_data);
                    elixirs_total.set(total);
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to load elixirs: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
            loading_elixirs.set(false);
        });
    };

    let load_drafts = move || {
        spawn(async move {
            loading_drafts.set(true);
            match api::elixirs_drafts_list().await {
                Ok(resp) => {
                    drafts.set(resp.drafts);
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to load drafts: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
            loading_drafts.set(false);
        });
    };

    use_hook(|| {
        load_elixirs();
        load_drafts();
    });

    let approve_draft = move |draft_id: Uuid| {
        spawn(async move {
            match api::elixirs_draft_approve(draft_id).await {
                Ok(_) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: "Draft approved successfully".to_string(),
                        toast_type: ToastType::Success,
                        duration_secs: 5,
                    });
                    load_drafts();
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to approve draft: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
        });
    };

    let reject_draft = move |draft_id: Uuid| {
        spawn(async move {
            match api::elixirs_draft_reject(draft_id).await {
                Ok(_) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: "Draft rejected successfully".to_string(),
                        toast_type: ToastType::Success,
                        duration_secs: 5,
                    });
                    load_drafts();
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to reject draft: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
        });
    };

    let approve_batch = move || {
        let ids = selected_draft_ids.read().clone();
        spawn(async move {
            let mut success_count = 0;
            let mut error_count = 0;

            for draft_id in &ids {
                match api::elixirs_draft_approve(*draft_id).await {
                    Ok(_) => success_count += 1,
                    Err(_) => error_count += 1,
                }
            }

            if success_count > 0 {
                toasts.write().push(ToastMessage {
                    id: Uuid::new_v4().to_string(),
                    message: format!("✅ {success_count} draft(s) approved"),
                    toast_type: ToastType::Success,
                    duration_secs: 5,
                });
            }
            if error_count > 0 {
                toasts.write().push(ToastMessage {
                    id: Uuid::new_v4().to_string(),
                    message: format!("❌ {error_count} draft(s) failed"),
                    toast_type: ToastType::Error,
                    duration_secs: 5,
                });
            }

            selected_draft_ids.set(Vec::new());
            load_drafts();
        });
    };

    let mut open_detail = move |elixir: ElixirDetail| {
        selected_elixir.set(Some(elixir));
        show_detail.set(true);
    };

    let mut close_detail = move || {
        show_detail.set(false);
    };

    let mut export_json = move |elixir: ElixirDetail| match serde_json::to_string_pretty(&elixir) {
        Ok(json_str) => {
            toasts.write().push(ToastMessage {
                id: Uuid::new_v4().to_string(),
                message: format!("Exported JSON:\n{json_str}"),
                toast_type: ToastType::Success,
                duration_secs: 5,
            });
        }
        Err(e) => {
            toasts.write().push(ToastMessage {
                id: Uuid::new_v4().to_string(),
                message: format!("Failed to export: {e}"),
                toast_type: ToastType::Error,
                duration_secs: 5,
            });
        }
    };

    let mut display_elixirs = elixirs.read().clone();
    let sort_field_val = sort_field.read().clone();
    let sort_asc_val = *sort_asc.read();

    display_elixirs.sort_by(|a, b| {
        let cmp = match sort_field_val.as_str() {
            "quality_score" => a
                .quality_score
                .partial_cmp(&b.quality_score)
                .unwrap_or(std::cmp::Ordering::Equal),
            "created_at" => a.created_at.cmp(&b.created_at),
            _ => a.name.cmp(&b.name),
        };
        if sort_asc_val { cmp } else { cmp.reverse() }
    });

    let pending_drafts: Vec<ElixirDraft> = drafts
        .read()
        .iter()
        .filter(|d| d.status == "pending")
        .cloned()
        .collect();

    let theme_class = theme.to_class();

    let active_tab_val = active_tab.read().clone();
    let drafts_pending_count = pending_drafts.len();

    rsx! {
        div { class: "page elixirs-page {theme_class}",
            div { class: "page-header",
                h1 { "Elixirs 🧪" }
                p { class: "subtitle", "Curated elixir library and draft review queue" }
            }

            div { class: "tab-bar",
                button {
                    class: if active_tab_val == "library" { "tab active" } else { "tab" },
                    onclick: move |_| active_tab.set("library".to_string()),
                    "Library"
                }
                button {
                    class: if active_tab_val == "drafts" { "tab active" } else { "tab" },
                    onclick: move |_| active_tab.set("drafts".to_string()),
                    "Drafts Queue"
                    if drafts_pending_count > 0 {
                        span { class: "badge", "{drafts_pending_count}" }
                    }
                }
            }

            if active_tab_val == "library" {
                div { class: "filter-bar",
                    select {
                        value: "{filter_type.read()}",
                        oninput: move |evt| {
                            filter_type.set(evt.value());
                            load_elixirs();
                        },
                        option { value: "", "All Types" }
                        option { value: "prompt", "Prompt" }
                        option { value: "context", "Context" }
                        option { value: "tool", "Tool" }
                        option { value: "workflow", "Workflow" }
                    }

                    input {
                        r#type: "text",
                        placeholder: "Search elixirs...",
                        value: "{search_query.read()}",
                        oninput: move |evt| {
                            search_query.set(evt.value());
                            load_elixirs();
                        }
                    }

                    select {
                        value: "{sort_field.read()}",
                        oninput: move |evt| sort_field.set(evt.value()),
                        option { value: "name", "Name" }
                        option { value: "quality_score", "Quality Score" }
                        option { value: "created_at", "Created At" }
                    }

                    button {
                        onclick: move |_| {
                            let current = *sort_asc.read();
                            sort_asc.set(!current);
                        },
                        if *sort_asc.read() { "↑" } else { "↓" }
                    }
                }

                if *loading_elixirs.read() {
                    div { class: "loading", "Loading elixirs..." }
                } else if display_elixirs.is_empty() {
                    div { class: "empty-state", "No elixirs found" }
                } else {
                    div { class: "elixirs-grid",
                        for elixir in display_elixirs.iter() {
                            div {
                                key: "{elixir.elixir_id}",
                                class: "elixir-card",
                                onclick: {
                                    let elixir_clone = elixir.clone();
                                    move |_| open_detail(elixir_clone.clone())
                                },
                                div { class: "card-header",
                                    span { class: "elixir-icon", "🧪" }
                                    h3 { "{elixir.name}" }
                                    span { class: "type-badge", "{elixir.elixir_type}" }
                                }
                                div { class: "quality-gauge",
                                    span { class: "gauge-label", "Quality" }
                                    div { class: "gauge-bar",
                                        div {
                                            class: "gauge-fill",
                                            style: "width: {elixir.quality_score * 100.0}%"
                                        }
                                    }
                                    span { class: "gauge-value", "{elixir.quality_score * 100.0:.0}%" }
                                }
                                div { class: "card-footer",
                                    span { class: "version", "v{elixir.version}" }
                                    if elixir.active {
                                        span { class: "badge-active", "✅ Active" }
                                    } else {
                                        span { class: "badge-inactive", "Inactive" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if active_tab_val == "drafts" {
                if *loading_drafts.read() {
                    div { class: "loading", "Loading drafts..." }
                } else if pending_drafts.is_empty() {
                    div { class: "empty-state",
                        span { "🎉 No pending drafts" }
                        p { "Auto-drafts appear here when a skill reaches 100 uses." }
                    }
                } else {
                    div { class: "drafts-toolbar",
                        button {
                            class: "btn-primary",
                            disabled: selected_draft_ids.read().is_empty(),
                            onclick: move |_| approve_batch(),
                            "✅ Approve Selected ({selected_draft_ids.read().len()})"
                        }
                        span { class: "draft-count", "{pending_drafts.len()} pending" }
                    }

                    div { class: "drafts-list",
                        for draft in pending_drafts.iter() {
                            div {
                                key: "{draft.draft_id}",
                                class: "draft-row",
                                input {
                                    r#type: "checkbox",
                                    checked: selected_draft_ids.read().contains(&draft.draft_id),
                                    oninput: {
                                        let draft_id = draft.draft_id;
                                        move |_| {
                                            let mut ids = selected_draft_ids.write();
                                            if ids.contains(&draft_id) {
                                                ids.retain(|id| *id != draft_id);
                                            } else {
                                                ids.push(draft_id);
                                            }
                                        }
                                    }
                                }

                                div { class: "draft-info",
                                    span { class: "draft-name", "{draft.proposed_name}" }
                                    if let Some(ref reason) = draft.auto_created_reason {
                                        span { class: "draft-reason", "{reason}" }
                                    }
                                    div { class: "dataset-preview",
                                        {
                                            let dataset_str = serde_json::to_string(&draft.dataset)
                                                .unwrap_or_default();
                                            let preview = if dataset_str.len() > 120 {
                                                format!("{}...", &dataset_str[..120])
                                            } else {
                                                dataset_str
                                            };
                                            rsx! { "{preview}" }
                                        }
                                    }
                                }

                                div { class: "draft-actions",
                                    button {
                                        class: "btn-primary btn-sm",
                                        onclick: {
                                            let draft_id = draft.draft_id;
                                            move |_| approve_draft(draft_id)
                                        },
                                        "✅ Approve"
                                    }
                                    button {
                                        class: "btn-secondary btn-sm",
                                        onclick: {
                                            let draft_id = draft.draft_id;
                                            move |_| reject_draft(draft_id)
                                        },
                                        "❌ Reject"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if *show_detail.read() {
                if let Some(ref elixir) = *selected_elixir.read() {
                    div {
                        class: "detail-overlay",
                        onclick: move |_| close_detail(),

                        div {
                            class: "detail-panel",
                            onclick: move |e| e.stop_propagation(),

                            div { class: "panel-header",
                                h2 { "🧪 {elixir.name}" }
                                button {
                                    class: "btn-icon",
                                    onclick: move |_| close_detail(),
                                    "✕"
                                }
                            }

                            div { class: "panel-body",
                                div { class: "detail-section",
                                    label { "Type" }
                                    span { "{elixir.elixir_type}" }
                                    label { "Status" }
                                    span { if elixir.active { "Active" } else { "Inactive" } }
                                    label { "Version" }
                                    span { "v{elixir.version}" }
                                    if let Some(ref desc) = elixir.description {
                                        label { "Description" }
                                        p { "{desc}" }
                                    }
                                }

                                div { class: "detail-section",
                                    label { "Quality Score" }
                                    div { class: "quality-gauge-large",
                                        div {
                                            class: "gauge-fill",
                                            style: "width: {elixir.quality_score * 100.0}%"
                                        }
                                        span { "{elixir.quality_score * 100.0:.1}%" }
                                    }
                                    label { "Dataset Size" }
                                    span { "{elixir.size_bytes} bytes" }
                                    label { "Created" }
                                    span { "{elixir.created_at}" }
                                }

                                div { class: "detail-section",
                                    h4 { "Version History" }
                                    table { class: "version-table",
                                        thead {
                                            tr {
                                                th { "Version" }
                                                th { "Date" }
                                                th { "" }
                                            }
                                        }
                                        tbody {
                                            tr {
                                                td { "v{elixir.version}" }
                                                td { "{elixir.updated_at}" }
                                                td {
                                                    button {
                                                        class: "btn-secondary btn-sm",
                                                        disabled: true,
                                                        "Current"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    p { class: "hint", "Full history available via detail endpoint (future)" }
                                }
                            }

                            div { class: "panel-footer",
                                button {
                                    class: "btn-secondary",
                                    onclick: {
                                        let elixir_clone = elixir.clone();
                                        move |_| export_json(elixir_clone.clone())
                                    },
                                    "📤 Export JSON"
                                }
                            }
                        }
                    }
                }
            }

            for toast in toasts.read().iter() {
                Toast { toast: toast.clone() }
            }
        }
    }
}
