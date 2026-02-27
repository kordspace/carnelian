//! Skill Book Library page — curated skill catalog with activation.
//!
//! Features:
//! - Category filter tabs
//! - Grid of skill cards with activation status
//! - Inline activation wizard with config prompts
//! - Deactivate functionality

use dioxus::prelude::*;
use std::collections::HashMap;

use crate::api;
use crate::components::{Toast, ToastMessage, ToastType};
use crate::store::EventStreamStore;
use crate::theme::Theme;
use carnelian_common::types::{SkillBookCatalog, SkillBookEntry};

/// Skill Book Library page component.
#[component]
pub fn SkillBook() -> Element {
    let theme = use_context::<Theme>();
    let _event_store = use_context::<EventStreamStore>();
    let mut toasts = use_signal(Vec::new);

    // State
    let mut catalog = use_signal(|| None::<SkillBookCatalog>);
    let mut selected_category = use_signal(|| "all".to_string());
    let mut loading = use_signal(|| false);
    let mut show_activation_modal = use_signal(|| false);
    let mut selected_skill = use_signal(|| None::<SkillBookEntry>);
    let mut config_values = use_signal(HashMap::<String, String>::new);

    // Load catalog on mount
    let load_catalog = {
        let mut catalog = catalog.clone();
        let mut loading = loading.clone();
        move || {
            loading.set(true);
            spawn(async move {
                match api::list_skill_book().await {
                    Ok(c) => catalog.set(Some(c)),
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to load Skill Book");
                    }
                }
                loading.set(false);
            });
        }
    };

    use_hook({
        let mut load_catalog = load_catalog.clone();
        move || {
            load_catalog();
        }
    });

    // Filter skills by category
    let filtered_skills = {
        let cat = selected_category.read();
        catalog
            .read()
            .as_ref()
            .map(|c| {
                c.skills
                    .iter()
                    .filter(|s| *cat == "all" || s.category == *cat)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    // Categories for tabs - compute owned strings to avoid lifetime issues
    let categories: Vec<(String, String)> = catalog
        .read()
        .as_ref()
        .map(|c| {
            let mut cats = vec![("all".to_string(), "All".to_string())];
            cats.extend(c.categories.iter().map(|id| {
                let name = match id.as_str() {
                    "code" => "Code",
                    "research" => "Research",
                    "communication" => "Communication",
                    "creative" => "Creative",
                    "data" => "Data",
                    "automation" => "Automation",
                    _ => id.as_str(),
                };
                (id.clone(), name.to_string())
            }));
            cats
        })
        .unwrap_or_else(|| vec![("all".to_string(), "All".to_string())]);

    // Activation handler
    let activate_skill = {
        let mut toasts = toasts.clone();
        let mut show_modal = show_activation_modal.clone();
        let mut load_catalog = load_catalog.clone();
        move |skill: Option<SkillBookEntry>, config: HashMap<String, String>| {
            if let Some(ref s) = skill {
                let skill_id = s.id.clone();
                spawn(async move {
                    match api::activate_skill(&skill_id, config).await {
                        Ok(_) => {
                            let toast = ToastMessage {
                                id: uuid::Uuid::now_v7().to_string(),
                                message: format!("✅ {} activated successfully", skill_id),
                                toast_type: ToastType::Success,
                                duration_secs: 3,
                            };
                            toasts.push(toast);
                            show_modal.set(false);
                            load_catalog();
                        }
                        Err(e) => {
                            let toast = ToastMessage {
                                id: uuid::Uuid::now_v7().to_string(),
                                message: format!("❌ Failed to activate: {}", e),
                                toast_type: ToastType::Error,
                                duration_secs: 5,
                            };
                            toasts.push(toast);
                        }
                    }
                });
            }
        }
    };

    // Deactivation handler
    let deactivate_skill = {
        let mut toasts = toasts.clone();
        let mut load_catalog = load_catalog.clone();
        move |skill_id: String| {
            spawn(async move {
                match api::deactivate_skill(&skill_id).await {
                    Ok(_) => {
                        let toast = ToastMessage {
                            id: uuid::Uuid::now_v7().to_string(),
                            message: format!("✅ {} deactivated", skill_id),
                            toast_type: ToastType::Success,
                            duration_secs: 3,
                        };
                        toasts.push(toast);
                        load_catalog();
                    }
                    Err(e) => {
                        let toast = ToastMessage {
                            id: uuid::Uuid::now_v7().to_string(),
                            message: format!("❌ Failed to deactivate: {}", e),
                            toast_type: ToastType::Error,
                            duration_secs: 5,
                        };
                        toasts.push(toast);
                    }
                }
            });
        }
    };

    // Open activation modal
    let open_activation = {
        let mut show_modal = show_activation_modal.clone();
        let mut selected = selected_skill.clone();
        let mut config_vals = config_values.clone();
        move |skill: SkillBookEntry| {
            config_vals.set(HashMap::new());
            selected.set(Some(skill));
            show_modal.set(true);
        }
    };

    let theme_class = theme.to_class();

    rsx! {
        div { class: "page skill-book-page {theme_class}",
            // Header
            div { class: "page-header",
                h1 { "Skill Book" }
                p { class: "subtitle", "Curated library of ready-to-use skills" }
            }

            // Category tabs
            div { class: "category-tabs",
                for (id, name) in categories {
                    button {
                        class: if *selected_category.read() == id { "tab active" } else { "tab" },
                        onclick: move |_| selected_category.set(id.to_string()),
                        "{name}"
                    }
                }
            }

            // Skills grid
            if *loading.read() {
                div { class: "loading", "Loading skills..." }
            } else if filtered_skills.is_empty() {
                div { class: "empty-state", "No skills found in this category" }
            } else {
                div { class: "skills-grid",
                    for skill in filtered_skills {
                        div { class: "skill-card",
                            key: "{skill.id}",
                            div { class: "card-header",
                                h3 { "{skill.name}" }
                                span { class: "category-badge", "{skill.category}" }
                            }
                            p { class: "description", "{skill.description}" }
                            div { class: "card-meta",
                                span { class: "runtime", "🔧 {skill.runtime}" }
                                span { class: "version", "v{skill.version}" }
                            }
                            if skill.activated {
                                div { class: "activation-status active",
                                    "✅ Active"
                                    button {
                                        class: "btn-secondary btn-sm",
                                        onclick: {
                                            let id = skill.id.clone();
                                            let deactivate = deactivate_skill.clone();
                                            move |_| deactivate(id.clone())
                                        },
                                        "Deactivate"
                                    }
                                }
                            } else {
                                div { class: "activation-status inactive",
                                    "⏳ Not activated"
                                    button {
                                        class: "btn-primary btn-sm",
                                        onclick: {
                                            let s = skill.clone();
                                            let mut open = open_activation.clone();
                                            move |_| open(s.clone())
                                        },
                                        "Activate"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Activation modal
            if *show_activation_modal.read() {
                if let Some(ref skill) = *selected_skill.read() {
                    div { class: "modal-overlay",
                        div { class: "modal",
                            h3 { "Activate {skill.name}" }
                            p { "{skill.description}" }

                            if !skill.required_config.is_empty() {
                                div { class: "config-form",
                                    h4 { "Configuration Required" }
                                    for field in skill.required_config.iter() {
                                        div { class: "form-group",
                                            key: "{field.key}",
                                            label { "{field.label}" }
                                            input {
                                                r#type: if field.secret { "password" } else { "text" },
                                                placeholder: "Enter value...",
                                                oninput: {
                                                    let key = field.key.clone();
                                                    let mut vals = config_values.clone();
                                                    move |e: Event<FormData>| {
                                                        let mut v = vals.read().clone();
                                                        v.insert(key.clone(), e.value().clone());
                                                        vals.set(v);
                                                    }
                                                },
                                            }
                                        }
                                    }
                                }
                            }

                            div { class: "modal-actions",
                                button {
                                    class: "btn-secondary",
                                    onclick: move |_| show_activation_modal.set(false),
                                    "Cancel"
                                }
                                button {
                                    class: "btn-primary",
                                    onclick: move |_| {
                                        let skill = selected_skill.read().clone();
                                        let config = config_values.read().clone();
                                        activate_skill(skill, config);
                                    },
                                    "🚀 Activate"
                                }
                            }
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
