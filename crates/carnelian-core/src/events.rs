//! Priority-aware event streaming for 🔥 Carnelian OS
//!
//! This module implements a priority-aware ring buffer and event distribution system
//! that prevents UI freezes under load through intelligent backpressure management.
//!
//! # Architecture
//!
//! ```text
//! Publisher → PriorityRingBuffer → Broadcast Channel → WebSocket Clients
//!                    ↓
//!            Backpressure Logic
//!            (sampling/dropping)
//! ```
//!
//! # Priority Levels and Retention
//!
//! | Level | Buffer Fill | Behavior |
//! |-------|-------------|----------|
//! | ERROR | Any | Never dropped |
//! | WARN  | >90% | Dropped only if no DEBUG/TRACE available |
//! | INFO  | >75% | Dropped when buffer pressure high |
//! | DEBUG | >50% | Sampled at 1:10 ratio |
//! | TRACE | >25% | Sampled at 1:100 ratio |
//!
//! # Example
//!
//! ```ignore
//! use carnelian_core::events::EventStream;
//! use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
//! use serde_json::json;
//!
//! // Create event stream with 10,000 event capacity
//! let stream = EventStream::new(10_000, 100);
//!
//! // Subscribe (for WebSocket handler)
//! let mut rx = stream.subscribe();
//!
//! // Publish events
//! stream.publish(EventEnvelope::new(
//!     EventLevel::Info,
//!     EventType::TaskCreated,
//!     json!({"task_id": "123"})
//! ));
//!
//! // Receive events
//! while let Ok(event) = rx.recv().await {
//!     println!("Received: {:?}", event.event_type);
//! }
//! ```
//!
//! # Backpressure Behavior
//!
//! The system applies intelligent backpressure based on buffer fill level:
//!
//! - **<25% full**: All events stored
//! - **25-50% full**: TRACE events sampled at 1:100
//! - **50-75% full**: DEBUG events sampled at 1:10, TRACE at 1:100
//! - **75-90% full**: INFO events may be dropped, DEBUG at 1:10, TRACE at 1:100
//! - **>90% full**: Only ERROR and WARN retained, others aggressively dropped
//!
//! # Memory Usage
//!
//! With default settings (10,000 events, 64KB max payload):
//! - Worst case: ~640MB (all events at max payload)
//! - Typical: ~10-50MB (most events have small payloads)

use carnelian_common::types::{EventEnvelope, EventLevel};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

use crate::metrics::MetricsCollector;

/// Sampling counter for deterministic sampling
static SAMPLE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Priority-aware ring buffer for event storage
///
/// This buffer implements intelligent backpressure by:
/// - Never dropping ERROR events (unbounded error buffer)
/// - Sampling DEBUG/TRACE events under pressure
/// - Dropping WARN only after exhausting lower-priority events
/// - Tracking dropped event metrics for observability
#[derive(Debug)]
pub struct PriorityRingBuffer {
    /// Main event storage (non-ERROR events)
    buffer: VecDeque<EventEnvelope>,
    /// Maximum buffer capacity for non-ERROR events
    capacity: usize,
    /// Separate storage for ERROR events (unbounded, never dropped)
    error_buffer: Vec<EventEnvelope>,
    /// Count of dropped events per level
    dropped_counts: HashMap<EventLevel, usize>,
    /// Maximum payload size in bytes
    max_payload_bytes: usize,
    /// Total events received
    total_received: usize,
    /// Total events stored
    total_stored: usize,
}

impl PriorityRingBuffer {
    /// Default buffer capacity
    pub const DEFAULT_CAPACITY: usize = 10_000;
    /// Default max payload size (64KB)
    pub const DEFAULT_MAX_PAYLOAD_BYTES: usize = 65_536;

