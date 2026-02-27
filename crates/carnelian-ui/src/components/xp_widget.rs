//! XP Widget component — compact card showing level, progress, and recent gains.

#![allow(clippy::manual_map)]
#![allow(clippy::bool_to_int_with_if)]
#![allow(clippy::suboptimal_flops)]

use dioxus::prelude::*;

use crate::store::EventStreamStore;

/// Compact XP widget card with level, progress bar, and recent history.
#[component]
pub fn XpWidget() -> Element {
    let store = use_context::<EventStreamStore>();
    let xp = store.xp_state.read();

    let level = xp.level;
    let total_xp = xp.total_xp;
    let xp_to_next = xp.xp_to_next_level;
    let progress_pct = xp.progress_pct;
    let milestone = xp.milestone_feature.clone();
    let identity_id = xp.identity_id;
    let width_style = format!("width: {progress_pct:.1}%");

    let history = use_resource(move || async move {
        if let Some(id) = identity_id {
            crate::api::get_xp_history(id, 1, 10).await.ok()
        } else {
            None
        }
    });

    rsx! {
        div { class: "xp-widget",
            // Header row
            div { class: "flex-row", style: "justify-content: space-between; align-items: center; margin-bottom: 12px;",
                div { class: "flex-row", style: "gap: 8px; align-items: center;",
                    span { class: "xp-level-badge", "Lv. {level}" }
                    span { style: "font-size: 16px; font-weight: 600; color: #E0E0E0;", "Level {level}" }
                }
                span { style: "font-size: 14px; color: #A0A0A0;", "{total_xp} XP" }
            }

            // Progress bar (larger)
            div { class: "xp-progress-bar-container", style: "height: 10px; margin-bottom: 8px;",
                div { class: "xp-progress-bar-fill", style: "{width_style}; height: 100%;" }
            }

            // XP to next level
            div { style: "font-size: 12px; color: #A0A0A0; margin-bottom: 16px;",
                "{xp_to_next} XP to next level"
                if let Some(ref feat) = milestone {
                    span { style: "margin-left: 8px; color: #F39C12;", "Next: {feat}" }
                }
            }

            // Source breakdown pie chart
            {
                let history_read = history.read();
                let events = (*history_read).as_ref().and_then(|o| o.as_ref());
                if let Some(resp) = events {
                    let mut task_count = 0u32;
                    let mut ledger_count = 0u32;
                    let mut skill_count = 0u32;
                    let mut quality_count = 0u32;
                    for evt in &resp.events {
                        match evt.source.as_str() {
                            "task_completion" => task_count += 1,
                            "ledger_signing" => ledger_count += 1,
                            "skill_usage" => skill_count += 1,
                            "quality_bonus" => quality_count += 1,
                            _ => {}
                        }
                    }
                    let total = f64::from((task_count + ledger_count + skill_count + quality_count).max(1));
                    let slices: Vec<(&str, f64, &str)> = vec![
                        ("Tasks", f64::from(task_count) / total, "#4A90E2"),
                        ("Ledger", f64::from(ledger_count) / total, "#9B59B6"),
                        ("Skills", f64::from(skill_count) / total, "#2ECC71"),
                        ("Quality", f64::from(quality_count) / total, "#F39C12"),
                    ];

                    // Build SVG pie chart paths
                    let mut start_angle: f64 = 0.0;
                    let mut paths = Vec::new();
                    for (label, frac, color) in &slices {
                        if *frac > 0.0 {
                            let sweep = frac * 360.0;
                            let end_angle = start_angle + sweep;
                            let large_arc = if sweep > 180.0 { 1 } else { 0 };
                            let r = 40.0_f64;
                            let cx = 50.0_f64;
                            let cy = 50.0_f64;
                            let x1 = cx + r * start_angle.to_radians().cos();
                            let y1 = cy + r * start_angle.to_radians().sin();
                            let x2 = cx + r * end_angle.to_radians().cos();
                            let y2 = cy + r * end_angle.to_radians().sin();
                            let d = format!(
                                "M {cx} {cy} L {x1:.2} {y1:.2} A {r} {r} 0 {large_arc} 1 {x2:.2} {y2:.2} Z"
                            );
                            paths.push((d, (*color).to_string(), (*label).to_string()));
                            start_angle = end_angle;
                        }
                    }

                    rsx! {
                        div { style: "display: flex; gap: 16px; align-items: center; margin-bottom: 16px;",
                            div { class: "xp-pie-chart",
                                svg {
                                    width: "80",
                                    height: "80",
                                    view_box: "0 0 100 100",
                                    for (d, color, _label) in &paths {
                                        path {
                                            d: "{d}",
                                            fill: "{color}",
                                            opacity: "0.8",
                                        }
                                    }
                                }
                            }
                            div { style: "display: flex; flex-direction: column; gap: 4px;",
                                for (label, frac, color) in &slices {
                                    {
                                        let pct_str = format!("{}: {:.0}%", label, frac * 100.0);
                                        let dot_style = format!("width: 8px; height: 8px; border-radius: 50%; background: {color}; display: inline-block;");
                                        rsx! {
                                            div { style: "display: flex; align-items: center; gap: 6px; font-size: 11px;",
                                                span { style: "{dot_style}" }
                                                span { style: "color: #A0A0A0;", "{pct_str}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    rsx! {
                        div { class: "state-message", style: "padding: 12px;",
                            span { "Loading XP history..." }
                        }
                    }
                }
            }

            // Recent gains list
            {
                let history_read = history.read();
                let events = (*history_read).as_ref().and_then(|o| o.as_ref());
                if let Some(resp) = events {
                    rsx! {
                        div { style: "margin-top: 4px;",
                            h3 { style: "font-size: 13px; margin-bottom: 8px;", "Recent Gains" }
                            for evt in resp.events.iter().take(10) {
                                {
                                    let source = &evt.source;
                                    let xp_str = format!("+{} XP", evt.xp_amount);
                                    let ts_str = evt.created_at.format("%m/%d %H:%M").to_string();
                                    rsx! {
                                        div { style: "display: flex; justify-content: space-between; padding: 4px 0; border-bottom: 1px solid rgba(255,255,255,0.04); font-size: 12px;",
                                            div { style: "display: flex; gap: 8px; align-items: center;",
                                                span { class: "badge-status", style: "font-size: 10px; padding: 1px 6px;", "{source}" }
                                            }
                                            span { style: "color: #2ECC71; font-weight: 600;", "{xp_str}" }
                                            span { style: "color: #7F8C8D; font-size: 11px;", "{ts_str}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}
