//! Sub-agents management panel with filters, sorting, CRUD actions,
//! lifecycle controls (pause/resume/terminate), and real-time updates.

use std::collections::HashMap;

use carnelian_common::types::{
    CreateSubAgentApiRequest, EventType, ProviderDetail, SubAgentDetail, UpdateSubAgentApiRequest,
};
use dioxus::prelude::*;
use uuid::Uuid;

use crate::store::EventStreamStore;

/// Sub-agents page.
#[component]
pub fn SubAgents() -> Element {
    let store = use_context::<EventStreamStore>();

    // ── Data fetching (signal-driven refresh) ───────────────
    let mut refresh = use_signal(|| 0_u64);

    let agents_resource = use_resource(move || async move {
        let _ = refresh();
        crate::api::list_sub_agents(None, false)
            .await
            .map(|r| r.sub_agents)
            .unwrap_or_default()
    });

    // Auto-refresh every 5 seconds.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            refresh += 1;
        }
    });

    // Trigger refresh on sub-agent events from WebSocket.
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            match &last.event_type {
                EventType::SubAgentCreated
                | EventType::SubAgentUpdated
                | EventType::SubAgentTerminated
                | EventType::SubAgentPaused
                | EventType::SubAgentResumed => {
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
    let mut show_create = use_signal(|| false);
    let mut show_edit = use_signal(|| Option::<SubAgentDetail>::None);
    let mut show_confirm_action = use_signal(|| Option::<(Uuid, String, String)>::None);

    // ── Provider cache (for name lookup) ─────────────────────
    let providers_resource = use_resource(|| async {
        crate::api::list_providers()
            .await
            .map(|r| r.providers)
            .unwrap_or_default()
    });

    // ── Derived: filtered + sorted ──────────────────────────
    let agents_read = agents_resource.read();
    let all_agents: Vec<SubAgentDetail> = (*agents_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);

    let providers_read = providers_resource.read();
    let all_providers: Vec<ProviderDetail> = (*providers_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);
    let provider_map: HashMap<Uuid, String> = all_providers
        .iter()
        .map(|p| (p.provider_id, p.name.clone()))
        .collect();

    let filtered = filter_sub_agents(&all_agents, &filter_status.read(), &filter_search.read());
    let sorted = sort_sub_agents(filtered, &sort_col.read(), *sort_asc.read());

    rsx! {
        div { class: "page-panel panel-page",
            // ── Filter bar ──────────────────────────────────
            div { class: "filter-bar",
                select {
                    class: "filter-select",
                    aria_label: "Filter by status",
                    value: "{filter_status}",
                    onchange: move |e| filter_status.set(e.value()),
                    option { value: "All", "All Statuses" }
                    option { value: "Active", "Active" }
                    option { value: "Paused", "Paused" }
                    option { value: "Terminated", "Terminated" }
                }
                input {
                    class: "filter-input",
                    r#type: "text",
                    placeholder: "Search name / role\u{2026}",
                    aria_label: "Search sub-agents",
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
                        onclick: move |_| show_create.set(true),
                        "+ Create Sub-Agent"
                    }
                }
            }

            // ── Table ───────────────────────────────────────
            if agents_read.is_none() {
                div { class: "state-message",
                    div { class: "spinner" }
                    span { "Loading sub-agents\u{2026}" }
                }
            } else if sorted.is_empty() {
                div { class: "state-message",
                    span { class: "state-icon", "\u{1F916}" }
                    span { "No sub-agents found. Create one to get started." }
                }
            } else {
                div { class: "panel-scroll",
                    table { class: "data-table",
                        thead {
                            tr {
                                { sortable_th("Name", "name", &sort_col, &sort_asc) }
                                { sortable_th("Role", "role", &sort_col, &sort_asc) }
                                th { "Model" }
                                { sortable_th("Status", "status", &sort_col, &sort_asc) }
                                { sortable_th("Last Active", "last_active_at", &sort_col, &sort_asc) }
                                { sortable_th("Created", "created_at", &sort_col, &sort_asc) }
                                th { "Actions" }
                            }
                        }
                        tbody {
                            for agent in sorted {
                                { render_sub_agent_row(agent, &refresh, &show_edit, &show_confirm_action, &provider_map) }
                            }
                        }
                    }
                }
            }

            // ── Create Sub-Agent Modal ──────────────────────
            if *show_create.read() {
                CreateSubAgentModal {
                    on_close: move || show_create.set(false),
                    on_created: move || {
                        show_create.set(false);
                        refresh += 1;
                    },
                }
            }

            // ── Edit Sub-Agent Modal ────────────────────────
            if let Some(agent) = &*show_edit.read() {
                {
                    let agent = agent.clone();
                    rsx! {
                        EditSubAgentModal {
                            sub_agent: agent,
                            on_close: move || show_edit.set(None),
                            on_updated: move || {
                                show_edit.set(None);
                                refresh += 1;
                            },
                        }
                    }
                }
            }

            // ── Confirmation Dialog ─────────────────────────
            if let Some((id, action, name)) = &*show_confirm_action.read() {
                {
                    let id = *id;
                    let action = action.clone();
                    let name = name.clone();
                    rsx! {
                        ConfirmActionModal {
                            sub_agent_id: id,
                            sub_agent_name: name,
                            action: action,
                            on_close: move || show_confirm_action.set(None),
                            on_confirmed: move || {
                                show_confirm_action.set(None);
                                refresh += 1;
                            },
                        }
                    }
                }
            }
        }
    }
}

