//! Events page — real-time event log with filters.

use dioxus::prelude::*;

/// Events page placeholder.
#[component]
pub fn Events() -> Element {
    rsx! {
        div { class: "glass-panel page-panel",
            h1 { "Event Stream" }
            p { "Real-time event log with filters will appear here." }
        }
    }
}
