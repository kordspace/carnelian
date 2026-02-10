//! Tasks page — task list with filters and actions.

use dioxus::prelude::*;

/// Tasks page placeholder.
#[component]
pub fn Tasks() -> Element {
    rsx! {
        div { class: "glass-panel page-panel",
            h1 { "Task Queue" }
            p { "Task list with filters and actions will appear here." }
        }
    }
}
