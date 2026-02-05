//! Carnelian OS Desktop UI
//!
//! Dioxus-based desktop application for monitoring and controlling
//! the Carnelian orchestrator.

use dioxus::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();

    dioxus::launch(app);
}

fn app() -> Element {
    rsx! {
        div {
            h1 { "Carnelian OS" }
            p { "Version: {carnelian_common::VERSION}" }
            p { "A local-first AI agent mainframe" }
        }
    }
}
