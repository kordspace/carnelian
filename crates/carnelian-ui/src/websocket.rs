//! WebSocket client for real-time event streaming from the Carnelian server.
//!
//! Connects to `ws://localhost:18789/v1/events/ws`, deserializes incoming
//! `EventEnvelope` messages, and forwards them to the `EventStreamStore`
//! via an `mpsc` channel. Implements exponential backoff reconnection
//! (1s → 30s) on connection failures.

use std::time::Duration;

use carnelian_common::types::EventEnvelope;
use futures_util::StreamExt;
use tokio::sync::mpsc;

/// WebSocket connection state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// Minimum backoff duration (1 second).
const BACKOFF_MIN: Duration = Duration::from_secs(1);

/// Maximum backoff duration (30 seconds).
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Start a background WebSocket client that connects to the Carnelian server,
/// streams events, and sends them through the provided `mpsc` sender.
///
/// Connection state changes are reported via `state_tx`.
///
/// The returned `JoinHandle` runs until aborted.
pub fn start_websocket_client(
    url: String,
    event_tx: mpsc::UnboundedSender<EventEnvelope>,
    state_tx: mpsc::UnboundedSender<ConnectionState>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = BACKOFF_MIN;

        loop {
            let _ = state_tx.send(ConnectionState::Connecting);
            tracing::info!(url = %url, "WebSocket connecting");

            match tokio_tungstenite::connect_async(&url).await {
                Ok((ws_stream, _response)) => {
                    tracing::info!("WebSocket connected");
                    let _ = state_tx.send(ConnectionState::Connected);
                    backoff = BACKOFF_MIN; // reset on success

                    let (_write, mut read) = ws_stream.split();

                    loop {
                        match read.next().await {
                            Some(Ok(msg)) => {
                                if msg.is_text() {
                                    let text = msg.into_text().unwrap_or_default();
                                    match serde_json::from_str::<EventEnvelope>(&text) {
                                        Ok(envelope) => {
                                            if event_tx.send(envelope).is_err() {
                                                tracing::debug!(
                                                    "Event receiver dropped, stopping WebSocket"
                                                );
                                                return;
                                            }
                                        }
                                        Err(e) => {
                                            tracing::warn!(
                                                error = %e,
                                                "Failed to deserialize WebSocket message"
                                            );
                                        }
                                    }
                                } else if msg.is_close() {
                                    tracing::info!("WebSocket received close frame");
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                tracing::warn!(error = %e, "WebSocket read error");
                                break;
                            }
                            None => {
                                tracing::info!("WebSocket stream ended");
                                break;
                            }
                        }
                    }

                    let _ = state_tx.send(ConnectionState::Disconnected);
                }
                Err(e) => {
                    let msg = format!("{e}");
                    tracing::warn!(
                        error = %e,
                        backoff_secs = backoff.as_secs(),
                        "WebSocket connection failed"
                    );
                    let _ = state_tx.send(ConnectionState::Error(msg));
                }
            }

            // Exponential backoff before reconnecting
            tracing::debug!(backoff_secs = backoff.as_secs(), "Waiting before reconnect");
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(BACKOFF_MAX);
        }
    })
}
