//! Spam detection with scoring algorithm.
//!
//! Tracks message frequency, duplicate content, and command spam per channel
//! user. Returns a score in the range `[0.0, 1.0]` where values above the
//! configured threshold (default 0.8) indicate likely spam.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::json;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::EventStream;

use crate::events;

/// Per-user spam score state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpamScore {
    /// Cumulative spam score in `[0.0, 1.0]`.
    pub score: f32,
    /// Timestamp of the last message (serialized as epoch millis for portability).
    #[serde(skip)]
    pub last_message_time: Option<Instant>,
    /// Count of consecutive duplicate messages.
    pub duplicate_count: u32,
    /// Count of commands issued in the current window.
    pub command_count: u32,
    /// Hash of the last message content for duplicate detection.
    #[serde(skip)]
    last_content_hash: u64,
    /// Total messages in the current scoring window.
    pub message_count: u32,
}

impl Default for SpamScore {
    fn default() -> Self {
        Self {
            score: 0.0,
            last_message_time: None,
            duplicate_count: 0,
            command_count: 0,
            last_content_hash: 0,
            message_count: 0,
        }
    }
}

/// Spam detector that maintains per-user scoring state.
pub struct SpamDetector {
    scores: DashMap<String, SpamScore>,
    /// Score threshold above which a message is considered spam.
    threshold: f32,
    /// TTL for score entries — entries older than this are cleaned up.
    ttl: Duration,
    event_stream: Option<Arc<EventStream>>,
}

impl SpamDetector {
    /// Create a new spam detector.
    ///
    /// - `threshold`: score above which spam is flagged (default recommendation: 0.8)
    /// - `ttl`: how long to keep score entries before cleanup (default: 1 hour)
    #[must_use]
    pub fn new(threshold: f32, ttl: Duration, event_stream: Option<Arc<EventStream>>) -> Self {
        Self {
            scores: DashMap::new(),
            threshold,
            ttl,
            event_stream,
        }
    }

    /// Update the spam score for a channel user and return the new score.
    ///
    /// The scoring algorithm considers:
    /// - **Message frequency**: rapid-fire messages increase the score
    /// - **Duplicate content**: repeated identical messages increase the score
    /// - **Command spam**: excessive command usage increases the score
    /// - **Decay**: scores decay over time when the user is idle
    #[must_use]
    pub fn update_score(
        &self,
        channel_type: &str,
        channel_user_id: &str,
        message_content: &str,
    ) -> f32 {
        let key = format!("{channel_type}:{channel_user_id}");
        let content_hash = simple_hash(message_content);
        let is_command = message_content.starts_with('/');
        let now = Instant::now();

        let mut entry = self.scores.entry(key).or_default();
        let state = entry.value_mut();

        // Decay: reduce score based on time since last message
        if let Some(last_time) = state.last_message_time {
            let elapsed = now.duration_since(last_time);
            let decay = (elapsed.as_secs_f32() / 60.0) * 0.1; // 0.1 per minute
            state.score = (state.score - decay).max(0.0);
        }

        state.message_count += 1;

        // Frequency penalty: messages within 2 seconds of each other
        if let Some(last_time) = state.last_message_time {
            let elapsed = now.duration_since(last_time);
            if elapsed < Duration::from_secs(2) {
                state.score += 0.15;
            } else if elapsed < Duration::from_secs(5) {
                state.score += 0.05;
            }
        }

        // Duplicate content penalty
        if content_hash == state.last_content_hash && state.last_content_hash != 0 {
            state.duplicate_count += 1;
            #[allow(clippy::cast_precision_loss)]
            let dup_penalty = (state.duplicate_count as f32).min(3.0);
            state.score += 0.2 * dup_penalty;
        } else {
            state.duplicate_count = 0;
        }

        // Command spam penalty
        if is_command {
            state.command_count += 1;
            if state.command_count > 5 {
                state.score += 0.1;
            }
        }

        // Clamp to [0.0, 1.0]
        state.score = state.score.clamp(0.0, 1.0);
        state.last_message_time = Some(now);
        state.last_content_hash = content_hash;

        let final_score = state.score;
        let duplicate_count = state.duplicate_count;
        let command_count = state.command_count;
        
        // Drop entry early to release lock
        drop(entry);

        // Emit spam detection event if above threshold
        if final_score >= self.threshold {
            if let Some(ref stream) = self.event_stream {
                stream.publish(EventEnvelope::new(
                    EventLevel::Warn,
                    EventType::Custom(events::CHANNEL_SPAM_DETECTED.to_string()),
                    json!({
                        "channel_type": channel_type,
                        "channel_user_id": channel_user_id,
                        "spam_score": final_score,
                        "threshold": self.threshold,
                        "duplicate_count": duplicate_count,
                        "command_count": command_count,
                    }),
                ));
            }
        }

        final_score
    }

    /// Returns `true` if the given score exceeds the spam threshold.
    #[must_use]
    pub fn is_spam(&self, score: f32) -> bool {
        score >= self.threshold
    }

    /// Returns the configured spam threshold.
    #[must_use]
    pub const fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Remove score state for a specific user.
    pub fn remove(&self, channel_type: &str, channel_user_id: &str) {
        let key = format!("{channel_type}:{channel_user_id}");
        self.scores.remove(&key);
    }

    /// Clean up expired score entries (older than TTL since last message).
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        self.scores.retain(|_key, score| {
            score
                .last_message_time
                .is_some_and(|t| now.duration_since(t) < self.ttl)
        });
    }

    /// Clear all stored scores.
    pub fn clear(&self) {
        self.scores.clear();
    }
}

/// Simple non-cryptographic hash for duplicate content detection.
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u64::from(byte));
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spam_score_increases_on_duplicates() {
        let detector = SpamDetector::new(0.8, Duration::from_secs(3600), None);
        let s1 = detector.update_score("telegram", "user1", "hello");
        let s2 = detector.update_score("telegram", "user1", "hello");
        assert!(s2 > s1, "Duplicate messages should increase spam score");
    }

    #[test]
    fn test_is_spam_threshold() {
        let detector = SpamDetector::new(0.5, Duration::from_secs(3600), None);
        assert!(!detector.is_spam(0.3));
        assert!(detector.is_spam(0.5));
        assert!(detector.is_spam(0.9));
    }
}
