//! First-run wizard component for initial setup.

#![allow(clippy::clone_on_copy)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::redundant_closure_call)]
#![allow(clippy::shadow_unrelated)]
#![allow(clippy::redundant_locals)]
//! 4. Workspace & Models configuration
//! 5. Starter Skills activation

use dioxus::prelude::*;

use crate::api;
use crate::components::{Toast, ToastMessage, ToastType};
use crate::theme::Theme;
use carnelian_common::types::{DetailedHealthResponse, IdentityResponse};

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
    let step = use_signal(|| 1u8);
    let toasts = use_signal(Vec::new);

    // Step 1: Prerequisites
    let health_status = use_signal(|| None::<DetailedHealthResponse>);

    // Step 2: Machine Profile
    let selected_profile = use_signal(|| "thummim".to_string());

    // Step 3: Identity
    let identity = use_signal(|| None::<IdentityResponse>);

    // Step 4: Workspace
    let workspace_path = use_signal(|| ".".to_string());
    let model_provider = use_signal(|| "ollama".to_string());

    // Step 5: Starter Skills
    let selected_skills = use_signal(|| {
        vec![
            ("file-analyzer", true),
            ("code-review", true),
            ("model-usage", true),
            ("web-search", false),
            ("telegram-notify", false),
        ]
    });

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
    let mut next_step = {
        let mut step = step.clone();
        move || {
            let current = *step.read();
            if current < 6 {
                step.set(current + 1);
            }
        }
    };

    let mut prev_step = {
        let mut step = step.clone();
        move || {
            let current = *step.read();
            if current > 1 {
                step.set(current - 1);
            }
        }
    };

    let complete_wizard = {
        let on_complete = props.on_complete.clone();
        let mut toasts = toasts.clone();
        let selected_skills = selected_skills.clone();
        move || {
            spawn(async move {
                // First, activate selected starter skills
                let skills_to_activate: Vec<&str> = selected_skills
                    .read()
                    .iter()
                    .filter(|(_, checked)| *checked)
                    .map(|(id, _)| *id)
                    .collect();

                for skill_id in skills_to_activate {
                    let config = std::collections::HashMap::new();
                    if let Err(e) = api::activate_skill(skill_id, config).await {
                        tracing::warn!(error = %e, skill_id = skill_id, "Failed to activate skill");
                        let toast = ToastMessage {
                            id: uuid::Uuid::now_v7().to_string(),
                            message: format!(
                                "⚠️ Failed to activate {}: {}",
                                skill_name(skill_id),
                                e
                            ),
                            toast_type: ToastType::Warning,
                            duration_secs: 5,
                        };
                        toasts.push(toast);
                    }
                }

                // Then mark setup complete
                match api::mark_setup_complete().await {
                    Ok(_) => {
                        let toast = ToastMessage {
                            id: uuid::Uuid::now_v7().to_string(),
                            message: "🎉 Setup complete! Welcome to Carnelian OS".to_string(),
                            toast_type: ToastType::Success,
                            duration_secs: 5,
                        };
                        toasts.push(toast);
                        on_complete.call(());
                    }
                    Err(e) => {
                        let toast = ToastMessage {
                            id: uuid::Uuid::now_v7().to_string(),
                            message: format!("Failed to complete setup: {e}"),
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
        6 => rsx! {
            Step6MagicSetup {}
        },
        _ => rsx! { div {} },
    };

    rsx! {
        div { class: "modal-overlay {theme_class}",
            div { class: "wizard-modal",
                // Header
                div { class: "wizard-header",
                    h2 { "🔥 Carnelian OS Setup" }
                    p { "Step {*step.read()} of 6" }
                    div { class: "progress-bar",
                        div {
                            class: "progress-fill",
                            style: "width: {*step.read() * 100 / 6}%",
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
                    if *step.read() < 6 {
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
    let docker_ok = health.as_ref().is_some_and(|h| h.database == "connected");
    let db_ok = health.as_ref().is_some_and(|h| h.database == "connected");

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
    let public_key = identity
        .as_ref()
        .map(|i| i.public_key.clone())
        .unwrap_or_default();

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
                    oninput: move |e| workspace_path.set(e.value()),
                }
                p { class: "help", "Directory to scan for skills and projects" }
            }

            div { class: "form-group",
                label { "Primary Model Provider" }
                select {
                    value: "{model_provider}",
                    onchange: move |e| model_provider.set(e.value()),
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
    let skills_snapshot: Vec<(&'static str, bool)> = selected_skills.read().clone();
    rsx! {
        div { class: "wizard-step",
            h3 { "Step 5: Starter Skills" }
            p { "Select recommended skills to activate now" }

            div { class: "skills-checklist",
                for (skill_id, checked) in skills_snapshot {
                    label {
                        class: "skill-checkbox",
                        key: "{skill_id}",
                        input {
                            r#type: "checkbox",
                            checked: checked,
                            onchange: {
                                let skill_id = skill_id;
                                move |_| {
                                    let mut skills = selected_skills.read().clone();
                                    if let Some(idx) = skills.iter().position(|(id, _)| *id == skill_id) {
                                        skills[idx].1 = !skills[idx].1;
                                        selected_skills.set(skills);
                                    }
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
        _ => "Unknown Skill",
    }
}

#[component]
fn Step6MagicSetup() -> Element {
    let mut quantum_origin_key = use_signal(String::new);
    let mut quantinuum_email = use_signal(String::new);
    let mut quantinuum_password = use_signal(String::new);
    let mut auth_status = use_signal(|| None::<carnelian_common::types::MagicAuthStatusResponse>);
    let mut loading = use_signal(|| false);
    let mut feedback = use_signal(String::new);

    use_hook(|| {
        spawn(async move {
            if let Ok(status) = api::magic_auth_status().await {
                auth_status.set(Some(status));
            }
        });
    });

    let save_quantum_origin = move || {
        spawn(async move {
            loading.set(true);
            let key = quantum_origin_key.read().clone();
            match api::magic_update_config(Some(key), None, None).await {
                Ok(_) => {
                    if let Ok(status) = api::magic_auth_status().await {
                        auth_status.set(Some(status));
                    }
                    feedback.set("Quantum Origin key saved ✓".to_string());
                }
                Err(e) => {
                    feedback.set(format!("Error: {}", e));
                }
            }
            loading.set(false);
        });
    };

    let authenticate_quantinuum = move || {
        spawn(async move {
            loading.set(true);
            let email = quantinuum_email.read().clone();
            let password = quantinuum_password.read().clone();
            match api::magic_quantinuum_login(email, password).await {
                Ok(resp) => {
                    if let Ok(status) = api::magic_auth_status().await {
                        auth_status.set(Some(status));
                    }
                    feedback.set(format!("Authenticated ✓ {}", resp.message));
                    quantinuum_password.set(String::new());
                }
                Err(e) => {
                    feedback.set(format!("Error: {}", e));
                }
            }
            loading.set(false);
        });
    };

    let test_ibm_connection = move || {
        spawn(async move {
            loading.set(true);
            match api::magic_update_config(None, None, Some(true)).await {
                Ok(_) => {
                    match api::magic_entropy_health().await {
                        Ok(health) => {
                            // Validate provider-specific readiness from health payload
                            let qiskit_available = health.get("qiskit-rng")
                                .and_then(|v| v.get("available"))
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            
                            if qiskit_available {
                                feedback.set("IBM Quantum (Qiskit) provider verified ✓".to_string());
                            } else {
                                // Provider not ready - revert qiskit_enabled
                                let _ = api::magic_update_config(None, None, Some(false)).await;
                                let error_msg = health.get("qiskit-rng")
                                    .and_then(|v| v.get("error"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Provider not available");
                                feedback.set(format!("IBM Quantum provider not ready: {}", error_msg));
                            }
                        }
                        Err(e) => {
                            // Revert qiskit_enabled on health check failure
                            let _ = api::magic_update_config(None, None, Some(false)).await;
                            feedback.set(format!("Connection test failed: {}", e));
                        }
                    }
                }
                Err(e) => {
                    feedback.set(format!("Error: {}", e));
                }
            }
            loading.set(false);
        });
    };

    rsx! {
        div { class: "step-content",
            h3 { "✨ MAGIC Setup (Optional)" }
            p { "Configure quantum entropy providers for enhanced randomness" }

            div { class: "info-box",
                p { "Install quantum skill dependencies:" }
                code { "pip install qiskit qiskit-ibm-runtime pennylane" }
            }

            // Quantum Origin Section
            div { class: "provider-section",
                h4 { "Quantum Origin" }
                {
                    if let Some(status) = auth_status.read().as_ref() {
                        if status.quantum_origin.configured {
                            rsx! { span { class: "badge-success", "✅ Configured" } }
                        } else {
                            rsx! { span { class: "badge-inactive", "⚪ Not Configured" } }
                        }
                    } else {
                        rsx! { span {} }
                    }
                }
                input {
                    r#type: "password",
                    value: "{quantum_origin_key}",
                    oninput: move |e| quantum_origin_key.set(e.value().clone()),
                    placeholder: "API Key",
                    disabled: *loading.read()
                }
                div { class: "button-group",
                    button {
                        onclick: move |_| save_quantum_origin(),
                        disabled: *loading.read() || quantum_origin_key.read().is_empty(),
                        "Save Key"
                    }
                    button {
                        class: "btn-secondary",
                        onclick: move |_| quantum_origin_key.set(String::new()),
                        "Skip"
                    }
                }
            }

            // Quantinuum Section
            div { class: "provider-section",
                h4 { "Quantinuum" }
                {
                    if let Some(status) = auth_status.read().as_ref() {
                        if status.quantinuum.authenticated {
                            rsx! {
                                span { class: "badge-success", "✅ Authenticated" }
                                if let Some(exp) = &status.quantinuum.expiry {
                                    span { class: "expiry-text", " until {exp}" }
                                }
                            }
                        } else {
                            rsx! { span { class: "badge-inactive", "⚪ Not Authenticated" } }
                        }
                    } else {
                        rsx! { span {} }
                    }
                }
                input {
                    r#type: "email",
                    value: "{quantinuum_email}",
                    oninput: move |e| quantinuum_email.set(e.value().clone()),
                    placeholder: "Email",
                    disabled: *loading.read()
                }
                input {
                    r#type: "password",
                    value: "{quantinuum_password}",
                    oninput: move |e| quantinuum_password.set(e.value().clone()),
                    placeholder: "Password",
                    disabled: *loading.read()
                }
                div { class: "button-group",
                    button {
                        onclick: move |_| authenticate_quantinuum(),
                        disabled: *loading.read() || quantinuum_email.read().is_empty() || quantinuum_password.read().is_empty(),
                        "Authenticate"
                    }
                    button {
                        class: "btn-secondary",
                        onclick: move |_| {
                            quantinuum_email.set(String::new());
                            quantinuum_password.set(String::new());
                        },
                        "Skip"
                    }
                }
            }

            // IBM Quantum Section
            div { class: "provider-section",
                h4 { "IBM Quantum (Qiskit)" }
                p { class: "provider-note", "Enable Qiskit RNG provider (requires qiskit installation)" }
                div { class: "button-group",
                    button {
                        onclick: move |_| test_ibm_connection(),
                        disabled: *loading.read(),
                        "Enable & Test Provider"
                    }
                    button {
                        class: "btn-secondary",
                        onclick: move |_| feedback.set(String::new()),
                        "Skip"
                    }
                }
            }

            // Feedback message
            if !feedback.read().is_empty() {
                div { class: "feedback-message",
                    p { "{feedback}" }
                }
            }

            div { class: "info-box",
                p { "You can configure MAGIC providers later from the ✨ MAGIC page" }
            }
        }
    }
}