// ── Status Helpers ──────────────────────────────────────────

fn is_paused(agent: &SubAgentDetail) -> bool {
    agent
        .directives
        .as_ref()
        .and_then(|d| d.get("_paused"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn status_label(agent: &SubAgentDetail) -> &'static str {
    if agent.terminated_at.is_some() {
        "Terminated"
    } else if is_paused(agent) {
        "Paused"
    } else {
        "Active"
    }
}

fn status_badge_class(agent: &SubAgentDetail) -> &'static str {
    if agent.terminated_at.is_some() {
        "badge-status badge-cancelled"
    } else if is_paused(agent) {
        "badge-status badge-pending"
    } else {
        "badge-status badge-completed"
    }
}

// ── Filter & Sort ───────────────────────────────────────────

fn filter_sub_agents<'a>(
    agents: &'a [SubAgentDetail],
    status: &str,
    search: &str,
) -> Vec<&'a SubAgentDetail> {
    let search_lower = search.to_lowercase();
    agents
        .iter()
        .filter(|a| match status {
            "Active" => a.terminated_at.is_none() && !is_paused(a),
            "Paused" => a.terminated_at.is_none() && is_paused(a),
            "Terminated" => a.terminated_at.is_some(),
            _ => true,
        })
        .filter(|a| {
            if search_lower.is_empty() {
                return true;
            }
            a.name.to_lowercase().contains(&search_lower)
                || a.role.to_lowercase().contains(&search_lower)
        })
        .collect()
}

fn sort_sub_agents<'a>(
    mut agents: Vec<&'a SubAgentDetail>,
    col: &str,
    asc: bool,
) -> Vec<&'a SubAgentDetail> {
    agents.sort_by(|a, b| {
        let ord = match col {
            "name" => a.name.cmp(&b.name),
            "role" => a.role.cmp(&b.role),
            "status" => status_label(a).cmp(status_label(b)),
            "last_active_at" => a.last_active_at.cmp(&b.last_active_at),
            _ => a.created_at.cmp(&b.created_at),
        };
        if asc { ord } else { ord.reverse() }
    });
    agents
}

// ── Sortable Header ─────────────────────────────────────────

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

// ── Row Rendering ───────────────────────────────────────────

