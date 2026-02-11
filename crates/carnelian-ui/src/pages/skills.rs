//! Skills registry panel — list, filter, enable/disable, refresh,
//! and view manifest details.

use carnelian_common::types::{EventType, SkillDetail};
use dioxus::prelude::*;

use crate::store::EventStreamStore;

/// Skills page.
#[component]
pub fn Skills() -> Element {
    let store = use_context::<EventStreamStore>();

    // ── Data fetching (signal-driven refresh) ───────────────
    let mut refresh = use_signal(|| 0_u64);

    let skills_resource = use_resource(move || async move {
        let _ = refresh();
        crate::api::list_skills()
            .await
            .map(|r| r.skills)
            .unwrap_or_default()
    });

    // Auto-refresh every 10 seconds.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            refresh += 1;
        }
    });

    // Trigger refresh on skill events from WebSocket.
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            match &last.event_type {
                EventType::SkillDiscovered | EventType::SkillUpdated | EventType::SkillRemoved => {
                    refresh += 1;
                }
                _ => {}
            }
        }
    });

    // ── Local UI state ──────────────────────────────────────
    let mut filter_runtime = use_signal(|| "All".to_string());
    let mut filter_status = use_signal(|| "All".to_string());
    let mut filter_search = use_signal(String::new);
    let sort_col = use_signal(|| "name".to_string());
    let sort_asc = use_signal(|| true);
    let mut selected_skill = use_signal(|| Option::<SkillDetail>::None);
    let mut refreshing = use_signal(|| false);

    // ── Derived: filtered + sorted ──────────────────────────
    let skills_read = skills_resource.read();
    let all_skills: Vec<SkillDetail> = (*skills_read).as_ref().map_or_else(Vec::new, std::clone::Clone::clone);

    let filtered = filter_skills(
        &all_skills,
        &filter_runtime.read(),
        &filter_status.read(),
        &filter_search.read(),
    );
    let sorted = sort_skills(filtered, &sort_col.read(), *sort_asc.read());

    rsx! {
        div { class: "page-panel panel-page",
            // ── Filter bar ──────────────────────────────────
            div { class: "filter-bar",
                select {
                    class: "filter-select",
                    aria_label: "Filter by runtime",
                    value: "{filter_runtime}",
                    onchange: move |e| filter_runtime.set(e.value()),
                    option { value: "All", "All Runtimes" }
                    option { value: "node", "Node" }
                    option { value: "python", "Python" }
                    option { value: "shell", "Shell" }
                }
                select {
                    class: "filter-select",
                    aria_label: "Filter by status",
                    value: "{filter_status}",
                    onchange: move |e| filter_status.set(e.value()),
                    option { value: "All", "All Statuses" }
                    option { value: "enabled", "Enabled" }
                    option { value: "disabled", "Disabled" }
                }
                input {
                    class: "filter-input",
                    r#type: "text",
                    placeholder: "Search skills\u{2026}",
                    aria_label: "Search skills",
                    value: "{filter_search}",
                    oninput: move |e| filter_search.set(e.value()),
                }
                div { class: "filter-bar-actions",
                    button {
                        class: "btn-primary btn-sm",
                        disabled: *refreshing.read(),
                        onclick: move |_| {
                            refreshing.set(true);
                            spawn(async move {
                                match crate::api::refresh_skills().await {
                                    Ok(r) => {
                                        tracing::info!(
                                            discovered = r.discovered,
                                            updated = r.updated,
                                            removed = r.removed,
                                            "Skills refreshed"
                                        );
                                    }
                                    Err(e) => tracing::warn!(error = %e, "Skill refresh failed"),
                                }
                                refresh += 1;
                                refreshing.set(false);
                            });
                        },
                        if *refreshing.read() { "Refreshing\u{2026}" } else { "\u{21BB} Refresh Skills" }
                    }
                }
            }

            // ── Table ───────────────────────────────────────
            if skills_read.is_none() {
                div { class: "state-message",
                    div { class: "spinner" }
                    span { "Loading skills\u{2026}" }
                }
            } else if sorted.is_empty() {
                div { class: "state-message",
                    span { class: "state-icon", "\u{1F9E9}" }
                    span { "No skills found. Place skill manifests in the registry directory and click Refresh." }
                }
            } else {
                div { class: "panel-scroll",
                    table { class: "data-table",
                        thead {
                            tr {
                                { sortable_th("Name", "name", &sort_col, &sort_asc) }
                                { sortable_th("Runtime", "runtime", &sort_col, &sort_asc) }
                                { sortable_th("Status", "enabled", &sort_col, &sort_asc) }
                                { sortable_th("Discovered", "discovered_at", &sort_col, &sort_asc) }
                                { sortable_th("Updated", "updated_at", &sort_col, &sort_asc) }
                                th { "Actions" }
                            }
                        }
                        tbody {
                            for skill in sorted {
                                { render_skill_row(skill, &refresh, &selected_skill) }
                            }
                        }
                    }
                }
            }

            // ── Manifest detail modal ───────────────────────
            if let Some(skill) = &*selected_skill.read() {
                SkillManifestModal {
                    skill: skill.clone(),
                    on_close: move || selected_skill.set(None),
                }
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────

fn filter_skills<'a>(
    skills: &'a [SkillDetail],
    runtime: &str,
    status: &str,
    search: &str,
) -> Vec<&'a SkillDetail> {
    let search_lower = search.to_lowercase();
    skills
        .iter()
        .filter(|s| runtime == "All" || s.runtime == runtime)
        .filter(|s| match status {
            "enabled" => s.enabled,
            "disabled" => !s.enabled,
            _ => true,
        })
        .filter(|s| {
            if search_lower.is_empty() {
                return true;
            }
            s.name.to_lowercase().contains(&search_lower)
                || s.description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&search_lower)
        })
        .collect()
}

