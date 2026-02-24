//! Channel adapters for 🔥 Carnelian OS
//!
//! This crate provides Telegram and Discord bot adapters that integrate with
//! the Carnelian session management, event streaming, and capability-based
//! security systems. Each adapter implements a pairing flow, rate limiting,
//! spam detection, and trust-level classification.
//!
//! ## Architecture
//!
//! ```text
//! User ──► Bot API ──► Adapter ──► RateLimiter ──► SpamDetector
//!                                      │
//!                         ┌────────────┼────────────┐
//!                         ▼            ▼            ▼
//!                   PolicyEngine  SessionManager  EventStream
//!                         │            │            │
//!                         └────────────┼────────────┘
//!                                      ▼
//!                                   Database
//! ```
//!
//! ## Modules
//!
//! - [`types`] — Core channel types: `ChannelType`, `TrustLevel`, `ChannelSession`
//! - [`rate_limiter`] — Per-channel-user rate limiting via `governor`
//! - [`spam_detector`] — Message frequency and duplicate content scoring
//! - [`telegram`] — Telegram bot adapter using `teloxide`
//! - [`discord`] — Discord bot adapter using `serenity`
//! - [`db`] — Database operations for `channel_sessions` table
//! - [`config`] — Adapter configuration and credential management
//! - [`testing`] — Mock adapters and test utilities

pub mod config;
pub mod db;
pub mod discord;
pub mod rate_limiter;
pub mod spam_detector;
pub mod telegram;
pub mod testing;
pub mod types;

pub use config::AdapterConfig;
pub use types::{ChannelConfig, ChannelSession, ChannelType, TrustLevel};

// Re-export the ChannelAdapter trait from carnelian-common so both
// carnelian-core and carnelian-adapters share the same trait definition
// without introducing a cyclic dependency.
pub use carnelian_common::ChannelAdapter;

/// Events emitted by channel adapters for the `EventStream`.
pub mod events {
    /// Custom event type string for channel connection established.
    pub const CHANNEL_CONNECTED: &str = "ChannelConnected";
    /// Custom event type string for channel disconnection.
    pub const CHANNEL_DISCONNECTED: &str = "ChannelDisconnected";
    /// Custom event type string for incoming message received.
    pub const CHANNEL_MESSAGE_RECEIVED: &str = "ChannelMessageReceived";
    /// Custom event type string for outgoing message sent.
    pub const CHANNEL_MESSAGE_SENT: &str = "ChannelMessageSent";
    /// Custom event type string for pairing completion.
    pub const CHANNEL_PAIRED: &str = "ChannelPaired";
    /// Custom event type string for session unpairing.
    pub const CHANNEL_UNPAIRED: &str = "ChannelUnpaired";
    /// Custom event type string for rate limit hit.
    pub const CHANNEL_RATE_LIMITED: &str = "ChannelRateLimited";
    /// Custom event type string for spam detection.
    pub const CHANNEL_SPAM_DETECTED: &str = "ChannelSpamDetected";
}
