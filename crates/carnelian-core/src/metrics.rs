//! Performance metrics collection for Carnelian OS
//!
//! Tracks task execution latencies and event stream throughput using
//! sliding windows for real-time performance monitoring.
//!
//! # Metrics Collected
//!
//! - **Task Latency**: Time from task creation to execution start (P50, P95, P99)
//! - **Event Throughput**: Events published per second over 60s window
//! - **Event Stream Stats**: Buffer utilization and dropped event counts

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use uuid::Uuid;

use crate::events::EventStreamStats;

/// Maximum number of task latency samples to retain.
const MAX_LATENCY_SAMPLES: usize = 1000;

/// Type alias for the throughput sliding window.
type ThroughputWindow = Arc<RwLock<VecDeque<(DateTime<Utc>, usize)>>>;

/// Default throughput window duration (60 seconds).
const DEFAULT_WINDOW_SECS: u64 = 60;

/// A single task latency measurement.
#[derive(Debug, Clone)]
pub struct TaskLatencyMetric {
    /// Task that was measured
    pub task_id: Uuid,
    /// When the task was created
    pub created_at: DateTime<Utc>,
    /// When the task started executing
    pub started_at: DateTime<Utc>,
    /// Computed latency in milliseconds
    pub latency_ms: f64,
}

/// Aggregated latency statistics.
#[derive(Debug, Clone, Serialize)]
pub struct LatencyStats {
    pub mean_ms: f64,
    pub median_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub sample_count: usize,
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self {
            mean_ms: 0.0,
            median_ms: 0.0,
            p50_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
            sample_count: 0,
        }
    }
}

/// Snapshot of all collected metrics at a point in time.
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub task_latency: LatencyStats,
    pub event_throughput_per_sec: f64,
    pub event_stream_buffer_len: usize,
    pub event_stream_buffer_capacity: usize,
    pub event_stream_fill_percentage: f64,
    pub event_stream_total_received: usize,
    pub event_stream_total_stored: usize,
    pub event_stream_subscriber_count: usize,
    /// Average UI render duration in milliseconds (reported by the UI client).
    pub render_time_ms: f64,
    pub timestamp: DateTime<Utc>,
}

/// Thread-safe metrics collector using sliding windows.
#[derive(Debug)]
pub struct MetricsCollector {
    /// Sliding window of recent task latency measurements
    task_latencies: Arc<RwLock<VecDeque<TaskLatencyMetric>>>,
    /// Time-windowed event counts: (timestamp, count)
    event_throughput_window: ThroughputWindow,
    /// Duration of the throughput calculation window
    window_duration: Duration,
}

