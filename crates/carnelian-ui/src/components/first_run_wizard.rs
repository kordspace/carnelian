//! First-Run Wizard component — 5-step onboarding modal.
//!
//! Steps:
//! 1. Prerequisites Check (Docker, DB, GPU status)
//! 2. Machine Profile selection (Urim / Thummim / Custom)
//! 3. Owner Keypair display and copy
//! 4. Workspace & Models configuration
//! 5. Starter Skills activation

use dioxus::prelude::*;

use crate::api;
use crate::components::{Toast, ToastType};
use crate::store::EventStreamStore;
use crate::theme::Theme;
use carnelian_common::types::{DetailedHealthResponse, IdentityResponse, SetupStatusResponse};

/// First-Run Wizard component props.
#[derive(Props, Clone, PartialEq)]
pub struct FirstRunWizardProps {
    /// Callback when wizard completes.
    pub on_complete: EventHandler<()>,
}

/// First-Run Wizard component.
#[component]
pub fn FirstRunWizard(props: FirstRunWizardProps) -> Element {
    let theme = use_context::<Theme>();
    let mut step = use_signal(|| 1u8);
    let mut toasts = use_signal(Vec::new);

    // Step 1: Prerequisites
    let mut health_status = use_signal(|| None::<DetailedHealthResponse>);

    // Step 2: Machine Profile
    let mut selected_profile = use_signal(|| "thummim".to_string());

    // Step 3: Identity
    let mut identity = use_signal(|| None::<IdentityResponse>);

    // Step 4: Workspace
    let mut workspace_path = use_signal(|| ".".to_string());
    let mut model_provider = use_signal(|| "ollama".to_string());

    // Step 5: Starter Skills
    let mut selected_skills = use_signal(|| vec![
        ("file-analyzer", true),
        ("code-review", true),
        ("model-usage", true),
        ("web-search", false),
        ("telegram-notify", false),
    ]);

    // Load initial data
    use_hook({
        let mut health_status = health_status.clone();
        let mut identity = identity.clone();
        move || {
            spawn(async move {
                // Load health status
                match api::get_detailed_health().await {
                    Ok(h) => health_status.set(Some(h)),
                    Err(e) => tracing::warn!(error = %e, "Failed to load health status"),
                }
                // Load identity
                match api::get_identity().await {
                    Ok(i) => identity.set(Some(i)),
                    Err(e) => tracing::warn!(error = %e, "Failed to load identity"),
                }
            });
        }
    });

    let theme_class = theme.to_class();

    // Navigation handlers
    let next_step = {
        let mut step = step.clone();
        move || {
            if *step.read() < 5 {
                step.set(*step.read() + 1);
            }
        }
    };

    let prev_step = {
        let mut step = step.clone();
        move || {
            if *step.read() > 1 {
                step.set(*step.read() - 1);
            }
        }
    };

    let complete_wizard = {
        let on_complete = props.on_complete.clone();
        let mut toasts = toasts.clone();
        move || {
            spawn(async move {
                match api::mark_setup_complete().await {
                    Ok(_) => {
                        let toast = Toast {
                            id: uuid::Uuid::now_v7().to_string(),
                            message: "🎉 Setup complete! Welcome to Carnelian OS".to_string(),
                            toast_type: ToastType::Success,
                            duration_secs: 5,
                        };
                        toasts.push(toast);
                        on_complete.call(());
                    }
                    Err(e) => {
                        let toast = Toast {
                            id: uuid::Uuid::now_v7().to_string(),
                            message: format!("Failed to complete setup: {}", e),
                            toast_type: ToastType::Error,
                            duration_secs: 5,
                        };
                        toasts.push(toast);
                    }
                }
            });
        }
    };

    // Render current step
    let step_content = match *step.read() {
        1 => rsx! {
            Step1Prerequisites {
                health: health_status.read().clone(),
            }
        },
        2 => rsx! {
            Step2MachineProfile {
                selected: selected_profile.clone(),
            }
        },
        3 => rsx! {
            Step3Identity {
                identity: identity.read().clone(),
            }
        },
        4 => rsx! {
            Step4Workspace {
                workspace_path: workspace_path.clone(),
                model_provider: model_provider.clone(),
            }
        },
        5 => rsx! {
            Step5StarterSkills {
                selected_skills: selected_skills.clone(),
            }
        },
        _ => rsx! { div {} },
    };

    rsx! {
        div { class: "modal-overlay {theme_class}",
            div { class: "wizard-modal",
                // Header
                div { class: "wizard-header",
                    h2 { "🔥 Carnelian OS Setup" }
                    p { "Step {*step.read()} of 5" }
                    div { class: "progress-bar",
                        div {
                            class: "progress-fill",
                            style: "width: {*step.read() * 20}%",
                        }
                    }
                }

                // Step content
                div { class: "wizard-content",
                    {step_content}
                }

                // Footer with navigation
                div { class: "wizard-footer",
                    if *step.read() > 1 {
                        button {
                            class: "btn-secondary",
                            onclick: move |_| prev_step(),
                            "← Back"
                        }
                    }
                    if *step.read() < 5 {
                        button {
                            class: "btn-primary",
                            onclick: move |_| next_step(),
                            "Next →"
                        }
                    } else {
                        button {
                            class: "btn-success",
                            onclick: move |_| complete_wizard(),
                            "🎉 Complete Setup"
                        }
                    }
                }
            }

            // Toasts
            for toast in toasts.read().iter() {
                Toast { toast: toast.clone() }
            }
        }
    }
}

