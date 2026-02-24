//! Top bar component with system status, profile badge, and settings.

use dioxus::prelude::*;

use crate::store::EventStreamStore;
use crate::websocket::ConnectionState;
use crate::Route;

/// Format an uptime duration in seconds into a human-readable string.
fn format_uptime(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

/// Top bar rendered at the top of the application window.
///
/// Displays the Carnelian logo, connection status indicator with
/// system health/version/uptime, machine profile badge, and a
/// settings button.
#[component]
pub fn TopBar() -> Element {
    let store = use_context::<EventStreamStore>();
    let connection_state = store.connection_state.read();
    let system_status = store.system_status.read();
    let machine_profile = store.machine_profile.read();

    let (dot_class, status_label) = match &*connection_state {
        ConnectionState::Connected => ("status-dot connected", "Connected"),
        ConnectionState::Connecting => ("status-dot connecting", "Connecting..."),
        ConnectionState::Disconnected => ("status-dot disconnected", "Disconnected"),
        ConnectionState::Error(_) => ("status-dot disconnected", "Error"),
    };

    let health_badge_class = if system_status.healthy {
        "system-status-badge healthy"
    } else {
        "system-status-badge unhealthy"
    };
    let health_label = if system_status.healthy {
        "Running"
    } else {
        "Stopped"
    };

    let version_text = if system_status.version.is_empty() {
        String::new()
    } else {
        format!("v{}", system_status.version)
    };

    let uptime_text = system_status
        .uptime_seconds
        .map(format_uptime)
        .unwrap_or_default();

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

            // Center: Connection status + system health
            div { class: "top-bar-center",
                span { class: "{dot_class}" }
                span { class: "status-label", "{status_label}" }
                div { class: "system-status",
                    span { class: "{health_badge_class}", "{health_label}" }
                    if !version_text.is_empty() {
                        span { class: "system-version", "{version_text}" }
                    }
                    if !uptime_text.is_empty() {
                        span { class: "system-uptime", "\u{23F1} {uptime_text}" }
                    }
                }
            }

            // Right: Profile badge + XP indicator + settings
            div { class: "top-bar-right",
                span { class: "{badge_class}", "{profile_text}" }
                {
                    let xp = store.xp_state.read();
                    let level = xp.level;
                    let progress_pct = xp.progress_pct;
                    let total_xp = xp.total_xp;
                    let width_style = format!("width: {progress_pct:.1}%");
                    rsx! {
                        span { class: "xp-level-badge", "Lv. {level}" }
                        div { class: "xp-progress-bar-container",
                            div { class: "xp-progress-bar-fill", style: "{width_style}" }
                        }
                        span { class: "xp-progress-label", "{total_xp} XP" }
                    }
                }
                let navigator = use_navigator();
                button {
                    class: "btn-icon",
                    title: "Settings",
                    onclick: move |_| {
                        navigator.push(Route::Settings {});
                    },
                    "⚙️"
                }
            }
        }
    }
}
