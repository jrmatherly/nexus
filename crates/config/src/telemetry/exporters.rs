use duration_str::deserialize_duration;
use serde::Deserialize;
use std::time::Duration;
use url::Url;

/// Exporters configuration for telemetry
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ExportersConfig {
    /// OTLP exporter configuration (when present, endpoint is required)
    pub otlp: Option<OtlpExporterConfig>,
}

/// OTLP exporter configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OtlpExporterConfig {
    /// Whether this exporter is enabled
    #[serde(default)]
    pub enabled: bool,

    /// OTLP endpoint URL (required when enabled)
    pub endpoint: Url,

    /// Protocol to use (grpc or http)
    #[serde(default)]
    pub protocol: OtlpProtocol,

    /// Request timeout
    #[serde(deserialize_with = "deserialize_duration", default = "default_timeout")]
    pub timeout: Duration,

    /// Batch export configuration
    #[serde(default)]
    pub batch_export: BatchExportConfig,
}

impl ExportersConfig {
    /// Get the OTLP exporter configuration if configured
    pub fn otlp(&self) -> Option<&OtlpExporterConfig> {
        self.otlp.as_ref()
    }
}

// No Default impl for OtlpExporterConfig since endpoint is required

fn default_timeout() -> Duration {
    Duration::from_secs(60)
}

/// OTLP protocol selection
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OtlpProtocol {
    #[default]
    Grpc,
    Http,
}

/// Batch export configuration for OTLP
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BatchExportConfig {
    /// Delay between batch exports
    #[serde(deserialize_with = "deserialize_duration", default = "default_scheduled_delay")]
    pub scheduled_delay: Duration,

    /// Maximum queue size
    pub max_queue_size: usize,

    /// Maximum batch size for export
    pub max_export_batch_size: usize,

    /// Maximum concurrent exports
    pub max_concurrent_exports: usize,
}

impl Default for BatchExportConfig {
    fn default() -> Self {
        Self {
            scheduled_delay: default_scheduled_delay(),
            max_queue_size: 2048,
            max_export_batch_size: 512,
            max_concurrent_exports: 1,
        }
    }
}

fn default_scheduled_delay() -> Duration {
    Duration::from_secs(5)
}
