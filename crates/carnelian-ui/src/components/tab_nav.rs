//! Tab navigation component for switching between pages.

use dioxus::prelude::*;

use crate::Route;

/// Horizontal tab navigation bar.
///
/// Renders four tabs (Dashboard, Tasks, Events, Skills) using
/// Dioxus router `Link` components. The active tab is highlighted
/// based on the current route.
#[component]
pub fn TabNav() -> Element {
    let route: Route = use_route();

    let tabs: Vec<(&str, Route)> = vec![
        ("Dashboard", Route::Dashboard {}),
        ("Tasks", Route::Tasks {}),
        ("Events", Route::Events {}),
        ("Skills", Route::Skills {}),
        ("Approvals", Route::Approvals {}),
        ("Capabilities", Route::Capabilities {}),
        ("Heartbeat", Route::Heartbeat {}),
        ("Identity", Route::Identity {}),
        ("Providers", Route::Providers {}),
        ("Sub-Agents", Route::SubAgents {}),
        ("Channels", Route::Channels {}),
        ("Ledger", Route::Ledger {}),
        ("Settings", Route::Settings {}),
        ("Skill Book", Route::SkillBook {}),
        ("Elixirs 🧪", Route::Elixirs {}),
        ("✨ MAGIC", Route::Magic {}),
        ("Workflows", Route::Workflows {}),
        ("XP", Route::XpProgression {}),
        ("Voice", Route::VoiceSettingsPage {}),
    ];

    rsx! {
        nav { class: "tab-nav",
            role: "tablist",
            for (label, target) in tabs {
                Link {
                    to: target.clone(),
                    class: if route == target { "tab-link active" } else { "tab-link" },
                    role: "tab",
                    aria_selected: if route == target { "true" } else { "false" },
                    "{label}"
                }
            }
        }
    }
}
