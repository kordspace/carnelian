//! Per-channel-user rate limiting using the `governor` crate.
//!
//! Each unique `(channel_type, channel_user_id)` pair gets its own rate limiter
//! with limits determined by the user's trust level. When a rate limit is
//! exceeded, a `ChannelRateLimited` event is emitted to the `EventStream`.

use std::num::NonZeroU32;
use std::sync::Arc;

use dashmap::DashMap;
use governor::{Quota, RateLimiter as GovRateLimiter, clock::DefaultClock, state::InMemoryState};
use serde_json::json;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_core::EventStream;

use crate::events;
use crate::types::TrustLevel;

/// Composite key for per-user rate limiters.
type LimiterKey = String;

/// A single-cell rate limiter (token bucket with 1 token per cell).
type CellLimiter = GovRateLimiter<governor::state::NotKeyed, InMemoryState, DefaultClock>;

/// Per-channel-user rate limiter.
///
/// Maintains a `DashMap` of individual `governor` rate limiters keyed by
/// `"{channel_type}:{channel_user_id}"`. Limits are determined by the
/// caller-supplied `TrustLevel`.
pub struct RateLimiter {
    limiters: DashMap<LimiterKey, Arc<CellLimiter>>,
    event_stream: Option<Arc<EventStream>>,
}

impl RateLimiter {
    /// Create a new rate limiter with an optional event stream for emitting
    /// rate-limit events.
    #[must_use]
    pub fn new(event_stream: Option<Arc<EventStream>>) -> Self {
        Self {
            limiters: DashMap::new(),
            event_stream,
        }
    }

    /// Check whether the given channel user is within their rate limit.
    ///
    /// Returns `Ok(())` if the request is allowed, or `Err(RateLimitError)`
    /// if the limit has been exceeded.
    ///
    /// # Errors
    ///
    /// Returns `RateLimitError` if the rate limit is exceeded.
    ///
    /// # Panics
    ///
    /// Never panics in practice - the `unwrap()` calls have safe fallbacks.
    pub fn check_rate_limit(
        &self,
        channel_type: &str,
        channel_user_id: &str,
        trust_level: TrustLevel,
    ) -> Result<(), RateLimitError> {
        let key = format!("{channel_type}:{channel_user_id}");
        let limiter = self
            .limiters
            .entry(key)
            .or_insert_with(|| {
                let per_minute = trust_level.rate_limit_per_minute();
                let quota = Quota::per_minute(
                    NonZeroU32::new(per_minute).unwrap_or(NonZeroU32::new(1).unwrap()),
                );
                Arc::new(GovRateLimiter::direct(quota))
            })
            .clone();

        match limiter.check() {
            Ok(()) => Ok(()),
            Err(_not_until) => {
                // Emit rate-limit event
                if let Some(ref stream) = self.event_stream {
                    stream.publish(EventEnvelope::new(
                        EventLevel::Warn,
                        EventType::Custom(events::CHANNEL_RATE_LIMITED.to_string()),
                        json!({
                            "channel_type": channel_type,
                            "channel_user_id": channel_user_id,
                            "trust_level": trust_level.as_str(),
                            "limit_per_minute": trust_level.rate_limit_per_minute(),
                        }),
                    ));
                }

                Err(RateLimitError {
                    channel_type: channel_type.to_string(),
                    channel_user_id: channel_user_id.to_string(),
                    limit_per_minute: trust_level.rate_limit_per_minute(),
                })
            }
        }
    }

    /// Remove the rate limiter for a specific channel user (e.g., on unpair).
    pub fn remove(&self, channel_type: &str, channel_user_id: &str) {
        let key = format!("{channel_type}:{channel_user_id}");
        self.limiters.remove(&key);
    }

    /// Clear all stored rate limiters.
    pub fn clear(&self) {
        self.limiters.clear();
    }
}

/// Error returned when a rate limit is exceeded.
#[derive(Debug, Clone)]
pub struct RateLimitError {
    pub channel_type: String,
    pub channel_user_id: String,
    pub limit_per_minute: u32,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Rate limit exceeded for {}:{} ({}/min)",
            self.channel_type, self.channel_user_id, self.limit_per_minute
        )
    }
}

impl std::error::Error for RateLimitError {}
