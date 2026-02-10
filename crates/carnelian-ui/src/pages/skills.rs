//! Skills page — skill registry with enable/disable actions.

use dioxus::prelude::*;

/// Skills page placeholder.
#[component]
pub fn Skills() -> Element {
    rsx! {
        div { class: "glass-panel page-panel",
            h1 { "Skills" }
            p { "Skill registry with enable/disable actions will appear here." }
        }
    }
}
