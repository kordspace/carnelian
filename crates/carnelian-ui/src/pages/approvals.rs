//! Approval queue panel with filters, batch actions, and confirmation modals.

use carnelian_common::types::{ApprovalRequestDetail, EventType};
use dioxus::prelude::*;

use crate::store::EventStreamStore;

/// Approval queue page.
#[component]
pub fn Approvals() -> Element {
    let store = use_context::<EventStreamStore>();

    // ── Data fetching (signal-driven refresh) ───────────────
    let mut refresh = use_signal(|| 0_u64);

    let approvals_resource = use_resource(move || async move {
        let _ = refresh();
        crate::api::list_pending_approvals(100)
            .await
            .map(|r| r.approvals)
            .unwrap_or_default()
    });

    // Auto-refresh every 3 seconds.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            refresh += 1;
        }
    });

    // Trigger refresh on approval events from WebSocket.
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            match &last.event_type {
                EventType::ApprovalQueued
                | EventType::ApprovalApproved
                | EventType::ApprovalDenied => {
                    refresh += 1;
                }
                _ => {}
            }
        }
    });

    // ── Local UI state ──────────────────────────────────────
    let mut filter_action_type = use_signal(|| "All".to_string());
    let mut selected_ids = use_signal(std::collections::HashSet::<uuid::Uuid>::new);
    let mut show_confirm = use_signal(|| Option::<(uuid::Uuid, String)>::None);
    let mut show_batch_confirm = use_signal(|| false);

    // ── Derived: filtered ───────────────────────────────────
    let approvals_read = approvals_resource.read();
    let all_approvals: Vec<ApprovalRequestDetail> = (*approvals_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);

    let filtered: Vec<&ApprovalRequestDetail> = all_approvals
        .iter()
        .filter(|a| {
            let at = filter_action_type.read();
            *at == "All" || a.action_type == *at
        })
        .collect();

    rsx! {
        div { class: "page-panel panel-page",
            // ── Filter bar ──────────────────────────────────
            div { class: "filter-bar",
                select {
                    class: "filter-select",
                    aria_label: "Filter by action type",
                    value: "{filter_action_type}",
                    onchange: move |e| filter_action_type.set(e.value()),
                    option { value: "All", "All Action Types" }
                    option { value: "capability.grant", "Capability Grant" }
                    option { value: "capability.revoke", "Capability Revoke" }
                    option { value: "config.change", "Config Change" }
                    option { value: "db.migration", "DB Migration" }
                }
                div { class: "filter-bar-actions",
                    button {
                        class: "btn-secondary btn-sm",
                        onclick: move |_| { refresh += 1; },
                        "\u{21BB} Refresh"
                    }
                    if !selected_ids.read().is_empty() {
                        button {
                            class: "btn-primary btn-sm",
                            onclick: move |_| show_batch_confirm.set(true),
                            "Batch Approve ({selected_ids.read().len()})"
                        }
                    }
                }
            }

            // ── Table ───────────────────────────────────────
            if approvals_read.is_none() {
                div { class: "state-message",
                    div { class: "spinner" }
                    span { "Loading approvals\u{2026}" }
                }
            } else if filtered.is_empty() {
                div { class: "state-message",
                    span { class: "state-icon", "\u{2705}" }
                    span { "No pending approvals." }
                }
            } else {
                div { class: "panel-scroll",
                    table { class: "data-table",
                        thead {
                            tr {
                                th { "" }
                                th { "ID" }
                                th { "Action Type" }
                                th { "Requested By" }
                                th { "Requested At" }
                                th { "Payload" }
                                th { "Actions" }
                            }
                        }
                        tbody {
                            for approval in filtered {
                                { render_approval_row(approval, &selected_ids, &show_confirm) }
                            }
                        }
                    }
                }
            }

            // ── Single Confirmation Modal ───────────────────
            if let Some((id, action)) = &*show_confirm.read() {
                {
                    let id = *id;
                    let action = action.clone();
                    rsx! {
                        ConfirmModal {
                            approval_id: id,
                            action: action,
                            on_close: move || show_confirm.set(None),
                            on_confirmed: move || {
                                show_confirm.set(None);
                                refresh += 1;
                            },
                        }
                    }
                }
            }

            // ── Batch Confirmation Modal ────────────────────
            if *show_batch_confirm.read() {
                {
                    let ids: Vec<uuid::Uuid> = selected_ids.read().iter().copied().collect();
                    rsx! {
                        BatchConfirmModal {
                            approval_ids: ids,
                            on_close: move || show_batch_confirm.set(false),
                            on_confirmed: move || {
                                show_batch_confirm.set(false);
                                selected_ids.write().clear();
                                refresh += 1;
                            },
                        }
                    }
                }
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────

fn status_badge_class(status: &str) -> &'static str {
    match status {
        "pending" => "badge-status badge-pending",
        "approved" => "badge-status badge-completed",
        "denied" => "badge-status badge-failed",
        _ => "badge-status",
    }
}

fn render_approval_row(
    approval: &ApprovalRequestDetail,
    selected_ids: &Signal<std::collections::HashSet<uuid::Uuid>>,
    show_confirm: &Signal<Option<(uuid::Uuid, String)>>,
) -> Element {
    let id = approval.id;
    let id_short = id.to_string();
    let id_display = &id_short[..8.min(id_short.len())];
    let requested_at = approval.requested_at.format("%Y-%m-%d %H:%M").to_string();
    let requested_by = approval.requested_by.map_or_else(
        || "system".to_string(),
        |u| u.to_string()[..8.min(u.to_string().len())].to_string(),
    );
    let payload_preview = {
        let s = serde_json::to_string(&approval.payload).unwrap_or_default();
        if s.len() > 60 {
            format!("{}\u{2026}", &s[..60])
        } else {
            s
        }
    };
    let badge = status_badge_class(&approval.status);
    let mut selected = *selected_ids;
    let is_selected = selected_ids.read().contains(&id);
    let mut confirm = *show_confirm;

    rsx! {
        tr {
            td {
                input {
                    r#type: "checkbox",
                    checked: is_selected,
                    onchange: move |_| {
                        let mut set = selected.write();
                        if set.contains(&id) {
                            set.remove(&id);
                        } else {
                            set.insert(id);
                        }
                    },
                }
            }
            td { class: "cell-mono", "{id_display}" }
            td { span { class: "{badge}", "{approval.action_type}" } }
            td { "{requested_by}" }
            td { "{requested_at}" }
            td { class: "cell-mono", title: "{payload_preview}", "{payload_preview}" }
            td {
                button {
                    class: "btn-primary btn-sm",
                    onclick: move |_| {
                        confirm.set(Some((id, "approve".to_string())));
                    },
                    "Approve"
                }
                button {
                    class: "btn-danger btn-sm",
                    onclick: move |_| {
                        confirm.set(Some((id, "deny".to_string())));
                    },
                    "Deny"
                }
            }
        }
    }
}

// ── Confirmation Modal (single approval) ────────────────────

#[component]
fn ConfirmModal(
    approval_id: uuid::Uuid,
    action: String,
    on_close: EventHandler,
    on_confirmed: EventHandler,
) -> Element {
    let mut submitting = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut signature = use_signal(String::new);

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Confirm {action}",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Confirm {action}" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    p { "Are you sure you want to {action} approval {approval_id}?" }
                    p { "Provide the Ed25519 signature (hex) of the approval ID to authorize this action." }
                    div { class: "form-group",
                        label { class: "form-label", "Signature (hex)" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "Ed25519 signature hex (128 chars)",
                            value: "{signature}",
                            oninput: move |e| signature.set(e.value()),
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
                        class: if action == "approve" { "btn-primary" } else { "btn-danger" },
                        disabled: *submitting.read(),
                        onclick: move |_| {
                            let sig = signature.read().clone();
                            if sig.trim().is_empty() {
                                error_msg.set(Some("Signature is required.".to_string()));
                                return;
                            }
                            submitting.set(true);
                            let act = action.clone();
                            spawn(async move {
                                let result = if act == "approve" {
                                    crate::api::approve_approval(approval_id, sig).await.map(|_| ())
                                } else {
                                    crate::api::deny_approval(approval_id, sig).await.map(|_| ())
                                };
                                match result {
                                    Ok(()) => on_confirmed.call(()),
                                    Err(e) => {
                                        error_msg.set(Some(e));
                                        submitting.set(false);
                                    }
                                }
                            });
                        },
                        if *submitting.read() { "Processing\u{2026}" } else if action == "approve" { "Approve" } else { "Deny" }
                    }
                }
            }
        }
    }
}

