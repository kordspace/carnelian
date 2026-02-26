//! Channel management page with table view, wizard modal, edit modal,
//! confirmation dialogs, and pairing status indicators.

use carnelian_common::types::{
    ChannelDetail, CreateChannelApiRequest, EventType, UpdateChannelApiRequest,
};
use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use serde_json::json;
use uuid::Uuid;

use crate::store::EventStreamStore;

// ── Main Page ───────────────────────────────────────────────

/// Channel management page.
#[component]
pub fn Channels() -> Element {
    let store = use_context::<EventStreamStore>();

    let mut refresh = use_signal(|| 0_u64);
    let mut filter_type = use_signal(|| "All".to_string());
    let mut filter_search = use_signal(String::new);
    let mut show_wizard = use_signal(|| false);
    let mut edit_channel = use_signal(|| Option::<ChannelDetail>::None);
    let mut confirm_delete = use_signal(|| Option::<ChannelDetail>::None);

    let channels_resource = use_resource(move || async move {
        let _ = refresh();
        let type_filter = {
            let v = filter_type.read().clone();
            if v == "All" { None } else { Some(v) }
        };
        crate::api::list_channels(type_filter)
            .await
            .map(|r| r.channels)
            .unwrap_or_default()
    });

    // Auto-refresh every 10 seconds.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            refresh += 1;
        }
    });

    // Trigger refresh on channel WebSocket events.
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            if let EventType::Custom(ref name) = last.event_type {
                if name == "ChannelCreated"
                    || name == "ChannelUpdated"
                    || name == "ChannelDeleted"
                    || name == "ChannelPaired"
                {
                    refresh += 1;
                }
            }
        }
    });

    let channels_read = channels_resource.read();
    let all_channels: Vec<ChannelDetail> = (*channels_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);

    let search_lower = filter_search.read().to_lowercase();
    let filtered: Vec<&ChannelDetail> = all_channels
        .iter()
        .filter(|c| {
            if search_lower.is_empty() {
                return true;
            }
            c.channel_user_id.to_lowercase().contains(&search_lower)
                || c.session_id.to_string().contains(&search_lower)
                || c.channel_type.to_lowercase().contains(&search_lower)
        })
        .collect();

    rsx! {
        div { class: "page-panel panel-page",
            // ── Filter bar ──────────────────────────────────
            div { class: "filter-bar",
                select {
                    class: "filter-select",
                    aria_label: "Filter by channel type",
                    value: "{filter_type}",
                    onchange: move |e| filter_type.set(e.value()),
                    option { value: "All", "All Types" }
                    option { value: "telegram", "\u{1F4F1} Telegram" }
                    option { value: "discord", "\u{1F4AC} Discord" }
                    option { value: "whatsapp", "\u{1F4DE} Whatsapp" }
                    option { value: "slack", "\u{1F4BC} Slack" }
                    option { value: "ui", "\u{1F5A5}\u{FE0F} UI" }
                }
                input {
                    class: "filter-input",
                    r#type: "text",
                    placeholder: "Search channels\u{2026}",
                    aria_label: "Search channels",
                    value: "{filter_search}",
                    oninput: move |e| filter_search.set(e.value()),
                }
                div { class: "filter-bar-actions",
                    button {
                        class: "btn-secondary btn-sm",
                        onclick: move |_| { refresh += 1; },
                        "\u{21BB} Refresh"
                    }
                    button {
                        class: "btn-primary btn-sm",
                        onclick: move |_| show_wizard.set(true),
                        "+ Add Channel"
                    }
                }
            }

            // ── Table ───────────────────────────────────────
            if channels_read.is_none() {
                div { class: "state-message",
                    div { class: "spinner" }
                    span { "Loading channels\u{2026}" }
                }
            } else if filtered.is_empty() {
                div { class: "state-message",
                    span { class: "state-icon", "\u{1F4E1}" }
                    span { "No channels match the current filters." }
                }
            } else {
                div { class: "panel-scroll",
                    table { class: "data-table",
                        thead {
                            tr {
                                th { "Name" }
                                th { "Type" }
                                th { "Status" }
                                th { "Trust Level" }
                                th { "Pairing" }
                                th { "Last Message" }
                                th { "Actions" }
                            }
                        }
                        tbody {
                            for channel in filtered {
                                { render_channel_row(channel, &mut edit_channel, &mut confirm_delete, &mut refresh) }
                            }
                        }
                    }
                }
            }

            // ── Wizard Modal ────────────────────────────────
            if *show_wizard.read() {
                ChannelWizardModal {
                    on_close: move || show_wizard.set(false),
                    on_created: move || {
                        show_wizard.set(false);
                        refresh += 1;
                    },
                }
            }

            // ── Edit Modal ──────────────────────────────────
            if edit_channel.read().is_some() {
                EditChannelModal {
                    channel: edit_channel.read().clone().unwrap(),
                    on_close: move || edit_channel.set(None),
                    on_saved: move || {
                        edit_channel.set(None);
                        refresh += 1;
                    },
                }
            }

            // ── Delete Confirmation ─────────────────────────
            if confirm_delete.read().is_some() {
                ConfirmDeleteDialog {
                    channel: confirm_delete.read().clone().unwrap(),
                    on_close: move || confirm_delete.set(None),
                    on_confirmed: move || {
                        confirm_delete.set(None);
                        refresh += 1;
                    },
                }
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────

fn channel_type_icon(ct: &str) -> &'static str {
    match ct {
        "telegram" => "\u{1F4F1}",
        "discord" => "\u{1F4AC}",
        "whatsapp" => "\u{1F4DE}",
        "slack" => "\u{1F4BC}",
        "ui" => "\u{1F5A5}\u{FE0F}",
        _ => "\u{1F4E1}",
    }
}

fn trust_level_badge_class(trust: &str) -> &'static str {
    match trust {
        "owner" => "badge-status badge-owner",
        "conversational" => "badge-status badge-running",
        "untrusted" => "badge-status badge-cancelled",
        _ => "badge-status",
    }
}

fn pairing_status_text(metadata: &serde_json::Value) -> &'static str {
    match metadata.get("pairing_status").and_then(|v| v.as_str()) {
        Some("confirmed") => "Paired",
        Some("pending") => "Pending",
        _ => "Not Paired",
    }
}