    /// Create a new priority ring buffer with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of non-ERROR events to store
    /// * `max_payload_bytes` - Maximum payload size before truncation
    #[must_use]
    pub fn new(capacity: usize, max_payload_bytes: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
            error_buffer: Vec::new(), // Unbounded - ERROR events are never dropped
            dropped_counts: HashMap::new(),
            max_payload_bytes,
            total_received: 0,
            total_stored: 0,
        }
    }

    /// Get the current fill percentage of the buffer (0.0 to 1.0).
    #[must_use]
    pub fn fill_percentage(&self) -> f64 {
        self.buffer.len() as f64 / self.capacity as f64
    }

    /// Get the current number of events in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len() + self.error_buffer.len()
    }

    /// Check if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty() && self.error_buffer.is_empty()
    }

    /// Get dropped event counts per level.
    #[must_use]
    pub fn dropped_counts(&self) -> &HashMap<EventLevel, usize> {
        &self.dropped_counts
    }

    /// Get total events received.
    #[must_use]
    pub fn total_received(&self) -> usize {
        self.total_received
    }

    /// Get total events stored.
    #[must_use]
    pub fn total_stored(&self) -> usize {
        self.total_stored
    }

    /// Push an event into the buffer with priority-aware backpressure.
    ///
    /// Returns `true` if the event was stored, `false` if it was dropped.
    ///
    /// # Priority Rules
    ///
    /// - ERROR events: Always stored (unbounded buffer, never dropped)
    /// - WARN events: Dropped only after exhausting lower-priority events
    /// - INFO/DEBUG/TRACE: Subject to sampling and backpressure
    pub fn push(&mut self, mut event: EventEnvelope) -> bool {
        self.total_received += 1;

        // Truncate payload if needed using configured max size
        self.truncate_event_payload(&mut event);

        // ERROR events go to separate unbounded buffer and are NEVER dropped
        if event.is_critical() {
            self.error_buffer.push(event.clone());
            self.total_stored += 1;
            tracing::debug!(
                event_level = ?event.level,
                event_type = ?event.event_type,
                event_id = ?event.event_id,
                "ERROR event stored in unbounded buffer"
            );
            return true;
        }

        // Apply sampling based on fill level and event level
        if !self.should_store(&event) {
            *self.dropped_counts.entry(event.level).or_insert(0) += 1;
            let fill_pct = self.fill_percentage() * 100.0;
            tracing::warn!(
                event_level = ?event.level,
                event_type = ?event.event_type,
                fill_percentage = format!("{:.1}%", fill_pct),
                "Event dropped due to backpressure"
            );
            return false;
        }

        // Make room if buffer is full
        if self.buffer.len() >= self.capacity {
            self.evict_lowest_priority();
        }

        tracing::debug!(
            event_level = ?event.level,
            event_type = ?event.event_type,
            event_id = ?event.event_id,
            buffer_len = self.buffer.len(),
            "Event stored successfully"
        );

        self.buffer.push_back(event);
        self.total_stored += 1;
        true
    }

    /// Truncate event payload if it exceeds the configured max size.
    fn truncate_event_payload(&self, event: &mut EventEnvelope) {
        if let Ok(serialized) = serde_json::to_string(&event.payload) {
            if serialized.len() > self.max_payload_bytes {
                event.payload = serde_json::json!({
                    "...": "payload truncated",
                    "original_size_bytes": serialized.len(),
                    "max_allowed_bytes": self.max_payload_bytes
                });
                event.truncated = true;
            }
        }
    }

    /// Determine if an event should be stored based on backpressure rules.
    ///
    /// WARN events are only dropped if there are no lower-priority events to evict.
    fn should_store(&self, event: &EventEnvelope) -> bool {
        let fill = self.fill_percentage();

        match event.level {
            EventLevel::Error => true, // Always store (handled separately)
            EventLevel::Warn => {
                // WARN events: only drop if buffer is >90% AND no lower-priority events exist
                if fill < 0.90 {
                    true
                } else {
                    // Check if there are lower-priority events we can evict instead
                    self.has_lower_priority_events(EventLevel::Warn)
                }
            }
            EventLevel::Info => fill < 0.75,
            EventLevel::Debug => {
                if fill < 0.50 {
                    true
                } else {
                    // Sample at 1:10 ratio
                    let sampled = Self::sample(10);
                    tracing::trace!(
                        event_level = ?event.level,
                        sampled = sampled,
                        ratio = "1:10",
                        "DEBUG event sampling decision"
                    );
                    sampled
                }
            }
            EventLevel::Trace => {
                if fill < 0.25 {
                    true
                } else {
                    // Sample at 1:100 ratio
                    let sampled = Self::sample(100);
                    tracing::trace!(
                        event_level = ?event.level,
                        sampled = sampled,
                        ratio = "1:100",
                        "TRACE event sampling decision"
                    );
                    sampled
                }
            }
        }
    }

    /// Check if there are events with lower priority than the given level.
    fn has_lower_priority_events(&self, level: EventLevel) -> bool {
        let target_priority = level.priority();
        self.buffer
            .iter()
            .any(|e| e.level.priority() > target_priority)
    }

    /// Deterministic sampling - returns true for 1 in N events.
    fn sample(ratio: u64) -> bool {
        let counter = SAMPLE_COUNTER.fetch_add(1, Ordering::Relaxed);
        counter % ratio == 0
    }

    /// Evict the lowest priority event from the buffer.
    fn evict_lowest_priority(&mut self) {
        // Find and remove the lowest priority (highest level value) event
        // Start from the front (oldest) and find first TRACE, then DEBUG, etc.
        let mut evict_idx = None;
        let mut lowest_priority = 0u8;

        for (idx, event) in self.buffer.iter().enumerate() {
            let priority = event.level.priority();
            if priority > lowest_priority {
                lowest_priority = priority;
                evict_idx = Some(idx);
            }
            // TRACE is lowest priority, stop searching
            if priority == 4 {
                break;
            }
        }

        if let Some(idx) = evict_idx {
            if let Some(evicted) = self.buffer.remove(idx) {
                tracing::debug!(
                    evicted_level = ?evicted.level,
                    buffer_len = self.buffer.len(),
                    "Evicted lowest priority event"
                );
                *self.dropped_counts.entry(evicted.level).or_insert(0) += 1;
            }
        } else {
            // No low-priority events, remove oldest
            if let Some(evicted) = self.buffer.pop_front() {
                tracing::debug!(
                    evicted_level = ?evicted.level,
                    buffer_len = self.buffer.len(),
                    "Evicted oldest event (no lower priority available)"
                );
                *self.dropped_counts.entry(evicted.level).or_insert(0) += 1;
            }
        }
    }

    /// Get the number of ERROR events stored.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.error_buffer.len()
    }

    /// Get all events in order (errors first, then by timestamp).
    #[must_use]
    pub fn drain_all(&mut self) -> Vec<EventEnvelope> {
        let mut events: Vec<EventEnvelope> = self.error_buffer.drain(..).collect();
        events.extend(self.buffer.drain(..));
        events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        events
    }

    /// Get recent events without removing them.
    #[must_use]
    pub fn recent(&self, count: usize) -> Vec<&EventEnvelope> {
        let mut events: Vec<&EventEnvelope> =
            self.error_buffer.iter().chain(self.buffer.iter()).collect();
        events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); // Most recent first
        events.truncate(count);
        events
    }
}

