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

use carnelian_common::types::{AgentXpResponse, EventEnvelope, EventType};
use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use tokio::sync::mpsc;
use uuid::Uuid;

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

/// A notification about an approval lifecycle event.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ApprovalNotification {
    pub approval_id: String,
    pub event_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// State for the Heartbeat panel.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct HeartbeatState {
    pub current_mantra: Option<String>,
    pub last_heartbeat_time: Option<DateTime<Utc>>,
    pub next_heartbeat_time: Option<DateTime<Utc>>,
    pub recent_mantras: Vec<HeartbeatRecord>,
}

/// A single heartbeat record from the history table.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HeartbeatRecord {
    pub heartbeat_id: Uuid,
    pub ts: DateTime<Utc>,
    pub mantra: String,
    pub status: String,
}

/// State for the Identity panel.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct IdentityState {
    pub identity_id: Option<Uuid>,
    pub name: String,
    pub pronouns: Option<String>,
    pub soul_file_preview: String,
    pub directive_count: usize,
}

/// XP progression state.
#[derive(Debug, Clone, Default)]
pub struct XpState {
    pub identity_id: Option<Uuid>,
    pub total_xp: i64,
    pub level: i32,
    pub xp_to_next_level: i64,
    pub progress_pct: f64,
    pub milestone_feature: Option<String>,
}

/// Kind of toast notification.
#[derive(Debug, Clone)]
pub enum ToastKind {
    XpGained { amount: i32, source: String },
    LevelUp { new_level: i32 },
}

/// A transient toast notification.
#[derive(Debug, Clone)]
pub struct ToastNotification {
    pub id: u64,
    pub kind: ToastKind,
    pub timestamp: DateTime<Utc>,
}

/// State for the Channels panel.
#[derive(Debug, Clone, Default)]
pub struct ChannelState {
    pub total_channels: usize,
    pub running_channels: usize,
    pub last_channel_event: Option<DateTime<Utc>>,
}

/// State for the Providers panel.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ProviderState {
    pub ollama_connected: bool,
    pub ollama_url: String,
    pub available_models: Vec<String>,
    pub providers: Vec<ProviderInfo>,
}

/// Information about a single model provider.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProviderInfo {
    pub provider_id: Uuid,
    pub name: String,
    pub provider_type: String,
    pub enabled: bool,
}

/// Global application state backed by Dioxus signals.
#[derive(Clone)]
pub struct EventStreamStore {
    pub events: Signal<VecDeque<EventEnvelope>>,
    pub connection_state: Signal<ConnectionState>,
    pub system_status: Signal<SystemStatus>,
    pub machine_profile: Signal<String>,
    pub approval_notifications: Signal<Vec<ApprovalNotification>>,
    pub heartbeat_state: Signal<HeartbeatState>,
    pub identity_state: Signal<IdentityState>,
    pub provider_state: Signal<ProviderState>,
    pub channel_state: Signal<ChannelState>,
    pub xp_state: Signal<XpState>,
    pub toast_notifications: Signal<Vec<ToastNotification>>,
    /// Running total of events received (used to trigger reactive refreshes).
    pub event_count: Signal<u64>,
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
            approval_notifications: Signal::new(Vec::new()),
            heartbeat_state: Signal::new(HeartbeatState::default()),
            identity_state: Signal::new(IdentityState::default()),
            provider_state: Signal::new(ProviderState::default()),
            channel_state: Signal::new(ChannelState::default()),
            xp_state: Signal::new(XpState::default()),
            toast_notifications: Signal::new(Vec::new()),
            event_count: Signal::new(0u64),
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

        let (xp_tx, xp_rx) = mpsc::unbounded_channel::<AgentXpResponse>();

        // --- Tokio tasks (Send-safe, no signals) ---
        crate::websocket::start_websocket_client(WS_URL.to_string(), event_tx, state_tx);
        start_status_polling(status_tx);
        start_xp_polling(xp_tx);