// ── Batch Confirmation Modal ────────────────────────────────

#[component]
fn BatchConfirmModal(
    approval_ids: Vec<uuid::Uuid>,
    on_close: EventHandler,
    on_confirmed: EventHandler,
) -> Element {
    let mut submitting = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut signature = use_signal(String::new);
    let count = approval_ids.len();

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Confirm batch approve",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Batch Approve ({count} items)" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    p { "Approve {count} selected approval requests?" }
                    p { "Provide the Ed25519 signature (hex) of the sorted, comma-joined approval IDs." }
                    div { class: "form-group",
                        label { class: "form-label", "Signature (hex)" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "Ed25519 signature hex (128 chars)",
                            value: "{signature}",
                            oninput: move |e| signature.set(e.value()),
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
                            let sig = signature.read().clone();
                            if sig.trim().is_empty() {
                                error_msg.set(Some("Signature is required.".to_string()));
                                return;
                            }
                            submitting.set(true);
                            let ids = approval_ids.clone();
                            spawn(async move {
                                match crate::api::batch_approve_approvals(ids, sig).await {
                                    Ok(_) => on_confirmed.call(()),
                                    Err(e) => {
                                        error_msg.set(Some(e));
                                        submitting.set(false);
                                    }
                                }
                            });
                        },
                        if *submitting.read() { "Processing\u{2026}" } else { "Batch Approve" }
                    }
                }
            }
        }
    }
}