impl Default for PriorityRingBuffer {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY, Self::DEFAULT_MAX_PAYLOAD_BYTES)
    }
}

/// Event stream for publishing and subscribing to events
///
/// This wraps a `tokio::sync::broadcast` channel with a priority ring buffer
/// for event storage and backpressure management.
#[derive(Debug)]
pub struct EventStream {
    /// Ring buffer for event storage
    buffer: Arc<RwLock<PriorityRingBuffer>>,
    /// Broadcast sender for event distribution
    sender: broadcast::Sender<EventEnvelope>,
    /// Optional metrics collector for throughput tracking
    metrics: RwLock<Option<Arc<MetricsCollector>>>,
}

impl EventStream {
    /// Create a new event stream with default max payload size.
    ///
    /// # Arguments
    ///
    /// * `buffer_capacity` - Maximum events in the ring buffer
    /// * `broadcast_capacity` - Broadcast channel capacity (for slow consumers)
    #[must_use]
    pub fn new(buffer_capacity: usize, broadcast_capacity: usize) -> Self {
        Self::with_max_payload(
            buffer_capacity,
            broadcast_capacity,
            PriorityRingBuffer::DEFAULT_MAX_PAYLOAD_BYTES,
        )
    }

    /// Create a new event stream with custom max payload size.
    ///
    /// # Arguments
    ///
    /// * `buffer_capacity` - Maximum events in the ring buffer
    /// * `broadcast_capacity` - Broadcast channel capacity (for slow consumers)
    /// * `max_payload_bytes` - Maximum payload size before truncation
    #[must_use]
    pub fn with_max_payload(
        buffer_capacity: usize,
        broadcast_capacity: usize,
        max_payload_bytes: usize,
    ) -> Self {
        let (sender, _) = broadcast::channel(broadcast_capacity);
        tracing::info!(
            buffer_capacity = buffer_capacity,
            broadcast_capacity = broadcast_capacity,
            max_payload_bytes = max_payload_bytes,
            "EventStream initialized"
        );
        Self {
            buffer: Arc::new(RwLock::new(PriorityRingBuffer::new(
                buffer_capacity,
                max_payload_bytes,
            ))),
            sender,
            metrics: RwLock::new(None),
        }
    }

