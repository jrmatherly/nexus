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

impl TelemetryGuard {
    /// Force flush all pending metrics immediately
    /// Useful for tests to ensure metrics are exported before assertions
    pub fn force_flush(&self) -> anyhow::Result<()> {
        self.meter_provider
            .force_flush()
            .map_err(|e| anyhow::anyhow!("Failed to flush metrics: {}", e))
    }
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
