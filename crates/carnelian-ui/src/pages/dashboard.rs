//! Dashboard page — task queue summary, system health, and recent events.

use dioxus::prelude::*;

/// Dashboard page placeholder.
#[component]
pub fn Dashboard() -> Element {
    rsx! {
        div { class: "glass-panel page-panel",
            h1 { "Dashboard" }
            p { "Task queue summary, system health, and recent events will appear here." }
        }
    }
}
