//! HTTP metrics middleware
//!
//! Records OpenTelemetry metrics for all HTTP requests following semantic conventions:
//! - `http.server.request.duration`: Histogram of request latencies in milliseconds
//!   (also provides count and sum automatically)
//!
//! Attributes recorded:
//! - `http.request.method`: HTTP method (GET, POST, etc.)
//! - `http.route`: Matched route pattern
//! - `http.response.status_code`: HTTP response status code

use axum::{body::Body, extract::MatchedPath};
use http::{Request, Response};
use std::{
    fmt::Display,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};
use telemetry::{self, KeyValue, metrics};
use tower::Layer;

/// Layer for HTTP metrics tracking
#[derive(Clone)]
pub struct MetricsLayer;

impl MetricsLayer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MetricsLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<Service> Layer<Service> for MetricsLayer
where
    Service: Send + Clone,
{
    type Service = MetricsService<Service>;

    fn layer(&self, next: Service) -> Self::Service {
        MetricsService { next }
    }
}

/// Service that tracks HTTP metrics
#[derive(Clone)]
pub struct MetricsService<Service> {
    next: Service,
}

impl<Service, ReqBody> tower::Service<Request<ReqBody>> for MetricsService<Service>
where
    Service: tower::Service<Request<ReqBody>, Response = Response<Body>> + Send + Clone + 'static,
    Service::Future: Send,
    Service::Error: Display + 'static,
    ReqBody: http_body::Body + Send + 'static,
{
    type Response = Response<Body>;
    type Error = Service::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response<Body>, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.next.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let start = Instant::now();

        // Extract request attributes
        let method = req.method().to_string();
        let path = req
            .extensions()
            .get::<MatchedPath>()
            .map(|matched_path| matched_path.as_str().to_owned())
            .unwrap_or_else(|| "unknown".to_string());

        // Clone the service for the async block
        let mut next = self.next.clone();

        Box::pin(async move {
            let response = next.call(req).await?;

            let duration = start.elapsed().as_secs_f64() * 1000.0; // Convert to milliseconds
            let status = response.status().as_u16();

            // Following OpenTelemetry semantic conventions for HTTP metrics
            // See: https://opentelemetry.io/docs/specs/semconv/http/http-metrics/
            // The histogram automatically tracks count, sum, and distribution

            let duration_histogram = telemetry::histogram(metrics::HTTP_SERVER_REQUEST_DURATION);

            duration_histogram.record(
                duration,
                &[
                    KeyValue::new("http.request.method", method),
                    KeyValue::new("http.route", path),
                    KeyValue::new("http.response.status_code", status as i64),
                ],
            );

            Ok(response)
        })
    }
}
