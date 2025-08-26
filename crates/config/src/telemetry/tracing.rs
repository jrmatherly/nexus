use super::exporters::ExportersConfig;
use serde::Deserialize;

/// Tracing configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TracingConfig {
    /// Sampling rate (0.0 to 1.0)
    pub sampling: f64,

    /// Use parent-based sampler
    pub parent_based_sampler: bool,

    /// Collection limits configuration
    #[serde(default)]
    pub collect: CollectConfig,

    /// Trace context propagation configuration
    #[serde(default)]
    pub propagation: PropagationConfig,

    /// Override global exporters for traces (optional)
    #[serde(default)]
    exporters: Option<ExportersConfig>,
}

impl TracingConfig {
    /// Get the exporters if configured
    pub fn exporters(&self) -> Option<&ExportersConfig> {
        self.exporters.as_ref()
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            sampling: 0.15,
            parent_based_sampler: false,
            collect: CollectConfig::default(),
            propagation: PropagationConfig::default(),
            exporters: None,
        }
    }
}

/// Collection limits for tracing
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CollectConfig {
    /// Maximum events per span
    pub max_events_per_span: usize,

    /// Maximum attributes per span
    pub max_attributes_per_span: usize,

    /// Maximum links per span
    pub max_links_per_span: usize,

    /// Maximum attributes per event
    pub max_attributes_per_event: usize,

    /// Maximum attributes per link
    pub max_attributes_per_link: usize,
}

impl Default for CollectConfig {
    fn default() -> Self {
        Self {
            max_events_per_span: 128,
            max_attributes_per_span: 128,
            max_links_per_span: 128,
            max_attributes_per_event: 128,
            max_attributes_per_link: 128,
        }
    }
}

/// Trace context propagation configuration
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct PropagationConfig {
    /// W3C Trace Context propagation
    pub trace_context: bool,

    /// W3C Baggage propagation
    pub baggage: bool,

    /// AWS X-Ray propagation
    pub aws_xray: bool,

    /// Jaeger propagation
    pub jaeger: bool,
}