fn pairing_status_class(metadata: &serde_json::Value) -> &'static str {
    match metadata.get("pairing_status").and_then(|v| v.as_str()) {
        Some("confirmed") => "badge-status badge-completed",
        Some("pending") => "badge-status badge-pending",
        _ => "badge-status badge-cancelled",
    }
}

fn format_relative_time(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);
    if diff.num_seconds() < 60 {
        "just now".to_string()
    } else if diff.num_minutes() < 60 {
        let m = diff.num_minutes();
        format!("{m} min{} ago", if m == 1 { "" } else { "s" })
    } else if diff.num_hours() < 24 {
        let h = diff.num_hours();
        format!("{h} hour{} ago", if h == 1 { "" } else { "s" })
    } else {
        let d = diff.num_days();
        format!("{d} day{} ago", if d == 1 { "" } else { "s" })
    }
}

fn render_channel_row(
    channel: &ChannelDetail,
    edit_channel: &mut Signal<Option<ChannelDetail>>,
    confirm_delete: &mut Signal<Option<ChannelDetail>>,
    refresh: &mut Signal<u64>,
) -> Element {
    let icon = channel_type_icon(&channel.channel_type);
    let trust_badge = trust_level_badge_class(&channel.trust_level);
    let pairing_text = pairing_status_text(&channel.metadata);
    let pairing_class = pairing_status_class(&channel.metadata);
    let status_text = if channel.adapter_running {
        "Running"
    } else {
        "Stopped"
    };
    let status_class = if channel.adapter_running {
        "badge-status badge-running"
    } else {
        "badge-status badge-cancelled"
    };
    let status_dot = if channel.adapter_running {
        "\u{1F7E2}"
    } else {
        "\u{26AB}"
    };

    // Comment 4: Show last message text + relative time, falling back to last_seen_at
    let last_message_display =
        if let (Some(text), Some(at)) = (&channel.last_message_text, channel.last_message_at) {
            let truncated = if text.len() > 30 {
                format!("{}\u{2026}", &text[..30])
            } else {
                text.clone()
            };
            format!("{} ({})", truncated, format_relative_time(at))
        } else {
            format_relative_time(channel.last_seen_at)
        };

    let name_display = if channel.channel_user_id.len() > 24 {
        format!("{}\u{2026}", &channel.channel_user_id[..24])
    } else {
        channel.channel_user_id.clone()
    };

    let ch_edit = channel.clone();
    let ch_delete = channel.clone();
    let mut edit_sig = *edit_channel;
    let mut delete_sig = *confirm_delete;

    // Comment 3: Disable/enable toggle
    let is_enabled = channel.enabled;
    let toggle_label = if is_enabled { "Disable" } else { "Enable" };
    let toggle_class = if is_enabled {
        "btn-secondary btn-sm"
    } else {
        "btn-primary btn-sm"
    };
    let session_id = channel.session_id;
    let mut refresh_sig = *refresh;

    rsx! {
        tr {
            td { class: "cell-mono", title: "{channel.channel_user_id}", "{name_display}" }
            td { "{icon} {channel.channel_type}" }
            td { span { class: "{status_class}", "{status_dot} {status_text}" } }
            td { span { class: "{trust_badge}", "{channel.trust_level}" } }
            td { span { class: "{pairing_class}", "{pairing_text}" } }
            td { "{last_message_display}" }
            td {
                button {
                    class: "btn-secondary btn-sm",
                    onclick: move |_| {
                        edit_sig.set(Some(ch_edit.clone()));
                    },
                    "Edit"
                }
                button {
                    class: "{toggle_class}",
                    onclick: move |_| {
                        let new_enabled = !is_enabled;
                        spawn(async move {
                            let req = UpdateChannelApiRequest {
                                trust_level: None,
                                bot_token: None,
                                metadata: None,
                                enabled: Some(new_enabled),
                            };
                            if crate::api::update_channel(session_id, req).await.is_ok() {
                                refresh_sig += 1;
                            }
                        });
                    },
                    "{toggle_label}"
                }
                button {
                    class: "btn-danger btn-sm",
                    onclick: move |_| {
                        delete_sig.set(Some(ch_delete.clone()));
                    },
                    "Delete"
                }
            }
        }
    }
}

