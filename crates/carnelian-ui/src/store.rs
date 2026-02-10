//! Centralized state management for the Carnelian UI.
//!
//! `EventStreamStore` holds reactive Dioxus signals for connection state,
//! system status, machine profile, and a bounded ring buffer of recent
//! events.
//!
//! Because Dioxus signals are **not `Send`**, background tokio tasks cannot
//! write to them directly. Instead, tokio tasks push updates through
//! `mpsc` channels, and Dioxus-local `spawn` coroutines drain those
//! channels on the UI thread where signal writes are safe.

use std::collections::VecDeque;
use std::time::Duration;

use carnelian_common::types::EventEnvelope;
use dioxus::prelude::*;
use tokio::sync::mpsc;

use crate::websocket::ConnectionState;

/// Maximum number of events retained in the ring buffer.
const MAX_EVENTS: usize = 1000;

/// Default server base URL.
const SERVER_BASE_URL: &str = "http://localhost:18789";

/// WebSocket endpoint URL.
const WS_URL: &str = "ws://localhost:18789/v1/events/ws";

/// System status fetched from the server.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemStatus {
    pub healthy: bool,
    pub version: String,
    pub uptime_seconds: Option<u64>,
}

/// Parsed status response forwarded from the tokio polling task.
#[derive(Debug, Clone)]
pub struct StatusUpdate {
    pub status: SystemStatus,
    pub profile: String,
}

/// Global application state backed by Dioxus signals.
#[derive(Clone)]
pub struct EventStreamStore {
    pub events: Signal<VecDeque<EventEnvelope>>,
    pub connection_state: Signal<ConnectionState>,
    pub system_status: Signal<SystemStatus>,
    pub machine_profile: Signal<String>,
}

impl EventStreamStore {
    /// Create a new store with default values.
    ///
    /// After creation, call `start()` from within a Dioxus component
    /// to spawn the WebSocket client and bridge tasks.
    pub fn new() -> Self {
        Self {
            events: Signal::new(VecDeque::with_capacity(MAX_EVENTS)),
            connection_state: Signal::new(ConnectionState::Disconnected),
            system_status: Signal::new(SystemStatus::default()),
            machine_profile: Signal::new("Unknown".to_string()),
        }
    }

    /// Spawn all background tasks.
    ///
    /// - A **tokio** task runs the WebSocket client (Send-safe).
    /// - A **tokio** task polls the `/v1/status` endpoint (Send-safe).
    /// - **Dioxus** `spawn` coroutines drain the channels and write
    ///   to signals on the UI thread (not Send, but safe here).
    pub fn start(&self) {
        // Channels: tokio tasks → Dioxus UI thread
        let (event_tx, event_rx) = mpsc::unbounded_channel::<EventEnvelope>();
        let (state_tx, state_rx) = mpsc::unbounded_channel::<ConnectionState>();
        let (status_tx, status_rx) = mpsc::unbounded_channel::<StatusUpdate>();

        // --- Tokio tasks (Send-safe, no signals) ---
        crate::websocket::start_websocket_client(WS_URL.to_string(), event_tx, state_tx);
        start_status_polling(status_tx);

        // --- Dioxus coroutines (UI thread, signal writes OK) ---
        self.bridge_events(event_rx);
        self.bridge_connection_state(state_rx);
        self.bridge_status(status_rx);
    }

    /// Return the most recent `count` events (newest first).
    #[allow(dead_code)]
    pub fn recent_events(&self, count: usize) -> Vec<EventEnvelope> {
        let events = self.events.read();
        events.iter().rev().take(count).cloned().collect()
    }

    /// Dioxus-local coroutine that drains the event channel into the
    /// signal-backed ring buffer.
    fn bridge_events(&self, mut rx: mpsc::UnboundedReceiver<EventEnvelope>) {
        let mut events = self.events;
        spawn(async move {
            while let Some(envelope) = rx.recv().await {
                let mut buf = events.write();
                buf.push_back(envelope);
                if buf.len() > MAX_EVENTS {
                    buf.pop_front();
                }
            }
        });
    }

    /// Dioxus-local coroutine that drains connection state changes.
    fn bridge_connection_state(&self, mut rx: mpsc::UnboundedReceiver<ConnectionState>) {
        let mut connection_state = self.connection_state;
        spawn(async move {
            while let Some(state) = rx.recv().await {
                connection_state.set(state);
            }
        });
    }

    /// Dioxus-local coroutine that drains status updates.
    fn bridge_status(&self, mut rx: mpsc::UnboundedReceiver<StatusUpdate>) {
        let mut system_status = self.system_status;
        let mut machine_profile = self.machine_profile;
        spawn(async move {
            while let Some(update) = rx.recv().await {
                system_status.set(update.status);
                machine_profile.set(update.profile);
            }
        });
    }
}

/// Tokio task that polls the server status endpoint every 30 seconds
/// and sends parsed results through the channel.
fn start_status_polling(status_tx: mpsc::UnboundedSender<StatusUpdate>) {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let url = format!("{SERVER_BASE_URL}/v1/status");

        loop {
            match client.get(&url).send().await {
                Ok(resp) => {
                    if let Ok(body) = resp.text().await {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                            let version = json
                                .get("version")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            let uptime = json
                                .get("uptime_seconds")
                                .and_then(serde_json::Value::as_u64);

                            let profile = json
                                .get("machine_profile")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Custom")
                                .to_string();

                            let _ = status_tx.send(StatusUpdate {
                                status: SystemStatus {
                                    healthy: true,
                                    version,
                                    uptime_seconds: uptime,
                                },
                                profile,
                            });
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to fetch system status");
                    let _ = status_tx.send(StatusUpdate {
                        status: SystemStatus::default(),
                        profile: "Unknown".to_string(),
                    });
                }
            }

            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
}
