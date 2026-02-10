//! Top bar component with system status, profile badge, and settings.

use dioxus::prelude::*;

use crate::store::EventStreamStore;
use crate::websocket::ConnectionState;

/// Top bar rendered at the top of the application window.
///
/// Displays the Carnelian logo, connection status indicator,
/// machine profile badge, and a settings button.
#[component]
pub fn TopBar() -> Element {
    let store = use_context::<EventStreamStore>();
    let connection_state = store.connection_state.read();
    let machine_profile = store.machine_profile.read();

    let (dot_class, status_label) = match &*connection_state {
        ConnectionState::Connected => ("status-dot connected", "Connected"),
        ConnectionState::Connecting => ("status-dot connecting", "Connecting..."),
        ConnectionState::Disconnected => ("status-dot disconnected", "Disconnected"),
        ConnectionState::Error(_) => ("status-dot disconnected", "Error"),
    };

    let profile_text = machine_profile.clone();
    let badge_class = match profile_text.to_lowercase().as_str() {
        "thummim" => "badge badge-thummim",
        "urim" => "badge badge-urim",
        _ => "badge badge-custom",
    };

    rsx! {
        div { class: "top-bar",
            // Left: Logo
            div { class: "top-bar-left",
                span { "\u{1F525} Carnelian OS" }
            }

            // Center: Connection status
            div { class: "top-bar-center",
                span { class: "{dot_class}" }
                span { class: "status-label", "{status_label}" }
            }

            // Right: Profile badge + settings
            div { class: "top-bar-right",
                span { class: "{badge_class}", "{profile_text}" }
                button {
                    class: "btn-icon",
                    title: "Settings",
                    onclick: move |_| {
                        tracing::info!("Settings clicked");
                    },
                    "\u{2699}\u{FE0F}"
                }
            }
        }
    }
}