/// Step 1: Prerequisites Check
#[component]
fn Step1Prerequisites(health: Option<DetailedHealthResponse>) -> Element {
    let docker_ok = health.as_ref().map(|h| h.database == "connected").unwrap_or(false);
    let db_ok = health.as_ref().map(|h| h.database == "connected").unwrap_or(false);

    rsx! {
        div { class: "wizard-step",
            h3 { "Step 1: Prerequisites Check" }
            p { "Ensure your system is ready for Carnelian OS" }

            div { class: "checklist",
                div { class: if docker_ok { "check-item success" } else { "check-item pending" },
                    span { class: "icon", if docker_ok { "✅" } else { "⏳" } }
                    span { "Docker running" }
                }
                div { class: if db_ok { "check-item success" } else { "check-item pending" },
                    span { class: "icon", if db_ok { "✅" } else { "⏳" } }
                    span { "PostgreSQL database connected" }
                }
                div { class: "check-item info",
                    span { class: "icon", "ℹ️" }
                    span { "GPU/VRAM: Auto-detected (optional)" }
                }
            }

            if !docker_ok || !db_ok {
                div { class: "setup-guide",
                    h4 { "Setup Required" }
                    p { "Please ensure Docker is running and the database is accessible." }
                    p { "Run: " code { "docker-compose up -d postgres" } }
                }
            }
        }
    }
}

/// Step 2: Machine Profile Selection
#[component]
fn Step2MachineProfile(selected: Signal<String>) -> Element {
    rsx! {
        div { class: "wizard-step",
            h3 { "Step 2: Machine Profile" }
            p { "Select the hardware profile that matches your system" }

            div { class: "profile-options",
                div {
                    class: if *selected.read() == "urim" { "profile-card selected" } else { "profile-card" },
                    onclick: move |_| selected.set("urim".to_string()),
                    h4 { "🔥 Urim" }
                    p { "High-end workstation" }
                    ul {
                        li { "≥ 48 GB RAM" }
                        li { "≥ 10 GB VRAM" }
                        li { "Local LLMs + heavy workloads" }
                    }
                }
                div {
                    class: if *selected.read() == "thummim" { "profile-card selected" } else { "profile-card" },
                    onclick: move |_| selected.set("thummim".to_string()),
                    h4 { "🦎 Thummim" }
                    p { "Standard desktop/laptop" }
                    ul {
                        li { "16-48 GB RAM" }
                        li { "Integrated or modest GPU" }
                        li { "Balanced local + API usage" }
                    }
                }
                div {
                    class: if *selected.read() == "custom" { "profile-card selected" } else { "profile-card" },
                    onclick: move |_| selected.set("custom".to_string()),
                    h4 { "⚙️ Custom" }
                    p { "Manual configuration" }
                    ul {
                        li { "Define your own limits" }
                        li { "Specialized hardware" }
                        li { "Expert mode" }
                    }
                }
            }
        }
    }
}

