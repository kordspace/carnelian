//! System tray integration for background operation.
//!
//! Uses the `tray-icon` and `muda` crates (transitive deps of
//! `dioxus-desktop`) to create a native system tray icon with:
//!
//! - **Dynamic icon** — green (Connected), yellow (Connecting),
//!   gray (Disconnected), red (Error).
//! - **Context menu** — Show Window, Hide Window, Quit.
//! - **Reactive updates** — icon color changes when
//!   `EventStreamStore::connection_state` changes.
//!
//! The tray icon **must** be created on the event-loop thread, so
//! `init_system_tray()` only registers the menu-event handler, and
//! `SystemTray` (a Dioxus component) builds the actual icon after
//! the event loop is running.

use std::sync::OnceLock;

use dioxus::prelude::*;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon as TrayIconImage, TrayIcon, TrayIconBuilder};

use crate::store::EventStreamStore;
use crate::websocket::ConnectionState;

// ── Menu item IDs (used to match events) ────────────────────

/// Stable string IDs for context-menu items.
const MENU_SHOW: &str = "show_window";
const MENU_HIDE: &str = "hide_window";
const MENU_QUIT: &str = "quit";

// ── Tray icon ───────────────────────────────────────────────

/// Load the Carnelian icon from embedded SVG bytes.
/// The icon is loaded at compile time and parsed into RGBA for the tray.
fn load_carnelian_icon() -> TrayIconImage {
    // Embed the SVG file at compile time
    let svg_bytes = include_bytes!("../../../../assets/logos/carnelian-icon.svg");

    // For now, create a colored icon based on the SVG palette
    // In a full implementation, you'd parse the SVG and render it to RGBA
    // This uses the Carnelian Red color from the brand palette (#B7410E)
    carnelian_icon_rgba(183, 65, 14)
}