        // --- Dioxus coroutines (UI thread, signal writes OK) ---
        self.bridge_events(event_rx);
        self.bridge_connection_state(state_rx);
        self.bridge_status(status_rx);
        self.bridge_xp(xp_rx);
    }

    /// Return the most recent `count` events (newest first).
    #[allow(dead_code)]
    pub fn recent_events(&self, count: usize) -> Vec<EventEnvelope> {
        let events = self.events.read();
        events.iter().rev().take(count).cloned().collect()
    }

    /// Dioxus-local coroutine that drains the event channel into the
    /// signal-backed ring buffer. Also populates `approval_notifications`
    /// for approval lifecycle events.
    // Signal bridge handles all event variants; extraction is intentionally monolithic
    #[allow(clippy::too_many_lines)]
    fn bridge_events(&self, mut rx: mpsc::UnboundedReceiver<EventEnvelope>) {
        let mut events = self.events;
        let mut event_count = self.event_count;
        let mut approval_notifications = self.approval_notifications;
        let mut heartbeat_state = self.heartbeat_state;
        let mut identity_state = self.identity_state;
        let mut provider_state = self.provider_state;
        let mut channel_state = self.channel_state;
        let mut toast_notifications = self.toast_notifications;
        spawn(async move {
            while let Some(envelope) = rx.recv().await {
                // Track approval lifecycle events
                match &envelope.event_type {
                    carnelian_common::types::EventType::ApprovalQueued
                    | carnelian_common::types::EventType::ApprovalApproved
                    | carnelian_common::types::EventType::ApprovalDenied => {
                        let approval_id = envelope
                            .payload
                            .get("approval_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let event_type = format!("{:?}", envelope.event_type);
                        let mut notifs = approval_notifications.write();
                        notifs.push(ApprovalNotification {
                            approval_id,
                            event_type,
                            timestamp: envelope.timestamp,
                        });
                        // Keep only the last 50 notifications
                        if notifs.len() > 50 {
                            let excess = notifs.len() - 50;
                            notifs.drain(..excess);
                        }
                    }
                    _ => {}
                }

                // Track XP events from SSE
                if let EventType::Custom(ref name) = envelope.event_type {
                    if name == "XpAwarded" {
                        // XP amounts bounded < i32::MAX in practice
                        #[allow(clippy::cast_possible_truncation)]
                        let amount = envelope
                            .payload
                            .get("xp_amount")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0) as i32;
                        let source = envelope
                            .payload
                            .get("source")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let mut toasts = toast_notifications.write();
                        let next_id = toasts.len() as u64;
                        toasts.push(ToastNotification {
                            id: next_id,
                            kind: ToastKind::XpGained { amount, source },
                            timestamp: envelope.timestamp,
                        });
                        if toasts.len() > 10 {
                            let excess = toasts.len() - 10;
                            toasts.drain(..excess);
                        }
                    } else if name == "XpLevelUp" {
                        // XP levels bounded < i32::MAX (max level 99)
                        #[allow(clippy::cast_possible_truncation)]
                        let new_level = envelope
                            .payload
                            .get("new_level")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0) as i32;
                        let mut toasts = toast_notifications.write();
                        let next_id = toasts.len() as u64;
                        toasts.push(ToastNotification {
                            id: next_id,
                            kind: ToastKind::LevelUp { new_level },
                            timestamp: envelope.timestamp,
                        });
                        if toasts.len() > 10 {
                            let excess = toasts.len() - 10;
                            toasts.drain(..excess);
                        }
                    }
                }

                // Track heartbeat events
                if matches!(envelope.event_type, EventType::HeartbeatTick) {
                    let mantra = envelope
                        .payload
                        .get("mantra")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let mut hb = heartbeat_state.write();
                    hb.current_mantra = mantra;
                    hb.last_heartbeat_time = Some(envelope.timestamp);
                }

                // Track provider/gateway events — HeartbeatOk implies a
                // successful gateway round-trip, so mark Ollama connected.
                // GatewayRequestEnd and GatewayRateLimited are reserved for
                // future use when the backend starts emitting them.
                if matches!(
                    envelope.event_type,
                    EventType::HeartbeatOk
                        | EventType::GatewayRequestEnd
                        | EventType::GatewayRateLimited
                ) {
                    let mut ps = provider_state.write();
                    // HeartbeatOk means the gateway responded successfully
                    if matches!(envelope.event_type, EventType::HeartbeatOk) {
                        ps.ollama_connected = true;
                    }
                    if let Some(url) = envelope.payload.get("gateway_url").and_then(|v| v.as_str())
                    {
                        ps.ollama_url = url.to_string();
                    }
                    if let Some(models) = envelope
                        .payload
                        .get("available_models")
                        .and_then(|v| v.as_array())
                    {
                        ps.available_models = models
                            .iter()
                            .filter_map(|m| m.as_str().map(String::from))
                            .collect();
                    }
                }

                // Track channel lifecycle events
                if let EventType::Custom(ref name) = envelope.event_type {
                    if name == "ChannelCreated"
                        || name == "ChannelUpdated"
                        || name == "ChannelDeleted"
                        || name == "ChannelPaired"
                    {
                        let mut cs = channel_state.write();
                        cs.last_channel_event = Some(envelope.timestamp);
                        // Update counts from payload if available
                        if let Some(total) = envelope
                            .payload
                            .get("total_channels")
                            .and_then(serde_json::Value::as_u64)
                        {
                            cs.total_channels = total as usize;
                        }
                        if let Some(running) = envelope
                            .payload
                            .get("running_channels")
                            .and_then(serde_json::Value::as_u64)
                        {
                            cs.running_channels = running as usize;
                        }
                    }
                }

                // Track soul update events
                if matches!(envelope.event_type, EventType::SoulUpdated) {
                    let mut id_state = identity_state.write();
                    #[allow(clippy::cast_possible_truncation)]
                    let directive_count = envelope
                        .payload
                        .get("directive_count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as usize;
                    id_state.directive_count = directive_count;
                }

                let mut buf = events.write();
                buf.push_back(envelope);
                if buf.len() > MAX_EVENTS {
                    buf.pop_front();
                }
                *event_count.write() += 1;
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

    /// Dioxus-local coroutine that drains XP updates.
    fn bridge_xp(&self, mut rx: mpsc::UnboundedReceiver<AgentXpResponse>) {
        let mut xp_state = self.xp_state;
        spawn(async move {
            while let Some(resp) = rx.recv().await {
                xp_state.set(XpState {
                    identity_id: Some(resp.identity_id),
                    total_xp: resp.total_xp,
                    level: resp.level,
                    xp_to_next_level: resp.xp_to_next_level,
                    progress_pct: resp.progress_pct,
                    milestone_feature: resp.milestone_feature,
                });
            }
        });
    }
}

/// Tokio task that polls the XP endpoint every 60 seconds.
fn start_xp_polling(xp_tx: mpsc::UnboundedSender<AgentXpResponse>) {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let status_url = format!("{SERVER_BASE_URL}/v1/status");

        loop {
            // First get the identity_id from /v1/status
            if let Ok(resp) = client.get(&status_url).send().await {
                if let Ok(body) = resp.text().await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(id_str) = json.get("identity_id").and_then(|v| v.as_str()) {
                            if let Ok(identity_id) = id_str.parse::<Uuid>() {
                                let xp_url =
                                    format!("{SERVER_BASE_URL}/v1/xp/agents/{identity_id}");
                                if let Ok(xp_resp) = client.get(&xp_url).send().await {
                                    if let Ok(xp) = xp_resp.json::<AgentXpResponse>().await {
                                        let _ = xp_tx.send(xp);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
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