    /// Set a shared metrics collector for throughput tracking.
    pub fn set_metrics(&self, metrics: Arc<MetricsCollector>) {
        let mut guard = self.metrics.write().unwrap();
        *guard = Some(metrics);
    }

    /// Subscribe to the event stream.
    ///
    /// Returns a receiver that will receive all future events.
    /// If the receiver falls behind, it will miss events (broadcast behavior).
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        let rx = self.sender.subscribe();
        tracing::debug!(
            subscriber_count = self.sender.receiver_count(),
            "New event stream subscription"
        );
        rx
    }

    /// Get the number of active subscribers.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Publish an event to the stream.
    ///
    /// The event is stored in the ring buffer (with backpressure) and
    /// broadcast to all subscribers.
    ///
    /// # Returns
    ///
    /// `true` if the event was stored in the buffer, `false` if dropped due to backpressure.
    pub fn publish(&self, event: EventEnvelope) -> bool {
        let event_id = event.event_id;
        let event_level = event.level;
        let event_type = event.event_type.clone();
        let correlation_id = event.correlation_id.clone();

        let stored = {
            let mut buffer = self.buffer.write().unwrap();
            buffer.push(event.clone())
        };

        // Record event throughput metric
        if let Some(ref metrics) = *self.metrics.read().unwrap() {
            metrics.record_event_batch(1, chrono::Utc::now());
        }

        // Always attempt to broadcast, even if not stored in buffer
        // This allows real-time subscribers to see all events
        let subscriber_count = self.sender.receiver_count();
        match self.sender.send(event) {
            Ok(_) => {
                tracing::trace!(
                    event_id = ?event_id,
                    event_level = ?event_level,
                    event_type = ?event_type,
                    correlation_id = ?correlation_id,
                    subscriber_count = subscriber_count,
                    stored = stored,
                    "Event published"
                );
            }
            Err(_) => {
                tracing::debug!(
                    event_id = ?event_id,
                    event_level = ?event_level,
                    event_type = ?event_type,
                    "Event broadcast skipped (no subscribers)"
                );
            }
        }

        stored
    }

    /// Get buffer statistics for monitoring.
    #[must_use]
    pub fn stats(&self) -> EventStreamStats {
        let buffer = self.buffer.read().unwrap();
        EventStreamStats {
            buffer_len: buffer.len(),
            buffer_capacity: buffer.capacity,
            fill_percentage: buffer.fill_percentage(),
            dropped_counts: buffer.dropped_counts().clone(),
            total_received: buffer.total_received(),
            total_stored: buffer.total_stored(),
            subscriber_count: self.subscriber_count(),
        }
    }

    /// Get recent events from the buffer.
    pub fn recent_events(&self, count: usize) -> Vec<EventEnvelope> {
        let buffer = self.buffer.read().unwrap();
        buffer.recent(count).into_iter().cloned().collect()
    }
}

