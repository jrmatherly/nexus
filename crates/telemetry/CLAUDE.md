# Telemetry Crate

This crate provides OpenTelemetry integration for Nexus, handling metrics collection and export.

## Purpose

- Initialize OpenTelemetry SDK with OTLP exporters
- Provide convenient metric creation functions (counter, histogram, updown_counter)
- Manage telemetry lifecycle with proper shutdown via `TelemetryGuard`
- Export metrics to configured backends (e.g., OTLP collectors)

## Structure

```
telemetry/
├── lib.rs              # Public API and convenience functions
└── metrics/
    ├── mod.rs          # Metrics initialization and OTLP setup
    └── names.rs        # Standard metric name constants
```

## Metric Names

All metric names follow OpenTelemetry semantic conventions and are defined as constants in `metrics::names`:

- `HTTP_SERVER_REQUEST_DURATION` - HTTP request duration histogram (milliseconds)
  - Automatically provides count, sum, and distribution
  - Attributes: http.request.method, http.route, http.response.status_code

## Usage Patterns

### Creating Metrics

```rust
use telemetry::{counter, histogram, KeyValue};

// Get metrics using convenience functions
let request_counter = telemetry::counter(telemetry::metrics::MY_COUNTER);
let duration_hist = telemetry::histogram(telemetry::metrics::HTTP_SERVER_REQUEST_DURATION);

// Record with attributes
duration_hist.record(
    100.0,
    &[KeyValue::new("status", 200)]
);
```

### Initialization

```rust
// Initialize at startup, keep guard alive for app lifetime
let _guard = telemetry::init(&config).await?;
// Metrics are now available globally
// Guard ensures proper shutdown when dropped
```

## Key Design Decisions

- **Global Meter**: Uses a single "nexus" meter for all metrics
- **Lazy Creation**: Metrics are created on first use, not at startup
- **No Static Metrics**: Avoids static initialization issues
- **Histogram Over Counter**: Use histograms when you need both count and distribution
