//! In-memory rate limit storage using the governor crate.

use std::borrow::Cow;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use governor::clock::{Clock, DefaultClock};
use governor::state::keyed::DefaultKeyedStateStore;
use governor::{InsufficientCapacity, Quota, RateLimiter};
use mini_moka::sync::Cache;
use tokio::sync::Mutex;

use super::{RateLimitContext, RateLimitResult, RateLimitStorage, StorageError, TokenRateLimitContext};

type KeyedRateLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

/// Default cache capacity for rate limiters
const DEFAULT_CACHE_CAPACITY: u64 = 1_000;

/// Default time-to-idle for cached rate limiters (1 hour)
const DEFAULT_CACHE_TTL_SECS: u64 = 3600;

/// Time-to-idle for creation locks (5 minutes)
/// Locks are automatically removed after this period of inactivity
const CREATION_LOCK_TTL_SECS: u64 = 300;

/// Configuration for creating a rate limiter.
struct LimiterConfig<'a> {
    cache_key: &'a str,
    limit: u32,
    interval: Duration,
    limiter_type: LimiterType,
}

/// Type of rate limiter to create.
enum LimiterType {
    /// Standard request-based rate limiting (10% burst capacity)
    Request,
    /// Token-based rate limiting (full burst capacity)
    Token,
}

impl<'a> LimiterConfig<'a> {
    /// Create a new configuration for request-based rate limiting.
    fn for_requests(cache_key: &'a str, limit: u32, interval: Duration) -> Self {
        Self {
            cache_key,
            limit,
            interval,
            limiter_type: LimiterType::Request,
        }
    }

    /// Create a new configuration for token-based rate limiting.
    fn for_tokens(cache_key: &'a str, limit: u32, interval: Duration) -> Self {
        Self {
            cache_key,
            limit,
            interval,
            limiter_type: LimiterType::Token,
        }
    }

    /// Get the appropriate quota configuration based on the limiter type.
    fn create_quota(&self) -> Result<Quota, StorageError> {
        match self.limiter_type {
            LimiterType::Request => quota_from_config(self.limit, self.interval),
            LimiterType::Token => quota_from_token_config(self.limit, self.interval),
        }
    }
}

/// In-memory rate limit storage implementation.
pub struct InMemoryStorage {
    /// Cache of rate limiters by quota configuration.
    limiters: Cache<String, Arc<KeyedRateLimiter>>,
    /// Cache of locks to prevent thundering herd when creating rate limiters.
    /// Automatically cleaned up after CREATION_LOCK_TTL_SECS of inactivity.
    creation_locks: Cache<String, Arc<Mutex<()>>>,
    /// Shared clock instance to avoid repeated instantiation
    clock: DefaultClock,
}

impl InMemoryStorage {
    /// Create a new in-memory storage instance.
    pub fn new() -> Self {
        let limiters = Cache::builder()
            .max_capacity(DEFAULT_CACHE_CAPACITY)
            .time_to_idle(Duration::from_secs(DEFAULT_CACHE_TTL_SECS))
            .build();

        let creation_locks = Cache::builder()
            // Much smaller capacity since these are temporary locks
            .max_capacity(200)
            .time_to_idle(Duration::from_secs(CREATION_LOCK_TTL_SECS))
            .build();

        Self {
            limiters,
            creation_locks,
            clock: DefaultClock::default(),
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
        context: &RateLimitContext<'_>,
        limit: u32,
        interval: Duration,
    ) -> Result<RateLimitResult, StorageError> {
        // Create keys for cache lookup and rate limit tracking
        let (base_cache_key, tracking_key) = self.generate_context_keys(context);

        // Include limit and interval in cache key to ensure each unique configuration gets its own limiter
        let cache_key = format!("{}:limit:{}:interval:{}", base_cache_key, limit, interval.as_secs());

        log::debug!("Checking rate limit for key '{tracking_key}': {limit} requests allowed per {interval:?}");

        // Get or create the rate limiter for this cache key
        let config = LimiterConfig::for_requests(&cache_key, limit, interval);
        let limiter = self.get_or_create_limiter(config).await?;

        self.check_rate_limit(limiter, tracking_key.as_ref())
    }

    async fn check_and_consume_tokens(
        &self,
        context: &TokenRateLimitContext<'_>,
        tokens: u32,
        limit: u32,
        interval: Duration,
    ) -> Result<RateLimitResult, StorageError> {
        // Create cache key for the rate limiter instance
        // Include limit and interval to ensure each unique configuration gets its own limiter
        let cache_key = match context.model {
            Some(model) => {
                format!(
                    "provider:{}:model:{}:limit:{}:interval:{}",
                    context.provider,
                    model,
                    limit,
                    interval.as_secs()
                )
            }
            _ => format!(
                "provider:{}:limit:{}:interval:{}",
                context.provider,
                limit,
                interval.as_secs()
            ),
        };

        // Create tracking key for this specific client
        let tracking_key = format!(
            "token:{}:{}:{}:{}",
            context.client_id,
            context.group.unwrap_or("default"),
            context.provider,
            context.model.unwrap_or("default")
        );

        log::debug!(
            "Checking token rate limit for client '{}': consuming {} tokens out of {} per {:?}",
            context.client_id,
            tokens,
            limit,
            interval
        );

        // Get or create the rate limiter for this cache key
        let config = LimiterConfig::for_tokens(&cache_key, limit, interval);
        let limiter = self.get_or_create_limiter(config).await?;

        self.check_token_rate_limit(limiter, &tracking_key, tokens)
    }
}

impl InMemoryStorage {
    /// Generate cache and tracking keys for rate limit contexts.
    /// Returns (cache_key, tracking_key) as Cow to avoid unnecessary allocations.
    fn generate_context_keys<'a>(&self, context: &'a RateLimitContext<'a>) -> (Cow<'a, str>, Cow<'a, str>) {
        match context {
            RateLimitContext::Global => (Cow::Borrowed("global"), Cow::Borrowed("global")),
            RateLimitContext::PerIp { ip } => (Cow::Borrowed("per_ip"), Cow::Owned(format!("ip:{ip}"))),
            RateLimitContext::PerServer { server } => {
                let key: Cow<'a, str> = Cow::Owned(format!("server:{server}"));
                (key.clone(), key)
            }
            RateLimitContext::PerTool { server, tool } => {
                let key: Cow<'a, str> = Cow::Owned(format!("server:{server}:tool:{tool}"));
                (key.clone(), key)
            }
        }
    }