impl Default for EventStream {
    fn default() -> Self {
        Self::new(PriorityRingBuffer::DEFAULT_CAPACITY, 100)
    }
}

/// Statistics about the event stream for monitoring
#[derive(Debug, Clone)]
pub struct EventStreamStats {
    /// Current number of events in buffer
    pub buffer_len: usize,
    /// Maximum buffer capacity
    pub buffer_capacity: usize,
    /// Current fill percentage (0.0 to 1.0)
    pub fill_percentage: f64,
    /// Count of dropped events per level
    pub dropped_counts: HashMap<EventLevel, usize>,
    /// Total events received
    pub total_received: usize,
    /// Total events stored
    pub total_stored: usize,
    /// Number of active subscribers
    pub subscriber_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use carnelian_common::types::EventType;
    use serde_json::json;

    /// Helper to create a test event
    fn create_test_event(level: EventLevel, payload_size: usize) -> EventEnvelope {
        let payload = if payload_size > 0 {
            json!({"data": "x".repeat(payload_size)})
        } else {
            json!({})
        };
        EventEnvelope::new(level, EventType::Custom("test".to_string()), payload)
    }

    /// Helper to fill buffer to a target percentage
    fn fill_buffer_to_percentage(buffer: &mut PriorityRingBuffer, target: f64, level: EventLevel) {
        let target_count = (buffer.capacity as f64 * target) as usize;
        for _ in 0..target_count {
            buffer.push(create_test_event(level, 10));
        }
    }

    #[test]
    fn test_event_envelope_creation() {
        let event = EventEnvelope::new(
            EventLevel::Info,
            EventType::TaskCreated,
            json!({"task_id": "123"}),
        );

        assert_eq!(event.level, EventLevel::Info);
        assert!(!event.truncated);
        assert!(event.actor_id.is_none());
        assert!(event.correlation_id.is_none());
    }

    #[test]
    fn test_event_envelope_builder() {
        let correlation_id = uuid::Uuid::new_v4();
        let event = EventEnvelope::new(EventLevel::Info, EventType::TaskCreated, json!({}))
            .with_actor_id("worker-1")
            .with_correlation_id(correlation_id);

        assert_eq!(event.actor_id, Some("worker-1".to_string()));
        assert_eq!(event.correlation_id, Some(correlation_id));
    }

    #[test]
    fn test_payload_truncation() {
        let large_payload = "x".repeat(100_000); // 100KB
        let mut event = EventEnvelope::new(
            EventLevel::Info,
            EventType::Custom("test".to_string()),
            json!({"data": large_payload}),
        );

        let truncated = event.truncate_payload_if_needed();

        assert!(truncated);
        assert!(event.truncated);
        assert!(event.payload.get("...").is_some());
    }

    #[test]
    fn test_ring_buffer_capacity() {
        let mut buffer = PriorityRingBuffer::new(100, 65_536);

        // Fill beyond capacity
        for _ in 0..150 {
            buffer.push(create_test_event(EventLevel::Info, 10));
        }

        // Should not exceed capacity
        assert!(buffer.len() <= 100);
    }

