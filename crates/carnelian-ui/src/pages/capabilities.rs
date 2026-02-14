//! Capability grants management panel with filters, grant/revoke actions, and modals.

use carnelian_common::types::{CapabilityGrantDetail, GrantCapabilityRequest};
use dioxus::prelude::*;

/// Capability management page.
#[component]
pub fn Capabilities() -> Element {
    // ── Data fetching (signal-driven refresh) ───────────────
    let mut refresh = use_signal(|| 0_u64);

    let mut filter_subject_type = use_signal(|| "All".to_string());
    let mut filter_search = use_signal(String::new);
    let mut show_grant = use_signal(|| false);

    let caps_resource = use_resource(move || async move {
        let _ = refresh();
        crate::api::list_capabilities(None, None)
            .await
            .map(|r| r.grants)
            .unwrap_or_default()
    });

    // Auto-refresh every 3 seconds.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            refresh += 1;
        }
    });

    // ── Derived: filtered ───────────────────────────────────
    let caps_read = caps_resource.read();
    let all_grants: Vec<CapabilityGrantDetail> = (*caps_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);

    let filtered: Vec<&CapabilityGrantDetail> = all_grants
        .iter()
        .filter(|g| {
            let st = filter_subject_type.read();
            *st == "All" || g.subject_type == *st
        })
        .filter(|g| {
            let search = filter_search.read();
            if search.is_empty() {
                return true;
            }
            let s = search.to_lowercase();
            g.subject_id.to_lowercase().contains(&s) || g.capability_key.to_lowercase().contains(&s)
        })
        .collect();

    rsx! {
        div { class: "page-panel panel-page",
            // ── Filter bar ──────────────────────────────────
            div { class: "filter-bar",
                select {
                    class: "filter-select",
                    aria_label: "Filter by subject type",
                    value: "{filter_subject_type}",
                    onchange: move |e| filter_subject_type.set(e.value()),
                    option { value: "All", "All Subject Types" }
                    option { value: "identity", "Identity" }
                    option { value: "skill", "Skill" }
                    option { value: "channel", "Channel" }
                    option { value: "session", "Session" }
                }
                input {
                    class: "filter-input",
                    r#type: "text",
                    placeholder: "Search subject ID or capability\u{2026}",
                    aria_label: "Search capabilities",
                    value: "{filter_search}",
                    oninput: move |e| filter_search.set(e.value()),
                }
                div { class: "filter-bar-actions",
                    button {
                        class: "btn-secondary btn-sm",
                        onclick: move |_| { refresh += 1; },
                        "\u{21BB} Refresh"
                    }
                    button {
                        class: "btn-primary btn-sm",
                        onclick: move |_| show_grant.set(true),
                        "+ Grant Capability"
                    }
                }
            }

            // ── Table ───────────────────────────────────────
            if caps_read.is_none() {
                div { class: "state-message",
                    div { class: "spinner" }
                    span { "Loading capabilities\u{2026}" }
                }
            } else if filtered.is_empty() {
                div { class: "state-message",
                    span { class: "state-icon", "\u{1F512}" }
                    span { "No capability grants match the current filters." }
                }
            } else {
                div { class: "panel-scroll",
                    table { class: "data-table",
                        thead {
                            tr {
                                th { "Grant ID" }
                                th { "Subject Type" }
                                th { "Subject ID" }
                                th { "Capability" }
                                th { "Scope" }
                                th { "Expires" }
                                th { "Status" }
                                th { "Actions" }
                            }
                        }
                        tbody {
                            for grant in filtered {
                                { render_grant_row(grant, &refresh) }
                            }
                        }
                    }
                }
            }

            // ── Grant Capability Modal ──────────────────────
            if *show_grant.read() {
                GrantCapabilityModal {
                    on_close: move || show_grant.set(false),
                    on_granted: move || {
                        show_grant.set(false);
                        refresh += 1;
                    },
                }
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────

fn grant_status(grant: &CapabilityGrantDetail) -> &'static str {
    if let Some(expires) = grant.expires_at {
        if expires < chrono::Utc::now() {
            return "expired";
        }
    }
    "active"
}

fn grant_status_badge(status: &str) -> &'static str {
    match status {
        "active" => "badge-status badge-completed",
        "expired" => "badge-status badge-failed",
        _ => "badge-status",
    }
}

#[allow(clippy::map_unwrap_or)]
fn render_grant_row(grant: &CapabilityGrantDetail, refresh: &Signal<u64>) -> Element {
    let id_short = grant.grant_id.to_string();
    let id_display = &id_short[..8.min(id_short.len())];
    let scope_preview = grant
        .scope
        .as_ref()
        .map(|s| {
            let txt = serde_json::to_string(s).unwrap_or_default();
            if txt.len() > 40 {
                format!("{}\u{2026}", &txt[..40])
            } else {
                txt
            }
        })
        .unwrap_or_else(|| "\u{2014}".to_string());
    let expires = grant.expires_at.map_or_else(
        || "Never".to_string(),
        |dt| dt.format("%Y-%m-%d %H:%M").to_string(),
    );
    let status = grant_status(grant);
    let badge = grant_status_badge(status);
    let grant_id = grant.grant_id;
    let is_active = status == "active";
    let mut refresh = *refresh;

    rsx! {
        tr {
            td { class: "cell-mono", "{id_display}" }
            td { "{grant.subject_type}" }
            td { class: "cell-mono", "{grant.subject_id}" }
            td { "{grant.capability_key}" }
            td { class: "cell-mono", title: "{scope_preview}", "{scope_preview}" }
            td { "{expires}" }
            td { span { class: "{badge}", "{status}" } }
            td {
                if is_active {
                    button {
                        class: "btn-danger btn-sm",
                        onclick: move |_| {
                            spawn(async move {
                                let _ = crate::api::revoke_capability(grant_id).await;
                                refresh += 1;
                            });
                        },
                        "Revoke"
                    }
                }
            }
        }
    }
}

// ── Grant Capability Modal ──────────────────────────────────

#[component]
fn GrantCapabilityModal(on_close: EventHandler, on_granted: EventHandler) -> Element {
    let mut subject_type = use_signal(|| "identity".to_string());
    let mut subject_id = use_signal(String::new);
    let mut capability_key = use_signal(String::new);
    let mut scope_json = use_signal(String::new);
    let mut constraints_json = use_signal(String::new);
    let mut expires_at = use_signal(String::new);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut submitting = use_signal(|| false);

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Grant Capability",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Grant Capability" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    div { class: "form-group",
                        label { class: "form-label", "Subject Type" }
                        select {
                            class: "form-input",
                            value: "{subject_type}",
                            onchange: move |e| subject_type.set(e.value()),
                            option { value: "identity", "Identity" }
                            option { value: "skill", "Skill" }
                            option { value: "channel", "Channel" }
                            option { value: "session", "Session" }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Subject ID" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "UUID or external reference",
                            value: "{subject_id}",
                            oninput: move |e| subject_id.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Capability Key" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "e.g. fs.read, net.http, task.create",
                            value: "{capability_key}",
                            oninput: move |e| capability_key.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Scope (JSON, optional)" }
                        textarea {
                            class: "form-textarea",
                            placeholder: "scope JSON object",
                            value: "{scope_json}",
                            oninput: move |e| scope_json.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Constraints (JSON, optional)" }
                        textarea {
                            class: "form-textarea",
                            placeholder: "constraints JSON object",
                            value: "{constraints_json}",
                            oninput: move |e| constraints_json.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Expires At (ISO 8601, optional)" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "2025-12-31T23:59:59Z",
                            value: "{expires_at}",
                            oninput: move |e| expires_at.set(e.value()),
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
                            let sid = subject_id.read().clone();
                            let ck = capability_key.read().clone();
                            if sid.trim().is_empty() || ck.trim().is_empty() {
                                error_msg.set(Some("Subject ID and Capability Key are required.".to_string()));
                                return;
                            }

                            let scope = {
                                let v = scope_json.read().clone();
                                if v.trim().is_empty() {
                                    None
                                } else {
                                    match serde_json::from_str(&v) {
                                        Ok(val) => Some(val),
                                        Err(e) => {
                                            error_msg.set(Some(format!("Invalid scope JSON: {e}")));
                                            return;
                                        }
                                    }
                                }
                            };
                            let constraints = {
                                let v = constraints_json.read().clone();
                                if v.trim().is_empty() {
                                    None
                                } else {
                                    match serde_json::from_str(&v) {
                                        Ok(val) => Some(val),
                                        Err(e) => {
                                            error_msg.set(Some(format!("Invalid constraints JSON: {e}")));
                                            return;
                                        }
                                    }
                                }
                            };
                            let exp = {
                                let v = expires_at.read().clone();
                                if v.trim().is_empty() {
                                    None
                                } else {
                                    match v.parse::<chrono::DateTime<chrono::Utc>>() {
                                        Ok(dt) => Some(dt),
                                        Err(e) => {
                                            error_msg.set(Some(format!("Invalid date: {e}")));
                                            return;
                                        }
                                    }
                                }
                            };

                            submitting.set(true);
                            let st = subject_type.read().clone();
                            spawn(async move {
                                let req = GrantCapabilityRequest {
                                    subject_type: st,
                                    subject_id: sid,
                                    capability_key: ck,
                                    scope,
                                    constraints,
                                    expires_at: exp,
                                };
                                match crate::api::grant_capability(req).await {
                                    Ok(_) => on_granted.call(()),
                                    Err(e) => {
                                        error_msg.set(Some(e));
                                        submitting.set(false);
                                    }
                                }
                            });
                        },
                        if *submitting.read() { "Granting\u{2026}" } else { "Grant" }
                    }
                }
            }
        }
    }
}
