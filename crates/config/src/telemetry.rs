use std::collections::BTreeMap;

use serde::Deserialize;

pub mod exporters;
pub mod logs;
pub mod metrics;
pub mod tracing;

pub use self::exporters::ExportersConfig;
pub use self::logs::LogsConfig;
pub use self::metrics::MetricsConfig;
pub use self::tracing::TracingConfig;

/// Telemetry configuration for observability
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct TelemetryConfig {
    /// Service name for telemetry identification
    service_name: Option<String>,

    /// Custom resource attributes to attach to all telemetry
    #[serde(default)]
    resource_attributes: BTreeMap<String, String>,

    /// Global exporters configuration (required, will always have a value)
    #[serde(default)]
    exporters: ExportersConfig,

    /// Tracing-specific configuration
    #[serde(default)]
    tracing: TracingConfig,

    /// Metrics-specific configuration
    #[serde(default)]
    metrics: MetricsConfig,

    /// Logs-specific configuration
    #[serde(default)]
    logs: LogsConfig,
}

impl TelemetryConfig {
    /// Get the service name
    pub fn service_name(&self) -> Option<&str> {
        self.service_name.as_deref()
    }

    /// Get the resource attributes
    pub fn resource_attributes(&self) -> &BTreeMap<String, String> {
        &self.resource_attributes
    }

    /// Get the global exporters configuration
    pub fn global_exporters(&self) -> &ExportersConfig {
        &self.exporters
    }

    /// Get the tracing configuration
    pub fn tracing(&self) -> &TracingConfig {
        &self.tracing
    }

    /// Get the exporters configuration for metrics
    /// Returns specific metrics exporters if configured, otherwise falls back to global
    pub fn metrics_exporters(&self) -> &ExportersConfig {
        self.metrics.exporters().unwrap_or(&self.exporters)
    }

    /// Get the exporters configuration for logs
    /// Returns specific logs exporters if configured, otherwise falls back to global
    pub fn logs_exporters(&self) -> &ExportersConfig {
        self.logs.exporters().unwrap_or(&self.exporters)
    }

    /// Get the exporters configuration for traces
    /// Returns specific trace exporters if configured, otherwise falls back to global
    pub fn traces_exporters(&self) -> &ExportersConfig {
        self.tracing.exporters().unwrap_or(&self.exporters)
    }

    /// Get the effective OTLP configuration for metrics
    /// Returns metrics-specific config if set and enabled, otherwise falls back to global config
    pub fn metrics_otlp_config(&self) -> Option<&exporters::OtlpExporterConfig> {
        // Check metrics-specific config first
        if let Some(metrics_exporters) = self.metrics.exporters()
            && metrics_exporters.otlp.enabled
        {
            return Some(&metrics_exporters.otlp);
        }

        // Fall back to global config
        if self.exporters.otlp.enabled {
            Some(&self.exporters.otlp)
        } else {
            None
        }
    }

    /// Get the effective OTLP configuration for traces
    /// Returns traces-specific config if set and enabled, otherwise falls back to global config
    pub fn traces_otlp_config(&self) -> Option<&exporters::OtlpExporterConfig> {
        // Check traces-specific config first
        if let Some(traces_exporters) = self.tracing.exporters()
            && traces_exporters.otlp.enabled
        {
            return Some(&traces_exporters.otlp);
        }

        // Fall back to global config
        if self.exporters.otlp.enabled {
            Some(&self.exporters.otlp)
        } else {
            None
        }
    }
}