    #[test]
    fn test_priority_retention_errors() {
        let mut buffer = PriorityRingBuffer::new(100, 65_536);

        // Fill with INFO events
        for _ in 0..100 {
            buffer.push(create_test_event(EventLevel::Info, 10));
        }

        // Add ERROR events
        for _ in 0..20 {
            buffer.push(create_test_event(EventLevel::Error, 10));
        }

        // ERROR events should all be retained
        let events = buffer.drain_all();
        let error_count = events
            .iter()
            .filter(|e| e.level == EventLevel::Error)
            .count();
        assert_eq!(error_count, 20);
    }

    #[test]
    fn test_backpressure_debug_sampling() {
        let mut buffer = PriorityRingBuffer::new(1000, 65_536);

        // Fill to 60%
        fill_buffer_to_percentage(&mut buffer, 0.60, EventLevel::Info);
        let initial_len = buffer.len();

        // Push 100 DEBUG events (should be sampled at ~1:10)
        for _ in 0..100 {
            buffer.push(create_test_event(EventLevel::Debug, 10));
        }

        let debug_stored = buffer.len() - initial_len;
        // Should store roughly 10% (with some variance)
        assert!(
            debug_stored < 30,
            "Expected ~10 DEBUG events, got {debug_stored}"
        );
    }

    #[test]
    fn test_backpressure_trace_sampling() {
        let mut buffer = PriorityRingBuffer::new(1000, 65_536);

        // Fill to 30%
        fill_buffer_to_percentage(&mut buffer, 0.30, EventLevel::Info);
        let initial_len = buffer.len();

        // Push 1000 TRACE events (should be sampled at ~1:100)
        for _ in 0..1000 {
            buffer.push(create_test_event(EventLevel::Trace, 10));
        }

        let trace_stored = buffer.len() - initial_len;
        // Should store roughly 1% (with some variance)
        assert!(
            trace_stored < 50,
            "Expected ~10 TRACE events, got {trace_stored}"
        );
    }

    #[test]
    fn test_dropped_event_metrics() {
        let mut buffer = PriorityRingBuffer::new(100, 65_536);

        // Fill completely
        for _ in 0..100 {
            buffer.push(create_test_event(EventLevel::Info, 10));
        }

        // Try to add more INFO events (should be dropped at >75% fill)
        for _ in 0..50 {
            buffer.push(create_test_event(EventLevel::Info, 10));
        }

        let dropped = buffer.dropped_counts();
        assert!(dropped.get(&EventLevel::Info).unwrap_or(&0) > &0);
    }

    #[tokio::test]
    async fn test_event_stream_broadcast() {
        let stream = EventStream::new(100, 10);
        let mut rx = stream.subscribe();

        let event = EventEnvelope::new(EventLevel::Info, EventType::TaskCreated, json!({}));
        stream.publish(event);

        let received = rx.recv().await.unwrap();
        assert_eq!(received.level, EventLevel::Info);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let stream = EventStream::new(100, 10);
        let mut rx1 = stream.subscribe();
        let mut rx2 = stream.subscribe();

        let event = EventEnvelope::new(EventLevel::Info, EventType::TaskCreated, json!({}));
        stream.publish(event);

        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        assert_eq!(received1.event_id, received2.event_id);
    }

    #[test]
    fn test_event_stream_stats() {
        let stream = EventStream::new(100, 10);

        for _ in 0..50 {
            stream.publish(create_test_event(EventLevel::Info, 10));
        }

        let stats = stream.stats();
        assert_eq!(stats.total_received, 50);
        assert!(stats.fill_percentage > 0.0);
    }

    #[test]
    fn test_recent_events() {
        let stream = EventStream::new(100, 10);

        for i in 0..20 {
            let mut event = create_test_event(EventLevel::Info, 10);
            event.actor_id = Some(format!("actor-{i}"));
            stream.publish(event);
        }

        let recent = stream.recent_events(5);
        assert_eq!(recent.len(), 5);
    }

    #[test]
    fn test_event_level_priority() {
        assert!(EventLevel::Error.priority() < EventLevel::Warn.priority());
        assert!(EventLevel::Warn.priority() < EventLevel::Info.priority());
        assert!(EventLevel::Info.priority() < EventLevel::Debug.priority());
        assert!(EventLevel::Debug.priority() < EventLevel::Trace.priority());
    }