fn render_sub_agent_row(
    agent: &SubAgentDetail,
    _refresh: &Signal<u64>,
    show_edit: &Signal<Option<SubAgentDetail>>,
    show_confirm: &Signal<Option<(Uuid, String, String)>>,
    provider_map: &HashMap<Uuid, String>,
) -> Element {
    let badge = status_badge_class(agent);
    let label = status_label(agent);
    let model_display = agent
        .model_provider
        .and_then(|id| provider_map.get(&id).cloned())
        .unwrap_or_else(|| "Default".to_string());
    let last_active = agent.last_active_at.format("%Y-%m-%d %H:%M").to_string();
    let created = agent.created_at.format("%Y-%m-%d %H:%M").to_string();
    let is_active = agent.terminated_at.is_none() && !is_paused(agent);
    let is_paused_state = agent.terminated_at.is_none() && is_paused(agent);
    let is_terminated = agent.terminated_at.is_some();
    let agent_id = agent.sub_agent_id;
    let agent_name = agent.name.clone();
    let agent_clone = agent.clone();
    let mut edit = *show_edit;
    let mut confirm = *show_confirm;
    rsx! {
        tr {
            td { "{agent.name}" }
            td { "{agent.role}" }
            td { "{model_display}" }
            td { span { class: "{badge}", "{label}" } }
            td { "{last_active}" }
            td { "{created}" }
            td {
                if !is_terminated {
                    button {
                        class: "btn-secondary btn-sm",
                        onclick: {
                            let ac = agent_clone;
                            move |_| edit.set(Some(ac.clone()))
                        },
                        "Edit"
                    }
                }
                if is_active {
                    button {
                        class: "btn-secondary btn-sm",
                        onclick: {
                            let name = agent_name.clone();
                            move |_| {
                                confirm.set(Some((agent_id, "pause".to_string(), name.clone())));
                            }
                        },
                        "Pause"
                    }
                }
                if is_paused_state {
                    button {
                        class: "btn-primary btn-sm",
                        onclick: {
                            let name = agent_name.clone();
                            move |_| {
                                confirm.set(Some((agent_id, "resume".to_string(), name.clone())));
                            }
                        },
                        "Resume"
                    }
                }
                if !is_terminated {
                    button {
                        class: "btn-danger btn-sm",
                        onclick: {
                            let name = agent_name;
                            move |_| {
                                confirm.set(Some((agent_id, "terminate".to_string(), name.clone())));
                            }
                        },
                        "Terminate"
                    }
                }
            }
        }
    }
}

// ── Create Sub-Agent Modal ──────────────────────────────────

