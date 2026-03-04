//! MAGIC page — quantum entropy, mantra library, auth, and configuration.

#![allow(
    clippy::option_if_let_else,
    clippy::nonminimal_bool,
    clippy::collapsible_else_if,
    clippy::if_not_else
)]

use crate::api;
use crate::components::{Toast, ToastMessage, ToastType};
use crate::theme::Theme;
use carnelian_common::types::{
    EntropyLogEntry, EntropySampleResponse, MagicAuthStatusResponse, MagicConfigResponse,
    MantraCategory, MantraEntryDetail, MantraHistoryRecord, MantraSimulateResponse,
};
use dioxus::prelude::*;
use uuid::Uuid;

#[component]
pub fn Magic() -> Element {
    let theme = use_context::<Theme>();
    let theme_class = theme.to_class();
    let mut active_tab = use_signal(|| "entropy".to_string());
    let toasts = use_signal(Vec::<ToastMessage>::new);

    rsx! {
        div { class: "page {theme_class}",
            div { class: "page-header",
                h1 { "✨ MAGIC" }
                p { class: "subtitle", "Mixed Authenticated Quantum Intelligence Core" }
            }

            div { class: "tab-bar",
                button {
                    class: if *active_tab.read() == "entropy" { "tab active" } else { "tab" },
                    onclick: move |_| active_tab.set("entropy".to_string()),
                    "Entropy"
                }
                button {
                    class: if *active_tab.read() == "mantras" { "tab active" } else { "tab" },
                    onclick: move |_| active_tab.set("mantras".to_string()),
                    "Mantras"
                }
                button {
                    class: if *active_tab.read() == "quantum" { "tab active" } else { "tab" },
                    onclick: move |_| active_tab.set("quantum".to_string()),
                    "Quantum Jobs"
                }
                button {
                    class: if *active_tab.read() == "elixir" { "tab active" } else { "tab" },
                    onclick: move |_| active_tab.set("elixir".to_string()),
                    "Elixir Integration"
                }
                button {
                    class: if *active_tab.read() == "auth" { "tab active" } else { "tab" },
                    onclick: move |_| active_tab.set("auth".to_string()),
                    "Auth & Settings"
                }
            }

            div { class: "tab-content",
                {
                    match active_tab.read().as_str() {
                        "entropy" => rsx! { EntropyDashboard { toasts } },
                        "mantras" => rsx! { MantraLibrary { toasts } },
                        "quantum" => rsx! { QuantumJobs { toasts } },
                        "elixir" => rsx! { ElixirSkillIntegration { toasts } },
                        "auth" => rsx! { AuthSettings { toasts } },
                        _ => rsx! { div { "Unknown tab" } },
                    }
                }
            }

            for toast in toasts.read().iter() {
                Toast { toast: toast.clone() }
            }
        }
    }
}

