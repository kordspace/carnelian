//! System tray integration for background operation.
//!
//! Dioxus 0.6 desktop does not expose a stable cross-platform system tray
//! API. This module provides:
//!
//! - **`init_system_tray()`** — pre-launch initialization (logging, future
//!   native tray setup).
//! - **`WindowControls`** — in-app window control buttons (minimize, hide
//!   to background, quit) that serve as the tray-menu equivalent.
//! - **`TrayStatusBadge`** — connection-state-aware status indicator that
//!   mirrors what a native tray icon would display.
//!
//! When Dioxus desktop gains a stable tray API, `init_system_tray` will
//! register a native icon and context menu, and the in-app controls will
//! become optional.

use dioxus::prelude::*;

use crate::store::EventStreamStore;
use crate::websocket::ConnectionState;

/// Initialize system tray behavior.
///
/// Called before `dioxus::launch`. On platforms where Dioxus exposes a
/// native tray API this will register the icon and menu. Currently logs
/// the initialization and defers to in-app `WindowControls`.
pub fn init_system_tray() {
    tracing::info!(
        "System tray: using in-app window controls (native tray unavailable in Dioxus 0.6)"
    );

    // Platform-specific native tray setup would go here:
    //
    // #[cfg(target_os = "windows")]
    // { /* Win32 tray icon via `windows` crate */ }
    //
    // #[cfg(target_os = "macos")]
    // { /* NSStatusItem via `objc` crate */ }
    //
    // #[cfg(target_os = "linux")]
    // { /* libappindicator or StatusNotifierItem */ }
}

/// In-app window control buttons that replicate a tray context menu.
///
/// Renders three actions:
/// - **Minimize** — minimize the window (logged; native minimize requires
///   platform-specific window handle access).
/// - **Hide** — hide the window (background operation).
/// - **Quit** — close the application.
///
/// Window minimize/hide rely on the native title bar in Dioxus 0.6 desktop.
/// The buttons here provide explicit UI affordances and log the intent;
/// full programmatic control will be wired when Dioxus exposes stable
/// window-handle APIs.
#[component]
pub fn WindowControls() -> Element {
    rsx! {
        div { class: "window-controls",
            button {
                class: "btn-icon btn-window-control",
                title: "Minimize",
                onclick: move |_| {
                    tracing::debug!("Window minimize requested");
                },
                "\u{2013}" // en-dash as minimize icon
            }
            button {
                class: "btn-icon btn-window-control",
                title: "Hide Window",
                onclick: move |_| {
                    tracing::debug!("Window hide requested");
                },
                "\u{2012}" // figure-dash as hide icon
            }
            button {
                class: "btn-icon btn-window-control btn-quit",
                title: "Quit",
                onclick: move |_| {
                    tracing::info!("Application quit requested");
                    // Spawn exit on a separate thread to avoid the `!` return
                    // type interfering with Dioxus event handler signatures.
                    std::thread::spawn(|| std::process::exit(0));
                },
                "\u{2715}" // multiplication-x as close icon
            }
        }
    }
}

/// Connection-state-aware status badge mirroring a native tray icon.
///
/// Displays a colored indicator and label reflecting the current
/// connection state: Running (green), Connecting (yellow), Stopped (red),
/// or Error (red).
#[component]
pub fn TrayStatusBadge() -> Element {
    let store = use_context::<EventStreamStore>();
    let connection_state = store.connection_state.read();

    let (icon, label, class) = match &*connection_state {
        ConnectionState::Connected => ("\u{1F7E2}", "Running", "tray-badge tray-running"),
        ConnectionState::Connecting => ("\u{1F7E1}", "Connecting", "tray-badge tray-connecting"),
        ConnectionState::Disconnected => ("\u{1F534}", "Stopped", "tray-badge tray-stopped"),
        ConnectionState::Error(msg) => {
            tracing::debug!(error = %msg, "Tray badge showing error state");
            ("\u{1F534}", "Error", "tray-badge tray-error")
        }
    };

    rsx! {
        div { class: "{class}",
            span { class: "tray-icon", "{icon}" }
            span { class: "tray-label", "{label}" }
        }
    }
}