// ── Channel Wizard Modal (Steps 1-5) ───────────────────────

#[component]
fn ChannelWizardModal(on_close: EventHandler, on_created: EventHandler) -> Element {
    let mut current_step = use_signal(|| 1_u8);
    let channel_type = use_signal(|| "telegram".to_string());
    let mut channel_user_id = use_signal(String::new);
    let mut bot_token = use_signal(String::new);
    let mut show_token = use_signal(|| false);
    let trust_level = use_signal(|| "conversational".to_string());
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut submitting = use_signal(|| false);
    let mut created_session_id = use_signal(|| Option::<Uuid>::None);
    let mut test_status = use_signal(|| Option::<String>::None);
    let mut pairing_token = use_signal(|| Option::<String>::None);
    let mut pairing_expires = use_signal(|| Option::<String>::None);

    // Additional signals for WhatsApp and Slack credentials
    let mut whatsapp_phone_number_id = use_signal(String::new);
    let mut whatsapp_verify_token = use_signal(String::new);
    let mut slack_signing_secret = use_signal(String::new);

    let step = *current_step.read();

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Add Channel",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                style: "max-width: 560px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Add Channel \u{2014} Step {step} of 5" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    // Step 1: Select Channel Type
                    if step == 1 {
                        div { class: "form-group",
                            label { class: "form-label", "Channel Type" }
                            div { style: "display: flex; flex-direction: column; gap: 8px;",
                                { wizard_type_option("telegram", "\u{1F4F1} Telegram", "Connect a Telegram bot to receive and send messages.", &channel_type) }
                                { wizard_type_option("discord", "\u{1F4AC} Discord", "Connect a Discord bot to a server channel.", &channel_type) }
                                { wizard_type_option("whatsapp", "\u{1F4DE} Whatsapp", "Connect via WhatsApp Business API.", &channel_type) }
                                { wizard_type_option("slack", "\u{1F4BC} Slack", "Connect a Slack bot to a workspace.", &channel_type) }
                            }
                        }
                    }

                    // Step 2: Enter Credentials
                    if step == 2 {
                        div { class: "form-group",
                            label { class: "form-label", "Channel User ID" }
                            input {
                                class: "form-input",
                                r#type: "text",
                                placeholder: match channel_type.read().as_str() {
                                    "telegram" => "Telegram chat ID (e.g. -1001234567890)",
                                    "discord" => "Discord channel ID (e.g. 1234567890)",
                                    "whatsapp" => "Phone number (e.g. +1234567890)",
                                    "slack" => "Slack channel ID (e.g. C01234ABCDE)",
                                    _ => "Channel identifier",
                                },
                                value: "{channel_user_id}",
                                oninput: move |e| channel_user_id.set(e.value()),
                            }
                        }
                        // Channel-type-conditional credential fields
                        {match channel_type.read().as_str() {
                            "whatsapp" => rsx! {
                                div { class: "form-group",
                                    label { class: "form-label", "Phone Number ID" }
                                    input {
                                        class: "form-input",
                                        r#type: "text",
                                        placeholder: "Meta Phone Number ID (e.g. 123456789012345)",
                                        value: "{whatsapp_phone_number_id}",
                                        oninput: move |e| whatsapp_phone_number_id.set(e.value()),
                                    }
                                }
                                div { class: "form-group",
                                    label { class: "form-label", "Access Token" }
                                    div { style: "display: flex; gap: 8px; align-items: center;",
                                        input {
                                            class: "form-input",
                                            r#type: if *show_token.read() { "text" } else { "password" },
                                            placeholder: "Paste your WhatsApp access token here",
                                            value: "{bot_token}",
                                            oninput: move |e| bot_token.set(e.value()),
                                            style: "flex: 1;",
                                        }
                                        button {
                                            class: "btn-secondary btn-sm",
                                            onclick: move |_| {
                                                let v = *show_token.read();
                                                show_token.set(!v);
                                            },
                                            if *show_token.read() { "Hide" } else { "Show" }
                                        }
                                    }
                                    p { style: "color: #8E8E93; font-size: 12px; margin-top: 4px;",
                                        "\u{1F512} Bot tokens are encrypted at rest. Never share your tokens."
                                    }
                                }
                                div { class: "form-group",
                                    label { class: "form-label", "Verify Token" }
                                    input {
                                        class: "form-input",
                                        r#type: "text",
                                        placeholder: "Webhook verify token (for Meta verification)",
                                        value: "{whatsapp_verify_token}",
                                        oninput: move |e| whatsapp_verify_token.set(e.value()),
                                    }
                                    p { style: "color: #8E8E93; font-size: 12px; margin-top: 4px;",
                                        "\u{1F512} Bot tokens are encrypted at rest. Never share your tokens."
                                    }
                                }
                            },
                            "slack" => rsx! {
                                div { class: "form-group",
                                    label { class: "form-label", "Bot Token" }
                                    div { style: "display: flex; gap: 8px; align-items: center;",
                                        input {
                                            class: "form-input",
                                            r#type: if *show_token.read() { "text" } else { "password" },
                                            placeholder: "Paste your Slack bot token here (xoxb-...)",
                                            value: "{bot_token}",
                                            oninput: move |e| bot_token.set(e.value()),
                                            style: "flex: 1;",
                                        }
                                        button {
                                            class: "btn-secondary btn-sm",
                                            onclick: move |_| {
                                                let v = *show_token.read();
                                                show_token.set(!v);
                                            },
                                            if *show_token.read() { "Hide" } else { "Show" }
                                        }
                                    }
                                    p { style: "color: #8E8E93; font-size: 12px; margin-top: 4px;",
                                        "\u{1F512} Bot tokens are encrypted at rest. Never share your tokens."
                                    }
                                }
                                div { class: "form-group",
                                    label { class: "form-label", "Signing Secret" }
                                    div { style: "display: flex; gap: 8px; align-items: center;",
                                        input {
                                            class: "form-input",
                                            r#type: if *show_token.read() { "text" } else { "password" },
                                            placeholder: "Paste your Slack signing secret here",
                                            value: "{slack_signing_secret}",
                                            oninput: move |e| slack_signing_secret.set(e.value()),
                                            style: "flex: 1;",
                                        }
                                        button {
                                            class: "btn-secondary btn-sm",
                                            onclick: move |_| {
                                                let v = *show_token.read();
                                                show_token.set(!v);
                                            },
                                            if *show_token.read() { "Hide" } else { "Show" }
                                        }
                                    }
                                    p { style: "color: #8E8E93; font-size: 12px; margin-top: 4px;",
                                        "\u{1F512} Bot tokens are encrypted at rest. Never share your tokens."
                                    }
                                }
                            },
                            _ => rsx! {
                                // Default for telegram and discord
                                div { class: "form-group",
                                    label { class: "form-label", "Bot Token" }
                                    div { style: "display: flex; gap: 8px; align-items: center;",
                                        input {
                                            class: "form-input",
                                            r#type: if *show_token.read() { "text" } else { "password" },
                                            placeholder: "Paste your bot token here",
                                            value: "{bot_token}",
                                            oninput: move |e| bot_token.set(e.value()),
                                            style: "flex: 1;",
                                        }
                                        button {
                                            class: "btn-secondary btn-sm",
                                            onclick: move |_| {
                                                let v = *show_token.read();
                                                show_token.set(!v);
                                            },
                                            if *show_token.read() { "Hide" } else { "Show" }
                                        }
                                    }
                                    p { style: "color: #8E8E93; font-size: 12px; margin-top: 4px;",
                                        "\u{1F512} Bot tokens are encrypted at rest. Never share your tokens."
                                    }
                                }
                            },
                        }}
                    }

                    // Step 3: Set Trust Level
                    if step == 3 {
                        div { class: "form-group",
                            label { class: "form-label", "Trust Level" }
                            div { style: "display: flex; flex-direction: column; gap: 8px;",
                                { wizard_trust_option("owner", "\u{1F451} Owner", "Full access. Can manage system settings, approve actions, and control all capabilities.", &trust_level) }
                                { wizard_trust_option("conversational", "\u{1F4AC} Conversational", "Standard access. Can send/receive messages and use paired capabilities.", &trust_level) }
                                { wizard_trust_option("untrusted", "\u{1F512} Untrusted", "Minimal access. Can only send messages. No capability grants.", &trust_level) }
                            }
                        }
                    }

                    // Step 4: Test Connection
                    if step == 4 {
                        div { class: "form-group",
                            label { class: "form-label", "Configuration Summary" }
                            div { style: "background: rgba(255,255,255,0.05); border-radius: 8px; padding: 12px; font-size: 13px;",
                                p { strong { "Type: " } "{channel_type_icon(&channel_type.read())} {channel_type}" }
                                p { strong { "Channel ID: " } "{channel_user_id}" }
                                p { strong { "Trust Level: " } "{trust_level}" }
                                p { strong { "Token: " } "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}" }
                            }
                        }
                        if let Some(ref status) = *test_status.read() {
                            p { style: if status.starts_with("Error") { "color: #E74C3C; font-size: 13px;" } else { "color: #2ECC71; font-size: 13px;" },
                                "{status}"
                            }
                        }
                    }

                    // Step 5: Pair
                    if step == 5 {
                        if let Some(ref token) = *pairing_token.read() {
                            div { class: "form-group",
                                label { class: "form-label", "Pairing Token" }
                                div { style: "background: rgba(255,255,255,0.05); border-radius: 8px; padding: 12px;",
                                    p { style: "font-family: monospace; font-size: 14px; word-break: break-all;",
                                        "{token}"
                                    }
                                    if let Some(ref exp) = *pairing_expires.read() {
                                        p { style: "color: #8E8E93; font-size: 12px; margin-top: 4px;",
                                            "Expires: {exp}"
                                        }
                                    }
                                }
                                p { style: "color: #8E8E93; font-size: 13px; margin-top: 8px;",
                                    match channel_type.read().as_str() {
                                        "telegram" => "Send /pair {token} to your Telegram bot to complete pairing.",
                                        "discord" => "Send !pair {token} in your Discord channel to complete pairing.",
                                        "whatsapp" => "Send /pair {token} to your WhatsApp bot to complete pairing.",
                                        "slack" => "Run /carnelian pair {token} in your Slack workspace to complete pairing.",
                                        _ => "Use the pairing token in your channel to complete pairing.",
                                    }
                                }
                            }
                        } else {
                            div { class: "form-group",
                                p { "Channel created successfully. You can now initiate pairing or skip this step." }
                            }
                        }
                    }

                    if let Some(ref err) = *error_msg.read() {
                        p { style: "color: #E74C3C; font-size: 13px;", "{err}" }
                    }
                }
                div { class: "modal-footer",
                    if step > 1 && step < 5 {
                        button {
                            class: "btn-secondary",
                            onclick: move |_| {
                                error_msg.set(None);
                                current_step -= 1;
                            },
                            "\u{25C0} Back"
                        }
                    }
                    if step < 4 {
                        button {
                            class: "btn-primary",
                            onclick: move |_| {
                                error_msg.set(None);
                                // Validate current step
                                match step {
                                    2 => {
                                        if channel_user_id.read().trim().is_empty() {
                                            error_msg.set(Some("Channel User ID is required.".to_string()));
                                            return;
                                        }
                                        // Channel-type-specific validation
                                        match channel_type.read().as_str() {
                                            "whatsapp" => {
                                                if whatsapp_phone_number_id.read().trim().is_empty() {
                                                    error_msg.set(Some("Phone Number ID is required for WhatsApp.".to_string()));
                                                    return;
                                                }
                                                if bot_token.read().trim().is_empty() {
                                                    error_msg.set(Some("Access Token is required for WhatsApp.".to_string()));
                                                    return;
                                                }
                                                if whatsapp_verify_token.read().trim().is_empty() {
                                                    error_msg.set(Some("Verify Token is required for WhatsApp.".to_string()));
                                                    return;
                                                }
                                            }
                                            "slack" => {
                                                if bot_token.read().trim().is_empty() {
                                                    error_msg.set(Some("Bot Token is required for Slack.".to_string()));
                                                    return;
                                                }
                                                if slack_signing_secret.read().trim().is_empty() {
                                                    error_msg.set(Some("Signing Secret is required for Slack.".to_string()));
                                                    return;
                                                }
                                            }
                                            _ => {
                                                // telegram / discord
                                                if bot_token.read().trim().is_empty() {
                                                    error_msg.set(Some("Bot Token is required.".to_string()));
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                                current_step += 1;
                            },
                            "Next \u{25B6}"
                        }
                    }
                    if step == 4 {
                        button {
                            class: "btn-primary",
                            disabled: *submitting.read(),
                            onclick: move |_| {
                                submitting.set(true);
                                error_msg.set(None);
                                test_status.set(None);
                                let ct = channel_type.read().clone();
                                let cuid = channel_user_id.read().clone();
                                let token = bot_token.read().clone();
                                let tl = trust_level.read().clone();
                                // Read additional credentials before moving into async block
                                let phone_number_id = whatsapp_phone_number_id.read().clone();
                                let verify_token = whatsapp_verify_token.read().clone();
                                let signing_secret = slack_signing_secret.read().clone();
                                spawn(async move {
                                    // Build metadata based on channel type
                                    let metadata = match ct.as_str() {
                                        "whatsapp" => json!({
                                            "phone_number_id": phone_number_id,
                                            "verify_token": verify_token
                                        }),
                                        "slack" => json!({
                                            "signing_secret": signing_secret
                                        }),
                                        _ => json!({}),
                                    };
                                    let req = CreateChannelApiRequest {
                                        channel_type: ct,
                                        channel_user_id: cuid,
                                        bot_token: Some(token),
                                        trust_level: tl,
                                        identity_id: None,
                                        metadata,
                                        enabled: false,
                                    };
                                    match crate::api::create_channel(req).await {
                                        Ok(resp) => {
                                            created_session_id.set(Some(resp.session_id));
                                            test_status.set(Some(format!("\u{2705} Connection successful! Status: {}", resp.status)));
                                            // Clear the token from memory
                                            bot_token.set(String::new());
                                            submitting.set(false);
                                            current_step.set(5);
                                        }
                                        Err(e) => {
                                            test_status.set(Some(format!("Error: {e}")));
                                            submitting.set(false);
                                        }
                                    }
                                });
                            },
                            if *submitting.read() { "Testing\u{2026}" } else { "Test & Create" }
                        }
                    }
                    if step == 5 {
                        if pairing_token.read().is_none() {
                            button {
                                class: "btn-secondary",
                                disabled: *submitting.read(),
                                onclick: move |_| {
                                    if let Some(sid) = *created_session_id.read() {
                                        submitting.set(true);
                                        error_msg.set(None);
                                        let tl = trust_level.read().clone();
                                        spawn(async move {
                                            match crate::api::pair_channel(sid, Some(tl)).await {
                                                Ok(resp) => {
                                                    pairing_token.set(Some(resp.pairing_token.to_string()));
                                                    pairing_expires.set(Some(resp.expires_at));
                                                    submitting.set(false);
                                                }
                                                Err(e) => {
                                                    error_msg.set(Some(e));
                                                    submitting.set(false);
                                                }
                                            }
                                        });
                                    }
                                },
                                if *submitting.read() { "Pairing\u{2026}" } else { "Initiate Pairing" }
                            }
                        }
                        button {
                            class: "btn-primary",
                            onclick: move |_| on_created.call(()),
                            "Finish"
                        }
                    }
                }
            }
        }
    }
}

fn wizard_type_option(
    value: &'static str,
    label: &'static str,
    desc: &'static str,
    selected: &Signal<String>,
) -> Element {
    let is_selected = *selected.read() == value;
    let border = if is_selected {
        "border: 1px solid #3498DB;"
    } else {
        "border: 1px solid rgba(255,255,255,0.1);"
    };
    let bg = if is_selected {
        "background: rgba(52,152,219,0.15);"
    } else {
        "background: rgba(255,255,255,0.03);"
    };
    let mut sig = *selected;
    rsx! {
        div {
            style: "padding: 10px 14px; border-radius: 8px; cursor: pointer; {border} {bg}",
            onclick: move |_| sig.set(value.to_string()),
            strong { "{label}" }
            p { style: "color: #8E8E93; font-size: 12px; margin: 2px 0 0;", "{desc}" }
        }
    }
}

fn wizard_trust_option(
    value: &'static str,
    label: &'static str,
    desc: &'static str,
    selected: &Signal<String>,
) -> Element {
    let is_selected = *selected.read() == value;
    let border_color = match value {
        "owner" => "rgb(241,196,15)",
        "conversational" => "rgb(52,152,219)",
        _ => "rgb(149,165,166)",
    };
    let border = if is_selected {
        format!("border: 1px solid {border_color};")
    } else {
        "border: 1px solid rgba(255,255,255,0.1);".to_string()
    };
    let bg = if is_selected {
        format!("background: rgba(255,255,255,0.08);")
    } else {
        "background: rgba(255,255,255,0.03);".to_string()
    };
    let mut sig = *selected;
    rsx! {
        div {
            style: "padding: 10px 14px; border-radius: 8px; cursor: pointer; {border} {bg}",
            onclick: move |_| sig.set(value.to_string()),
            strong { "{label}" }
            p { style: "color: #8E8E93; font-size: 12px; margin: 2px 0 0;", "{desc}" }
        }
    }
}

// ── Edit Channel Modal ──────────────────────────────────────

#[component]
fn EditChannelModal(
    channel: ChannelDetail,
    on_close: EventHandler,
    on_saved: EventHandler,
) -> Element {
    let mut trust_level = use_signal(|| channel.trust_level.clone());
    let mut change_token = use_signal(|| false);
    let mut new_token = use_signal(String::new);
    let mut show_token = use_signal(|| false);
    let mut metadata_text = use_signal(|| {
        serde_json::to_string_pretty(&channel.metadata).unwrap_or_else(|_| "{}".to_string())
    });
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut submitting = use_signal(|| false);

    let session_id = channel.session_id;

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Edit Channel",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Edit Channel" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    div { class: "form-group",
                        label { class: "form-label", "Trust Level" }
                        select {
                            class: "form-input",
                            value: "{trust_level}",
                            onchange: move |e| trust_level.set(e.value()),
                            option { value: "owner", "\u{1F451} Owner" }
                            option { value: "conversational", "\u{1F4AC} Conversational" }
                            option { value: "untrusted", "\u{1F512} Untrusted" }
                        }
                    }
                    div { class: "form-group",
                        label { style: "display: flex; align-items: center; gap: 8px;",
                            input {
                                r#type: "checkbox",
                                checked: *change_token.read(),
                                onchange: move |e| change_token.set(e.checked()),
                            }
                            span { class: "form-label", style: "margin: 0;", "Change Bot Token" }
                        }
                        if *change_token.read() {
                            div { style: "display: flex; gap: 8px; align-items: center; margin-top: 8px;",
                                input {
                                    class: "form-input",
                                    r#type: if *show_token.read() { "text" } else { "password" },
                                    placeholder: "New bot token",
                                    value: "{new_token}",
                                    oninput: move |e| new_token.set(e.value()),
                                    style: "flex: 1;",
                                }
                                button {
                                    class: "btn-secondary btn-sm",
                                    onclick: move |_| {
                                        let v = *show_token.read();
                                        show_token.set(!v);
                                    },
                                    if *show_token.read() { "Hide" } else { "Show" }
                                }
                            }
                            p { style: "color: #F39C12; font-size: 12px; margin-top: 4px;",
                                "\u{26A0}\u{FE0F} Changing the bot token will restart the adapter."
                            }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Metadata (JSON)" }
                        textarea {
                            class: "form-textarea",
                            rows: "5",
                            value: "{metadata_text}",
                            oninput: move |e| metadata_text.set(e.value()),
                        }
                    }
                    if let Some(ref err) = *error_msg.read() {
                        p { style: "color: #E74C3C; font-size: 13px;", "{err}" }
                    }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn-secondary",
                        onclick: move |_| on_close.call(()),
                        "Cancel"
                    }
                    button {
                        class: "btn-primary",
                        disabled: *submitting.read(),
                        onclick: move |_| {
                            error_msg.set(None);

                            // Validate metadata JSON
                            let meta_val: Option<serde_json::Value> = {
                                let text = metadata_text.read().clone();
                                if text.trim().is_empty() {
                                    None
                                } else {
                                    match serde_json::from_str(&text) {
                                        Ok(v) => Some(v),
                                        Err(e) => {
                                            error_msg.set(Some(format!("Invalid JSON: {e}")));
                                            return;
                                        }
                                    }
                                }
                            };

                            let token_val = if *change_token.read() {
                                let t = new_token.read().clone();
                                if t.trim().is_empty() {
                                    error_msg.set(Some("Token cannot be empty.".to_string()));
                                    return;
                                }
                                Some(t)
                            } else {
                                None
                            };

                            submitting.set(true);
                            let tl = trust_level.read().clone();
                            spawn(async move {
                                let req = UpdateChannelApiRequest {
                                    trust_level: Some(tl),
                                    bot_token: token_val,
                                    metadata: meta_val,
                                    enabled: None,
                                };
                                match crate::api::update_channel(session_id, req).await {
                                    Ok(_) => on_saved.call(()),
                                    Err(e) => {
                                        error_msg.set(Some(e));
                                        submitting.set(false);
                                    }
                                }
                            });
                        },
                        if *submitting.read() { "Saving\u{2026}" } else { "Save" }
                    }
                }
            }
        }
    }
}