#[component]
fn EntropyDashboard(toasts: Signal<Vec<ToastMessage>>) -> Element {
    let mut health = use_signal(|| None::<serde_json::Value>);
    let mut sample = use_signal(|| None::<EntropySampleResponse>);
    let mut log_entries = use_signal(Vec::<EntropyLogEntry>::new);
    let mut loading = use_signal(|| false);
    let mut sample_bytes = use_signal(|| "32".to_string());

    use_hook(|| {
        spawn(async move {
            if let Ok(h) = api::magic_entropy_health().await {
                health.set(Some(h));
            }
            if let Ok(log) = api::magic_entropy_log(20).await {
                log_entries.set(log.entries);
            }
        });
    });

    let sample_entropy = move || {
        spawn(async move {
            loading.set(true);
            let bytes = sample_bytes.read().parse::<usize>().unwrap_or(32);
            match api::magic_entropy_sample(bytes, None).await {
                Ok(resp) => {
                    sample.set(Some(resp));
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to sample entropy: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
            loading.set(false);
        });
    };

    rsx! {
        div { class: "entropy-dashboard",
            h2 { "Entropy Provider Status" }
            div { class: "provider-status-row",
                {
                    if let Some(h) = health.read().as_ref() {
                        rsx! {
                            div { class: "status-card",
                                h3 { "Quantum Origin" }
                                p {
                                    {
                                        if let Some(qo) = h.get("quantum-origin") {
                                            if qo.get("available").and_then(serde_json::Value::as_bool).unwrap_or(false) {
                                                "✅ Available"
                                            } else {
                                                "❌ Unavailable"
                                            }
                                        } else {
                                            "⚠️ Not Configured"
                                        }
                                    }
                                }
                            }
                            div { class: "status-card",
                                h3 { "Quantinuum" }
                                p {
                                    {
                                        if let Some(qq) = h.get("quantinuum-h2") {
                                            if qq.get("available").and_then(serde_json::Value::as_bool).unwrap_or(false) {
                                                "✅ Available"
                                            } else {
                                                "❌ Unavailable"
                                            }
                                        } else {
                                            "⚠️ Not Configured"
                                        }
                                    }
                                }
                            }
                            div { class: "status-card",
                                h3 { "Qiskit" }
                                p {
                                    {
                                        if let Some(qk) = h.get("qiskit-rng") {
                                            if qk.get("available").and_then(serde_json::Value::as_bool).unwrap_or(false) {
                                                "✅ Available"
                                            } else {
                                                "❌ Unavailable"
                                            }
                                        } else {
                                            "⚠️ Not Configured"
                                        }
                                    }
                                }
                            }
                            div { class: "status-card",
                                h3 { "OS Random" }
                                p {
                                    {
                                        if let Some(os) = h.get("os") {
                                            if os.get("available").and_then(serde_json::Value::as_bool).unwrap_or(false) {
                                                "✅ Available"
                                            } else {
                                                "❌ Unavailable"
                                            }
                                        } else {
                                            "⚠️ Unknown"
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        rsx! { p { "Loading provider status..." } }
                    }
                }
            }

            h2 { "Sample Entropy" }
            div { class: "sample-panel",
                input {
                    r#type: "number",
                    value: "{sample_bytes}",
                    oninput: move |e| sample_bytes.set(e.value()),
                    placeholder: "Bytes"
                }
                button {
                    onclick: move |_| sample_entropy(),
                    disabled: *loading.read(),
                    "Sample Entropy"
                }
                {
                    if let Some(s) = sample.read().as_ref() {
                        rsx! {
                            div { class: "sample-result",
                                p { strong { "Hex: " } code { "{s.hex}" } }
                                p { strong { "Source: " } "{s.source}" }
                                p { strong { "Bytes: " } "{s.bytes}" }
                            }
                        }
                    } else {
                        rsx! { div {} }
                    }
                }
            }

            h2 { "Entropy Log" }
            table { class: "entropy-log-table",
                thead {
                    tr {
                        th { "Timestamp" }
                        th { "Source" }
                        th { "Bytes" }
                        th { "Quantum?" }
                        th { "Latency (ms)" }
                    }
                }
                tbody {
                    for entry in log_entries.read().iter() {
                        tr {
                            td { "{entry.ts}" }
                            td { "{entry.source}" }
                            td { "{entry.bytes_requested}" }
                            td { if entry.quantum_available { "✅" } else { "❌" } }
                            td {
                                {
                                    if let Some(lat) = entry.latency_ms {
                                        format!("{lat}")
                                    } else {
                                        "—".to_string()
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn MantraLibrary(toasts: Signal<Vec<ToastMessage>>) -> Element {
    let mut categories = use_signal(Vec::<MantraCategory>::new);
    let mut selected_category = use_signal(|| None::<MantraCategory>);
    let mut entries = use_signal(Vec::<MantraEntryDetail>::new);
    let mut loading_cats = use_signal(|| false);
    let mut loading_entries = use_signal(|| false);
    let mut simulate_result = use_signal(|| None::<MantraSimulateResponse>);
    let mut new_text = use_signal(String::new);
    let mut new_elixir_id = use_signal(String::new);
    let mut history = use_signal(Vec::<MantraHistoryRecord>::new);
    let mut show_history = use_signal(|| false);
    let mut context_data = use_signal(|| None::<serde_json::Value>);
    let mut loading_context = use_signal(|| false);

    let load_categories = move || {
        spawn(async move {
            loading_cats.set(true);
            match api::magic_mantras_list().await {
                Ok(resp) => {
                    categories.set(resp.categories);
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to load categories: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
            loading_cats.set(false);
        });
    };

    let load_entries = move |category_id: Uuid| {
        spawn(async move {
            loading_entries.set(true);
            match api::magic_mantras_by_category(category_id).await {
                Ok(resp) => {
                    entries.set(resp.entries);
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to load entries: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
            loading_entries.set(false);
        });
    };

    use_hook(|| {
        load_categories();
    });

    let toggle_entry = move |entry_id: Uuid, enabled: bool| {
        spawn(async move {
            match api::magic_mantra_update(entry_id, None, Some(!enabled), None).await {
                Ok(_) => {
                    if let Some(cat) = selected_category.read().as_ref() {
                        load_entries(cat.category_id);
                    }
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: "Entry updated".to_string(),
                        toast_type: ToastType::Success,
                        duration_secs: 3,
                    });
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to update entry: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
        });
    };

    let add_entry = move || {
        spawn(async move {
            if let Some(cat) = selected_category.read().as_ref() {
                let text = new_text.read().clone();
                let elixir_id = if new_elixir_id.read().is_empty() {
                    None
                } else {
                    Uuid::parse_str(&new_elixir_id.read()).ok()
                };
                match api::magic_mantra_add(cat.category_id, text, elixir_id).await {
                    Ok(_) => {
                        load_entries(cat.category_id);
                        new_text.set(String::new());
                        new_elixir_id.set(String::new());
                        toasts.write().push(ToastMessage {
                            id: Uuid::new_v4().to_string(),
                            message: "Mantra added".to_string(),
                            toast_type: ToastType::Success,
                            duration_secs: 3,
                        });
                    }
                    Err(e) => {
                        toasts.write().push(ToastMessage {
                            id: Uuid::new_v4().to_string(),
                            message: format!("Failed to add mantra: {e}"),
                            toast_type: ToastType::Error,
                            duration_secs: 5,
                        });
                    }
                }
            }
        });
    };

    let load_history = move || {
        spawn(async move {
            match api::magic_mantra_history().await {
                Ok(resp) => {
                    history.set(resp.history);
                    show_history.set(true);
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to load history: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
        });
    };

    let load_context = move || {
        spawn(async move {
            loading_context.set(true);
            match api::magic_mantra_context().await {
                Ok(ctx) => {
                    context_data.set(Some(ctx));
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to load context: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
            loading_context.set(false);
        });
    };

    let simulate_mantra = move || {
        spawn(async move {
            match api::magic_mantra_simulate().await {
                Ok(resp) => {
                    simulate_result.set(Some(resp));
                    // Refresh context after successful simulation
                    load_context();
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to simulate: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
        });
    };

    // Auto-load context on component mount
    use_effect(move || {
        load_context();
    });

    rsx! {
        div { class: "mantra-library",
            h2 { "Mantra Categories" }
            div { class: "category-pills",
                for cat in categories.read().iter() {
                    button {
                        key: "{cat.category_id}",
                        class: "pill",
                        style: if !cat.enabled { "opacity: 0.4;" } else { "" },
                        onclick: {
                            let cat_clone = cat.clone();
                            let cat_id = cat.category_id;
                            move |_| {
                                selected_category.set(Some(cat_clone.clone()));
                                load_entries(cat_id);
                            }
                        },
                        "{cat.name} "
                        span { class: "badge", "{cat.entry_count}" }
                    }
                }
            }

            {
                if let Some(cat) = selected_category.read().as_ref() {
                    rsx! {
                        div { class: "entries-panel",
                            h3 { "Entries for {cat.name}" }
                            div { class: "entries-list",
                                for entry in entries.read().iter() {
                                    div {
                                        key: "{entry.entry_id}",
                                        class: "entry-row",
                                        p { "{entry.text}" }
                                        span { class: "use-count", "Used: {entry.use_count}" }
                                        button {
                                            onclick: {
                                                let entry_id = entry.entry_id;
                                                let entry_enabled = entry.enabled;
                                                move |_| toggle_entry(entry_id, entry_enabled)
                                            },
                                            if entry.enabled { "Disable" } else { "Enable" }
                                        }
                                        if entry.elixir_id.is_some() {
                                            span { class: "badge", "🔗 Elixir" }
                                        }
                                    }
                                }
                            }

                            h3 { "Add New Entry" }
                            div { class: "add-entry-form",
                                textarea {
                                    value: "{new_text}",
                                    oninput: move |e| new_text.set(e.value()),
                                    placeholder: "Mantra text"
                                }
                                input {
                                    r#type: "text",
                                    value: "{new_elixir_id}",
                                    oninput: move |e| new_elixir_id.set(e.value()),
                                    placeholder: "Elixir UUID (optional)"
                                }
                                button {
                                    onclick: move |_| add_entry(),
                                    "Add Mantra"
                                }
                            }
                        }
                    }
                } else {
                    rsx! { p { "Select a category to view entries" } }
                }
            }

            h2 { "History & Simulation" }
            div { class: "history-simulate",
                button {
                    onclick: move |_| load_history(),
                    "Show History"
                }
                button {
                    onclick: move |_| simulate_mantra(),
                    "Simulate Selection"
                }
                button {
                    onclick: move |_| load_context(),
                    "Refresh Context"
                }
            }

            {
                if *show_history.read() {
                    rsx! {
                        table { class: "history-table",
                            thead {
                                tr {
                                    th { "Timestamp" }
                                    th { "Category" }
                                    th { "Entropy Source" }
                                    th { "Elixir?" }
                                }
                            }
                            tbody {
                                for rec in history.read().iter() {
                                    tr {
                                        td { "{rec.ts}" }
                                        td { "{rec.category_id}" }
                                        td { "{rec.entropy_source}" }
                                        td { if rec.elixir_reference.is_some() { "✅" } else { "—" } }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    rsx! { div {} }
                }
            }

            {
                if let Some(sim) = simulate_result.read().as_ref() {
                    rsx! {
                        div { class: "simulate-result",
                            h3 { "Simulation Result" }
                            p { strong { "Category: " } "{sim.category}" }
                            p { strong { "Mantra: " } "{sim.mantra_text}" }
                            p { strong { "System Message: " } "{sim.system_message}" }
                            p { strong { "User Message: " } "{sim.user_message}" }
                            p { strong { "Entropy Source: " } "{sim.entropy_source}" }
                            p { strong { "Context Weights: " } code { "{sim.context_weights:?}" } }
                        }
                    }
                } else {
                    rsx! { div {} }
                }
            }

            {
                if let Some(ctx) = context_data.read().as_ref() {
                    rsx! {
                        div { class: "context-weights-panel",
                            h3 { "Context Weights" }
                            p { strong { "Pending Tasks: " } "{ctx.get(\"pending_task_count\").and_then(|v| v.as_i64()).unwrap_or(0)}" }
                            p { strong { "Recent Errors: " } "{ctx.get(\"recent_error_count\").and_then(|v| v.as_i64()).unwrap_or(0)}" }
                            p { strong { "Idle Beats: " } "{ctx.get(\"idle_beats\").and_then(|v| v.as_i64()).unwrap_or(0)}" }
                            p { strong { "Active Sessions: " } "{ctx.get(\"active_sessions\").and_then(|v| v.as_i64()).unwrap_or(0)}" }
                        }
                    }
                } else {
                    rsx! { div {} }
                }
            }
        }
    }
}

#[component]
fn QuantumJobs(toasts: Signal<Vec<ToastMessage>>) -> Element {
    let mut running = use_signal(|| false);
    let mut last_result = use_signal(|| None::<EntropySampleResponse>);

    let run_quantum_sample = move || {
        spawn(async move {
            running.set(true);
            match api::magic_entropy_sample(64, None).await {
                Ok(resp) => {
                    let source = resp.source.clone();
                    last_result.set(Some(resp));
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Entropy sample completed (source: {source})"),
                        toast_type: ToastType::Success,
                        duration_secs: 3,
                    });
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Entropy sample failed: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
            running.set(false);
        });
    };

    let rehash_elixirs = move || {
        spawn(async move {
            match api::magic_elixirs_rehash().await {
                Ok(resp) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Rehashed {} elixirs", resp.rehashed),
                        toast_type: ToastType::Success,
                        duration_secs: 5,
                    });
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Rehash failed: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
        });
    };

    rsx! {
        div { class: "quantum-jobs",
            h2 { "Quantum Circuit Jobs" }
            p { "Run quantum random number generation on Quantinuum H2 or Qiskit backends." }

            div { class: "job-actions",
                button {
                    onclick: move |_| run_quantum_sample(),
                    disabled: *running.read(),
                    "Request Entropy Sample (Quantum-First)"
                }
                button {
                    onclick: move |_| rehash_elixirs(),
                    "Rehash Elixirs with Fresh Entropy"
                }
            }

            {
                if let Some(result) = last_result.read().as_ref() {
                    rsx! {
                        div { class: "result-display",
                            h3 { "Last Result" }
                            pre { code { "{result.hex}" } }
                            p { "Source: {result.source}" }
                        }
                    }
                } else {
                    rsx! { div {} }
                }
            }
        }
    }
}

#[component]
fn ElixirSkillIntegration(toasts: Signal<Vec<ToastMessage>>) -> Element {
    let mut categories = use_signal(Vec::<MantraCategory>::new);
    let mut loading = use_signal(|| false);

    use_hook(|| {
        spawn(async move {
            loading.set(true);
            match api::magic_mantras_list().await {
                Ok(resp) => {
                    categories.set(resp.categories);
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Failed to load categories: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
            loading.set(false);
        });
    });

    let rehash_all = move || {
        spawn(async move {
            match api::magic_elixirs_rehash().await {
                Ok(resp) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Rehashed {} elixirs", resp.rehashed),
                        toast_type: ToastType::Success,
                        duration_secs: 5,
                    });
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Rehash failed: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
        });
    };

    rsx! {
        div { class: "elixir-integration",
            h2 { "Elixir & Skill Integration" }
            p { "MAGIC selects mantras that can reference elixir skills, providing quantum-enhanced guidance for agent operations." }

            button {
                onclick: move |_| rehash_all(),
                "Rehash All Elixirs"
            }

            h3 { "Mantra Categories" }
            div { class: "category-grid",
                for cat in categories.read().iter() {
                    div { class: "category-card",
                        h4 { "{cat.name}" }
                        p { "Base Weight: {cat.base_weight}" }
                        p { "Cooldown: {cat.cooldown_beats} beats" }
                        p { "Entries: {cat.entry_count}" }
                        span {
                            class: "badge",
                            if cat.enabled { "✅ Enabled" } else { "❌ Disabled" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AuthSettings(toasts: Signal<Vec<ToastMessage>>) -> Element {
    let mut auth_status = use_signal(|| None::<MagicAuthStatusResponse>);
    let mut config = use_signal(|| None::<MagicConfigResponse>);
    let mut loading = use_signal(|| false);
    let mut qq_email = use_signal(String::new);
    let mut qq_password = use_signal(String::new);
    let mut origin_key = use_signal(String::new);
    let mut qiskit_on = use_signal(|| false);
    let mut quantinuum_on = use_signal(|| false);

    use_hook(|| {
        spawn(async move {
            loading.set(true);
            if let Ok(status) = api::magic_auth_status().await {
                auth_status.set(Some(status));
            }
            if let Ok(cfg) = api::magic_get_config().await {
                qiskit_on.set(cfg.qiskit_enabled);
                quantinuum_on.set(cfg.quantinuum_enabled);
                config.set(Some(cfg));
            }
            loading.set(false);
        });
    });

    let login_quantinuum = move || {
        spawn(async move {
            let email = qq_email.read().clone();
            let password = qq_password.read().clone();
            match api::magic_quantinuum_login(email, password).await {
                Ok(resp) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Login successful: {}", resp.message),
                        toast_type: ToastType::Success,
                        duration_secs: 5,
                    });
                    qq_password.set(String::new());
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Login failed: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
        });
    };

    let refresh_token = move || {
        spawn(async move {
            match api::magic_quantinuum_refresh().await {
                Ok(resp) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Token refreshed: {}", resp.message),
                        toast_type: ToastType::Success,
                        duration_secs: 5,
                    });
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Refresh failed: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
        });
    };

    let update_config = move || {
        spawn(async move {
            let key = if origin_key.read().is_empty() {
                None
            } else {
                Some(origin_key.read().clone())
            };
            match api::magic_update_config(
                key,
                Some(*quantinuum_on.read()),
                Some(*qiskit_on.read()),
            )
            .await
            {
                Ok(_) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: "Config updated".to_string(),
                        toast_type: ToastType::Success,
                        duration_secs: 3,
                    });
                    if let Ok(cfg) = api::magic_get_config().await {
                        config.set(Some(cfg));
                    }
                }
                Err(e) => {
                    toasts.write().push(ToastMessage {
                        id: Uuid::new_v4().to_string(),
                        message: format!("Update failed: {e}"),
                        toast_type: ToastType::Error,
                        duration_secs: 5,
                    });
                }
            }
        });
    };

    rsx! {
        div { class: "auth-settings",
            h2 { "Authentication Status" }
            {
                if let Some(status) = auth_status.read().as_ref() {
                    rsx! {
                        div { class: "auth-status-row",
                            div { class: "status-badge",
                                strong { "Quantinuum: " }
                                if status.quantinuum.authenticated {
                                    "✅ Authenticated"
                                } else {
                                    "❌ Not Authenticated"
                                }
                                {
                                    if let Some(exp) = &status.quantinuum.expiry {
                                        rsx! { p { "Expires: {exp}" } }
                                    } else {
                                        rsx! { p {} }
                                    }
                                }
                            }
                            div { class: "status-badge",
                                strong { "Quantum Origin: " }
                                if status.quantum_origin.configured {
                                    "✅ Configured"
                                } else {
                                    "⚪ Not Configured"
                                }
                            }
                        }
                    }
                } else {
                    rsx! { p { "Loading auth status..." } }
                }
            }

            h2 { "Quantinuum Login" }
            div { class: "login-form",
                input {
                    r#type: "email",
                    value: "{qq_email}",
                    oninput: move |e| qq_email.set(e.value()),
                    placeholder: "Email"
                }
                input {
                    r#type: "password",
                    value: "{qq_password}",
                    oninput: move |e| qq_password.set(e.value()),
                    placeholder: "Password"
                }
                button {
                    onclick: move |_| login_quantinuum(),
                    "Authenticate"
                }
                button {
                    onclick: move |_| refresh_token(),
                    "Refresh Token"
                }
            }

            h2 { "Configuration" }
            {
                if let Some(cfg) = config.read().as_ref() {
                    rsx! {
                        div { class: "config-panel",
                            p { strong { "Quantum Origin URL: " } "{cfg.quantum_origin_url}" }
                            p { strong { "Quantinuum Device: " } "{cfg.quantinuum_device}" }
                            p { strong { "Qiskit Backend: " } "{cfg.qiskit_backend}" }
                            p { strong { "Entropy Mix Ratio: " } "{cfg.entropy_mix_ratio}" }
                            p { strong { "Mantra Cooldown: " } "{cfg.mantra_cooldown_beats} beats" }

                            h3 { "Update Config" }
                            input {
                                r#type: "text",
                                value: "{origin_key}",
                                oninput: move |e| origin_key.set(e.value()),
                                placeholder: "Quantum Origin API Key"
                            }
                            label {
                                input {
                                    r#type: "checkbox",
                                    checked: *qiskit_on.read(),
                                    onchange: move |e| qiskit_on.set(e.checked()),
                                }
                                " Enable Qiskit"
                            }
                            label {
                                input {
                                    r#type: "checkbox",
                                    checked: *quantinuum_on.read(),
                                    onchange: move |e| quantinuum_on.set(e.checked()),
                                }
                                " Enable Quantinuum"
                            }
                            button {
                                onclick: move |_| update_config(),
                                "Update Config"
                            }
                        }
                    }
                } else {
                    rsx! { p { "Loading config..." } }
                }
            }
        }
    }
}
