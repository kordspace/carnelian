//! Toast overlay component for transient XP notifications.

use chrono::Utc;
use dioxus::prelude::*;

use crate::store::{EventStreamStore, ToastKind};

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
