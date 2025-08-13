//! In-memory rate limit storage using the governor crate.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use governor::clock::{Clock, DefaultClock};
use governor::state::keyed::DefaultKeyedStateStore;
use governor::{InsufficientCapacity, Quota, RateLimiter};
use mini_moka::sync::Cache;
use tokio::sync::Mutex;

use super::{RateLimitResult, RateLimitStorage, StorageError};

type KeyedRateLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

/// In-memory rate limit storage implementation.
pub struct InMemoryStorage {
    /// Cache of rate limiters by quota configuration.
    limiters: Cache<String, Arc<KeyedRateLimiter>>,
    /// Lock to prevent thundering herd when creating rate limiters.
    /// Maps cache key to a lock for that specific configuration.
    creation_locks: DashMap<String, Arc<Mutex<()>>>,
}

impl InMemoryStorage {
    /// Create a new in-memory storage instance.
    pub fn new() -> Self {
        let limiters = Cache::builder()
            .max_capacity(10000)
            .time_to_idle(Duration::from_secs(3600))
            .build();

        Self {
            limiters,
            creation_locks: DashMap::new(),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitStorage for InMemoryStorage {
    async fn check_and_consume(
        &self,
        key: &str,
        limit: u32,
        interval: Duration,
    ) -> Result<RateLimitResult, StorageError> {
        // Create a cache key based on limit and interval configuration.
        // We cache rate limiters by their configuration (limit + interval) rather than by the
        // actual key for efficiency. This allows multiple keys with the same rate limit
        // configuration to share a single rate limiter instance, reducing memory usage.
        //
        // The governor crate's keyed rate limiter internally tracks separate rate limit states
        // for each key, so sharing the rate limiter instance doesn't affect the per-key
        // rate limiting behavior. Each key still gets its own independent rate limit tracking.
        let interval_millis = interval.as_millis();
        let cache_key = format!("{limit}-{interval_millis}ms");

        log::debug!("Checking rate limit for key '{key}': {limit} requests allowed per {interval:?}");

        // Get or create the rate limiter for this configuration
        // First, try to get an existing limiter
        if let Some(limiter) = self.limiters.get(&cache_key) {
            log::debug!("Reusing cached rate limiter for configuration: {cache_key}");
            return self.check_rate_limit(limiter, key);
        }

        // If not found, we need to create one. Get or create a lock for this specific cache key
        // to prevent multiple threads from creating the same rate limiter
        let creation_lock = self
            .creation_locks
            .entry(cache_key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();

        // Hold the lock while creating the rate limiter
        let _guard = creation_lock.lock().await;

        // Double-check if another thread created it while we were waiting for the lock
        if let Some(limiter) = self.limiters.get(&cache_key) {
            log::debug!("Another thread created the rate limiter while waiting for lock: {cache_key}");

            // Clean up the creation lock
            drop(_guard);

            self.creation_locks.remove(&cache_key);
            return self.check_rate_limit(limiter, key);
        }

        // Create the rate limiter
        let quota = quota_from_config(limit, interval)?;
        let limiter = Arc::new(RateLimiter::keyed(quota));

        // Insert into cache
        self.limiters.insert(cache_key.clone(), limiter.clone());
        log::debug!("Created new rate limiter instance for configuration: {cache_key}");

        // Clean up the creation lock
        drop(_guard);

        self.creation_locks.remove(&cache_key);
        self.check_rate_limit(limiter, key)
    }

    async fn check_and_consume_tokens(
        &self,
        key: &str,
        tokens: u32,
        limit: u32,
        interval: Duration,
    ) -> Result<RateLimitResult, StorageError> {
        log::debug!(
            "Checking token rate limit for key '{key}': consuming {tokens} tokens out of {limit} per {interval:?}"
        );

        // Create a unique configuration cache key that includes the limit and interval
        // This allows us to reuse rate limiters for the same configuration across different keys
        let interval_millis = interval.as_millis();
        let cache_key = format!("{limit}-{interval_millis}ms");

        // Get or create the rate limiter for this configuration
        if let Some(limiter) = self.limiters.get(&cache_key) {
            log::debug!("Reusing cached rate limiter for configuration: {cache_key}");
            return self.check_token_rate_limit(limiter, key, tokens);
        }

        // Create rate limiter if it doesn't exist
        let creation_lock = self
            .creation_locks
            .entry(cache_key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();

        let _guard = creation_lock.lock().await;

        // Double-check if another thread created it while we were waiting
        if let Some(limiter) = self.limiters.get(&cache_key) {
            log::debug!("Another thread created the rate limiter while waiting for lock: {cache_key}");
            drop(_guard);
            self.creation_locks.remove(&cache_key);
            return self.check_token_rate_limit(limiter, key, tokens);
        }

        // Create the rate limiter with full burst capacity for token rate limiting
        let quota = quota_from_token_config(limit, interval)?;
        let limiter = Arc::new(RateLimiter::keyed(quota));

        // Insert into cache
        self.limiters.insert(cache_key.clone(), limiter.clone());
        log::debug!("Created new rate limiter instance for configuration: {cache_key}");

        // Clean up
        drop(_guard);
        self.creation_locks.remove(&cache_key);

        self.check_token_rate_limit(limiter, key, tokens)
    }
}

impl InMemoryStorage {
    fn check_token_rate_limit(
        &self,
        limiter: Arc<KeyedRateLimiter>,
        key: &str,
        tokens: u32,
    ) -> Result<RateLimitResult, StorageError> {
        // For token rate limiting, consume all tokens atomically using check_key_n
        let key_string = key.to_string();

        // Convert tokens to NonZeroU32
        let n = NonZeroU32::new(tokens)
            .ok_or_else(|| StorageError::Internal("Token count must be greater than zero".to_string()))?;

        // Use check_key_n to atomically check and consume multiple tokens
        match limiter.check_key_n(&key_string, n) {
            Ok(Ok(())) => {
                // All tokens were successfully consumed
                log::debug!("Token request allowed for key '{key}' - consumed {tokens} tokens within rate limit");
                Ok(RateLimitResult {
                    allowed: true,
                    retry_after: None,
                })
            }
            Ok(Err(not_until)) => {
                // Not all tokens can be consumed right now
                let retry_after = not_until.wait_time_from(DefaultClock::default().now());
                log::debug!(
                    "Token request blocked for key '{key}' - cannot consume all {tokens} tokens, retry after {retry_after:?}"
                );
                Ok(RateLimitResult {
                    allowed: false,
                    retry_after: Some(retry_after),
                })
            }
            Err(InsufficientCapacity(_)) => {
                // The rate limit is too low to ever allow this many tokens
                log::warn!(
                    "Token request for key '{key}' requires {tokens} tokens but rate limit capacity is insufficient"
                );
                // Return a permanent failure - this request can never succeed
                Ok(RateLimitResult {
                    allowed: false,
                    retry_after: None, // No point in retrying
                })
            }
        }
    }

    fn check_rate_limit(&self, limiter: Arc<KeyedRateLimiter>, key: &str) -> Result<RateLimitResult, StorageError> {
        // Check the rate limit
        match limiter.check_key(&key.to_string()) {
            Ok(_) => {
                log::debug!("Request allowed for key '{key}' - within rate limit");
                Ok(RateLimitResult {
                    allowed: true,
                    retry_after: None,
                })
            }
            Err(not_until) => {
                let retry_after = not_until.wait_time_from(DefaultClock::default().now());
                log::debug!("Request blocked for key '{key}' - rate limit exceeded, retry after {retry_after:?}");
                Ok(RateLimitResult {
                    allowed: false,
                    retry_after: Some(retry_after),
                })
            }
        }
    }
}

/// Creates a rate limiting quota configuration for request-based rate limiting.
///
/// This function configures the governor crate's token bucket algorithm for standard
/// HTTP request rate limiting. It uses a conservative burst capacity to smooth out
/// traffic patterns while preventing abuse.
///
/// # How it works
///
/// 1. **Per-second rate calculation**: Converts the limit/interval into a per-second rate.
///    For example, 100 requests per 60 seconds becomes ~1.67 requests per second.
///
/// 2. **Limited burst capacity**: Sets burst to 10% of the limit (minimum 5, capped at limit).
///    This conservative burst policy:
///    - Prevents sudden traffic spikes from overwhelming the system
///    - Smooths out request patterns over time
///    - Still allows for reasonable bursts during normal usage
///
/// 3. **Token bucket behavior**: The governor crate's token bucket:
///    - Refills at the calculated per-second rate
///    - Allows bursting up to the burst capacity
///    - Each request consumes exactly 1 token
///
/// # Burst Capacity Strategy
///
/// The 10% burst capacity is carefully chosen:
/// - **Too low** (e.g., 1): Would force perfectly uniform request spacing
/// - **Too high** (e.g., 100%): Would allow consuming the entire quota instantly
/// - **10% with minimum 5**: Balances flexibility with protection
///
/// # Example
///
/// For a configuration of 100 requests per 60 seconds:
/// - Per-second rate: ~1.67 requests/second (100/60)
/// - Burst capacity: 10 requests (10% of 100)
/// - Result: Can make 10 rapid requests, then limited to ~1.67/second
///
/// For a configuration of 30 requests per 60 seconds:
/// - Per-second rate: 0.5 requests/second (30/60)
/// - Burst capacity: 5 requests (minimum of 5, since 10% would be 3)
/// - Result: Can make 5 rapid requests, then limited to 0.5/second
///
/// # Parameters
///
/// * `limit` - Maximum number of requests allowed in the time window
/// * `interval` - Duration of the time window (e.g., 60 seconds)
///
/// # Returns
///
/// A `Quota` object configured for request-based rate limiting, or an error if the
/// parameters cannot be converted to valid NonZeroU32 values.
fn quota_from_config(limit: u32, interval: Duration) -> Result<Quota, StorageError> {
    // Convert the limit and interval into a per-second rate.
    // The governor crate operates on per-second rates internally.
    // We ensure a minimum of 1.0 to avoid zero rates which would block all requests.
    let per_second_f64 = (limit as f64 / interval.as_secs_f64()).max(1.0);
    let per_second = per_second_f64 as u32;

    // Calculate burst capacity as 10% of the limit.
    // This provides a reasonable burst allowance without allowing the entire
    // quota to be consumed instantly.
    // - .max(5): Ensures a minimum burst of 5 for low-limit configurations
    // - .min(limit): Ensures burst never exceeds the total limit
    let burst = (limit / 10).max(5).min(limit);

    log::debug!(
        "Calculating rate limit quota: {limit} requests per {interval:?}, converted to {per_second}/second with burst capacity of {burst}"
    );

    // Convert to NonZeroU32 as required by the governor crate's Quota API.
    // These conversions should never fail due to the .max(1.0) above and the
    // burst calculation guarantees, but we handle errors for safety.
    let per_second = per_second
        .try_into()
        .map_err(|_| StorageError::Internal(format!("Invalid per-second rate: {per_second}")))?;

    let burst = burst
        .try_into()
        .map_err(|_| StorageError::Internal(format!("Invalid burst size: {burst}")))?;

    // Create the quota with the calculated per-second rate and limited burst capacity.
    // This creates a token bucket that:
    // - Refills at `per_second` tokens per second
    // - Can hold up to `burst` tokens (10% of limit)
    // - Each request consumes exactly 1 token via check_key()
    let quota = Quota::per_second(per_second).allow_burst(burst);
    log::debug!("Successfully created rate limit quota configuration: {quota:?}");

    Ok(quota)
}

/// Creates a rate limiting quota configuration specifically designed for token-based rate limiting.
///
/// This function differs from `quota_from_config` in a critical way: it sets the burst capacity
/// to the full limit rather than a percentage. This is essential for token rate limiting because
/// LLM API calls often need to consume large numbers of tokens in a single request.
///
/// # How it works
///
/// 1. **Per-second rate calculation**: Converts the limit/interval into a per-second rate
///    that the governor crate uses internally. For example, 10000 tokens per 60 seconds
///    becomes ~166 tokens per second.
///
/// 2. **Full burst capacity**: Sets burst capacity equal to the full limit, allowing a
///    single request to consume all available tokens at once. This is crucial because:
///    - A GPT-4 request might consume 5000+ tokens for a complex prompt
///    - A Claude request with a large context might use 10000+ tokens
///    - Users shouldn't be blocked from making legitimate large requests
///
/// 3. **Token bucket algorithm**: The governor crate implements a token bucket that:
///    - Refills at the calculated per-second rate
///    - Allows bursting up to the full limit
///    - Tracks consumption per key (user/client)
///
/// # Example
///
/// For a configuration of 10000 tokens per 60 seconds:
/// - Per-second rate: 166 tokens/second (10000/60)
/// - Burst capacity: 10000 tokens (full limit)
/// - Result: User can consume all 10000 tokens at once, then must wait for refill
///
/// # Parameters
///
/// * `limit` - Maximum number of tokens allowed in the time window
/// * `interval` - Duration of the time window (e.g., 60 seconds)
///
/// # Returns
///
/// A `Quota` object configured for token-based rate limiting, or an error if the
/// parameters cannot be converted to valid NonZeroU32 values.
fn quota_from_token_config(limit: u32, interval: Duration) -> Result<Quota, StorageError> {
    // Convert the limit and interval into a per-second rate.
    // The governor crate works with per-second rates internally, so we need to convert
    // our "X tokens per Y seconds" into "Z tokens per second".
    // We ensure a minimum of 1.0 to avoid zero rates which would block all requests.
    let per_second_f64 = (limit as f64 / interval.as_secs_f64()).max(1.0);
    let per_second = per_second_f64 as u32;

    // CRITICAL: Use the full limit as burst capacity for token rate limiting.
    // Unlike request rate limiting (which uses 10% burst), token rate limiting needs
    // to allow consuming all available tokens in a single API call.
    // This accommodates varying prompt sizes and model requirements.
    let burst = limit;

    log::debug!(
        "Calculating token rate limit quota: {limit} tokens per {interval:?}, converted to {per_second}/second with full burst capacity of {burst}"
    );

    // Convert to NonZeroU32 as required by the governor crate's Quota API.
    // These conversions should never fail due to the .max(1.0) above, but we handle
    // errors properly for safety.
    let per_second = per_second
        .try_into()
        .map_err(|_| StorageError::Internal(format!("Invalid per-second rate: {per_second}")))?;

    let burst = burst
        .try_into()
        .map_err(|_| StorageError::Internal(format!("Invalid burst size: {burst}")))?;

    // Create the quota with the calculated per-second rate and full burst capacity.
    // This creates a token bucket that:
    // - Refills at `per_second` tokens per second
    // - Can hold up to `burst` tokens (which equals the full limit)
    // - Allows consuming multiple tokens atomically via check_key_n()
    let quota = Quota::per_second(per_second).allow_burst(burst);
    log::debug!("Successfully created token rate limit quota configuration: {quota:?}");

    Ok(quota)
}