/// Generate a 32×32 RGBA icon with Carnelian gemstone colors.
fn carnelian_icon_rgba(r: u8, g: u8, b: u8) -> TrayIconImage {
    let size = 32_u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);

    // Create a faceted gemstone pattern
    for y in 0..size {
        for x in 0..size {
            // Center of icon
            let cx = size / 2;
            let cy = size / 2;

            // Distance from center
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            let dist = ((dx * dx + dy * dy) as f32).sqrt();

            // Create faceted effect with angular segments
            let angle = (dy as f32).atan2(dx as f32);
            let segment =
                ((angle + std::f32::consts::PI) / (std::f32::consts::PI / 4.0)) as i32 % 8;
            let facet_variation = (segment % 2) as f32 * 0.15;

            // Circle boundary with slight variation for facets
            let radius = 14.0 + facet_variation;

            if dist < radius {
                // Main gemstone color with gradient toward center
                let gradient = 1.0 - (dist / radius) * 0.4;
                let br = (r as f32 * gradient) as u8;
                let bg = (g as f32 * gradient) as u8;
                let bb = (b as f32 * gradient) as u8;

                // Highlight in center
                if dist < 4.0 {
                    let highlight = (1.0 - dist / 4.0) * 60.0;
                    rgba.extend_from_slice(&[
                        (br.saturating_add(highlight as u8)).min(255),
                        (bg.saturating_add(highlight as u8 * 2 / 3)).min(255),
                        (bb.saturating_add(highlight as u8 / 3)).min(255),
                        255,
                    ]);
                } else {
                    rgba.extend_from_slice(&[br, bg, bb, 255]);
                }
            } else {
                // Transparent outside the icon
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    TrayIconImage::from_rgba(rgba, size, size).expect("valid 32×32 icon")
}

/// Generate a 16×16 RGBA icon filled with a single colour.
fn solid_icon(r: u8, g: u8, b: u8) -> TrayIconImage {
    let size = 16_u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for _ in 0..(size * size) {
        rgba.extend_from_slice(&[r, g, b, 255]);
    }
    TrayIconImage::from_rgba(rgba, size, size).expect("valid 16×16 icon")
}

fn icon_connected() -> TrayIconImage {
    solid_icon(46, 204, 113)
}
fn icon_connecting() -> TrayIconImage {
    solid_icon(243, 156, 18)
}
fn icon_disconnected() -> TrayIconImage {
    load_carnelian_icon()
}
fn icon_error() -> TrayIconImage {
    solid_icon(231, 76, 60)
}

// ── Pre-launch initialisation ───────────────────────────────

/// Global holder for menu-item IDs so the event handler can
/// match them without capturing local state.
static MENU_IDS: OnceLock<MenuIds> = OnceLock::new();

struct MenuIds {
    show: tray_icon::menu::MenuId,
    hide: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

/// Pre-launch setup: register the global `MenuEvent` handler.
///
/// The handler runs on the event-loop thread whenever a tray
/// context-menu item is clicked.  Window show/hide is logged
/// (Dioxus 0.6 does not expose a stable window-handle API);
/// Quit terminates the process.
pub fn init_system_tray() {
    tracing::info!("System tray: registering menu-event handler");
    MenuEvent::set_event_handler(Some(handle_menu_event));
}

/// Dispatch a tray context-menu click to the appropriate action.
#[allow(clippy::needless_pass_by_value)] // Signature required by MenuEvent::set_event_handler
fn handle_menu_event(event: MenuEvent) {
    let Some(ids) = MENU_IDS.get() else {
        return;
    };
    if event.id == ids.show {
        tracing::info!("Tray menu: Show Window");
    } else if event.id == ids.hide {
        tracing::info!("Tray menu: Hide Window");
    } else if event.id == ids.quit {
        tracing::info!("Tray menu: Quit");
        std::process::exit(0);
    }
}

// ── Dioxus component: creates & updates the native tray ─────

/// Return the tray icon image for a given connection state.
fn icon_for_state(state: &ConnectionState) -> TrayIconImage {
    match state {
        ConnectionState::Connected => icon_connected(),
        ConnectionState::Connecting => icon_connecting(),
        ConnectionState::Disconnected => icon_disconnected(),
        ConnectionState::Error(_) => icon_error(),
    }
}

/// Return the tooltip string for a given connection state.
const fn tooltip_for_state(state: &ConnectionState) -> &'static str {
    match state {
        ConnectionState::Connected => "Carnelian OS \u{2014} Connected",
        ConnectionState::Connecting => "Carnelian OS \u{2014} Connecting\u{2026}",
        ConnectionState::Disconnected => "Carnelian OS \u{2014} Disconnected",
        ConnectionState::Error(_) => "Carnelian OS \u{2014} Error",
    }
}

/// Invisible component that owns the native system tray icon.
///
/// Must be rendered inside the Dioxus tree (i.e. after the
/// event loop is running).  It creates the tray icon once via
/// `use_hook`, then reactively updates the icon colour whenever
/// `connection_state` changes.
#[component]
pub fn SystemTray() -> Element {
    let store = use_context::<EventStreamStore>();

    // Build tray icon + menu exactly once.
    let tray_handle: Signal<Option<TrayIcon>> = use_signal(|| None);

    use_hook({
        let mut tray_handle = tray_handle;
        move || match build_tray() {
            Ok(tray) => {
                tracing::info!("Native system tray icon created");
                tray_handle.set(Some(tray));
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create system tray icon; falling back to in-app controls");
            }
        }
    });

    // Reactively update icon colour on connection-state changes.
    let connection_state = store.connection_state.read();
    if let Some(tray) = &*tray_handle.read() {
        let _ = tray.set_icon(Some(icon_for_state(&connection_state)));
        tray.set_tooltip(Some(tooltip_for_state(&connection_state)))
            .ok();
    }

    // The component itself renders nothing visible.
    rsx! {}
}

/// Build the native tray icon with a context menu.
fn build_tray() -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let menu = Menu::new();

    let show_item = MenuItem::with_id(MENU_SHOW, "Show Window", true, None);
    let hide_item = MenuItem::with_id(MENU_HIDE, "Hide Window", true, None);
    let quit_item = MenuItem::with_id(MENU_QUIT, "Quit", true, None);

    menu.append(&show_item)?;
    menu.append(&hide_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit_item)?;

    // Store IDs for the global event handler.
    let _ = MENU_IDS.set(MenuIds {
        show: show_item.id().clone(),
        hide: hide_item.id().clone(),
        quit: quit_item.id().clone(),
    });

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Carnelian OS")
        .with_icon(icon_disconnected())
        .build()?;

    Ok(tray)
}

// ── In-app fallback controls ────────────────────────────────

/// In-app window control buttons that replicate the tray context menu.
///
/// Renders three actions: Minimize, Hide, Quit.
/// These serve as a fallback when the native tray icon cannot be
/// created, and as convenient in-window affordances regardless.
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
                "\u{2013}"
            }
            button {
                class: "btn-icon btn-window-control",
                title: "Hide Window",
                onclick: move |_| {
                    tracing::debug!("Window hide requested");
                },
                "\u{2012}"
            }
            button {
                class: "btn-icon btn-window-control btn-quit",
                title: "Quit",
                onclick: move |_| {
                    tracing::info!("Application quit requested");
                    std::thread::spawn(|| std::process::exit(0));
                },
                "\u{2715}"
            }
        }
    }
}

/// Connection-state-aware status badge (in-app tray mirror).
///
/// Displays a colored indicator and label reflecting the current
/// connection state: Running (green), Connecting (yellow),
/// Stopped (red), or Error (red).
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