fn sort_skills<'a>(mut skills: Vec<&'a SkillDetail>, col: &str, asc: bool) -> Vec<&'a SkillDetail> {
    skills.sort_by(|a, b| {
        let ord = match col {
            "name" => a.name.cmp(&b.name),
            "runtime" => a.runtime.cmp(&b.runtime),
            "enabled" => a.enabled.cmp(&b.enabled),
            "updated_at" => a.updated_at.cmp(&b.updated_at),
            _ => a.discovered_at.cmp(&b.discovered_at),
        };
        if asc { ord } else { ord.reverse() }
    });
    skills
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

fn render_skill_row(
    skill: &SkillDetail,
    refresh: &Signal<u64>,
    selected: &Signal<Option<SkillDetail>>,
) -> Element {
    let enabled_badge = if skill.enabled {
        "badge-status badge-enabled"
    } else {
        "badge-status badge-disabled"
    };
    let enabled_label = if skill.enabled { "Enabled" } else { "Disabled" };
    let discovered = skill.discovered_at.format("%Y-%m-%d %H:%M").to_string();
    let updated = skill.updated_at.format("%Y-%m-%d %H:%M").to_string();
    let desc = skill.description.as_deref().unwrap_or("\u{2014}");
    let skill_id = skill.skill_id;
    let is_enabled = skill.enabled;
    let skill_clone = skill.clone();
    let mut selected = *selected;
    let mut refresh = *refresh;

    rsx! {
        tr {
            td {
                div { "{skill.name}" }
                div { class: "text-secondary", style: "font-size:12px;", "{desc}" }
            }
            td { "{skill.runtime}" }
            td { span { class: "{enabled_badge}", "{enabled_label}" } }
            td { "{discovered}" }
            td { "{updated}" }
            td {
                button {
                    class: if is_enabled { "btn-secondary btn-sm" } else { "btn-success btn-sm" },
                    onclick: move |_| {
                        let sid = skill_id;
                        let enabled = is_enabled;
                        spawn(async move {
                            if enabled {
                                let _ = crate::api::disable_skill(sid).await;
                            } else {
                                let _ = crate::api::enable_skill(sid).await;
                            }
                            refresh += 1;
                        });
                    },
                    if is_enabled { "Disable" } else { "Enable" }
                }
                button {
                    class: "btn-secondary btn-sm",
                    onclick: {
                        move |_| selected.set(Some(skill_clone.clone()))
                    },
                    "Manifest"
                }
            }
        }
    }
}

// ── Manifest Modal ──────────────────────────────────────────

#[component]
fn SkillManifestModal(skill: SkillDetail, on_close: EventHandler) -> Element {
    let json = serde_json::to_string_pretty(&skill).unwrap_or_default();

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Skill Manifest",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Skill: {skill.name}" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    pre { "{json}" }
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
