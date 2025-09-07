use super::exporters::ExportersConfig;
use serde::Deserialize;

/// Metrics-specific configuration
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsConfig {
    /// Override global exporters for metrics (optional)
    #[serde(default)]
    exporters: Option<ExportersConfig>,
}

impl MetricsConfig {
    /// Get the exporters if configured
    pub fn exporters(&self) -> Option<&ExportersConfig> {
        self.exporters.as_ref()
    }
}