    #[test]
    fn test_is_critical() {
        let error_event = create_test_event(EventLevel::Error, 10);
        let info_event = create_test_event(EventLevel::Info, 10);

        assert!(error_event.is_critical());
        assert!(!info_event.is_critical());
    }

    #[test]
    fn test_empty_buffer() {
        let buffer = PriorityRingBuffer::default();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.fill_percentage(), 0.0);
    }

    #[test]
    fn test_buffer_default() {
        let buffer = PriorityRingBuffer::default();
        assert_eq!(buffer.capacity, PriorityRingBuffer::DEFAULT_CAPACITY);
    }

    #[test]
    fn test_error_events_never_dropped() {
        let mut buffer = PriorityRingBuffer::new(100, 65_536);

        // Add many ERROR events - more than any reasonable capacity
        for _ in 0..500 {
            buffer.push(create_test_event(EventLevel::Error, 10));
        }

        // ALL ERROR events should be retained (unbounded)
        assert_eq!(buffer.error_count(), 500);

        let events = buffer.drain_all();
        let error_count = events
            .iter()
            .filter(|e| e.level == EventLevel::Error)
            .count();
        assert_eq!(error_count, 500, "All ERROR events must be retained");
    }

    #[test]
    fn test_warn_retention_with_lower_priority_events() {
        let mut buffer = PriorityRingBuffer::new(100, 65_536);

        // Fill buffer to 95% with DEBUG events (lower priority than WARN)
        for _ in 0..95 {
            buffer.push(create_test_event(EventLevel::Debug, 10));
        }

        // Now add WARN events - they should be stored because DEBUG can be evicted
        let mut warn_stored = 0;
        for _ in 0..10 {
            if buffer.push(create_test_event(EventLevel::Warn, 10)) {
                warn_stored += 1;
            }
        }

        // WARN events should be stored since there are lower-priority DEBUG events
        assert!(
            warn_stored > 0,
            "WARN events should be stored when lower-priority events exist"
        );
    }

    #[test]
    fn test_warn_dropped_when_no_lower_priority() {
        let mut buffer = PriorityRingBuffer::new(100, 65_536);

        // Fill buffer to 95% with WARN events only (no lower priority to evict)
        for _ in 0..95 {
            buffer.push(create_test_event(EventLevel::Warn, 10));
        }

        // Count how many more WARN events can be stored
        let _initial_len = buffer.len();
        for _ in 0..20 {
            buffer.push(create_test_event(EventLevel::Warn, 10));
        }

        // Some WARN events may be dropped since there's nothing lower to evict
        // and buffer is >90% full
        let final_len = buffer.len();
        // Buffer should not grow much beyond capacity
        assert!(final_len <= 100, "Buffer should respect capacity");
    }

    #[test]
    fn test_configurable_payload_truncation() {
        // Create buffer with small max payload (1KB)
        let mut buffer = PriorityRingBuffer::new(100, 1024);

        // Create event with 2KB payload
        let event = create_test_event(EventLevel::Info, 2000);
        buffer.push(event);

        // Get the stored event
        let events = buffer.drain_all();
        assert_eq!(events.len(), 1);
        assert!(
            events[0].truncated,
            "Event should be truncated at 1KB limit"
        );
        assert!(events[0].payload.get("max_allowed_bytes").is_some());
    }

    #[test]
    fn test_event_stream_with_custom_max_payload() {
        let stream = EventStream::with_max_payload(100, 10, 2048);

        // Create event with 5KB payload
        let event = create_test_event(EventLevel::Info, 5000);
        stream.publish(event);

        let recent = stream.recent_events(1);
        assert_eq!(recent.len(), 1);
        assert!(
            recent[0].truncated,
            "Event should be truncated at 2KB limit"
        );
    }
}
