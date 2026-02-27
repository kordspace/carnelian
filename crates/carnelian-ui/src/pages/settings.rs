//! Settings Hub page — central configuration overview.
//!
//! Features:
//! - Sections as cards: Identity, Voice, Models, Workspace, Machine Profile, Security, About
//! - Each card links to relevant existing routes
//! - About section shows version from `system_status`

#![allow(clippy::clone_on_copy)]

use dioxus::prelude::*;

use crate::api;
use crate::theme::Theme;
use carnelian_common::types::StatusResponse;

/// Settings Hub page component.
#[component]
pub fn Settings() -> Element {
    let theme = use_context::<Theme>();
    let status = use_signal(|| None::<StatusResponse>);

    // Load system status on mount
    use_hook({
        let mut status = status.clone();
        move || {
            spawn(async move {
                match api::get_system_status().await {
                    Ok(s) => status.set(Some(s)),
                    Err(e) => tracing::warn!(error = %e, "Failed to load system status"),
                }
            });
        }
    });

    let theme_class = theme.to_class();
    let version = status
        .read()
        .as_ref()
        .map_or_else(|| "Unknown".to_string(), |s| s.version.clone());

    rsx! {
        div { class: "page settings-page {theme_class}",
            // Header
            div { class: "page-header",
                h1 { "Settings" }
                p { class: "subtitle", "Configure your Carnelian OS instance" }
            }

            // Settings grid
            div { class: "settings-grid",
                // Identity Card
                div { class: "settings-card",
                    div { class: "card-header",
                        span { class: "icon", "👤" }
                        h3 { "Identity" }
                    }
                    p { "Manage your agent identity and keypairs" }
                    Link { to: crate::Route::Identity {}, class: "card-link", "Configure →" }
                }

                // Voice Card
                div { class: "settings-card",
                    div { class: "card-header",
                        span { class: "icon", "🎙️" }
                        h3 { "Voice" }
                    }
                    p { "Configure text-to-speech and speech-to-text settings" }
                    Link { to: crate::Route::VoiceSettingsPage {}, class: "card-link", "Configure →" }
                }

                // Models Card
                div { class: "settings-card",
                    div { class: "card-header",
                        span { class: "icon", "🤖" }
                        h3 { "Models & Providers" }
                    }
                    p { "Configure LLM providers and model routing" }
                    Link { to: crate::Route::Providers {}, class: "card-link", "Configure →" }
                }

                // Workspace Card
                div { class: "settings-card",
                    div { class: "card-header",
                        span { class: "icon", "📁" }
                        h3 { "Workspace" }
                    }
                    p { "Manage skill directories and workspace paths" }
                    span { class: "card-note", "Edit machine.toml to configure" }
                }

                // Machine Profile Card
                div { class: "settings-card",
                    div { class: "card-header",
                        span { class: "icon", "⚙️" }
                        h3 { "Machine Profile" }
                    }
                    p { "View current hardware profile and capabilities" }
                    span { class: "card-value", "Profile: {status.read().as_ref().map(|s| s.machine_profile.clone()).unwrap_or_default()}" }
                }

                // Security Card
                div { class: "settings-card",
                    div { class: "card-header",
                        span { class: "icon", "🔒" }
                        h3 { "Security" }
                    }
                    p { "Manage capabilities and approval settings" }
                    Link { to: crate::Route::Capabilities {}, class: "card-link", "Capabilities →" }
                    Link { to: crate::Route::Approvals {}, class: "card-link", "Approvals →" }
                }

                // About Card
                div { class: "settings-card about-card",
                    div { class: "card-header",
                        span { class: "icon", "💎" }
                        h3 { "About" }
                    }
                    p { "🔥 Carnelian OS — Local-first AI agent mainframe" }
                    p { class: "version", "Version: {version}" }
                    p { class: "tagline", "Warm, fiery, and grounding — like the gemstone" }
                }
            }
        }
    }
}
