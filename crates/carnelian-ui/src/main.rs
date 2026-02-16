//! Carnelian OS Desktop UI
//!
//! Dioxus-based desktop application for monitoring and controlling
//! the Carnelian orchestrator. Features real-time event streaming
//! via WebSocket, a glassy dark theme, and router-based navigation.

mod api;
mod components;
mod pages;
mod store;
mod theme;
mod websocket;

use dioxus::prelude::*;

use components::system_tray::SystemTray;
use components::tab_nav::TabNav;
use components::top_bar::TopBar;

/// Application routes.
#[derive(Routable, Clone, Debug, PartialEq, Eq)]
pub enum Route {
    #[layout(Layout)]
    #[route("/")]
    Dashboard {},
    #[route("/tasks")]
    Tasks {},
    #[route("/events")]
    Events {},
    #[route("/skills")]
    Skills {},
    #[route("/approvals")]
    Approvals {},
    #[route("/capabilities")]
    Capabilities {},
    #[route("/heartbeat")]
    Heartbeat {},
    #[route("/identity")]
    Identity {},
    #[route("/providers")]
    Providers {},
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    components::system_tray::init_system_tray();

    dioxus::launch(app);
}

/// Root application component.
///
/// Initializes the `EventStreamStore` context provider, starts
/// background WebSocket and status-polling tasks, and renders
/// the router.
fn app() -> Element {
    let store = use_context_provider(store::EventStreamStore::new);
    use_hook(move || store.start());

    rsx! {
        style { {theme::GLOBAL_CSS} }
        SystemTray {}
        Router::<Route> {}
    }
}

/// Layout component wrapping all routed pages.
///
/// Renders the top bar, tab navigation, and page content area.
#[component]
fn Layout() -> Element {
    rsx! {
        div { class: "app-container",
            TopBar {}
            TabNav {}
            div { class: "main-content",
                Outlet::<Route> {}
            }
        }
    }
}

/// Dashboard page (delegates to `pages::dashboard`).
#[component]
fn Dashboard() -> Element {
    pages::dashboard::Dashboard()
}

/// Tasks page (delegates to `pages::tasks`).
#[component]
fn Tasks() -> Element {
    pages::tasks::Tasks()
}

/// Events page (delegates to `pages::events`).
#[component]
fn Events() -> Element {
    pages::events::Events()
}

/// Skills page (delegates to `pages::skills`).
#[component]
fn Skills() -> Element {
    pages::skills::Skills()
}

/// Approvals page (delegates to `pages::approvals`).
#[component]
fn Approvals() -> Element {
    pages::approvals::Approvals()
}

/// Capabilities page (delegates to `pages::capabilities`).
#[component]
fn Capabilities() -> Element {
    pages::capabilities::Capabilities()
}

/// Heartbeat page (delegates to `pages::heartbeat`).
#[component]
fn Heartbeat() -> Element {
    pages::heartbeat::Heartbeat()
}

/// Identity page (delegates to `pages::identity`).
#[component]
fn Identity() -> Element {
    pages::identity::Identity()
}

/// Providers page (delegates to `pages::providers`).
#[component]
fn Providers() -> Element {
    pages::providers::Providers()
}
