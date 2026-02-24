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
use components::toast::ToastOverlay;
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
    #[route("/sub-agents")]
    SubAgents {},
    #[route("/channels")]
    Channels {},
    #[route("/ledger")]
    Ledger {},
    #[route("/workflows")]
    Workflows {},
    #[route("/xp")]
    XpProgression {},
    #[route("/voice")]
    VoiceSettingsPage {},
    #[route("/settings")]
    Settings {},
    #[route("/skill-book")]
    SkillBook {},
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
/// background WebSocket and status-polling tasks, checks setup
/// status, and renders the router (or FirstRunWizard if setup
/// is not complete).
fn app() -> Element {
    let store = use_context_provider(store::EventStreamStore::new);
    use_hook(move || store.start());

    // Check setup status
    let mut setup_complete = use_signal(|| true); // Default to true to avoid flash
    let mut show_wizard = use_signal(|| false);

    use_hook({
        let mut setup_complete = setup_complete.clone();
        let mut show_wizard = show_wizard.clone();
        move || {
            spawn(async move {
                match api::get_setup_status().await {
                    Ok(status) => {
                        setup_complete.set(status.setup_complete);
                        show_wizard.set(!status.setup_complete);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to check setup status");
                        // If we can't check, assume complete to avoid blocking
                        setup_complete.set(true);
                        show_wizard.set(false);
                    }
                }
            });
        }
    });

    let complete_wizard = {
        let mut show_wizard = show_wizard.clone();
        move || {
            show_wizard.set(false);
        }
    };

    rsx! {
        style { {theme::GLOBAL_CSS} }
        SystemTray {}
        if *show_wizard.read() {
            components::first_run_wizard::FirstRunWizard {
                on_complete: complete_wizard,
            }
        }
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
            ToastOverlay {}
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

/// Sub-Agents page (delegates to `pages::sub_agents`).
#[component]
fn SubAgents() -> Element {
    pages::sub_agents::SubAgents()
}

/// Channels page (delegates to `pages::channels`).
#[component]
fn Channels() -> Element {
    pages::channels::Channels()
}

/// Ledger page (delegates to `pages::ledger`).
#[component]
fn Ledger() -> Element {
    pages::ledger::Ledger()
}

/// Workflows page (delegates to `pages::workflows`).
#[component]
fn Workflows() -> Element {
    pages::workflows::Workflows()
}

/// XP Progression page (delegates to `pages::xp_progression`).
#[component]
fn XpProgression() -> Element {
    pages::xp_progression::XpProgression()
}

/// Voice Settings page (renders the VoiceSettings component).
#[component]
fn VoiceSettingsPage() -> Element {
    rsx! {
        div { class: "page-section",
            components::voice_settings::VoiceSettings {}
        }
    }
}

/// Settings page (delegates to `pages::settings`).
#[component]
fn Settings() -> Element {
    pages::settings::Settings()
}

/// Skill Book page (delegates to `pages::skill_book`).
#[component]
fn SkillBook() -> Element {
    pages::skill_book::SkillBook()
}
