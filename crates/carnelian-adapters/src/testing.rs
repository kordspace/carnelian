//! Testing utilities for channel adapters.
//!
//! Provides mock adapters and helper functions for integration tests
//! without requiring real bot tokens.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use uuid::Uuid;

use crate::ChannelAdapter;
use crate::types::{ChannelSession, ChannelType, TrustLevel};

/// Mock channel adapter for testing.
///
/// Records sent messages and simulates adapter lifecycle without
/// connecting to any real bot API.
pub struct MockChannelAdapter {
    adapter_name: String,
    running: Arc<AtomicBool>,
    /// Messages sent through `send_message`, accessible for assertions.
    pub sent_messages: Arc<tokio::sync::Mutex<Vec<(String, String)>>>,
}

impl MockChannelAdapter {
    /// Create a new mock adapter with the given name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            adapter_name: name.to_string(),
            running: Arc::new(AtomicBool::new(false)),
            sent_messages: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    /// Returns all messages sent through this mock adapter.
    pub async fn get_sent_messages(&self) -> Vec<(String, String)> {
        self.sent_messages.lock().await.clone()
    }
}

#[async_trait]
impl ChannelAdapter for MockChannelAdapter {
    fn name(&self) -> &str {
        &self.adapter_name
    }

    async fn start(&self) -> anyhow::Result<()> {
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send_message(&self, channel_user_id: &str, text: &str) -> anyhow::Result<()> {
        self.sent_messages
            .lock()
            .await
            .push((channel_user_id.to_string(), text.to_string()));
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Create a test `ChannelSession` with sensible defaults.
#[must_use]
pub fn create_test_channel_session(
    channel_type: ChannelType,
    channel_user_id: &str,
    trust_level: TrustLevel,
) -> ChannelSession {
    ChannelSession {
        session_id: Uuid::now_v7(),
        channel_type: channel_type.as_str().to_string(),
        channel_user_id: channel_user_id.to_string(),
        trust_level: trust_level.as_str().to_string(),
        identity_id: None,
        created_at: chrono::Utc::now(),
        last_seen_at: chrono::Utc::now(),
        metadata: serde_json::json!({}),
    }
}

/// Simulate an incoming message for testing handler logic.
///
/// Returns the message content and metadata that would be passed to handlers.
#[must_use]
pub fn simulate_incoming_message(
    channel_type: ChannelType,
    channel_user_id: &str,
    content: &str,
) -> SimulatedMessage {
    SimulatedMessage {
        channel_type,
        channel_user_id: channel_user_id.to_string(),
        content: content.to_string(),
        correlation_id: Uuid::now_v7(),
    }
}

/// A simulated incoming message for testing.
#[derive(Debug, Clone)]
pub struct SimulatedMessage {
    pub channel_type: ChannelType,
    pub channel_user_id: String,
    pub content: String,
    pub correlation_id: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_adapter_lifecycle() {
        let adapter = MockChannelAdapter::new("test");
        assert!(!adapter.is_running());

        adapter.start().await.unwrap();
        assert!(adapter.is_running());

        adapter.send_message("user1", "hello").await.unwrap();
        adapter.send_message("user2", "world").await.unwrap();

        let messages = adapter.get_sent_messages().await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0], ("user1".to_string(), "hello".to_string()));

        adapter.stop().await.unwrap();
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_create_test_channel_session() {
        let session =
            create_test_channel_session(ChannelType::Telegram, "12345", TrustLevel::Conversational);
        assert_eq!(session.channel_type, "telegram");
        assert_eq!(session.channel_user_id, "12345");
        assert_eq!(session.trust_level, "conversational");
    }
}
