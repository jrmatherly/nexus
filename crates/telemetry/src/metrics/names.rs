//! Standard metric names following OpenTelemetry semantic conventions
//! See: https://opentelemetry.io/docs/specs/semconv/http/http-metrics/

/// HTTP server request duration in milliseconds
/// Note: Histograms automatically provide count and sum, so a separate counter is not needed
pub const HTTP_SERVER_REQUEST_DURATION: &str = "http.server.request.duration";
