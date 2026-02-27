//! XP Progression page — level history, leaderboard, and skill levels.

#![allow(clippy::nonminimal_bool)]
#![allow(clippy::suboptimal_flops)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::option_map_or_none)]

use dioxus::prelude::*;

use crate::components::xp_widget::XpWidget;
use crate::store::EventStreamStore;

/// XP Progression page with three sections: history, leaderboard, skill levels.
#[component]
pub fn XpProgression() -> Element {
    let store = use_context::<EventStreamStore>();
    let xp = store.xp_state.read();
    let identity_id = xp.identity_id;

    let mut refresh = use_signal(|| 0_u64);

    // Auto-refresh every 30 seconds
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            refresh += 1;
        }
    });

    rsx! {
        div { class: "page-panel panel-page",
            div { class: "section-header", h1 { "XP Progression" } }

            // XP Widget card
            XpWidget {}

            // Level History
            LevelHistory { identity_id, refresh }

            // Leaderboard
            Leaderboard { identity_id, refresh }

            // Skill Levels
            SkillLevels { refresh }
        }
    }
}

#[component]
fn LevelHistory(identity_id: Option<uuid::Uuid>, refresh: Signal<u64>) -> Element {
    let history = use_resource(move || async move {
        let _ = refresh();
        if let Some(id) = identity_id {
            crate::api::get_xp_history(id, 1, 50).await.ok()
        } else {
            None
        }
    });

    let history_read = history.read();
    let events = (*history_read).as_ref().and_then(|o| o.as_ref());

    rsx! {
        div { style: "margin-top: 24px;",
            div { class: "section-header", h2 { "Level History" } }

            if let Some(resp) = events {
                // SVG line chart
                {
                    let points = &resp.events;
                    if !points.is_empty() {
                        let max_cumulative = points.iter().map(|e| i64::from(e.xp_amount)).sum::<i64>().max(1);
                        let count = points.len();
                        let w = 600.0_f64;
                        let h = 120.0_f64;
                        let padding = 10.0_f64;

                        let mut cumulative = 0i64;
                        let mut path_points = Vec::new();
                        for (i, evt) in points.iter().rev().enumerate() {
                            cumulative += i64::from(evt.xp_amount);
                            let x = padding + (i as f64 / (count.max(2) - 1).max(1) as f64) * 2.0f64.mul_add(-padding, w);
                            let y = h - padding - (cumulative as f64 / max_cumulative as f64) * 2.0f64.mul_add(-padding, h);
                            path_points.push(format!("{x:.1},{y:.1}"));
                        }
                        let polyline_points = path_points.join(" ");

                        rsx! {
                            svg {
                                width: "100%",
                                height: "140",
                                view_box: "0 0 {w} {h}",
                                style: "background: rgba(20,20,35,0.5); border-radius: 8px; margin-bottom: 16px;",
                                polyline {
                                    points: "{polyline_points}",
                                    fill: "none",
                                    stroke: "#4A90E2",
                                    stroke_width: "2",
                                }
                            }
                        }
                    } else {
                        rsx! {}
                    }
                }

                // Table
                table { class: "data-table",
                    thead {
                        tr {
                            th { "Time" }
                            th { "Source" }
                            th { "XP" }
                            th { "Task/Skill" }
                        }
                    }
                    tbody {
                        for evt in &resp.events {
                            {
                                let ts_str = evt.created_at.format("%Y-%m-%d %H:%M").to_string();
                                let source = &evt.source;
                                let xp_str = format!("+{}", evt.xp_amount);
                                let id_str = evt.task_id.map(|t| t.to_string())
                                    .or_else(|| evt.skill_id.map(|s| s.to_string()))
                                    .unwrap_or_else(|| "\u{2014}".to_string());
                                rsx! {
                                    tr {
                                        td { class: "cell-mono", "{ts_str}" }
                                        td {
                                            span { class: "badge-status", "{source}" }
                                        }
                                        td { style: "color: #2ECC71; font-weight: 600;", "{xp_str}" }
                                        td { class: "cell-mono cell-truncate", "{id_str}" }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div { class: "state-message",
                    div { class: "spinner" }
                    span { "Loading XP history..." }
                }
            }
        }
    }
}

#[component]
fn Leaderboard(identity_id: Option<uuid::Uuid>, refresh: Signal<u64>) -> Element {
    let leaderboard = use_resource(move || async move {
        let _ = refresh();
        crate::api::get_xp_leaderboard().await.ok()
    });

    let lb_read = leaderboard.read();
    let entries = (*lb_read).as_ref().and_then(|o| o.as_ref());

    rsx! {
        div { style: "margin-top: 24px;",
            div { class: "section-header", h2 { "Leaderboard" } }

            if let Some(resp) = entries {
                table { class: "data-table",
                    thead {
                        tr {
                            th { "Rank" }
                            th { "Name" }
                            th { "Level" }
                            th { "Total XP" }
                        }
                    }
                    tbody {
                        for (i, entry) in resp.entries.iter().enumerate() {
                            {
                                let is_self = identity_id.map_or(false, |id| id == entry.identity_id);
                                let row_style = if is_self { "background: rgba(74, 144, 226, 0.12);" } else { "" };
                                {
                                    let rank = i + 1;
                                    let name = &entry.name;
                                    let level = entry.level;
                                    let total_xp = entry.total_xp;
                                    let td_style = if is_self { "font-weight: 700; color: #4A90E2;" } else { "" };
                                    rsx! {
                                        tr { style: "{row_style}",
                                            td { "{rank}" }
                                            td { style: "{td_style}", "{name}" }
                                            td { "{level}" }
                                            td { "{total_xp}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div { class: "state-message",
                    div { class: "spinner" }
                    span { "Loading leaderboard..." }
                }
            }
        }
    }
}

#[component]
fn SkillLevels(refresh: Signal<u64>) -> Element {
    let mut search_filter = use_signal(String::new);

    let skills = use_resource(move || async move {
        let _ = refresh();
        crate::api::get_top_skills(20).await.ok()
    });

    let skills_read = skills.read();
    let skill_list = (*skills_read).as_ref().and_then(|o| o.as_ref());
    let filter_val = search_filter.read().to_lowercase();

    rsx! {
        div { style: "margin-top: 24px;",
            div { class: "section-header",
                h2 { "Skill Levels" }
            }

            div { class: "filter-bar",
                input {
                    class: "filter-input",
                    placeholder: "Search skills...",
                    value: "{search_filter}",
                    oninput: move |e| search_filter.set(e.value()),
                }
            }

            if let Some(resp) = skill_list {
                table { class: "data-table",
                    thead {
                        tr {
                            th { "Skill Name" }
                            th { "Level" }
                            th { "Usage Count" }
                            th { "Success Rate" }
                            th { "Avg Duration" }
                            th { "Total XP" }
                        }
                    }
                    tbody {
                        for skill in resp.skills.iter().filter(|s| filter_val.is_empty() || s.skill_name.to_lowercase().contains(&filter_val)) {
                            {
                                let skill_name = &skill.skill_name;
                                let level = skill.skill_level;
                                let usage_count = skill.usage_count;
                                let success_str = format!("{:.1}%", skill.success_rate * 100.0);
                                let duration_str = format!("{:.0}ms", skill.avg_duration_ms);
                                let total_xp = skill.total_xp_earned;
                                rsx! {
                                    tr {
                                        td { "{skill_name}" }
                                        td { "{level}" }
                                        td { "{usage_count}" }
                                        td { "{success_str}" }
                                        td { "{duration_str}" }
                                        td { style: "color: #2ECC71; font-weight: 600;", "{total_xp}" }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div { class: "state-message",
                    div { class: "spinner" }
                    span { "Loading skill metrics..." }
                }
            }
        }
    }
}
