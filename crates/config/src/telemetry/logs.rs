use super::exporters::ExportersConfig;
use serde::Deserialize;

/// Logs-specific configuration
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct LogsConfig {
    /// Override global exporters for logs (optional)
    #[serde(default)]
    exporters: Option<ExportersConfig>,
}

impl LogsConfig {
    /// Get the exporters if configured
    pub fn exporters(&self) -> Option<&ExportersConfig> {
        self.exporters.as_ref()
    }
}