// ── Confirm Delete Dialog ───────────────────────────────────

#[component]
fn ConfirmDeleteDialog(
    channel: ChannelDetail,
    on_close: EventHandler,
    on_confirmed: EventHandler,
) -> Element {
    let mut submitting = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let session_id = channel.session_id;

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Confirm Delete",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                style: "max-width: 440px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Delete Channel" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    p { style: "font-size: 14px;",
                        "Are you sure you want to delete the "
                        strong { "{channel.channel_type}" }
                        " channel "
                        strong { "{channel.channel_user_id}" }
                        "?"
                    }
                    p { style: "color: #E74C3C; font-size: 13px; margin-top: 8px;",
                        "\u{26A0}\u{FE0F} This action cannot be undone. The adapter will be stopped and all session data will be lost."
                    }
                    if let Some(ref err) = *error_msg.read() {
                        p { style: "color: #E74C3C; font-size: 13px;", "{err}" }
                    }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn-secondary",
                        onclick: move |_| on_close.call(()),
                        "Cancel"
                    }
                    button {
                        class: "btn-danger",
                        disabled: *submitting.read(),
                        onclick: move |_| {
                            submitting.set(true);
                            error_msg.set(None);
                            spawn(async move {
                                match crate::api::delete_channel(session_id).await {
                                    Ok(()) => on_confirmed.call(()),
                                    Err(e) => {
                                        error_msg.set(Some(e));
                                        submitting.set(false);
                                    }
                                }
                            });
                        },
                        if *submitting.read() { "Deleting\u{2026}" } else { "Delete" }
                    }
                }
            }
        }
    }
}