    /// Get or create a rate limiter based on the provided configuration.
    /// This method ensures thread-safe creation with double-checked locking.
    async fn get_or_create_limiter(&self, config: LimiterConfig<'_>) -> Result<Arc<KeyedRateLimiter>, StorageError> {
        // Try to get existing rate limiter
        let cache_key = config.cache_key.to_string();

        if let Some(limiter) = self.limiters.get(&cache_key) {
            log::trace!("Reusing cached rate limiter for configuration: {}", config.cache_key);

            return Ok(limiter);
        }

        // Get or create a lock for this specific cache key
        // The cache will automatically clean up old locks after TTL
        let creation_lock = self.creation_locks.get(&cache_key).unwrap_or_else(|| {
            let lock = Arc::new(Mutex::new(()));
            self.creation_locks.insert(cache_key.clone(), lock.clone());
            lock
        });

        let _guard = creation_lock.lock().await;

        // Double-check if another thread created it while we were waiting
        if let Some(limiter) = self.limiters.get(&cache_key) {
            log::trace!(
                "Another thread created the rate limiter while waiting for lock: {}",
                config.cache_key
            );

            // Lock will be automatically cleaned up by cache TTL
            return Ok(limiter);
        }

        // Create new rate limiter with appropriate quota configuration
        let quota = config.create_quota()?;
        let limiter = Arc::new(RateLimiter::keyed(quota));

        self.limiters.insert(cache_key.clone(), limiter.clone());

        log::debug!(
            "Created new rate limiter instance for configuration: {}",
            config.cache_key
        );

        // Lock will be automatically cleaned up by cache TTL
        // No need for manual cleanup - the cache handles it
        Ok(limiter)
    }

    fn check_token_rate_limit(
        &self,
        limiter: Arc<KeyedRateLimiter>,
        key: &str,
        tokens: u32,
    ) -> Result<RateLimitResult, StorageError> {
        // Convert tokens to NonZeroU32
        let n = NonZeroU32::new(tokens)
            .ok_or_else(|| StorageError::Internal("Token count must be greater than zero".to_string()))?;

        // Use check_key_n to atomically check and consume multiple tokens
        // Note: No string allocation needed - check_key_n accepts &str
        self.check_limiter_n(limiter, key, n)
    }

    fn check_rate_limit(&self, limiter: Arc<KeyedRateLimiter>, key: &str) -> Result<RateLimitResult, StorageError> {
        // Governor expects String keys, but we can reuse the same String
        let key_string = key.to_string();
        match limiter.check_key(&key_string) {
            Ok(_) => {
                log::trace!("Request allowed for key '{key}' - within rate limit");

                Ok(RateLimitResult {
                    allowed: true,
                    retry_after: None,
                })
            }
            Err(not_until) => {
                let retry_after = not_until.wait_time_from(self.clock.now());
                log::debug!("Request blocked for key '{key}' - rate limit exceeded, retry after {retry_after:?}");

                Ok(RateLimitResult {
                    allowed: false,
                    retry_after: Some(retry_after),
                })
            }
        }
    }

    /// Generic rate limit check for N tokens/requests.
    /// Consolidates the logic for both single and multi-token checks.
    fn check_limiter_n(
        &self,
        limiter: Arc<KeyedRateLimiter>,
        key: &str,
        n: NonZeroU32,
    ) -> Result<RateLimitResult, StorageError> {
        // Governor expects String keys
        let key_string = key.to_string();
        match limiter.check_key_n(&key_string, n) {
            Ok(Ok(())) => {
                log::trace!("Request allowed for key '{key}' - consumed {n} tokens within rate limit");

                Ok(RateLimitResult {
                    allowed: true,
                    retry_after: None,
                })
            }
            Ok(Err(not_until)) => {
                let retry_after = not_until.wait_time_from(self.clock.now());
                log::debug!("Request blocked for key '{key}' - cannot consume {n} tokens, retry after {retry_after:?}");

                Ok(RateLimitResult {
                    allowed: false,
                    retry_after: Some(retry_after),
                })
            }
            Err(InsufficientCapacity(_)) => {
                // The rate limit capacity is insufficient for this request size.
                // This means the request requires more tokens than the bucket can ever hold.
                log::warn!("Request for key '{key}' requires {n} tokens but exceeds rate limit capacity");

                Ok(RateLimitResult {
                    allowed: false,
                    retry_after: None, // No retry possible - request size exceeds capacity
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
    log::trace!("Successfully created rate limit quota configuration: {quota:?}");

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

    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "Calculating token rate limit quota: {limit} tokens per {interval:?}, converted to {per_second}/second with full burst capacity of {burst}"
        );
    }

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
    log::trace!("Successfully created token rate limit quota configuration: {quota:?}");

    Ok(quota)
}