impl MetricsCollector {
    /// Create a new MetricsCollector with default window sizes.
    #[must_use]
    pub fn new() -> Self {
        Self {
            task_latencies: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_LATENCY_SAMPLES))),
            event_throughput_window: Arc::new(RwLock::new(VecDeque::with_capacity(1024))),
            window_duration: Duration::from_secs(DEFAULT_WINDOW_SECS),
        }
    }

    /// Record a task latency measurement.
    ///
    /// Calculates the latency between `created_at` and `started_at`, stores
    /// the result, and evicts old entries beyond `MAX_LATENCY_SAMPLES`.
    #[allow(clippy::significant_drop_tightening)]
    pub fn record_task_latency(
        &self,
        task_id: Uuid,
        created_at: DateTime<Utc>,
        started_at: DateTime<Utc>,
    ) {
        let latency_ms = (started_at - created_at).num_milliseconds().max(0) as f64;
        let metric = TaskLatencyMetric {
            task_id,
            created_at,
            started_at,
            latency_ms,
        };

        {
            let mut latencies = self.task_latencies.write().unwrap();
            latencies.push_back(metric);
            prune_old_entries(&mut latencies, MAX_LATENCY_SAMPLES);
        }
    }

    /// Record an event batch for throughput calculation.
    pub fn record_event_batch(&self, count: usize, timestamp: DateTime<Utc>) {
        let mut window = self.event_throughput_window.write().unwrap();
        window.push_back((timestamp, count));

        // Prune entries older than the window duration
        let cutoff =
            timestamp - chrono::Duration::from_std(self.window_duration).unwrap_or_default();
        while let Some(front) = window.front() {
            if front.0 < cutoff {
                window.pop_front();
            } else {
                break;
            }
        }
        drop(window);
    }

    /// Calculate aggregated task latency statistics.
    #[must_use]
    pub fn get_task_latency_stats(&self) -> LatencyStats {
        let latencies = self.task_latencies.read().unwrap();
        if latencies.is_empty() {
            return LatencyStats::default();
        }

        let mut values: Vec<f64> = latencies.iter().map(|m| m.latency_ms).collect();
        drop(latencies);
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let sum: f64 = values.iter().sum();
        let count = values.len();
        let mean_ms = sum / count as f64;
        let median_ms = calculate_percentile(&values, 50.0);
        let p50_ms = median_ms;
        let p95_ms = calculate_percentile(&values, 95.0);
        let p99_ms = calculate_percentile(&values, 99.0);

        LatencyStats {
            mean_ms,
            median_ms,
            p50_ms,
            p95_ms,
            p99_ms,
            sample_count: count,
        }
    }

    /// Calculate event throughput (events/sec) over the window duration.
    #[must_use]
    pub fn get_event_throughput(&self) -> f64 {
        let window = self.event_throughput_window.read().unwrap();
        if window.is_empty() {
            return 0.0;
        }

        let total_events: usize = window.iter().map(|(_, c)| c).sum();
        drop(window);
        let window_secs = self.window_duration.as_secs_f64();
        total_events as f64 / window_secs
    }

    /// Get a full metrics snapshot combining all collected data.
    pub fn get_snapshot(&self, event_stats: &EventStreamStats) -> MetricsSnapshot {
        MetricsSnapshot {
            task_latency: self.get_task_latency_stats(),
            event_throughput_per_sec: self.get_event_throughput(),
            event_stream_buffer_len: event_stats.buffer_len,
            event_stream_buffer_capacity: event_stats.buffer_capacity,
            event_stream_fill_percentage: event_stats.fill_percentage,
            event_stream_total_received: event_stats.total_received,
            event_stream_total_stored: event_stats.total_stored,
            event_stream_subscriber_count: event_stats.subscriber_count,
            render_time_ms: 0.0,
            timestamp: Utc::now(),
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate a percentile value from a sorted slice.
///
/// Uses linear interpolation between adjacent values.
fn calculate_percentile(sorted_values: &[f64], percentile: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    if sorted_values.len() == 1 {
        return sorted_values[0];
    }

    let rank = (percentile / 100.0) * (sorted_values.len() - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    let fraction = rank - lower as f64;

    if lower == upper {
        sorted_values[lower]
    } else {
        sorted_values[lower].mul_add(1.0 - fraction, sorted_values[upper] * fraction)
    }
}

/// Prune a deque to at most `max_size` entries by removing from the front.
fn prune_old_entries<T>(deque: &mut VecDeque<T>, max_size: usize) {
    while deque.len() > max_size {
        deque.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_calculation() {
        let collector = MetricsCollector::new();

        let base = Utc::now();
        for i in 0..100 {
            let created = base - chrono::Duration::milliseconds(1000 - i * 10);
            let started = created + chrono::Duration::milliseconds(100 + i);
            collector.record_task_latency(Uuid::new_v4(), created, started);
        }

        let stats = collector.get_task_latency_stats();
        assert_eq!(stats.sample_count, 100);
        assert!(stats.mean_ms > 0.0);
        assert!(stats.p50_ms > 0.0);
        assert!(stats.p95_ms >= stats.p50_ms);
        assert!(stats.p99_ms >= stats.p95_ms);
    }

    #[test]
    fn test_sliding_window_eviction() {
        let collector = MetricsCollector::new();

        let base = Utc::now();
        // Insert more than MAX_LATENCY_SAMPLES entries
        for i in 0..1500 {
            let created = base - chrono::Duration::milliseconds(i);
            let started = created + chrono::Duration::milliseconds(50);
            collector.record_task_latency(Uuid::new_v4(), created, started);
        }

        let latencies = collector.task_latencies.read().unwrap();
        assert!(
            latencies.len() <= MAX_LATENCY_SAMPLES,
            "Should prune to max {} entries, got {}",
            MAX_LATENCY_SAMPLES,
            latencies.len()
        );
    }

    #[test]
    fn test_throughput_calculation() {
        let collector = MetricsCollector::new();

        let now = Utc::now();
        // Record 100 events per second for 10 seconds
        for i in 0..10 {
            let ts = now - chrono::Duration::seconds(10 - i);
            collector.record_event_batch(100, ts);
        }

        let throughput = collector.get_event_throughput();
        // 1000 events over 60s window = ~16.67 events/sec
        assert!(
            throughput > 10.0,
            "Expected throughput > 10 events/sec, got {:.1}",
            throughput
        );
    }

    #[test]
    fn test_empty_metrics() {
        let collector = MetricsCollector::new();

        let stats = collector.get_task_latency_stats();
        assert_eq!(stats.sample_count, 0);
        assert_eq!(stats.mean_ms, 0.0);
        assert_eq!(stats.p50_ms, 0.0);
        assert_eq!(stats.p95_ms, 0.0);
        assert_eq!(stats.p99_ms, 0.0);

        let throughput = collector.get_event_throughput();
        assert_eq!(throughput, 0.0);
    }

    #[test]
    fn test_percentile_single_value() {
        let values = vec![42.0];
        assert_eq!(calculate_percentile(&values, 50.0), 42.0);
        assert_eq!(calculate_percentile(&values, 99.0), 42.0);
    }

    #[test]
    fn test_percentile_empty() {
        let values: Vec<f64> = vec![];
        assert_eq!(calculate_percentile(&values, 50.0), 0.0);
    }

    #[test]
    fn test_percentile_two_values() {
        let values = vec![10.0, 20.0];
        assert_eq!(calculate_percentile(&values, 0.0), 10.0);
        assert_eq!(calculate_percentile(&values, 50.0), 15.0);
        assert_eq!(calculate_percentile(&values, 100.0), 20.0);
    }
}
