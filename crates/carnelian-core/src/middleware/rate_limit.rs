//! Rate limiting middleware for CARNELIAN API
//!
//! Implements token bucket rate limiting to prevent API abuse.
//! Default configuration: 10 requests per second with burst capacity of 20.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Token bucket for rate limiting
#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_consume(&mut self) -> bool {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        self.tokens = elapsed.mul_add(self.refill_rate, self.tokens).min(self.capacity);
        self.last_refill = now;
    }
}

/// Rate limiter state
#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<IpAddr, TokenBucket>>>,
    capacity: f64,
    refill_rate: f64,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `requests_per_second` - Number of requests allowed per second
    /// * `burst_capacity` - Maximum burst size
    pub fn new(requests_per_second: u32, burst_capacity: u32) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            capacity: burst_capacity as f64,
            refill_rate: requests_per_second as f64,
        }
    }

    /// Check if request should be allowed
    pub fn check_rate_limit(&self, ip: IpAddr) -> bool {
        let mut buckets = self.buckets.lock().unwrap();

        let bucket = buckets
            .entry(ip)
            .or_insert_with(|| TokenBucket::new(self.capacity, self.refill_rate));

        bucket.try_consume()
    }

    /// Clean up old entries (call periodically)
    pub fn cleanup(&self) {
        let mut buckets = self.buckets.lock().unwrap();

        // Remove buckets that haven't been used in 5 minutes
        buckets.retain(|_, bucket| bucket.last_refill.elapsed() < Duration::from_secs(300));
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    limiter: Arc<RateLimiter>,
    req: Request,
    next: Next,
) -> Response {
    // Extract client IP from request
    let ip = extract_client_ip(&req).unwrap_or_else(|| "127.0.0.1".parse().unwrap());

    if limiter.check_rate_limit(ip) {
        next.run(req).await
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit exceeded. Please try again later.",
        )
            .into_response()
    }
}

/// Extract client IP from request
fn extract_client_ip(req: &Request) -> Option<IpAddr> {
    // Try X-Forwarded-For header first (for proxies)
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(first_ip) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse() {
                    return Some(ip);
                }
            }
        }
    }

    // Try X-Real-IP header
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            if let Ok(ip) = ip_str.parse() {
                return Some(ip);
            }
        }
    }

    // Fallback to connection info (would need to be passed in real implementation)
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_creation() {
        let bucket = TokenBucket::new(10.0, 5.0);
        assert_eq!(bucket.capacity, 10.0);
        assert_eq!(bucket.refill_rate, 5.0);
        assert_eq!(bucket.tokens, 10.0);
    }

    #[test]
    fn test_token_bucket_consume() {
        let mut bucket = TokenBucket::new(10.0, 5.0);

        assert!(bucket.try_consume());
        assert_eq!(bucket.tokens, 9.0);
    }

    #[test]
    fn test_token_bucket_exhaustion() {
        let mut bucket = TokenBucket::new(2.0, 1.0);

        assert!(bucket.try_consume());
        assert!(bucket.try_consume());
        assert!(!bucket.try_consume()); // Should fail
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(10.0, 10.0);

        // Consume all tokens
        for _ in 0..10 {
            bucket.try_consume();
        }

        // Wait for refill
        std::thread::sleep(Duration::from_millis(200));

        // Should have refilled some tokens
        assert!(bucket.try_consume());
    }

    #[test]
    fn test_rate_limiter_creation() {
        let limiter = RateLimiter::new(10, 20);
        assert_eq!(limiter.capacity, 20.0);
        assert_eq!(limiter.refill_rate, 10.0);
    }

    #[test]
    fn test_rate_limiter_check() {
        let limiter = RateLimiter::new(10, 2);
        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        assert!(limiter.check_rate_limit(ip));
        assert!(limiter.check_rate_limit(ip));
        assert!(!limiter.check_rate_limit(ip)); // Should be rate limited
    }

    #[test]
    fn test_rate_limiter_multiple_ips() {
        let limiter = RateLimiter::new(10, 2);
        let ip1: IpAddr = "127.0.0.1".parse().unwrap();
        let ip2: IpAddr = "127.0.0.2".parse().unwrap();

        assert!(limiter.check_rate_limit(ip1));
        assert!(limiter.check_rate_limit(ip1));
        assert!(!limiter.check_rate_limit(ip1));

        // Different IP should have its own bucket
        assert!(limiter.check_rate_limit(ip2));
        assert!(limiter.check_rate_limit(ip2));
    }

    #[test]
    fn test_rate_limiter_cleanup() {
        let limiter = RateLimiter::new(10, 20);
        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        limiter.check_rate_limit(ip);

        {
            let buckets = limiter.buckets.lock().unwrap();
            assert_eq!(buckets.len(), 1);
        }

        limiter.cleanup();

        {
            let buckets = limiter.buckets.lock().unwrap();
            assert_eq!(buckets.len(), 1); // Should still be there (not old enough)
        }
    }
}
