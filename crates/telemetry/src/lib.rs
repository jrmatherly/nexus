//! Telemetry library for Nexus
//!
//! Provides OpenTelemetry metrics, tracing, and logging integration.

pub mod metrics;

use config::TelemetryConfig;
use opentelemetry_sdk::metrics::SdkMeterProvider;

// Re-export common OpenTelemetry types for metrics
pub use opentelemetry::{
    KeyValue,
    metrics::{
        Counter, Gauge, Histogram, Meter, ObservableCounter, ObservableGauge, ObservableUpDownCounter, UpDownCounter,
    },
};

/// Guard that ensures proper cleanup of telemetry resources
pub struct TelemetryGuard {
    meter_provider: SdkMeterProvider,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Err(e) = self.meter_provider.shutdown() {
            log::error!("Failed to shutdown meter provider: {e}");
        }
    }
}

/// Initialize telemetry with the given configuration
///
/// Returns a guard that should be kept alive for the duration of the application.
/// When the guard is dropped, telemetry resources will be cleaned up.
pub async fn init(config: &TelemetryConfig) -> anyhow::Result<TelemetryGuard> {
    // Initialize metrics if enabled
    let meter_provider = metrics::init_metrics(config).await?;

    Ok(TelemetryGuard { meter_provider })
}

/// Get the global meter for recording metrics
pub fn meter(name: &'static str) -> Meter {
    opentelemetry::global::meter(name)
}

/// Create a counter metric using the global meter
pub fn counter<T>(name: T) -> Counter<u64>
where
    T: Into<std::borrow::Cow<'static, str>>,
{
    meter("nexus").u64_counter(name).build()
}

/// Create a histogram metric for measuring distributions using the global meter
pub fn histogram<T>(name: T) -> Histogram<f64>
where
    T: Into<std::borrow::Cow<'static, str>>,
{
    meter("nexus").f64_histogram(name).build()
}

/// Create an up-down counter that can increase or decrease using the global meter
pub fn updown_counter<T>(name: T) -> UpDownCounter<i64>
where
    T: Into<std::borrow::Cow<'static, str>>,
{
    meter("nexus").i64_up_down_counter(name).build()
}
