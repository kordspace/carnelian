//! Toast overlay component for transient XP notifications.

use chrono::Utc;
use dioxus::prelude::*;

use crate::store::{EventStreamStore, ToastKind};

// ── Generic toast types used by page components ──────────────────────────────

/// Severity level for a generic page-level toast notification.
#[derive(Debug, Clone, PartialEq)]
pub enum ToastType {
    Success,
    Error,
    Warning,
    Info,
}

/// A generic page-level toast notification data struct.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastMessage {
    pub id: String,
    pub message: String,
    pub toast_type: ToastType,
    pub duration_secs: u32,
}

/// Display component for a single generic toast notification.
#[component]
pub fn Toast(toast: ToastMessage) -> Element {
    let (icon, extra_class) = match toast.toast_type {
        ToastType::Success => ("✅", "toast-success"),
        ToastType::Error => ("❌", "toast-error"),
        ToastType::Warning => ("⚠️", "toast-warning"),
        ToastType::Info => ("ℹ️", "toast-info"),
    };
    rsx! {
        div { class: "toast {extra_class}",
            span { class: "toast-icon", "{icon}" }
            span { class: "toast-message", " {toast.message}" }
        }
    }
}

/// Fixed-position overlay that renders transient toast notifications.
#[component]
pub fn ToastOverlay() -> Element {
    let store = use_context::<EventStreamStore>();
    let mut toast_notifications = store.toast_notifications;

    // Auto-dismiss toasts older than 4 seconds
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let now = Utc::now();
            let mut toasts = toast_notifications.write();
            toasts.retain(|t| {
                let age = now.signed_duration_since(t.timestamp);
                age.num_seconds() < 4
            });
        }
    });

    let toasts = toast_notifications.read();

    rsx! {
        div { class: "toast-container",
            for toast in toasts.iter().rev() {
                {
                    let toast_class = match &toast.kind {
                        ToastKind::LevelUp { .. } => "toast toast-level-up",
                        _ => "toast",
                    };
                    match &toast.kind {
                        ToastKind::XpGained { amount, source } => {
                            rsx! {
                                div { class: "{toast_class}", key: "{toast.id}",
                                    span { style: "color: #2ECC71; font-weight: 700;", "+{amount} XP" }
                                    span { style: "color: #A0A0A0; font-size: 11px; margin-left: 6px;", "{source}" }
                                }
                            }
                        }
                        ToastKind::LevelUp { new_level } => {
                            rsx! {
                                div { class: "{toast_class}", key: "{toast.id}",
                                    span { "\u{1F389} Level Up! Now Level {new_level}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