/// Step 3: Owner Keypair Display
#[component]
fn Step3Identity(identity: Option<IdentityResponse>) -> Element {
    let public_key = identity.as_ref().map(|i| i.public_key.clone()).unwrap_or_default();

    rsx! {
        div { class: "wizard-step",
            h3 { "Step 3: Owner Keypair" }
            p { "Your Ed25519 keypair for signing and authentication" }

            div { class: "identity-display",
                div { class: "key-field",
                    label { "Public Key (hex)" }
                    code { "{public_key}" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| {
                            // Copy to clipboard would go here
                            tracing::info!("Copy public key clicked");
                        },
                        "📋 Copy"
                    }
                }

                div { class: "info-box",
                    p { "💡 This keypair identifies your Carnelian instance." }
                    p { "The private key is stored securely in the database." }
                }
            }
        }
    }
}

/// Step 4: Workspace & Models
#[component]
fn Step4Workspace(workspace_path: Signal<String>, model_provider: Signal<String>) -> Element {
    rsx! {
        div { class: "wizard-step",
            h3 { "Step 4: Workspace & Models" }
            p { "Configure workspace paths and model providers" }

            div { class: "form-group",
                label { "Workspace Path" }
                input {
                    r#type: "text",
                    value: "{workspace_path}",
                    oninput: move |e| workspace_path.set(e.value().clone()),
                }
                p { class: "help", "Directory to scan for skills and projects" }
            }

            div { class: "form-group",
                label { "Primary Model Provider" }
                select {
                    value: "{model_provider}",
                    onchange: move |e| model_provider.set(e.value().clone()),
                    option { value: "ollama", "Ollama (local)" }
                    option { value: "openai", "OpenAI" }
                    option { value: "anthropic", "Anthropic" }
                    option { value: "fireworks", "Fireworks AI" }
                }
                p { class: "help", "Provider for LLM inference" }
            }

            button {
                class: "btn-secondary",
                onclick: move |_| {
                    // Test connection would go here
                    tracing::info!("Test connection clicked");
                },
                "🧪 Test Connection"
            }
        }
    }
}

/// Step 5: Starter Skills
#[component]
fn Step5StarterSkills(selected_skills: Signal<Vec<(&'static str, bool)>>) -> Element {
    rsx! {
        div { class: "wizard-step",
            h3 { "Step 5: Starter Skills" }
            p { "Select recommended skills to activate now" }

            div { class: "skills-checklist",
                for (skill_id, checked) in selected_skills.read().iter() {
                    label {
                        class: "skill-checkbox",
                        key: "{skill_id}",
                        input {
                            r#type: "checkbox",
                            checked: *checked,
                            onchange: move |_| {
                                let mut skills = selected_skills.read().clone();
                                if let Some(idx) = skills.iter().position(|(id, _)| id == skill_id) {
                                    skills[idx].1 = !skills[idx].1;
                                    selected_skills.set(skills);
                                }
                            },
                        }
                        span { "{skill_name(skill_id)}" }
                    }
                }
            }

            div { class: "info-box",
                p { "You can activate more skills later from the Skill Book" }
            }
        }
    }
}

fn skill_name(id: &str) -> &'static str {
    match id {
        "file-analyzer" => "📁 File Analyzer",
        "code-review" => "🔍 Code Review",
        "model-usage" => "📊 Model Usage",
        "web-search" => "🌐 Web Search",
        "telegram-notify" => "💬 Telegram Notify",
        _ => id,
    }
}