#[component]
fn CreateSubAgentModal(on_close: EventHandler, on_created: EventHandler) -> Element {
    let mut name = use_signal(String::new);
    let mut role = use_signal(String::new);
    let mut runtime = use_signal(|| "node".to_string());
    let mut model_provider = use_signal(String::new);
    let mut directives = use_signal(String::new);
    let mut capabilities = use_signal(String::new);
    let mut ephemeral = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut submitting = use_signal(|| false);

    // Fetch providers for the dropdown
    let providers_resource = use_resource(|| async {
        crate::api::list_providers()
            .await
            .map(|r| r.providers)
            .unwrap_or_default()
    });
    let providers_read = providers_resource.read();
    let providers: Vec<ProviderDetail> = (*providers_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);

    // Directive preview
    let directives_preview = {
        let raw = directives.read().clone();
        if raw.trim().is_empty() {
            "No directives".to_string()
        } else {
            match serde_json::from_str::<serde_json::Value>(&raw) {
                Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                Err(e) => format!("Invalid JSON: {e}"),
            }
        }
    };

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Create Sub-Agent",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Create Sub-Agent" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    div { class: "form-group",
                        label { class: "form-label", "Name *" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "Sub-agent name",
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Role *" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "e.g. code_review, research, testing",
                            value: "{role}",
                            oninput: move |e| role.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Runtime" }
                        select {
                            class: "form-input",
                            value: "{runtime}",
                            onchange: move |e| runtime.set(e.value()),
                            option { value: "node", "Node.js" }
                            option { value: "python", "Python" }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Model Provider" }
                        select {
                            class: "form-input",
                            value: "{model_provider}",
                            onchange: move |e| model_provider.set(e.value()),
                            option { value: "", "Default (None)" }
                            for p in &providers {
                                option {
                                    value: "{p.provider_id}",
                                    "{p.name}"
                                }
                            }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Directives (JSON array)" }
                        div { style: "display: flex; gap: 8px;",
                            textarea {
                                class: "form-textarea",
                                style: "flex: 1;",
                                placeholder: "[\"directive 1\", \"directive 2\"]",
                                value: "{directives}",
                                oninput: move |e| directives.set(e.value()),
                            }
                            pre {
                                style: "flex: 1; font-size: 12px; padding: 8px; background: rgba(0,0,0,0.2); border-radius: 4px; overflow: auto; max-height: 120px; margin: 0;",
                                "{directives_preview}"
                            }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Capabilities (comma-separated)" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "sub_agent.create, memory.read",
                            value: "{capabilities}",
                            oninput: move |e| capabilities.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { style: "display: flex; align-items: center; gap: 8px; cursor: pointer;",
                            input {
                                r#type: "checkbox",
                                checked: *ephemeral.read(),
                                onchange: move |_| {
                                    let old = *ephemeral.read();
                                    ephemeral.set(!old);
                                },
                            }
                            "Ephemeral (auto-terminate when idle)"
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
                            let n = name.read().clone();
                            let r = role.read().clone();
                            if n.trim().is_empty() {
                                error_msg.set(Some("Name is required.".to_string()));
                                return;
                            }
                            if r.trim().is_empty() {
                                error_msg.set(Some("Role is required.".to_string()));
                                return;
                            }

                            // Parse directives
                            let dir_raw = directives.read().clone();
                            let parsed_directives = if dir_raw.trim().is_empty() {
                                None
                            } else {
                                match serde_json::from_str::<serde_json::Value>(&dir_raw) {
                                    Ok(v) => Some(v),
                                    Err(e) => {
                                        error_msg.set(Some(format!("Invalid directives JSON: {e}")));
                                        return;
                                    }
                                }
                            };

                            // Parse capabilities
                            let caps_raw = capabilities.read().clone();
                            let parsed_caps: Vec<String> = caps_raw
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();

                            let rt = runtime.read().clone();
                            let eph = *ephemeral.read();
                            let mp_raw = model_provider.read().clone();
                            let mp = if mp_raw.is_empty() {
                                None
                            } else {
                                mp_raw.parse::<Uuid>().ok()
                            };

                            submitting.set(true);
                            spawn(async move {
                                let request = CreateSubAgentApiRequest {
                                    name: n,
                                    role: r,
                                    parent_id: None,
                                    directives: parsed_directives,
                                    model_provider: mp,
                                    ephemeral: eph,
                                    capabilities: parsed_caps,
                                    runtime: rt,
                                };
                                match crate::api::create_sub_agent(request).await {
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

// ── Edit Sub-Agent Modal ────────────────────────────────────

#[component]
fn EditSubAgentModal(
    sub_agent: SubAgentDetail,
    on_close: EventHandler,
    on_updated: EventHandler,
) -> Element {
    let mut name = use_signal(|| sub_agent.name.clone());
    let mut role = use_signal(|| sub_agent.role.clone());
    let mut model_provider = use_signal(|| {
        sub_agent
            .model_provider
            .map_or_else(String::new, |id| id.to_string())
    });
    let mut directives = use_signal(|| {
        sub_agent
            .directives
            .as_ref()
            .map(|d| serde_json::to_string_pretty(d).unwrap_or_default())
            .unwrap_or_default()
    });
    let mut capabilities = use_signal(|| {
        sub_agent
            .directives
            .as_ref()
            .and_then(|d| d.get("_capabilities"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    });
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut submitting = use_signal(|| false);
    let agent_id = sub_agent.sub_agent_id;

    // Fetch providers for the dropdown
    let providers_resource = use_resource(|| async {
        crate::api::list_providers()
            .await
            .map(|r| r.providers)
            .unwrap_or_default()
    });
    let providers_read = providers_resource.read();
    let providers: Vec<ProviderDetail> = (*providers_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);

    // Directive preview
    let directives_preview = {
        let raw = directives.read().clone();
        if raw.trim().is_empty() {
            "No directives".to_string()
        } else {
            match serde_json::from_str::<serde_json::Value>(&raw) {
                Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                Err(e) => format!("Invalid JSON: {e}"),
            }
        }
    };

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Edit Sub-Agent",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Edit Sub-Agent" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    div { class: "form-group",
                        label { class: "form-label", "Name" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Role" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            value: "{role}",
                            oninput: move |e| role.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Model Provider" }
                        select {
                            class: "form-input",
                            value: "{model_provider}",
                            onchange: move |e| model_provider.set(e.value()),
                            option { value: "", "Default (None)" }
                            for p in &providers {
                                option {
                                    value: "{p.provider_id}",
                                    "{p.name}"
                                }
                            }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Directives (JSON)" }
                        div { style: "display: flex; gap: 8px;",
                            textarea {
                                class: "form-textarea",
                                style: "flex: 1;",
                                value: "{directives}",
                                oninput: move |e| directives.set(e.value()),
                            }
                            pre {
                                style: "flex: 1; font-size: 12px; padding: 8px; background: rgba(0,0,0,0.2); border-radius: 4px; overflow: auto; max-height: 120px; margin: 0;",
                                "{directives_preview}"
                            }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Capabilities (comma-separated)" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "sub_agent.create, memory.read",
                            value: "{capabilities}",
                            oninput: move |e| capabilities.set(e.value()),
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
                            let n = name.read().clone();
                            let r = role.read().clone();

                            // Parse directives
                            let dir_raw = directives.read().clone();
                            let parsed_directives = if dir_raw.trim().is_empty() {
                                None
                            } else {
                                match serde_json::from_str::<serde_json::Value>(&dir_raw) {
                                    Ok(v) => Some(v),
                                    Err(e) => {
                                        error_msg.set(Some(format!("Invalid directives JSON: {e}")));
                                        return;
                                    }
                                }
                            };

                            // Parse capabilities
                            let caps_raw = capabilities.read().clone();
                            let parsed_caps: Vec<String> = caps_raw
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            let caps_opt = if parsed_caps.is_empty() {
                                None
                            } else {
                                Some(parsed_caps)
                            };

                            // Only send changed fields
                            let name_opt = if n.trim().is_empty() { None } else { Some(n) };
                            let role_opt = if r.trim().is_empty() { None } else { Some(r) };

                            // Parse model provider
                            let mp_raw = model_provider.read().clone();
                            let mp = if mp_raw.is_empty() {
                                None
                            } else {
                                mp_raw.parse::<Uuid>().ok()
                            };

                            submitting.set(true);
                            spawn(async move {
                                let request = UpdateSubAgentApiRequest {
                                    name: name_opt,
                                    role: role_opt,
                                    directives: parsed_directives,
                                    model_provider: mp,
                                    capabilities: caps_opt,
                                };
                                match crate::api::update_sub_agent(agent_id, request).await {
                                    Ok(_) => on_updated.call(()),
                                    Err(e) => {
                                        error_msg.set(Some(e));
                                        submitting.set(false);
                                    }
                                }
                            });
                        },
                        if *submitting.read() { "Saving\u{2026}" } else { "Save" }
                    }
                }
            }
        }
    }
}

// ── Confirmation Action Modal ───────────────────────────────

#[component]
fn ConfirmActionModal(
    sub_agent_id: Uuid,
    sub_agent_name: String,
    action: String,
    on_close: EventHandler,
    on_confirmed: EventHandler,
) -> Element {
    let mut submitting = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);

    let title = match action.as_str() {
        "pause" => "Pause Sub-Agent",
        "resume" => "Resume Sub-Agent",
        "terminate" => "Terminate Sub-Agent",
        _ => "Confirm Action",
    };

    let message = match action.as_str() {
        "pause" => format!(
            "Are you sure you want to pause sub-agent \"{sub_agent_name}\"? Its worker process will be stopped."
        ),
        "resume" => format!(
            "Are you sure you want to resume sub-agent \"{sub_agent_name}\"? A new worker process will be spawned."
        ),
        "terminate" => format!(
            "Are you sure you want to terminate sub-agent \"{sub_agent_name}\"? This action cannot be undone."
        ),
        _ => format!("Are you sure you want to {action} sub-agent \"{sub_agent_name}\"?"),
    };

    let btn_class = if action == "terminate" {
        "btn-danger"
    } else {
        "btn-primary"
    };

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "{title}",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "{title}" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    p { "{message}" }
                    if action == "terminate" {
                        p { style: "color: #E74C3C; font-weight: 600;", "\u{26A0} This action cannot be undone." }
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
                        class: "{btn_class}",
                        disabled: *submitting.read(),
                        onclick: move |_| {
                            submitting.set(true);
                            let act = action.clone();
                            spawn(async move {
                                let result = match act.as_str() {
                                    "pause" => crate::api::pause_sub_agent(sub_agent_id).await.map(|_| ()),
                                    "resume" => crate::api::resume_sub_agent(sub_agent_id).await.map(|_| ()),
                                    "terminate" => crate::api::delete_sub_agent(sub_agent_id).await,
                                    _ => Err("Unknown action".to_string()),
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
                        if *submitting.read() { "Processing\u{2026}" } else { "{action}" }
                    }
                }
            }
        }
    }
}
