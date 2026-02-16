//! Identity page — core identity information and soul file preview.

use carnelian_common::types::EventType;
use dioxus::prelude::*;

use crate::store::EventStreamStore;

/// Identity information page.
#[component]
pub fn Identity() -> Element {
    let store = use_context::<EventStreamStore>();
    let mut refresh = use_signal(|| 0_u64);

    // Fetch identity data
    let identity = use_resource(move || async move {
        let _ = refresh();
        crate::api::get_identity().await.ok()
    });

    // Fetch soul content
    let soul_content = use_resource(move || async move {
        let _ = refresh();
        crate::api::get_soul_content().await.ok()
    });

    // Trigger refresh on SoulUpdated events
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            if matches!(last.event_type, EventType::SoulUpdated) {
                refresh += 1;
            }
        }
    });

    let identity_read = identity.read();
    let id_data = (*identity_read).as_ref().and_then(|o| o.as_ref());

    let name = id_data.map_or("Unknown", |i| i.name.as_str());
    let pronouns = id_data.and_then(|i| i.pronouns.as_deref()).unwrap_or("—");
    let identity_type = id_data.map_or("—", |i| i.identity_type.as_str());
    let directive_count = id_data.map_or(0, |i| i.directive_count);
    let soul_path = id_data
        .and_then(|i| i.soul_file_path.as_deref())
        .unwrap_or("—");
    let created = id_data.map_or_else(
        || "—".to_string(),
        |i| i.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    );
    let updated = id_data.map_or_else(
        || "—".to_string(),
        |i| i.updated_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    );

    let soul_read = soul_content.read();
    let soul_text = (*soul_read).as_ref().and_then(|o| o.as_ref()).map_or_else(
        || "Soul file not available.".to_string(),
        |s| {
            let char_count = s.chars().count();
            if char_count > 2000 {
                let truncated: String = s.chars().take(2000).collect();
                format!("{truncated}…\n\n(truncated — {char_count} total characters)")
            } else {
                s.clone()
            }
        },
    );

    rsx! {
        div { class: "page-panel panel-page",
            // ── Identity Information ────────────────────────
            div { class: "section-header", h2 { "Identity" } }
            div { class: "metrics-grid",
                div { class: "metric-card",
                    span { class: "metric-value", "{name}" }
                    span { class: "metric-label", "Name" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{pronouns}" }
                    span { class: "metric-label", "Pronouns" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{identity_type}" }
                    span { class: "metric-label", "Type" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{directive_count}" }
                    span { class: "metric-label", "Directives" }
                }
            }
            div { class: "metrics-grid",
                div { class: "metric-card",
                    span { class: "metric-value", "{soul_path}" }
                    span { class: "metric-label", "Soul File" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{created}" }
                    span { class: "metric-label", "Created" }
                }
                div { class: "metric-card",
                    span { class: "metric-value", "{updated}" }
                    span { class: "metric-label", "Updated" }
                }
            }

            // ── Soul File Preview ───────────────────────────
            div { class: "section-header", h2 { "Soul File (SOUL.md)" } }
            div { class: "soul-preview",
                pre { code { "{soul_text}" } }
            }
        }
    }
}
